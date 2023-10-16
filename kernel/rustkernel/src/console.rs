//! Console input and output, to the uart.
//
// Reads are a line at a time.
// Implements special input characters:
// - newline: end of line
// - ctrl-h: backspace
// - ctrl-u: kill line
// - ctrl-d: end of file
// - ctrl-p: print process list

use crate::{
    file::{devsw, CONSOLE},
    proc::{killed, myproc, sleep},
    spinlock::{initlock, Spinlock},
    uart::{uartinit, uartputc, uartputc_sync},
};
use core::{
    ffi::{c_void, CStr},
    ptr::addr_of_mut,
};

extern "C" {
    fn either_copyin(dst: *mut c_void, user_src: i32, src: u64, len: u64) -> i32;
    fn either_copyout(user_dst: i32, dst: u64, src: *mut c_void, len: u64) -> i32;

    pub fn consoleintr(c: i32);
    fn wakeup(chan: *mut c_void);
    fn procdump();
}

pub const BACKSPACE: i32 = 0x100;
pub const INPUT_BUF_SIZE: u64 = 128;

#[no_mangle]
pub static mut cons: Console = Console {
    lock: unsafe { Spinlock::uninitialized() },
    buffer: [0u8; INPUT_BUF_SIZE as usize],
    read_index: 0,
    write_index: 0,
    edit_index: 0,
};

/// ctrl-x
fn ctrl_x(x: char) -> char {
    ((x as u8) - b'@') as char
}

/// Send one character to the UART.
///
/// Called by printf(), and to echo input
/// characters but not from write().
#[no_mangle]
pub unsafe extern "C" fn consputc(c: i32) {
    if c == BACKSPACE {
        // If the user typed backspace, overwrite with a space.
        uartputc_sync('\x08' as i32);
        uartputc_sync(' ' as i32);
        uartputc_sync('\x08' as i32);
    } else {
        uartputc_sync(c);
    }
}

#[repr(C)]
pub struct Console {
    pub lock: Spinlock,
    pub buffer: [u8; INPUT_BUF_SIZE as usize],
    pub read_index: u32,
    pub write_index: u32,
    pub edit_index: u32,
}

/// User write()s to the console go here.
#[no_mangle]
pub unsafe extern "C" fn consolewrite(user_src: i32, src: u64, n: i32) -> i32 {
    for i in 0..n {
        let mut c = 0i8;

        if either_copyin(addr_of_mut!(c).cast(), user_src, src + i as u64, 1) == -1 {
            return i;
        } else {
            uartputc(c as i32);
        }
    }
    0
}

/// User read()s from the console go here.
///
/// Copy (up to) a whole input line to dst.
/// user_dst indicates whether dst is a user
/// or kernel address.
#[no_mangle]
pub unsafe extern "C" fn consoleread(user_dst: i32, mut dst: u64, mut n: i32) -> i32 {
    let target = n;
    let mut c;
    let mut cbuf;

    cons.lock.lock();

    while n > 0 {
        // Wait until interrupt handler has put
        // some input into cons.buffer.
        while cons.read_index == cons.write_index {
            if killed(myproc()) != 0 {
                cons.lock.unlock();
                return -1;
            }
            sleep(
                addr_of_mut!(cons.read_index).cast(),
                addr_of_mut!(cons.lock),
            );
        }

        c = cons.buffer[(cons.read_index % INPUT_BUF_SIZE as u32) as usize];
        cons.read_index += 1;

        // ctrl-D or EOF
        if c == ctrl_x('D') as u8 {
            if n < target {
                // Save ctrl-D for next time, to make
                // sure caller gets a 0-byte result.
                cons.read_index -= 1;
            }
            break;
        }

        // Copy the input byte to the user-space buffer.
        cbuf = c;
        if either_copyout(user_dst, dst, addr_of_mut!(cbuf).cast(), 1) == -1 {
            break;
        }

        dst += 1;
        n -= 1;

        if c == b'\n' {
            // A whole line has arrived,
            // return to the user-level read().
            break;
        }
    }

    cons.lock.unlock();

    target - n
}

// /// The console input interrupt handler.
// ///
// /// uartintr() calls this for input character.
// /// Do erase/kill processing, then append to cons.buf.
// /// Wake up consoleread() if a whole line has arrived.
// #[no_mangle]
// pub unsafe extern "C" fn consoleintr(c: i32) {
//     cons.lock.lock();
//
//     let ctrl_p = ctrl_x('P') as u8 as i8 as i32;
//     let ctrl_u = ctrl_x('P') as u8 as i8 as i32;
//     let ctrl_h = ctrl_x('P') as u8 as i8 as i32;
//     let ctrl_d = ctrl_x('D') as u8 as i8 as i32;
//     let cr = '\r' as u8 as i8 as i32;
//     let nl = '\n' as u8 as i8 as i32;
//
//     match c {
//         // Print process list.
//         ctrl_p => procdump(),
//         // Kill line
//         ctrl_u => {
//             while cons.edit_index != cons.write_index
//                 && cons.buffer[((cons.edit_index - 1) % INPUT_BUF_SIZE as u32) as usize]
//                     != '\n' as u8
//             {
//                 cons.edit_index -= 1;
//                 consputc(BACKSPACE);
//             }
//         }
//         // Backspace
//         ctrl_h => {
//             if cons.edit_index != cons.write_index {
//                 cons.edit_index -= 1;
//                 consputc(BACKSPACE);
//             }
//         }
//         c => {
//             if cons.edit_index - cons.read_index < INPUT_BUF_SIZE as u32 {
//                 let c = if c == cr { nl } else { c };
//
//                 // Echo back to the user.
//                 consputc(c);
//
//                 // Store for consumption by consoleread().
//                 cons.buffer[(cons.edit_index % INPUT_BUF_SIZE as u32) as usize] = c as i8 as u8;
//                 cons.edit_index += 1;
//
//                 if c == nl
//                     || c == ctrl_d
//                     || cons.edit_index - cons.read_index == INPUT_BUF_SIZE as u32
//                 {
//                     // Wake up consoleread() if a whole line (or EOF) has arrived.
//                     cons.write_index = cons.edit_index;
//                     wakeup(addr_of_mut!(cons.read_index).cast());
//                 }
//             }
//         }
//     }
//
//     cons.lock.unlock();
// }

pub unsafe fn consoleinit() {
    initlock(
        addr_of_mut!(cons.lock),
        CStr::from_bytes_with_nul(b"cons\0")
            .unwrap()
            .as_ptr()
            .cast_mut(),
    );
    uartinit();

    // Connect read and write syscalls
    // to consoleread and consolewrite.
    devsw[CONSOLE].read = consoleread as usize as *const i32;
    devsw[CONSOLE].write = consolewrite as usize as *const i32;
}
