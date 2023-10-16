// Physical memory layout

// QEMU -machine virt is setup like this,
// based on QEMU's hw/riscv/virt.c
//
// 00001000 - boot ROM, provided by qemu
// 02000000 - CLINT
// 0C000000 - PLIC
// 10000000 - uart0
// 10001000 - virtio disk
// 80000000 - boot ROM jumps here in machine mode (kernel loads the kernel here)
// unused after 8000000

// The kernel uses physical memory as so:
// 80000000 - entry.S, then kernel text and data
// end      - start of kernel page allocation data
// PHYSTOP  - end of RAM used by the kernel

use super::{MAXVA, PGSIZE};

// QEMU puts UART registers here in physical memory.
pub const UART0: u64 = 0x10000000;
pub const UART0_IRQ: i32 = 10;

// Virtio MMIO interface
pub const VIRTIO0: u64 = 0x10001000;
pub const VIRTIO0_IRQ: i32 = 1;

// Core Local Interrupter (CLINT), which contains the timer.
pub const CLINT: u64 = 0x2000000;
pub const CLINT_MTIME: u64 = CLINT + 0xbff8;
pub fn clint_mtimecmp(hartid: u64) -> u64 {
    CLINT + 0x4000 + (8 * hartid)
}

// QEMU puts platform-level interrupt controller (PLIC) here.
pub const PLIC: u64 = 0x0c000000;
pub const PLIC_PRIORITY: u64 = PLIC;
pub const PLIC_PENDING: u64 = PLIC + 0x1000;
pub fn plic_menable(hartid: u64) -> u64 {
    PLIC + 0x2000 + (0x100 * hartid)
}
pub fn plic_senable(hartid: u64) -> u64 {
    PLIC + 0x2080 + (0x100 * hartid)
}
pub fn plic_mpriority(hartid: u64) -> u64 {
    PLIC + 0x200000 + (0x2000 * hartid)
}
pub fn plic_spriority(hartid: u64) -> u64 {
    PLIC + 0x201000 + (0x2000 * hartid)
}
pub fn plic_mclaim(hartid: u64) -> u64 {
    PLIC + 0x200004 + (0x2000 * hartid)
}
pub fn plic_sclaim(hartid: u64) -> u64 {
    PLIC + 0x201004 + (0x2000 * hartid)
}

// The kernel expects there to be RAM
// for use by the kernel and user pages
// from physical address 0x80000000 to PHYSTOP.
pub const KERNBASE: u64 = 0x80000000;
pub const PHYSTOP: u64 = KERNBASE + 128 * 1024 * 1024;

// Map the trampoline page to the highest address,
// in both user and kernel space.
pub const TRAMPOLINE: u64 = MAXVA - PGSIZE;

// Map kernel stacks beneath the trampoline,
// each surrouned by invalid guard pages.
pub fn kstack(p: u64) -> u64 {
    TRAMPOLINE - (p + 1) * 2 * PGSIZE
}

// User memory layout.
// Address zero first:
// - text
// - original data and bss
// - fixed-size stack
// - expandable heap
//   ...
// - TRAPFRAME (p->trapframe, used by the trampoline)
// - TRAMPOLINE (the same page as in the kernel)
pub const TRAPFRAME: u64 = TRAMPOLINE - PGSIZE;
