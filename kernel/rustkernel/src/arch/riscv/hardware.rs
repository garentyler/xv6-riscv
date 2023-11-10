// QEMU puts UART registers here in physical memory.
pub const UART0: usize = 0x10000000;
pub const UART0_IRQ: usize = 10;

// Virtio MMIO interface
pub const VIRTIO0: usize = 0x10001000;
pub const VIRTIO0_IRQ: usize = 1;
