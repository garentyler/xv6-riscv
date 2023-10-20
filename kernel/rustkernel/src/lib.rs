#![no_main]
#![no_std]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]
#![feature(negative_impls)]

extern crate alloc;
extern crate core;

pub mod buf;
pub mod console;
pub mod file;
pub mod fs;
pub(crate) mod kalloc;
pub(crate) mod param;
pub mod printf;
pub mod proc;
pub(crate) mod riscv;
pub mod start;
pub mod string;
pub mod sync;
pub mod syscall;
pub mod sysproc;
pub mod trap;
pub mod uart;

extern "C" {
    // pub fn printfinit();
    // pub fn kinit();
    pub fn kvminit();
    pub fn kvminithart();
    pub fn procinit();
    pub fn binit();
    pub fn iinit();
    pub fn fileinit();
    pub fn virtio_disk_init();
    pub fn userinit();
    // pub fn scheduler();
}

use crate::{printf::print, proc::cpuid};
use core::ffi::{c_char, CStr};

pub static mut STARTED: bool = false;
pub static mut PANICKED: bool = false;

#[no_mangle]
pub unsafe extern "C" fn main() -> ! {
    if cpuid() == 0 {
        console::consoleinit();
        printf::printfinit();
        kalloc::kinit();
        print!("\nxv6 kernel is booting\n");
        kvminit();
        kvminithart();
        procinit();
        trap::trapinit();
        trap::trapinithart();
        riscv::plic::plicinit();
        riscv::plic::plicinithart();
        binit();
        iinit();
        fileinit();
        virtio_disk_init();
        userinit();
        STARTED = true;
    } else {
        while !STARTED {
            core::hint::spin_loop();
        }
        kvminithart();
        trap::trapinithart();
        riscv::plic::plicinithart();
    }

    proc::scheduler();
}

#[panic_handler]
fn panic_wrapper(panic_info: &core::panic::PanicInfo) -> ! {
    if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        crate::printf::print!("panic: {}\n", s);
    } else {
        crate::printf::print!("kernel panic\n");
    }

    unsafe { crate::PANICKED = true };

    loop {
        core::hint::spin_loop();
    }
}

#[no_mangle]
pub unsafe extern "C" fn panic(s: *const c_char) -> ! {
    let s = CStr::from_ptr(s).to_str().unwrap_or("panic from c");
    panic!("{}", s);
}
