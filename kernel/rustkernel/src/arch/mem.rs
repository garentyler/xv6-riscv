//! Architecture-agnostic memory management.

#[cfg(target_arch = "riscv64")]
pub use super::riscv::{
    asm::sfence_vma as flush_cached_pages,
    mem::{
        Pagetable, PagetableEntry, KERNEL_BASE, PAGE_SIZE, PHYSICAL_END, TRAMPOLINE, TRAPFRAME,
        PTE_V, PTE_R, PTE_W, PTE_X, PTE_U,
        VIRTUAL_MAX,
    },
};

pub fn round_up_page(size: usize) -> usize {
    (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

pub fn round_down_page(addr: usize) -> usize {
    addr & !(PAGE_SIZE - 1)
}
