//! Console input and output, to the uart.
//
// Reads are a line at a time.
// Implements special input characters:
// - newline: end of line
// - ctrl-h: backspace
// - ctrl-u: kill line
// - ctrl-d: end of file
// - ctrl-p: print process list

pub mod printf;
pub mod uart;

use crate::{
    fs::file::{devsw, CONSOLE},
    proc::{killed, myproc, procdump, sleep_mutex, wakeup},
    sync::spinmutex::SpinMutex,
};
use core::{ffi::c_void, ptr::addr_of_mut};
use uart::{uartinit, uartputc, Uart};

extern "C" {
    fn either_copyin(dst: *mut c_void, user_src: i32, src: u64, len: u64) -> i32;
    fn either_copyout(user_dst: i32, dst: u64, src: *mut c_void, len: u64) -> i32;
}

pub const BACKSPACE: u8 = 0x00;
pub const INPUT_BUF_SIZE: usize = 128;

pub struct Console {
    pub buffer: [u8; INPUT_BUF_SIZE],
    pub read_index: usize,
    pub write_index: usize,
    pub edit_index: usize,
}
impl Console {
    pub fn read_byte(&self) -> &u8 {
        &self.buffer[self.read_index % self.buffer.len()]
    }
    pub fn write_byte(&mut self) -> &mut u8 {
        let i = self.write_index % self.buffer.len();
        &mut self.buffer[i]
    }
    pub fn edit_byte(&mut self) -> &mut u8 {
        let i = self.edit_index % self.buffer.len();
        &mut self.buffer[i]
    }
}

#[no_mangle]
pub static cons: SpinMutex<Console> = SpinMutex::new(Console {
    buffer: [0u8; INPUT_BUF_SIZE],
    read_index: 0,
    write_index: 0,
    edit_index: 0,
});

/// ctrl-x
const fn ctrl_x(x: u8) -> u8 {
    x - b'@'
}

/// Send one character to the UART.
///
/// Called by printf(), and to echo input
/// characters but not from write().
pub fn consputc(c: u8) {
    if c == BACKSPACE {
        // If the user typed backspace, overwrite with a space.
        Uart::write_byte_sync(0x08);
        Uart::write_byte_sync(b' ');
        Uart::write_byte_sync(0x08);
    } else {
        Uart::write_byte_sync(c);
    }
}

/// User write()s to the console go here.
#[no_mangle]
pub unsafe extern "C" fn consolewrite(user_src: i32, src: u64, n: i32) -> i32 {
    for i in 0..n {
        let mut c = 0i8;

        if either_copyin(addr_of_mut!(c).cast(), user_src, src + i as u64, 1) == -1 {
            return i;
        } else {
            uartputc(c as u8);
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

    let mut console = cons.lock();

    while n > 0 {
        // Wait until interrupt handler has put
        // some input into cons.buffer.
        while console.read_index == console.write_index {
            if killed(myproc()) != 0 {
                // cons.lock.unlock();
                return -1;
            }
            // let channel = addr_of_mut!(console.read_index).cast();
            // console.sleep(channel);
            sleep_mutex(addr_of_mut!(console.read_index).cast(), &mut console);
        }

        c = *console.read_byte();
        console.read_index += 1;

        // ctrl-D or EOF
        if c == ctrl_x(b'D') {
            if n < target {
                // Save ctrl-D for next time, to make
                // sure caller gets a 0-byte result.
                console.read_index -= 1;
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

    // cons.lock.unlock();

    target - n
}

pub unsafe fn consoleinit() {
    uartinit();

    // Connect read and write syscalls
    // to consoleread and consolewrite.
    devsw[CONSOLE].read = consoleread as usize as *const i32;
    devsw[CONSOLE].write = consolewrite as usize as *const i32;
}

/// The console input interrupt handler.
///
/// uartintr() calls this for input character.
/// Do erase/kill processing, then append to cons.buf.
/// Wake up consoleread() if a whole line has arrived.
pub fn consoleintr(mut c: u8) {
    let mut console = cons.lock();

    if c == ctrl_x(b'P') {
        // Print process list.
        unsafe { procdump() };
    } else if c == ctrl_x(b'U') {
        // Kill line.
        while console.edit_index != console.write_index
            && console.buffer[(console.edit_index - 1) % INPUT_BUF_SIZE] != b'\n'
        {
            console.edit_index -= 1;
            consputc(BACKSPACE);
        }
    } else if c == ctrl_x(b'H') || c == 0x7f {
        // Backspace or delete key.
        if console.edit_index != console.write_index {
            console.edit_index -= 1;
            consputc(BACKSPACE);
        }
    } else if c != 0 && console.edit_index - console.read_index < INPUT_BUF_SIZE {
        c = if c == b'\r' { b'\n' } else { c };

        // Echo back to the user.
        consputc(c);

        // Store for consumption by consoleread().
        *console.edit_byte() = c;
        console.edit_index += 1;

        if c == b'\n'
            || c == ctrl_x(b'D')
            || console.edit_index - console.read_index == INPUT_BUF_SIZE
        {
            // Wake up consoleread() if a whole line (or EOF) has arrived.
            console.write_index = console.edit_index;
            unsafe { wakeup(addr_of_mut!(console.read_index).cast()) };
        }
    }
}
