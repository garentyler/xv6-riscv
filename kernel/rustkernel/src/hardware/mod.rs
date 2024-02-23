//! Device drivers and hardware implementations.

pub mod ramdisk;
pub mod uart;
pub mod virtio_disk;

use uart::BufferedUart;

pub static UARTS: [(usize, BufferedUart); 1] = [(10, BufferedUart::new(0x1000_0000))];
