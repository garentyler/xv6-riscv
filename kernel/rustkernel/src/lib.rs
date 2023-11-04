#![no_main]
#![no_std]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]
#![feature(negative_impls)]
#![feature(panic_info_message)]

extern crate alloc;
extern crate core;

mod arch;
mod console;
mod fs;
mod io;
mod mem;
mod proc;
mod queue;
mod start;
mod string;
mod sync;
mod syscall;
mod trap;

use crate::{proc::cpu::Cpu, sync::mutex::Mutex};
use core::ffi::{c_char, CStr};

pub(crate) use crate::console::printf::{print, println};

pub static mut STARTED: bool = false;
pub static PANICKED: Mutex<bool> = Mutex::new(false);

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

pub unsafe fn main() -> ! {
    if Cpu::current_id() == 0 {
        console::consoleinit();
        mem::kalloc::kinit();
        println!("\nxv6 kernel is booting");
        mem::virtual_memory::kvminit();
        mem::virtual_memory::kvminithart();
        proc::process::procinit();
        trap::trapinithart();
        arch::riscv::plic::plicinit();
        arch::riscv::plic::plicinithart();
        io::bio::binit();
        fs::iinit();
        fs::file::fileinit();
        fs::virtio_disk::virtio_disk_init();
        proc::process::userinit();
        STARTED = true;
    } else {
        while !STARTED {
            core::hint::spin_loop();
        }
        mem::virtual_memory::kvminithart();
        trap::trapinithart();
        arch::riscv::plic::plicinithart();
    }

    proc::process::scheduler();
}

#[panic_handler]
fn panic_wrapper(panic_info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = panic_info.location() {
        print!("kernel panic ({}): ", location.file());
    } else {
        print!("kernel panic: ");
    }

    if let Some(s) = panic_info.message() {
        println!("{}", s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        println!("{}", s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<&CStr>() {
        println!("{:?}", s);
    } else {
        println!("could not recover error message");
    }

    println!("███████╗██╗   ██╗ ██████╗██╗  ██╗██╗██╗");
    println!("██╔════╝██║   ██║██╔════╝██║ ██╔╝██║██║");
    println!("█████╗  ██║   ██║██║     █████╔╝ ██║██║");
    println!("██╔══╝  ██║   ██║██║     ██╔═██╗ ╚═╝╚═╝");
    println!("██║     ╚██████╔╝╚██████╗██║  ██╗██╗██╗");
    println!("╚═╝      ╚═════╝  ╚═════╝╚═╝  ╚═╝╚═╝╚═╝");

    unsafe {
        *crate::PANICKED.lock_spinning() = true;
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
