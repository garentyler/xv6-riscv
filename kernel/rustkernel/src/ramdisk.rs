//! Ramdisk that uses the disk image loaded by qemu -initrd fs.img

extern "C" {
    pub fn ramdiskrw(buffer: *mut Buf);
}

#[no_mangle]
pub extern "C" fn ramdiskinit() {}
