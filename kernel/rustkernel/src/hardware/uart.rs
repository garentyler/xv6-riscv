//! Low-level driver routines for 16550a UART.
#![allow(non_upper_case_globals)]

use crate::{
    arch::trap::InterruptBlocker,
    console::consoleintr,
    proc::scheduler::wakeup,
    queue::Queue,
    sync::mutex::{Mutex, MutexGuard},
};
use core::ptr::addr_of;

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
    pub fn as_offset(&self) -> usize {
        match self {
            Register::ReceiveHolding => 0,
            Register::TransmitHolding => 0,
            Register::InterruptEnable => 1,
            Register::FIFOControl => 2,
            Register::InterruptStatus => 2,
            Register::LineControl => 2,
            Register::LineStatus => 5,
        }
    }
    pub fn as_ptr(&self, base_address: usize) -> *mut u8 {
        (base_address + self.as_offset()) as *mut u8
    }
    pub fn read(&self, base_address: usize) -> u8 {
        unsafe { self.as_ptr(base_address).read_volatile() }
    }
    pub fn write(&self, base_address: usize, value: u8) {
        unsafe { self.as_ptr(base_address).write_volatile(value) }
    }
}

pub struct Uart {
    pub base_address: usize,
}
impl Uart {
    pub const fn new(base_address: usize) -> Uart {
        Uart { base_address }
    }
    /// Initialize the UART.
    pub unsafe fn initialize(&self) {
        // Disable interrupts.
        Register::InterruptEnable.write(self.base_address, 0x00);
        // Special mode to set baud rate.
        Register::LineControl.write(self.base_address, LCR_BAUD_LATCH);
        // LSB for baud rate of 38.4K.
        *(self.base_address as *mut u8) = 0x03;
        // MSB for baud rate of 38.4K.
        *((self.base_address + 1) as *mut u8) = 0x00;
        // Leave set-baud mode and set
        // word length to 8 bits, no parity.
        Register::LineControl.write(self.base_address, LCR_EIGHT_BITS);
        // Reset and enable FIFOs.
        Register::FIFOControl.write(self.base_address, FCR_FIFO_ENABLE | FCR_FIFO_CLEAR);
        // Enable transmit and receive interrupts.
        Register::InterruptEnable.write(self.base_address, IER_TX_ENABLE | IER_RX_ENABLE);
    }
    /// Handle an interrupt from the hardware.
    pub fn interrupt(&self) {
        // Read and process incoming data.
        while let Some(b) = self.read_byte() {
            consoleintr(b);
        }
    }
    /// Read one byte from the UART.
    pub fn read_byte(&self) -> Option<u8> {
        if Register::LineStatus.read(self.base_address) & 0x01 != 0 {
            // Input data is ready.
            Some(Register::ReceiveHolding.read(self.base_address))
        } else {
            None
        }
    }
    pub fn writer(&self) -> UartWriter<'_> {
        UartWriter(self)
    }
    pub fn can_write_byte(&self) -> bool {
        Register::LineStatus.read(self.base_address) & LSR_TX_IDLE != 0
    }
    /// Attempt to write one byte to the UART.
    /// Returns a bool representing whether the byte was written.
    pub fn write_byte(&self, byte: u8) -> bool {
        // Block interrupts to prevent TOCTOU manipulation.
        let _ = InterruptBlocker::new();
        if self.can_write_byte() {
            Register::TransmitHolding.write(self.base_address, byte);
            true
        } else {
            false
        }
    }
    pub fn write_byte_blocking(&self, byte: u8) {
        while !self.write_byte(byte) {
            core::hint::spin_loop();
        }
    }
    pub fn write_slice_blocking(&self, bytes: &[u8]) {
        for b in bytes {
            self.write_byte(*b);
        }
    }
}
impl From<BufferedUart> for Uart {
    fn from(value: BufferedUart) -> Self {
        value.inner
    }
}

#[derive(Copy, Clone)]
pub struct UartWriter<'u>(&'u Uart);
impl<'u> core::fmt::Write for UartWriter<'u> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_slice_blocking(s.as_bytes());
        core::fmt::Result::Ok(())
    }
}

pub struct BufferedUart {
    inner: Uart,
    buffer: Mutex<Queue<u8>>,
}
impl BufferedUart {
    pub const fn new(base_address: usize) -> BufferedUart {
        BufferedUart {
            inner: Uart::new(base_address),
            buffer: Mutex::new(Queue::new()),
        }
    }
    pub fn interrupt(&self) {
        let _ = InterruptBlocker::new();

        self.inner.interrupt();

        // Send buffered characters.
        let buf = self.buffer.lock_spinning();
        self.send_buffered_bytes(buf);
    }
    pub fn writer(&self) -> BufferedUartWriter<'_> {
        BufferedUartWriter(self)
    }
    pub fn writer_unbuffered(&self) -> UartWriter<'_> {
        self.inner.writer()
    }
    /// Write a byte to the UART and buffer it.
    /// Should not be used in interrupts.
    pub fn write_byte_buffered(&self, byte: u8) {
        let mut buf = self.buffer.lock_spinning();

        // Sleep until there is space in the buffer.
        while buf.space_remaining() == 0 {
            unsafe {
                buf.sleep(addr_of!(*self).cast_mut().cast());
            }
        }

        // Add the byte onto the end of the queue.
        buf.push_back(byte).expect("space in the uart queue");
        // Drop buf so that send_buffered_bytes() can lock it again.
        self.send_buffered_bytes(buf);
    }
    /// Write a slice to the UART and buffer it.
    /// Should not be used in interrupts.
    pub fn write_slice_buffered(&self, bytes: &[u8]) {
        for b in bytes {
            self.write_byte_buffered(*b);
        }
    }
    /// If the UART is idle and a character is
    /// waiting in the transmit buffer, send it.
    /// Returns how many bytes were sent.
    fn send_buffered_bytes(&self, mut buf: MutexGuard<'_, Queue<u8>>) -> usize {
        let mut i = 0;

        loop {
            if !self.inner.can_write_byte() {
                // The UART transmit holding register is full,
                // so we cannot give it another byte.
                // It will interrupt when it's ready for a new byte.
                break;
            }

            // Pop a byte from the front of the queue and send it.
            match buf.pop_front() {
                Some(b) => self.inner.write_byte(b),
                // The buffer is empty, we're finished sending bytes.
                None => return 0,
            };

            i += 1;

            // Check if uartputc() is waiting for space in the buffer.
            unsafe {
                wakeup(addr_of!(*self).cast_mut().cast());
            }
        }

        i
    }
}
impl core::ops::Deref for BufferedUart {
    type Target = Uart;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl From<Uart> for BufferedUart {
    fn from(value: Uart) -> Self {
        BufferedUart {
            inner: value,
            buffer: Mutex::new(Queue::new()),
        }
    }
}

#[derive(Copy, Clone)]
pub struct BufferedUartWriter<'u>(&'u BufferedUart);
impl<'u> core::fmt::Write for BufferedUartWriter<'u> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_slice_buffered(s.as_bytes());
        core::fmt::Result::Ok(())
    }
}
