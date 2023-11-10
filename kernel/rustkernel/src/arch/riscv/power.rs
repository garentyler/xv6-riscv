/// QEMU test interface. Used for power off and on.
pub const QEMU_POWER: usize = 0x100000;

pub unsafe fn shutdown() -> ! {
    let qemu_power = QEMU_POWER as *mut u32;
    qemu_power.write_volatile(0x5555u32);
    unreachable!();
}
