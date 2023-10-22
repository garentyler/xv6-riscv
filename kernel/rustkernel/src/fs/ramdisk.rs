//! Ramdisk that uses the disk image loaded by qemu -initrd fs.img

use crate::io::buf::Buffer;

extern "C" {
    pub fn ramdiskinit();
    pub fn ramdiskrw(buffer: *mut Buffer);
}
