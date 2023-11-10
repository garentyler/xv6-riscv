#[cfg(target_arch = "riscv64")]
pub mod riscv;
#[cfg(target_arch = "riscv64")]
pub use riscv::hardware;

pub mod cpu;
pub mod interrupt;
pub mod mem;
pub mod power;
