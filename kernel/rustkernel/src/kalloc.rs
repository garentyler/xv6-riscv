//! Physical memory allocator, for user processes,
//! kernel stacks, page-table pages,
//! and pipe buffers. Allocates whole 4096-byte pages.

use crate::{
    riscv::{memlayout::PHYSTOP, pg_round_up, PGSIZE},
    string::memset,
    sync::spinlock::Spinlock,
};
use core::{
    ffi::{c_char, CStr},
    ptr::{addr_of_mut, null_mut},
};

extern "C" {
    // oh my god this is so stupid why the fuck
    // this took me so long to figure out it's 3am rn
    // First address after kernel. Defined by kernel.ld.
    pub static mut end: [c_char; 0];
}

#[no_mangle]
pub static mut kmem: KernelMemory = KernelMemory {
    lock: unsafe { Spinlock::uninitialized() },
    freelist: null_mut(),
};

#[repr(C)]
pub struct Run {
    next: *mut Run,
}
#[repr(C)]
pub struct KernelMemory {
    pub lock: Spinlock,
    pub freelist: *mut Run,
}

#[no_mangle]
pub unsafe extern "C" fn kinit() {
    kmem.lock = Spinlock::new(
        CStr::from_bytes_with_nul(b"kmem\0")
            .unwrap()
            .as_ptr()
            .cast_mut(),
    );
    freerange(addr_of_mut!(end).cast(), PHYSTOP as *mut u8)
}

#[no_mangle]
pub unsafe extern "C" fn freerange(pa_start: *mut u8, pa_end: *mut u8) {
    let mut p = pg_round_up(pa_start as usize as u64) as *mut u8;

    while p.add(PGSIZE as usize) <= pa_end {
        kfree(p.cast());
        p = p.add(PGSIZE as usize);
    }
}

/// Free the page of physical memory pointed at by pa,
/// which normally should have been returned by a call
/// to kalloc(). The exception is when initializing the
/// allocator - see kinit above.
#[no_mangle]
pub unsafe extern "C" fn kfree(pa: *mut u8) {
    if (pa as usize as u64 % PGSIZE) != 0
        || pa <= addr_of_mut!(end) as *mut u8
        || pa >= PHYSTOP as *mut u8
    {
        panic!("kfree");
    }

    memset(pa, 0, PGSIZE as u32);

    let run: *mut Run = pa.cast();

    kmem.lock.lock();
    (*run).next = kmem.freelist;
    kmem.freelist = run;
    kmem.lock.unlock();
}

/// Allocate one 4096-byte page of physical memory.
///
/// Returns a pointer that the kernel can use.
/// Returns 0 if the memory cannot be allocated.
#[no_mangle]
pub unsafe extern "C" fn kalloc() -> *mut u8 {
    kmem.lock.lock();

    let run = kmem.freelist;
    if !run.is_null() {
        kmem.freelist = (*run).next;
    }

    kmem.lock.unlock();

    if !run.is_null() {
        memset(run.cast(), 0, PGSIZE as u32);
    }

    run as *mut u8
}

use core::alloc::{GlobalAlloc, Layout};

struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > 4096 {
            panic!("can only allocate one page of memory at a time");
        }
        let ptr = kalloc();
        if ptr.is_null() {
            panic!("kernel could not allocate memory");
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        kfree(ptr);
    }
}

#[global_allocator]
static GLOBAL: KernelAllocator = KernelAllocator;
