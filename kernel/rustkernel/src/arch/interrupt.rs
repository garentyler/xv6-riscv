//! Architecture-agnostic interrupt handling.

#[cfg(target_arch = "riscv64")]
pub use super::riscv::plic::plic_claim as handle_interrupt;
#[cfg(target_arch = "riscv64")]
pub use super::riscv::plic::plic_complete as complete_interrupt;
#[cfg(target_arch = "riscv64")]
pub use super::riscv::plic::plicinit as init;
#[cfg(target_arch = "riscv64")]
pub use super::riscv::plic::plicinithart as inithart;
