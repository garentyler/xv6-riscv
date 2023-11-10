//! Architecture-agnostic power handling.

#[cfg(target_arch = "riscv64")]
pub use super::riscv::power::shutdown;
