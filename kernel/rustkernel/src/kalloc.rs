extern "C" {
    fn kalloc() -> *mut u8;
    fn kfree(ptr: *mut u8);
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
