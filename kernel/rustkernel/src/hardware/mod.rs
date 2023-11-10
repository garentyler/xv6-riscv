//! Device drivers and hardware implementations.

pub mod ramdisk;
pub mod uart;
pub mod virtio_disk;

use uart::Uart;

pub static UARTS: [(usize, Uart); 1] = [(10, Uart::new(0x1000_0000))];
