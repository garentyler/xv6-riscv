use super::{
    asm,
    mem::{kstack, make_satp, pte2pa},
    plic::PLIC,
    power::QEMU_POWER,
};
use crate::{
    arch::{
        self,
        hardware::VIRTIO0,
        mem::{
            round_down_page, round_up_page, Pagetable, PagetableEntry, KERNEL_BASE, PAGE_SIZE,
            PHYSICAL_END, PTE_R, PTE_U, PTE_V, PTE_W, PTE_X, TRAMPOLINE, VIRTUAL_MAX,
        },
    },
    mem::{
        kalloc::{kalloc, kfree},
        memmove, memset,
    },
    proc::process::Process,
};
use core::ptr::{addr_of, addr_of_mut, null_mut};

extern "C" {
    /// kernel.ld sets this to end of kernel code.
    pub static etext: [u8; 0];
    /// trampoline.S
    pub static trampoline: [u8; 0];

    // pub fn either_copyin(dst: *mut u8, user_src: i32, src: u64, len: u64) -> i32;
    // pub fn either_copyout(user_dst: i32, dst: u64, src: *mut u8, len: u64) -> i32;
}

/// The kernel's pagetable.
pub static mut KERNEL_PAGETABLE: Pagetable = null_mut();

/// Make a direct-map page table for the kernel.
pub unsafe fn kvmmake() -> Pagetable {
    let pagetable = kalloc() as Pagetable;
    if pagetable.is_null() {
        panic!("kalloc");
    }
    memset(pagetable.cast(), 0, PAGE_SIZE);

    // QEMU test interface used for power management.
    kvmmap(
        pagetable,
        QEMU_POWER,
        QEMU_POWER,
        PAGE_SIZE,
        PTE_R | PTE_W,
    );

    // UART registers
    for (_, uart) in &crate::hardware::UARTS {
        kvmmap(
            pagetable,
            uart.base_address,
            uart.base_address,
            PAGE_SIZE,
            PTE_R | PTE_W,
        );
    }

    // VirtIO MMIO disk interface
    kvmmap(
        pagetable,
        VIRTIO0,
        VIRTIO0,
        PAGE_SIZE,
        PTE_R | PTE_W,
    );

    // PLIC
    kvmmap(
        pagetable,
        PLIC,
        PLIC,
        0x400000,
        PTE_R | PTE_W,
    );

    let etext_addr = addr_of!(etext) as usize;

    // Map kernel text executable and read-only.
    kvmmap(
        pagetable,
        KERNEL_BASE,
        KERNEL_BASE,
        etext_addr - KERNEL_BASE,
        PTE_R | PTE_X,
    );

    // Map kernel data and the physical RAM we'll make use of.
    kvmmap(
        pagetable,
        etext_addr,
        etext_addr,
        PHYSICAL_END - etext_addr,
        PTE_R | PTE_W,
    );

    // Map the trampoine for trap entry/exit to
    // the highest virtual address in the kernel.
    kvmmap(
        pagetable,
        TRAMPOLINE,
        addr_of!(trampoline) as usize,
        PAGE_SIZE,
        PTE_R | PTE_X,
    );

    // Allocate and map a kernel stack for each process.
    for i in 0..crate::NPROC {
        let page = kalloc();
        if page.is_null() {
            panic!("kalloc");
        }
        let virtual_addr = kstack(i);
        kvmmap(
            pagetable,
            virtual_addr,
            page as usize,
            PAGE_SIZE,
            PTE_R | PTE_W,
        );
    }

    pagetable
}

/// Initialize the one kernel_pagetable.
pub unsafe fn kvminit() {
    KERNEL_PAGETABLE = kvmmake();
}

/// Switch hardware pagetable register to the kernel's pagetable and enable paging.
pub unsafe fn kvminithart() {
    // Wait for any previous writes to the pagetable memory to finish.
    arch::mem::flush_cached_pages();

    asm::w_satp(make_satp(KERNEL_PAGETABLE));

    // Flush stale entries from the TLB.
    arch::mem::flush_cached_pages();
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
pub unsafe fn walk(mut pagetable: Pagetable, virtual_addr: usize, alloc: bool) -> *mut PagetableEntry {
    if virtual_addr > VIRTUAL_MAX {
        panic!("walk");
    }

    let mut level = 2;
    while level > 0 {
        let pte = addr_of_mut!(
            pagetable.as_mut().unwrap()[(virtual_addr >> (12 + (level * 9))) & 0x1ff]
        );

        if (*pte) & PTE_V as u64 > 0 {
            pagetable = (((*pte) >> 10) << 12) as usize as Pagetable;
        } else {
            if !alloc {
                return null_mut();
            }

            pagetable = kalloc() as Pagetable;

            if pagetable.is_null() {
                return null_mut();
            }

            memset(pagetable.cast(), 0, PAGE_SIZE);
            *pte = (((pagetable as usize) >> 12) << 10) as PagetableEntry | PTE_V as u64;
        }

        level -= 1;
    }

    addr_of_mut!(pagetable.as_mut().unwrap()[(virtual_addr >> 12) & 0x1ff])
}

/// Look up a virtual address and return the physical address or 0 if not mapped.
///
/// Can only be used to look up user pages.
#[no_mangle]
pub unsafe extern "C" fn walkaddr(pagetable: Pagetable, virtual_addr: usize) -> u64 {
    if virtual_addr > VIRTUAL_MAX {
        return 0;
    }

    let pte = walk(pagetable, virtual_addr , false);
    if pte.is_null() || *pte & PTE_V as u64 == 0 || *pte & PTE_U as u64 == 0 {
        return 0;
    }

    pte2pa(*pte as usize) as u64
}

/// Add a mapping to the kernel page table.
///
/// Only used when booting.
/// Does not flush TLB or enable paging.
pub unsafe fn kvmmap(
    pagetable: Pagetable,
    virtual_addr: usize,
    physical_addr: usize,
    size: usize,
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
pub unsafe fn mappages(
    pagetable: Pagetable,
    virtual_addr: usize,
    size: usize,
    mut physical_addr: usize,
    perm: i32,
) -> i32 {
    if size == 0 {
        panic!("mappages: size = 0");
    }

    let mut a = round_down_page(virtual_addr);
    let last = round_down_page(virtual_addr + size - 1);

    loop {
        let pte = walk(pagetable, a, true);

        if pte.is_null() {
            return -1;
        }
        if (*pte) & PTE_V as u64 > 0 {
            panic!("mappages: remap");
        }

        *pte = ((physical_addr as u64 >> 12) << 10) | perm as u64 | PTE_V as u64;

        if a == last {
            break;
        } else {
            a += PAGE_SIZE;
            physical_addr += PAGE_SIZE;
        }
    }

    0
}

/// Remove `npages` of mappings starting from `virtual_addr`.
///
/// `virtual_addr` amust be page-aligned. The mappings must exist.
/// Optionally free the physical memory.
pub unsafe fn uvmunmap(
    pagetable: Pagetable,
    virtual_addr: usize,
    num_pages: usize,
    free: bool,
) {
    if virtual_addr % PAGE_SIZE != 0 {
        panic!("uvmunmap: not aligned");
    }
    let mut a = virtual_addr;
    while a < virtual_addr + num_pages * PAGE_SIZE {
        let pte = walk(pagetable, a, false);
        if pte.is_null() {
            panic!("uvmunmap: walk");
        } else if (*pte) & PTE_V as u64 == 0 {
            panic!("uvmunmap: not mapped");
        } else if ((*pte) & 0x3ffu64) == PTE_V as u64 {
            panic!("uvmunmap: not a leaf");
        } else if free {
            let physical_addr = (((*pte) >> 10) << 12) as usize as *mut u8;
            kfree(physical_addr.cast());
        }

        *pte = 0;
        a += PAGE_SIZE;
    }
}

/// Create an empty user pagetable.
///
/// Returns 0 if out of memory.
pub unsafe fn uvmcreate() -> Pagetable {
    let pagetable = kalloc() as Pagetable;
    if pagetable.is_null() {
        return null_mut();
    }
    memset(pagetable.cast(), 0, PAGE_SIZE);
    pagetable
}

/// Load the user initcode into address 0 of pagetable for the very first process.
///
/// `size` must be less than `PAGE_SIZE`.
pub unsafe fn uvmfirst(pagetable: Pagetable, src: *mut u8, size: usize) {
    if size >= PAGE_SIZE {
        panic!("uvmfirst: more than a page");
    }

    let mem = kalloc();
    memset(mem, 0, PAGE_SIZE);
    mappages(
        pagetable,
        0,
        PAGE_SIZE,
        mem as usize,
        PTE_W | PTE_R | PTE_X | PTE_U,
    );
    memmove(mem, src, size as u32);
}

/// Allocate PagetableEntries and physical memory to grow process
/// from `old_size` to `new_size`, which need not be page aligned.
///
/// Returns new size or 0 on error.
#[no_mangle]
pub unsafe extern "C" fn uvmalloc(
    pagetable: Pagetable,
    mut old_size: usize,
    new_size: usize,
    xperm: i32,
) -> u64 {
    if new_size < old_size {
        return old_size as u64;
    }

    old_size = round_up_page(old_size);
    let mut a = old_size;

    while a < new_size {
        let mem = kalloc();
        if mem.is_null() {
            uvmdealloc(pagetable, a, old_size);
            return 0;
        }

        memset(mem.cast(), 0, PAGE_SIZE);

        if mappages(
            pagetable,
            a,
            PAGE_SIZE,
            mem as usize,
            PTE_R | PTE_U | xperm,
        ) != 0
        {
            kfree(mem.cast());
            uvmdealloc(pagetable, a, old_size);
            return 0;
        }

        a += PAGE_SIZE;
    }

    new_size as u64
}

/// Deallocate user pages to bring the process size from `old_size` to `new_size`.
///
/// `old_size` and `new_size` need not be page-aligned, nor does `new_size` need
/// to be less than `old_size`. `old_size` can be larget than the actual process
/// size. Returns the new process size.
#[no_mangle]
pub unsafe extern "C" fn uvmdealloc(pagetable: Pagetable, old_size: usize, new_size: usize) -> u64 {
    if new_size >= old_size {
        return old_size as u64;
    }

    if round_up_page(new_size) < round_up_page(old_size) {
        let num_pages =
            (round_up_page(old_size) - round_up_page(new_size)) / PAGE_SIZE;
        uvmunmap(
            pagetable,
            round_up_page(new_size),
            num_pages,
            true,
        );
    }

    new_size as u64
}

/// Recursively free pagetable pages.
///
/// All leaf mappings must have already been removed.
pub unsafe fn freewalk(pagetable: Pagetable) {
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
pub unsafe fn uvmfree(pagetable: Pagetable, size: usize) {
    uvmunmap(
        pagetable,
        0,
        round_up_page(size) / PAGE_SIZE,
        true,
    );
    freewalk(pagetable);
}

/// Given a parent process's pagetable, copy
/// its memory into a child's pagetable.
///
/// Copies both the pagetable and the physical memory.
/// Returns 0 on success, -1 on failure.
/// Frees any allocated pages on failure.
pub unsafe fn uvmcopy(old: Pagetable, new: Pagetable, size: usize) -> i32 {
    let mut i = 0;

    while i < size {
        let pte = walk(old, i, false);
        if pte.is_null() {
            panic!("uvmcopy: PagetableEntry should exist");
        } else if (*pte) & PTE_V as u64 == 0 {
            panic!("uvmcopy: page not present");
        }

        let pa = ((*pte) >> 10) << 12;
        let flags = (*pte) & 0x3ffu64;

        let mem = kalloc();
        if mem.is_null() {
            uvmunmap(new, 0, i / PAGE_SIZE, true);
            return -1;
        }

        memmove(
            mem.cast(),
            (pa as usize as *mut u8).cast(),
            PAGE_SIZE as u64 as u32,
        );

        if mappages(new, i, PAGE_SIZE, mem as usize, flags as i32) != 0 {
            kfree(mem.cast());
            uvmunmap(new, 0, i / PAGE_SIZE, true);
            return -1;
        }

        i += PAGE_SIZE;
    }

    0
}

/// Mark a PagetableEntry invalid for user access.
///
/// Used by exec for the user stack guard page.
#[no_mangle]
pub unsafe extern "C" fn uvmclear(pagetable: Pagetable, virtual_addr: usize) {
    let pte = walk(pagetable, virtual_addr, false);
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
    mut dst_virtual_addr: usize,
    mut src: *mut u8,
    mut len: usize,
) -> i32 {
    while len > 0 {
        let va0 = round_down_page(dst_virtual_addr);
        let pa0 = walkaddr(pagetable, va0) as usize;
        if pa0 == 0 {
            return -1;
        }

        let mut n = PAGE_SIZE - (dst_virtual_addr - va0);
        if n > len {
            n = len;
        }
        memmove(
            ((pa0 + dst_virtual_addr - va0) as *mut u8).cast(),
            src,
            n as u32,
        );

        len -= n;
        src = src.add(n);
        dst_virtual_addr = va0 + PAGE_SIZE;
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
    mut src_virtual_addr: usize,
    mut len: usize,
) -> i32 {
    while len > 0 {
        let va0 = round_down_page(src_virtual_addr);
        let pa0 = walkaddr(pagetable, va0) as usize;
        if pa0 == 0 {
            return -1;
        }

        let mut n = PAGE_SIZE - (src_virtual_addr - va0);
        if n > len {
            n = len;
        }
        memmove(
            dst.cast(),
            ((pa0 + src_virtual_addr - va0) as *mut u8).cast(),
            n as u32,
        );

        len -= n;
        dst = dst.add(n);
        src_virtual_addr = va0 + PAGE_SIZE;
    }
    0
}

// Copy to either a user address, or kernel address,
// depending on usr_dst.
// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn either_copyout(user_dst: i32, dst: usize, src: *mut u8, len: usize) -> i32 {
    let p = Process::current().unwrap();

    if user_dst > 0 {
        copyout(p.pagetable, dst, src, len)
    } else {
        memmove(dst as *mut u8, src, len as u32);
        0
    }
}

// Copy from either a user address, or kernel address,
// depending on usr_src.
// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn either_copyin(dst: *mut u8, user_src: i32, src: usize, len: usize) -> i32 {
    let p = Process::current().unwrap();

    if user_src > 0 {
        copyin(p.pagetable, dst, src, len)
    } else {
        memmove(dst, src as *mut u8, len as u32);
        0
    }
}

/// Copy a null-terminated string from user to kernel.
///
/// Copy bytes to `dst` from virtual address `src_virtual_addr`
/// in a given pagetable, until b'\0' or `max` is reached.
/// Returns 0 on success, -1 on error.
pub unsafe fn copyinstr(
    pagetable: Pagetable,
    mut dst: *mut u8,
    mut src_virtual_addr: usize,
    mut max: usize,
) -> i32 {
    let mut got_null = false;

    while !got_null && max > 0 {
        let va0 = round_down_page(src_virtual_addr);
        let pa0 = walkaddr(pagetable, va0) as usize;
        if pa0 == 0 {
            return -1;
        }

        let mut n = PAGE_SIZE - (src_virtual_addr - va0);
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

        src_virtual_addr = va0 + PAGE_SIZE;
    }

    if got_null {
        0
    } else {
        -1
    }
}
