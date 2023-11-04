use super::{context::Context, proc::Proc};
use crate::arch::riscv::asm::r_tp;
use core::ptr::{addr_of_mut, null_mut};

pub static mut CPUS: [Cpu; crate::NCPU] = [Cpu::new(); crate::NCPU];

/// Per-CPU state.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Cpu {
    pub proc: *mut Proc,
    /// swtch() here to enter scheduler()
    pub context: Context,
    /// Depth of push_off() nesting.
    pub interrupt_disable_layers: i32,
    /// Were interrupts enabled before push_off()?
    pub previous_interrupts_enabled: i32,
}
impl Cpu {
    pub const fn new() -> Cpu {
        Cpu {
            proc: null_mut(),
            context: Context::new(),
            interrupt_disable_layers: 0,
            previous_interrupts_enabled: 0,
        }
    }
    /// Must be called with interrupts disabled
    /// to prevent race with process being moved
    /// to a different CPU.
    pub fn current_id() -> usize {
        unsafe { r_tp() as usize }
    }
    /// Return this CPU's cpu struct.
    /// Interrupts must be disabled.
    pub fn current() -> &'static mut Cpu {
        unsafe { &mut CPUS[Cpu::current_id()] }
    }
}

/// Return this CPU's cpu struct.
/// Interrupts must be disabled.
#[no_mangle]
pub unsafe extern "C" fn mycpu() -> *mut Cpu {
    addr_of_mut!(*Cpu::current())
}
