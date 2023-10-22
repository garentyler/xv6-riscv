//! Low-level driver routines for 16550a UART.
#![allow(non_upper_case_globals)]

use crate::{
    console::consoleintr,
    proc::{sleep_lock, wakeup},
    riscv::memlayout::UART0,
    sync::spinlock::Spinlock,
    sync::spinmutex::SpinMutex,
    trap::InterruptBlocker,
};
use core::{ffi::CStr, ptr::addr_of_mut};

enum Register {
    ReceiveHolding,
    TransmitHolding,
    InterruptEnable,
    FIFOControl,
    InterruptStatus,
    LineControl,
    LineStatus,
}
impl Register {
    pub fn as_ptr(&self) -> *mut u8 {
        let addr = UART0
            + match self {
                Register::ReceiveHolding => 0,
                Register::TransmitHolding => 0,
                Register::InterruptEnable => 1,
                Register::FIFOControl => 2,
                Register::InterruptStatus => 2,
                Register::LineControl => 2,
                Register::LineStatus => 5,
            };
        addr as *mut u8
    }
    pub fn read(&self) -> u8 {
        unsafe { self.as_ptr().read_volatile() }
    }
    pub fn write(&self, value: u8) {
        unsafe { self.as_ptr().write_volatile(value) }
    }
}

pub static uart: SpinMutex<Uart> = SpinMutex::new(Uart {
    buffer: [0u8; UART_TX_BUF_SIZE],
    write_index: 0,
    read_index: 0,
});

pub struct Uart {
    pub buffer: [u8; UART_TX_BUF_SIZE],
    pub write_index: usize,
    pub read_index: usize,
}
impl Uart {
    /// Alternate version of Uart::write_byte() that doesn't
    /// use interrupts, for use by kernel printf() and
    /// to echo characters. It spins waiting for the UART's
    /// output register to be empty.
    pub fn write_byte_sync(x: u8) {
        let _ = InterruptBlocker::new();

        if unsafe { crate::PANICKED } {
            loop {
                core::hint::spin_loop();
            }
        }

        // Wait for Transmit Holding Empty to be set in LSR.
        while Register::LineStatus.read() & LSR_TX_IDLE == 0 {
            core::hint::spin_loop();
        }

        Register::TransmitHolding.write(x);
    }
    /// If the UART is idle, and a character is
    /// waiting in the transmit buffer, send it.
    pub fn send_queued_bytes(&mut self) {
        loop {
            self.write_index %= self.buffer.len();
            self.read_index %= self.buffer.len();

            if self.write_index == self.read_index {
                // Transmit buffer is ready.
                return;
            }

            let c = self.buffer[self.read_index];
            self.read_index += 1;

            // Maybe uartputc() is waiting for space in the buffer.
            unsafe { wakeup(addr_of_mut!(self.read_index).cast()) };

            Register::TransmitHolding.write(c);
        }
    }
}

// The UART control registers.
// Some have different meanings for read vs write.
// See http://byterunner.com/16550.html

/// Interrupt Enable Register
const IER_RX_ENABLE: u8 = 1 << 0;
const IER_TX_ENABLE: u8 = 1 << 1;
const FCR_FIFO_ENABLE: u8 = 1 << 0;
/// Clear the content of the two FIFOs.
const FCR_FIFO_CLEAR: u8 = 3 << 1;
const LCR_EIGHT_BITS: u8 = 3;
/// Special mode to set baud rate
const LCR_BAUD_LATCH: u8 = 1 << 7;
/// Input is waiting to be read from RHR
const LSR_RX_READY: u8 = 1 << 0;
/// THR can accept another character to send
const LSR_TX_IDLE: u8 = 1 << 5;

static mut uart_tx_lock: Spinlock = unsafe { Spinlock::uninitialized() };
const UART_TX_BUF_SIZE: usize = 32;
static mut uart_tx_buf: [u8; UART_TX_BUF_SIZE] = [0u8; UART_TX_BUF_SIZE];
// static uart_tx_buf: SpinMutex<[u8; UART_TX_BUF_SIZE]> = SpinMutex::new([0u8; UART_TX_BUF_SIZE]);
/// Write next to uart_tx_buf[uart_tx_w % UART_TX_BUF_SIZE]
static mut uart_tx_w: usize = 0;
/// Read next from uart_tx_buf[uart_tx_r % UART_TX_BUF_SIZE]
static mut uart_tx_r: usize = 0;

pub(crate) unsafe fn uartinit() {
    // Disable interrupts.
    Register::InterruptEnable.write(0x00);
    // Special mode to set baud rate.
    Register::LineControl.write(LCR_BAUD_LATCH);
    unsafe {
        // LSB for baud rate of 38.4K.
        *(UART0 as *mut u8) = 0x03;
        // MSB for baud rate of 38.4K.
        *((UART0 + 1) as *mut u8) = 0x00;
    }
    // Leave set-baud mode and set
    // word length to 8 bits, no parity.
    Register::LineControl.write(LCR_EIGHT_BITS);
    // Reset and enable FIFOs.
    Register::FIFOControl.write(FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);
    // Enable transmit and receive interrupts.
    Register::InterruptEnable.write(IER_TX_ENABLE | IER_RX_ENABLE);

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
pub(crate) unsafe fn uartputc(c: u8) {
    let _guard = uart_tx_lock.lock();
    // let mut buf = uart_tx_buf.lock_unguarded();
    // let u = uart.lock_unguarded();

    if crate::PANICKED {
        loop {
            core::hint::spin_loop();
        }
    }

    while uart_tx_w == uart_tx_r + UART_TX_BUF_SIZE {
        // Buffer is full.
        // Wait for uartstart() to open up space in the buffer.
        sleep_lock(
            addr_of_mut!(uart_tx_r).cast(),
            addr_of_mut!(uart_tx_lock).cast(),
        );
    }

    uart_tx_buf[uart_tx_w % UART_TX_BUF_SIZE] = c;
    uart_tx_w += 1;
    uartstart();
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
        if Register::LineStatus.read() & LSR_TX_IDLE == 0 {
            // The UART transmit holding register is full,
            // so we cannot give it another byte.
            // It will interrupt when it's ready for a new byte.
            return;
        }

        // let buf = uart_tx_buf.lock_unguarded();
        let c = uart_tx_buf[uart_tx_r % UART_TX_BUF_SIZE];
        uart_tx_r += 1;

        // Maybe uartputc() is waiting for space in the buffer.
        wakeup(addr_of_mut!(uart_tx_r).cast());

        Register::TransmitHolding.write(c);
    }
}

/// Read one input byte from the UART.
pub(crate) fn uartgetc() -> Option<u8> {
    if Register::LineStatus.read() & 0x01 != 0 {
        // Input data is ready.
        Some(Register::ReceiveHolding.read())
    } else {
        None
    }
}

/// Handle a UART interrupt, raised because input has
/// arrived, or the uart is ready for more output, or
/// both. Called from devintr().
pub(crate) unsafe fn uartintr() {
    // Read and process incoming characters.
    while let Some(c) = uartgetc() {
        consoleintr(c);
    }

    // Send buffered characters.
    let _guard = uart_tx_lock.lock();
    uartstart();
}
