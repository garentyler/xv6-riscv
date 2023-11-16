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

use crate::{
    arch::virtual_memory::{either_copyin, either_copyout},
    fs::file::{devsw, CONSOLE},
    hardware::uart::Uart,
    proc::{
        process::{procdump, Process},
        scheduler::wakeup,
    },
    sync::mutex::Mutex,
};
use core::ptr::addr_of_mut;

pub static UART0: &Uart = &crate::hardware::UARTS[0].1;

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
impl core::fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        UART0.write_slice(s.as_bytes());
        core::fmt::Result::Ok(())
    }
}

#[no_mangle]
pub static cons: Mutex<Console> = Mutex::new(Console {
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
        UART0.write_byte(0x08);
        UART0.write_byte(b' ');
        UART0.write_byte(0x08);
    } else {
        UART0.write_byte(c);
    }
}

/// User write()s to the console go here.
pub fn consolewrite(user_src: i32, src: u64, n: i32) -> i32 {
    unsafe {
        for i in 0..n {
            let mut c = 0i8;

            if either_copyin(
                addr_of_mut!(c).cast(),
                user_src,
                src as usize + i as u32 as usize,
                1,
            ) == -1
            {
                return i;
            } else {
                UART0.write_byte_buffered(c as u8);
            }
        }
        0
    }
}

/// User read()s from the console go here.
///
/// Copy (up to) a whole input line to dst.
/// user_dst indicates whether dst is a user
/// or kernel address.
pub fn consoleread(user_dst: i32, mut dst: u64, mut n: i32) -> i32 {
    unsafe {
        let target = n;
        let mut c;
        let mut cbuf;

        let mut console = cons.lock_spinning();

        while n > 0 {
            // Wait until interrupt handler has put
            // some input into cons.buffer.
            while console.read_index == console.write_index {
                if Process::current().unwrap().is_killed() {
                    // cons.lock.unlock();
                    return -1;
                }
                let channel = addr_of_mut!(console.read_index).cast();
                console.sleep(channel);
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
            if either_copyout(user_dst, dst as usize, addr_of_mut!(cbuf).cast(), 1) == -1 {
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
}

pub unsafe fn consoleinit() {
    UART0.initialize();

    // Connect read and write syscalls
    // to consoleread and consolewrite.
    devsw[CONSOLE].read = Some(consoleread);
    devsw[CONSOLE].write = Some(consolewrite);
}

/// The console input interrupt handler.
///
/// uartintr() calls this for input character.
/// Do erase/kill processing, then append to cons.buf.
/// Wake up consoleread() if a whole line has arrived.
pub fn consoleintr(mut c: u8) {
    let mut console = cons.lock_spinning();

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
