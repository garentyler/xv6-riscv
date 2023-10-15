#![no_main]
#![no_std]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]

extern crate alloc;
extern crate core;

extern "C" {
    fn print(message: *const c_char);
    fn panic(panic_message: *const c_char) -> !;
}

mod kalloc;
pub(crate) mod param;
pub mod proc;
pub(crate) mod riscv;
pub mod spinlock;

use core::ffi::{c_char, CStr};

pub use proc::*;
pub use spinlock::*;

#[no_mangle]
pub extern "C" fn rust_main() {
    unsafe {
        print(
            CStr::from_bytes_with_nul(b"Hello from Rust!\n\0")
                .unwrap()
                .as_ptr(),
        );
    }
}

#[panic_handler]
unsafe fn panic_wrapper(_panic_info: &core::panic::PanicInfo) -> ! {
    panic(
        CStr::from_bytes_with_nul(b"panic from rust\0")
            .unwrap_or_default()
            .as_ptr(),
    )
}
