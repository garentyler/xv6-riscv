use crate::{
    arch::riscv::{
        memlayout::{KERNBASE, PHYSTOP, TRAMPOLINE},
        *,
    },
    mem::{
        kalloc::{kalloc, kfree},
        memmove, memset,
    },
    proc::proc_mapstacks,
};
use core::ptr::{addr_of, addr_of_mut, null_mut};

extern "C" {
    /// kernel.ld sets this to end of kernel code.
    pub static etext: [u8; 0];
    /// trampoline.S
    pub static trampoline: [u8; 0];
}

/// The kernel's pagetable.
pub static mut KERNEL_PAGETABLE: Pagetable = null_mut();

/// Make a direct-map page table for the kernel.
pub unsafe fn kvmmake() -> Pagetable {
    let pagetable = kalloc() as Pagetable;
    memset(pagetable.cast(), 0, PGSIZE as u32);

    // QEMU test interface used for power management.
    kvmmap(pagetable, QEMU_POWER, QEMU_POWER, PGSIZE, PTE_R | PTE_W);

    // UART registers
    kvmmap(pagetable, UART0 as u64, UART0 as u64, PGSIZE, PTE_R | PTE_W);

    // VirtIO MMIO disk interface
    kvmmap(pagetable, VIRTIO0, VIRTIO0, PGSIZE, PTE_R | PTE_W);

    // PLIC
    kvmmap(pagetable, PLIC, PLIC, 0x400000u64, PTE_R | PTE_W);

    let etext_addr = addr_of!(etext) as usize as u64;

    // Map kernel text executable and read-only.
    kvmmap(
        pagetable,
        KERNBASE,
        KERNBASE,
        etext_addr - KERNBASE,
        PTE_R | PTE_X,
    );

    // Map kernel data and the physical RAM we'll make use of.
    kvmmap(
        pagetable,
        etext_addr,
        etext_addr,
        PHYSTOP - etext_addr,
        PTE_R | PTE_W,
    );

    // Map the trampoine for trap entry/exit to
    // the highest virtual address in the kernel.
    kvmmap(
        pagetable,
        TRAMPOLINE,
        addr_of!(trampoline) as usize as u64,
        PGSIZE,
        PTE_R | PTE_X,
    );

    // Allocate and map a kernel stack for each process.
    proc_mapstacks(pagetable);

    pagetable
}

/// Initialize the one kernel_pagetable.
pub unsafe fn kvminit() {
    KERNEL_PAGETABLE = kvmmake();
}

/// Switch hardware pagetable register to the kernel's pagetable and enable paging.
pub unsafe fn kvminithart() {
    // Wait for any previous writes to the pagetable memory to finish.
    asm::sfence_vma();

    asm::w_satp(make_satp(KERNEL_PAGETABLE));

    // Flush stale entries from the TLB.
    asm::sfence_vma();
}

/// Return the address of the PTE in pagetable
/// `pagetable` that corresponds to virtual address
/// `virtual_addr`. If `alloc` != 0, create any
/// required pagetable pages.
///
/// The RISC-V Sv39 scheme has three levels of pagetable
/// pages. A pagetable page contains 512 64-bit PTEs.
///
/// A 64-bit virtual address is split into five fields:
/// - 0..12: 12 bits of byte offset within the page.
/// - 12..20: 9 bits of level 0 index.
/// - 21..30: 9 bits of level 0 index.
/// - 30..39: 9 bits of level 0 index.
/// - 39..64: Must be zero.
pub unsafe fn walk(mut pagetable: Pagetable, virtual_addr: u64, alloc: i32) -> *mut PagetableEntry {
    if virtual_addr > MAXVA {
        panic!("walk");
    }

    let mut level = 2;
    while level > 0 {
        let pte = addr_of_mut!(
            pagetable.as_mut().unwrap()[((virtual_addr >> (12 + (level * 9))) & 0x1ffu64) as usize]
        );

        if (*pte) & PTE_V as u64 > 0 {
            pagetable = (((*pte) >> 10) << 12) as usize as Pagetable;
        } else {
            if alloc == 0 {
                return null_mut();
            }

            pagetable = kalloc() as Pagetable;

            if pagetable.is_null() {
                return null_mut();
            }

            memset(pagetable.cast(), 0, PGSIZE as u32);
            *pte = (((pagetable as usize) >> 12) << 10) as PagetableEntry | PTE_V as u64;
        }

        level -= 1;
    }

    addr_of_mut!(pagetable.as_mut().unwrap()[(virtual_addr as usize >> 12) & 0x1ffusize])
}

/// Look up a virtual address and return the physical address or 0 if not mapped.
///
/// Can only be used to look up user pages.
#[no_mangle]
pub unsafe extern "C" fn walkaddr(pagetable: Pagetable, virtual_addr: u64) -> u64 {
    if virtual_addr > MAXVA {
        return 0;
    }

    let pte = walk(pagetable, virtual_addr, 0);
    if pte.is_null() || *pte & PTE_V as u64 == 0 || *pte & PTE_U as u64 == 0 {
        return 0;
    }

    pte2pa(*pte as usize) as u64
}

/// Add a mapping to the kernel page table.
///
/// Only used when booting.
/// Does not flush TLB or enable paging.
#[no_mangle]
pub unsafe extern "C" fn kvmmap(
    pagetable: Pagetable,
    virtual_addr: u64,
    physical_addr: u64,
    size: u64,
    perm: i32,
) {
    if mappages(pagetable, virtual_addr, size, physical_addr, perm) != 0 {
        panic!("kvmmap");
    }
}

/// Create PagetableEntries for virtual addresses starting at `virtual_addr`
/// that refer to physical addresses starting at `physical_addr`.
///
/// `virtual_addr` and size might not be page-aligned.
/// Returns 0 on success, -1 if walk() couldn't allocate a needed pagetable page.
#[no_mangle]
pub unsafe extern "C" fn mappages(
    pagetable: Pagetable,
    virtual_addr: u64,
    size: u64,
    mut physical_addr: u64,
    perm: i32,
) -> i32 {
    if size == 0 {
        panic!("mappages: size = 0");
    }

    let mut a = pg_round_down(virtual_addr);
    let last = pg_round_down(virtual_addr + size - 1);

    loop {
        let pte = walk(pagetable, a, 1);

        if pte.is_null() {
            return -1;
        }
        if (*pte) & PTE_V as u64 > 0 {
            panic!("mappages: remap");
        }

        *pte = ((physical_addr >> 12) << 10) | perm as u64 | PTE_V as u64;

        if a == last {
            break;
        } else {
            a += PGSIZE;
            physical_addr += PGSIZE;
        }
    }

    0
}

/// Remove `npages` of mappings starting from `virtual_addr`.
///
/// `virtual_addr` amust be page-aligned. The mappings must exist.
/// Optionally free the physical memory.
#[no_mangle]
pub unsafe extern "C" fn uvmunmap(
    pagetable: Pagetable,
    virtual_addr: u64,
    num_pages: u64,
    do_free: i32,
) {
    if virtual_addr % PGSIZE != 0 {
        panic!("uvmunmap: not aligned");
    }
    let mut a = virtual_addr;
    while a < virtual_addr + num_pages * PGSIZE {
        let pte = walk(pagetable, a, 0);
        if pte.is_null() {
            panic!("uvmunmap: walk");
        } else if (*pte) & PTE_V as u64 == 0 {
            panic!("uvmunmap: not mapped");
        } else if ((*pte) & 0x3ffu64) == PTE_V as u64 {
            panic!("uvmunmap: not a leaf");
        } else if do_free > 0 {
            let physical_addr = (((*pte) >> 10) << 12) as usize as *mut u8;
            kfree(physical_addr.cast());
        }

        *pte = 0;
        a += PGSIZE;
    }
}

/// Create an empty user pagetable.
///
/// Returns 0 if out of memory.
#[no_mangle]
pub unsafe extern "C" fn uvmcreate() -> Pagetable {
    let pagetable = kalloc() as Pagetable;
    if pagetable.is_null() {
        return null_mut();
    }
    memset(pagetable.cast(), 0, PGSIZE as u32);
    pagetable
}

/// Load the user initcode into address 0 of pagetable for the very first process.
///
/// `size` must be less than `PGSIZE`.
#[no_mangle]
pub unsafe extern "C" fn uvmfirst(pagetable: Pagetable, src: *mut u8, size: u32) {
    if size >= PGSIZE as u32 {
        panic!("uvmfirst: more than a page");
    }

    let mem = kalloc();
    memset(mem, 0, PGSIZE as u32);
    mappages(
        pagetable,
        0,
        PGSIZE,
        mem as usize as u64,
        PTE_W | PTE_R | PTE_X | PTE_U,
    );
    memmove(mem, src, size);
}

/// Allocate PagetableEntries and physical memory to grow process
/// from `old_size` to `new_size`, which need not be page aligned.
///
/// Returns new size or 0 on error.
#[no_mangle]
pub unsafe extern "C" fn uvmalloc(
    pagetable: Pagetable,
    mut old_size: u64,
    new_size: u64,
    xperm: i32,
) -> u64 {
    if new_size < old_size {
        return old_size;
    }

    old_size = pg_round_up(old_size);
    let mut a = old_size;

    while a < new_size {
        let mem = kalloc();
        if mem.is_null() {
            uvmdealloc(pagetable, a, old_size);
            return 0;
        }

        memset(mem.cast(), 0, PGSIZE as u32);

        if mappages(
            pagetable,
            a,
            PGSIZE,
            mem as usize as u64,
            PTE_R | PTE_U | xperm,
        ) != 0
        {
            kfree(mem.cast());
            uvmdealloc(pagetable, a, old_size);
            return 0;
        }

        a += PGSIZE;
    }

    new_size
}

/// Deallocate user pages to bring the process size from `old_size` to `new_size`.
///
/// `old_size` and `new_size` need not be page-aligned, nor does `new_size` need
/// to be less than `old_size`. `old_size` can be larget than the actual process
/// size. Returns the new process size.
#[no_mangle]
pub unsafe extern "C" fn uvmdealloc(pagetable: Pagetable, old_size: u64, new_size: u64) -> u64 {
    if new_size >= old_size {
        return old_size;
    }

    if pg_round_up(new_size) < pg_round_up(old_size) {
        let num_pages = (pg_round_up(old_size) - pg_round_up(new_size)) / PGSIZE;
        uvmunmap(pagetable, pg_round_up(new_size), num_pages, 1);
    }

    new_size
}

/// Recursively free pagetable pages.
///
/// All leaf mappings must have already been removed.
#[no_mangle]
pub unsafe extern "C" fn freewalk(pagetable: Pagetable) {
    // There are 2^9 = 512 PagetableEntry's in a Pagetable.
    for i in 0..512 {
        let pte: &mut PagetableEntry = &mut pagetable.as_mut().unwrap()[i];
        if *pte & PTE_V as u64 > 0 && (*pte & (PTE_R | PTE_W | PTE_X) as u64) == 0 {
            // This PagetableEntry points to a lower-level pagetable.
            let child = ((*pte) >> 10) << 12;
            freewalk(child as usize as Pagetable);
            *pte = 0;
        } else if *pte & PTE_V as u64 > 0 {
            panic!("freewalk: leaf");
        }
    }
    kfree(pagetable.cast());
}

/// Free user memory pages, then free pagetable pages.
#[no_mangle]
pub unsafe extern "C" fn uvmfree(pagetable: Pagetable, size: u64) {
    if size > 0 {
        uvmunmap(pagetable, 0, pg_round_up(size) / PGSIZE, 1);
    }
    freewalk(pagetable);
}

/// Given a parent process's pagetable, copy
/// its memory into a child's pagetable.
///
/// Copies both the pagetable and the physical memory.
/// Returns 0 on success, -1 on failure.
/// Frees any allocated pages on failure.
#[no_mangle]
pub unsafe extern "C" fn uvmcopy(old: Pagetable, new: Pagetable, size: u64) -> i32 {
    let mut i = 0;

    while i < size {
        let pte = walk(old, i, 0);
        if pte.is_null() {
            panic!("uvmcopy: PagetableEntry should exist");
        } else if (*pte) & PTE_V as u64 == 0 {
            panic!("uvmcopy: page not present");
        }

        let pa = ((*pte) >> 10) << 12;
        let flags = (*pte) & 0x3ffu64;

        let mem = kalloc();
        if mem.is_null() {
            uvmunmap(new, 0, i / PGSIZE, 1);
            return -1;
        }

        memmove(mem.cast(), (pa as usize as *mut u8).cast(), PGSIZE as u32);

        if mappages(new, i, PGSIZE, mem as usize as u64, flags as i32) != 0 {
            kfree(mem.cast());
            uvmunmap(new, 0, i / PGSIZE, 1);
            return -1;
        }

        i += PGSIZE;
    }

    0
}

/// Mark a PagetableEntry invalid for user access.
///
/// Used by exec for the user stack guard page.
#[no_mangle]
pub unsafe extern "C" fn uvmclear(pagetable: Pagetable, virtual_addr: u64) {
    let pte = walk(pagetable, virtual_addr, 0);
    if pte.is_null() {
        panic!("uvmclear");
    }
    *pte &= !(PTE_U as u64);
}

/// Copy from kernel to user.
///
/// Copy `len` bytes from `src` to virtual address `dst_virtual_addr` in a given pagetable.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn copyout(
    pagetable: Pagetable,
    mut dst_virtual_addr: u64,
    mut src: *mut u8,
    mut len: u64,
) -> i32 {
    while len > 0 {
        let va0 = pg_round_down(dst_virtual_addr);
        let pa0 = walkaddr(pagetable, va0);
        if pa0 == 0 {
            return -1;
        }

        let mut n = PGSIZE - (dst_virtual_addr - va0);
        if n > len {
            n = len;
        }
        memmove(
            ((pa0 + dst_virtual_addr - va0) as usize as *mut u8).cast(),
            src,
            n as u32,
        );

        len -= n;
        src = src.add(n as usize);
        dst_virtual_addr = va0 + PGSIZE;
    }
    0
}

/// Copy from user to kernel.
///
/// Copy `len` bytes to `dst` from virtual address `src_virtual_addr` in a given pagetable.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn copyin(
    pagetable: Pagetable,
    mut dst: *mut u8,
    mut src_virtual_addr: u64,
    mut len: u64,
) -> i32 {
    while len > 0 {
        let va0 = pg_round_down(src_virtual_addr);
        let pa0 = walkaddr(pagetable, va0);
        if pa0 == 0 {
            return -1;
        }

        let mut n = PGSIZE - (src_virtual_addr - va0);
        if n > len {
            n = len;
        }
        memmove(
            dst.cast(),
            ((pa0 + src_virtual_addr - va0) as usize as *mut u8).cast(),
            n as u32,
        );

        len -= n;
        dst = dst.add(n as usize);
        src_virtual_addr = va0 + PGSIZE;
    }
    0
}

/// Copy a null-terminated string from user to kernel.
///
/// Copy bytes to `dst` from virtual address `src_virtual_addr`
/// in a given pagetable, until b'\0' or `max` is reached.
/// Returns 0 on success, -1 on error.
pub unsafe fn copyinstr(
    pagetable: Pagetable,
    mut dst: *mut u8,
    mut src_virtual_addr: u64,
    mut max: u64,
) -> i32 {
    let mut got_null = false;

    while !got_null && max > 0 {
        let va0 = pg_round_down(src_virtual_addr);
        let pa0 = walkaddr(pagetable, va0);
        if pa0 == 0 {
            return -1;
        }

        let mut n = PGSIZE - (src_virtual_addr - va0);
        if n > max {
            n = max;
        }

        let mut p = (pa0 + src_virtual_addr - va0) as *const u8;
        while n > 0 {
            if *p == b'\0' {
                *dst = b'\0';
                got_null = true;
                break;
            } else {
                *dst = *p;
            }

            n -= 1;
            max -= 1;
            p = p.add(1);
            dst = dst.add(1);
        }

        src_virtual_addr = va0 + PGSIZE;
    }

    if got_null {
        0
    } else {
        -1
    }
}
