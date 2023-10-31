#![no_main]
#![no_std]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]
#![feature(negative_impls)]
#![feature(panic_info_message)]

extern crate alloc;
extern crate core;

pub mod console;
pub mod fs;
pub mod io;
pub mod mem;
pub mod proc;
pub mod queue;
pub(crate) mod riscv;
pub mod start;
pub mod string;
pub mod sync;
pub mod syscall;
pub mod trap;

use crate::proc::cpuid;
use core::ffi::{c_char, CStr};

pub(crate) use crate::console::printf::print;

pub static mut STARTED: bool = false;
pub static mut PANICKED: bool = false;

/// Maximum number of processes
pub const NPROC: usize = 64;
/// Maximum number of CPUs
pub const NCPU: usize = 8;
/// Maximum number of open files per process
pub const NOFILE: usize = 16;
/// Maximum number of open files per system
pub const NFILE: usize = 100;
/// Maximum number of active inodes
pub const NINODE: usize = 50;
/// Maximum major device number
pub const NDEV: usize = 10;
/// Device number of file system root disk
pub const ROOTDEV: usize = 1;
/// Max exec arguments
pub const MAXARG: usize = 32;
/// Max num of blocks any FS op writes
pub const MAXOPBLOCKS: usize = 10;
/// Max data blocks in on-disk log
pub const LOGSIZE: usize = MAXOPBLOCKS * 3;
/// Size of disk block cache
pub const NBUF: usize = MAXOPBLOCKS * 3;
/// Size of file system in blocks
pub const FSSIZE: usize = 2000;
/// Maximum file path size
pub const MAXPATH: usize = 128;

#[no_mangle]
pub unsafe extern "C" fn main() -> ! {
    if cpuid() == 0 {
        console::consoleinit();
        console::printf::printfinit();
        mem::kalloc::kinit();
        print!("\nxv6 kernel is booting\n");
        mem::virtual_memory::kvminit();
        mem::virtual_memory::kvminithart();
        proc::procinit();
        trap::trapinit();
        trap::trapinithart();
        riscv::plic::plicinit();
        riscv::plic::plicinithart();
        io::bio::binit();
        fs::iinit();
        fs::file::fileinit();
        fs::virtio_disk::virtio_disk_init();
        proc::userinit();
        STARTED = true;
    } else {
        while !STARTED {
            core::hint::spin_loop();
        }
        mem::virtual_memory::kvminithart();
        trap::trapinithart();
        riscv::plic::plicinithart();
    }

    proc::scheduler();
}

#[panic_handler]
fn panic_wrapper(panic_info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = panic_info.location() {
        print!("kernel panic ({}): ", location.file());
    } else {
        print!("kernel panic: ");
    }

    if let Some(s) = panic_info.message() {
        print!("{}\n", s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        print!("{}\n", s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<&CStr>() {
        print!("{:?}\n", s);
    } else {
        print!("could not recover error message\n");
    }

    print!("███████╗██╗   ██╗ ██████╗██╗  ██╗██╗██╗\n");
    print!("██╔════╝██║   ██║██╔════╝██║ ██╔╝██║██║\n");
    print!("█████╗  ██║   ██║██║     █████╔╝ ██║██║\n");
    print!("██╔══╝  ██║   ██║██║     ██╔═██╗ ╚═╝╚═╝\n");
    print!("██║     ╚██████╔╝╚██████╗██║  ██╗██╗██╗\n");
    print!("╚═╝      ╚═════╝  ╚═════╝╚═╝  ╚═╝╚═╝╚═╝\n");

    unsafe {
        crate::PANICKED = true;
        // Quit QEMU for convenience.
        crate::syscall::Syscall::Shutdown.call();
    }

    loop {
        core::hint::spin_loop();
    }
}

#[no_mangle]
pub unsafe extern "C" fn panic(_: *const c_char) -> ! {
    panic!("panic from c");
    // let s = CStr::from_ptr(s).to_str().unwrap_or("panic from c");
    // panic!("{:?}", CStr::from_ptr(s));
}
