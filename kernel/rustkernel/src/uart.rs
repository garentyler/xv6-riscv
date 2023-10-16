//! Low-level driver routines for 16550a UART.
#![allow(non_upper_case_globals)]

use crate::{
    console::consoleintr,
    proc::{sleep, wakeup},
    riscv::memlayout::UART0,
    spinlock::{pop_off, push_off, Spinlock},
};
use core::{ffi::CStr, ptr::addr_of_mut};

/// The UART control registers are memory-mapped
/// at address UART0. This function returns the
/// address of one of the registers.
#[inline(always)]
fn get_register_addr<N: Into<u64>>(register: N) -> *mut u8 {
    let register: u64 = register.into();
    (UART0 + register) as *mut u8
}

// The UART control registers.
// Some have different meanings for read vs write.
// See http://byterunner.com/16550.html

/// Receive Holding Register (for input bytes)
const RHR: u8 = 0;
/// Transmit Holding Register (for output bytes)
const THR: u8 = 0;
/// Interrupt Enable Register
const IER: u8 = 1;
const IER_RX_ENABLE: u8 = 1 << 0;
const IER_TX_ENABLE: u8 = 1 << 1;
/// FIFO control register
const FCR: u8 = 2;
const FCR_FIFO_ENABLE: u8 = 1 << 0;
/// Clear the content of the two FIFOs.
const FCR_FIFO_CLEAR: u8 = 3 << 1;
/// Interrupt Status Register
const ISR: u8 = 2;
/// Line Control Register
const LCR: u8 = 2;
const LCR_EIGHT_BITS: u8 = 3;
/// Special mode to set baud rate
const LCR_BAUD_LATCH: u8 = 1 << 7;
/// Line Status Register
const LSR: u8 = 5;
/// Input is waiting to be read from RHR
const LSR_RX_READY: u8 = 1 << 0;
/// THR can accept another character to send
const LSR_TX_IDLE: u8 = 1 << 5;

#[inline(always)]
unsafe fn read_register<N: Into<u64>>(register: N) -> u8 {
    *get_register_addr(register)
}
#[inline(always)]
unsafe fn write_register<N: Into<u64>>(register: N, value: u8) {
    *get_register_addr(register) = value;
}

static mut uart_tx_lock: Spinlock = unsafe { Spinlock::uninitialized() };
const UART_TX_BUF_SIZE: u64 = 32;
static mut uart_tx_buf: [u8; UART_TX_BUF_SIZE as usize] = [0u8; UART_TX_BUF_SIZE as usize];
/// Write next to uart_tx_buf[uart_tx_w % UART_TX_BUF_SIZE]
static mut uart_tx_w: u64 = 0;
/// Read next from uart_tx_buf[uart_tx_r % UART_TX_BUF_SIZE]
static mut uart_tx_r: u64 = 0;

pub(crate) unsafe fn uartinit() {
    // Disable interrupts.
    write_register(IER, 0x00);
    // Special mode to set baud rate.
    write_register(LCR, LCR_BAUD_LATCH);
    // LSB for baud rate of 38.4K.
    write_register(0u8, 0x03);
    // MSB for baud rate of 38.4K.
    write_register(1u8, 0x00);
    // Leave set-baud mode and set
    // word length to 8 bits, no parity.
    write_register(LCR, LCR_EIGHT_BITS);
    // Reset and enable FIFOs.
    write_register(FCR, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);
    // Enable transmit and receive interrupts.
    write_register(IER, IER_TX_ENABLE | IER_RX_ENABLE);

    uart_tx_lock = Spinlock::new(
        CStr::from_bytes_with_nul(b"uart\0")
            .unwrap()
            .as_ptr()
            .cast_mut(),
    );
}

/// Add a character to the output buffer and tell the
/// UART to start sending if it isn't already.
/// Blocks if the output buffer is full.
/// Because it may block, it can't be called
/// from interrupts, it's only suitable for use
/// by write().
pub(crate) unsafe fn uartputc(c: i32) {
    uart_tx_lock.lock();

    if crate::PANICKED {
        loop {
            core::hint::spin_loop();
        }
    }

    while uart_tx_w == uart_tx_r + UART_TX_BUF_SIZE {
        // Buffer is full.
        // Wait for uartstart() to open up space in the buffer.
        sleep(
            addr_of_mut!(uart_tx_r).cast(),
            addr_of_mut!(uart_tx_lock).cast(),
        );
    }

    uart_tx_buf[(uart_tx_w % UART_TX_BUF_SIZE) as usize] = c as i8 as u8;
    uart_tx_w += 1;
    uartstart();
    uart_tx_lock.unlock();
}

/// Alternate version of uartputc() that doesn't
/// use interrupts, for use by kernel printf() and
/// to echo characters. It spins waiting for the UART's
/// output register to be empty.
pub(crate) unsafe fn uartputc_sync(c: i32) {
    push_off();

    if crate::PANICKED {
        loop {
            core::hint::spin_loop();
        }
    }

    // Wait for Transmit Holding Empty to be set in LSR.
    while read_register(LSR) & LSR_TX_IDLE == 0 {
        core::hint::spin_loop();
    }

    write_register(THR, c as i8 as u8);

    pop_off();
}

/// If the UART is idle, and a character is waiting
/// in the transmit buffer, send it.
/// Caller must hold uart_tx_lock.
/// Called from both the top and bottom halves.
unsafe fn uartstart() {
    loop {
        if uart_tx_w == uart_tx_r {
            // Transmit buffer is ready.
            return;
        }

        if read_register(LSR) & LSR_TX_IDLE == 0 {
            // The UART transmit holding register is full,
            // so we cannot give it another byte.
            // It will interrupt when it's ready for a new byte.
            return;
        }

        let c = uart_tx_buf[(uart_tx_r % UART_TX_BUF_SIZE) as usize];
        uart_tx_r += 1;

        // Maybe uartputc() is waiting for space in the buffer.
        wakeup(addr_of_mut!(uart_tx_r).cast());

        write_register(THR, c);
    }
}

/// Read one input character from the UART.
/// Return -1 if nothing is waiting.
unsafe fn uartgetc() -> i32 {
    if read_register(LSR) & 0x01 != 0 {
        // Input data is ready.
        read_register(RHR) as i32
    } else {
        -1
    }
}

/// Handle a UART interrupt, raised because input has
/// arrived, or the uart is ready for more output, or
/// both. Called from devintr().
#[no_mangle]
pub unsafe extern "C" fn uartintr() {
    // Read and process incoming characters.
    loop {
        let c = uartgetc();
        if c == -1 {
            break;
        }
        consoleintr(c);
    }

    // Send buffered characters.
    uart_tx_lock.lock();
    uartstart();
    uart_tx_lock.unlock();
}
