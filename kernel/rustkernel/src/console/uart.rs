//! Low-level driver routines for 16550a UART.
#![allow(non_upper_case_globals)]

use crate::{
    console::consoleintr, proc::wakeup, queue::Queue, sync::spinlock::Spinlock,
    trap::InterruptBlocker,
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

pub static UART0: Uart = Uart::new(crate::riscv::memlayout::UART0);

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
    pub lock: Spinlock,
    pub base_address: usize,
    pub buffer: Queue<u8>,
}
impl Uart {
    pub const fn new(base_address: usize) -> Uart {
        Uart {
            lock: Spinlock::new(),
            base_address,
            buffer: Queue::new(),
        }
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
    pub fn interrupt(&self) {
        // Read and process incoming data.
        while let Some(b) = self.read_byte() {
            consoleintr(b);
        }

        // Send buffered characters.
        let _guard = self.lock.lock();
        self.send_buffered_bytes();
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
    /// Write a byte to the UART without interrupts.
    /// Used for kernel printing and character echoing.
    pub fn write_byte(&self, b: u8) {
        let _ = InterruptBlocker::new();

        if unsafe { crate::PANICKED } {
            loop {
                core::hint::spin_loop();
            }
        }

        // Wait for Transmit Holding Empty to be set in LSR.
        while Register::LineStatus.read(self.base_address) & LSR_TX_IDLE == 0 {
            core::hint::spin_loop();
        }

        Register::TransmitHolding.write(self.base_address, b);
    }
    pub fn write_slice(&self, bytes: &[u8]) {
        for b in bytes {
            self.write_byte(*b);
        }
    }
    /// Write a byte to the UART and buffer it.
    /// Should not be used in interrupts.
    pub fn write_byte_buffered(&self, b: u8) {
        let guard = self.lock.lock();

        if unsafe { crate::PANICKED } {
            loop {
                core::hint::spin_loop();
            }
        }

        // Sleep until there is space in the buffer.
        while self.buffer.space_remaining() == 0 {
            unsafe {
                guard.sleep(addr_of!(*self).cast_mut().cast());
            }
        }

        // Unsafely cast self as mutable.
        // self.lock is held so it should be fine.
        let this: &mut Uart = unsafe { &mut *addr_of!(*self).cast_mut() };

        // Add the byte onto the end of the queue.
        this.buffer.push_back(b).expect("space in the uart queue");
        this.send_buffered_bytes();
    }
    pub fn write_slice_buffered(&self, bytes: &[u8]) {
        for b in bytes {
            self.write_byte_buffered(*b);
        }
    }
    /// If the UART is idle, and a character is
    /// waiting in the transmit buffer, send it.
    /// self.lock should be held.
    fn send_buffered_bytes(&self) {
        let this: &mut Uart = unsafe { &mut *addr_of!(*self).cast_mut() };

        loop {
            if Register::LineStatus.read(this.base_address) & LSR_TX_IDLE == 0 {
                // The UART transmit holding register is full,
                // so we cannot give it another byte.
                // It will interrupt when it's ready for a new byte.
                return;
            }

            // Pop a byte from the front of the queue.
            let Some(b) = this.buffer.pop_front() else {
                // The buffer is empty, we're finished sending bytes.
                return;
            };

            // Maybe uartputc() is waiting for space in the buffer.
            unsafe {
                wakeup(addr_of!(*self).cast_mut().cast());
            }

            Register::TransmitHolding.write(this.base_address, b);
        }
    }
}
