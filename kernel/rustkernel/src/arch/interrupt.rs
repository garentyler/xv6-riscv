//! Architecture-agnostic interrupt handling.

#[cfg(target_arch = "riscv64")]
pub use super::riscv::{
    asm::{
        intr_get as interrupts_enabled, intr_off as disable_interrupts,
        intr_on as enable_interrupts,
    },
    plic::{
        plic_claim as handle_interrupt, plic_complete as complete_interrupt, plicinit as init,
        plicinithart as inithart,
    },
};
