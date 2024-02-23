#[cfg(feature = "milk-v")]
mod milk_v;
#[cfg(feature = "milk-v")]
pub use milk_v::*;
#[cfg(feature = "qemu-riscv64")]
mod qemu_riscv64;
#[cfg(feature = "qemu-riscv64")]
pub use qemu_riscv64::*;

#[cfg(not(any(feature = "milk-v", feature = "qemu-riscv64")))]
compile_error!("a platform must be selected");
