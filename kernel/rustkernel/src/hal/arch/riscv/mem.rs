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

pub type PagetableEntry = u64;
pub type Pagetable = *mut [PagetableEntry; 512];

/// The PagetableEntry is valid.
pub const PTE_V: i32 = 1 << 0;
/// The PagetableEntry is readable.
pub const PTE_R: i32 = 1 << 1;
/// The PagetableEntry is writable.
pub const PTE_W: i32 = 1 << 2;
/// The PagetableEntry is executable.
pub const PTE_X: i32 = 1 << 3;
/// The PagetableEntry is user-accessible.
pub const PTE_U: i32 = 1 << 4;

/// Page-based 39-bit virtual addressing.
/// Details at section 5.4 of the RISC-V specification.
pub const SATP_SV39: u64 = 8 << 60;

pub fn make_satp(pagetable: Pagetable) -> u64 {
    SATP_SV39 | (pagetable as usize as u64 >> 12)
}

/// Bytes per page.
pub const PAGE_SIZE: usize = 4096;
/// Bits of offset within a page
const PAGE_OFFSET: usize = 12;
/// The kernel starts here.
pub const KERNEL_BASE: usize = 0x8000_0000;
/// The end of physical memory.
pub const PHYSICAL_END: usize = KERNEL_BASE + (128 * 1024 * 1024);
/// The maximum virtual address.
///
/// VIRTUAL_MAX is actually one bit less than the max allowed by
/// Sv39 to avoid having to sign-extend virtual addresses
/// that have the high bit set.
pub const VIRTUAL_MAX: usize = 1 << (9 + 9 + 9 + 12 - 1);
/// Map the trampoline page to the highest
/// address in both user and kernel space.
pub const TRAMPOLINE: usize = VIRTUAL_MAX - PAGE_SIZE;
/// Map kernel stacks beneath the trampoline,
/// each surrounded by invalid guard pages.
pub fn kstack(page: usize) -> usize {
    TRAMPOLINE - (page + 1) * 2 * PAGE_SIZE
}
/// User memory layout.
/// Address zero first:
/// - text
/// - original data and bss
/// - fixed-size stack
/// - expandable heap
///   ...
/// - TRAPFRAME (p->trapframe, used by the trampoline)
/// - TRAMPOLINE (the same page as in the kernel)
pub const TRAPFRAME: usize = TRAMPOLINE - PAGE_SIZE;

// Convert a physical address to a PagetableEntry.
pub fn pa2pte(pa: usize) -> usize {
    (pa >> 12) << 10
}
// Convert a PagetableEntry to a physical address.
pub fn pte2pa(pte: usize) -> usize {
    (pte >> 10) << 12
}

// Extract the three 9-bit page table indices from a virtual address.
const PXMASK: usize = 0x1ffusize; // 9 bits.

fn pxshift(level: usize) -> usize {
    PAGE_OFFSET + (level * 9)
}
pub fn px(level: usize, virtual_addr: usize) -> usize {
    (virtual_addr >> pxshift(level)) & PXMASK
}
