use crate::hal::hardware::uart::BufferedUart;

pub static DIRECT_MAPPED_PAGES: [usize; 1] = [QEMU_POWER];

// Devices: (IRQ, driver)
pub static UARTS: [(usize, BufferedUart); 1] = [(10, BufferedUart::new(0x1000_0000))];
pub static VIRTIO_DISKS: [(usize, usize); 1] = [(1, 0x10001000)];

// Virtio MMIO interface
pub const VIRTIO0: usize = 0x10001000;
pub const VIRTIO0_IRQ: usize = 1;

// Platform Interrupt Controller location
pub const PLIC_BASE_ADDR: usize = 0x0c000000;

/// QEMU test interface. Used for power off and on.
const QEMU_POWER: usize = 0x100000;

pub unsafe fn shutdown() -> ! {
    let qemu_power = QEMU_POWER as *mut u32;
    qemu_power.write_volatile(0x5555u32);
    unreachable!();
}
