//! Architecture-agnostic trap handling.

#[cfg(target_arch = "riscv64")]
pub use super::riscv::trap::trapinithart as inithart;

use super::interrupt;
use crate::proc::cpu::Cpu;

#[derive(Default)]
pub struct InterruptBlocker;
impl InterruptBlocker {
    pub fn new() -> InterruptBlocker {
        unsafe {
            let interrupts_before = interrupt::interrupts_enabled();
            let cpu = Cpu::current();

            interrupt::disable_interrupts();

            if cpu.interrupt_disable_layers == 0 {
                cpu.previous_interrupts_enabled = interrupts_before;
            }
            cpu.interrupt_disable_layers += 1;
            // crate::sync::spinlock::push_off();
        }
        InterruptBlocker
    }
}
impl core::ops::Drop for InterruptBlocker {
    fn drop(&mut self) {
        unsafe {
            let cpu = Cpu::current();

            if interrupt::interrupts_enabled() == 1 || cpu.interrupt_disable_layers < 1 {
                // panic!("pop_off mismatched");
                return;
            }

            cpu.interrupt_disable_layers -= 1;

            if cpu.interrupt_disable_layers == 0 && cpu.previous_interrupts_enabled == 1 {
                interrupt::enable_interrupts();
            }
            // crate::sync::spinlock::pop_off();
        }
    }
}
impl !Send for InterruptBlocker {}

pub unsafe fn push_intr_off() {
    let old = interrupt::interrupts_enabled();
    let cpu = Cpu::current();

    interrupt::disable_interrupts();
    if cpu.interrupt_disable_layers == 0 {
        cpu.previous_interrupts_enabled = old;
    }
    cpu.interrupt_disable_layers += 1;
}
pub unsafe fn pop_intr_off() {
    let cpu = Cpu::current();

    if interrupt::interrupts_enabled() == 1 {
        // crate::panic_byte(b'0');
        panic!("pop_intr_off - interruptible");
    } else if cpu.interrupt_disable_layers < 1 {
        // crate::panic_byte(b'1');
        panic!("pop_intr_off");
    }

    cpu.interrupt_disable_layers -= 1;

    if cpu.interrupt_disable_layers == 0 && cpu.previous_interrupts_enabled == 1 {
        interrupt::enable_interrupts();
    }
}
