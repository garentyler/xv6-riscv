#[cfg(target_arch = "riscv64")]
pub mod riscv;

pub mod trap;

pub mod cpu {
    #[cfg(target_arch = "riscv64")]
    pub use super::riscv::cpu::cpu_id;
}

pub mod interrupt {
    #[cfg(target_arch = "riscv64")]
    pub use crate::hal::{
        arch::riscv::asm::{
            intr_get as interrupts_enabled, intr_off as disable_interrupts,
            intr_on as enable_interrupts,
        },
        hardware::riscv::plic::{
            plic_claim as handle_interrupt, plic_complete as complete_interrupt, plicinit as init,
            plicinithart as inithart,
        },
    };
}

pub mod mem {
    #[cfg(target_arch = "riscv64")]
    pub use super::riscv::{
        asm::sfence_vma as flush_cached_pages,
        mem::{
            kstack, Pagetable, PAGE_SIZE, PHYSICAL_END, PTE_R, PTE_W, PTE_X, TRAMPOLINE, TRAPFRAME,
        },
    };

    pub fn round_up_page(size: usize) -> usize {
        (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
    }

    pub fn round_down_page(addr: usize) -> usize {
        addr & !(PAGE_SIZE - 1)
    }
}

pub mod virtual_memory {
    #[cfg(target_arch = "riscv64")]
    pub use super::riscv::virtual_memory::{
        copyin, copyinstr, copyout, either_copyin, either_copyout, kvminit as init,
        kvminithart as inithart, mappages, uvmalloc, uvmcopy, uvmcreate, uvmdealloc, uvmfirst,
        uvmfree, uvmunmap,
    };
}

pub mod clock {
    #[cfg(target_arch = "riscv64")]
    pub use super::riscv::trap::CLOCK_TICKS;
}
