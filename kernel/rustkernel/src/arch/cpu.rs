//! Architecture-agnostic CPU functions.

#[cfg(target_arch = "riscv64")]
pub use super::riscv::cpu::cpu_id;
