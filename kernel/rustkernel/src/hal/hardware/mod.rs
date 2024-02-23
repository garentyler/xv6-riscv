//! Device drivers and hardware implementations.

pub mod ramdisk;
pub mod uart;
pub mod virtio_disk;

#[cfg(target_arch = "riscv64")]
pub mod riscv;
