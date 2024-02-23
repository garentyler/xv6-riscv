#![no_main]
#![no_std]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]
#![feature(negative_impls)]
#![feature(panic_info_message)]
#![feature(str_from_raw_parts)]

extern crate alloc;
extern crate core;

mod arch;
mod console;
mod fs;
mod hardware;
mod io;
mod mem;
mod proc;
mod queue;
mod string;
mod sync;
mod syscall;

use crate::{proc::cpu::Cpu, sync::mutex::Mutex};
use core::{
    ffi::{c_char, CStr},
    ptr::addr_of,
};

pub(crate) use crate::console::printf::{print, println, uprint, uprintln};

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
        arch::virtual_memory::init();
        arch::virtual_memory::inithart();
        proc::process::procinit();
        arch::trap::inithart();
        arch::interrupt::init();
        arch::interrupt::inithart();
        io::bio::binit();
        fs::inode::iinit();
        hardware::virtio_disk::virtio_disk_init();
        proc::process::userinit();
        STARTED = true;
    } else {
        while !STARTED {
            core::hint::spin_loop();
        }
        arch::virtual_memory::inithart();
        arch::trap::inithart();
        arch::interrupt::inithart();
    }
    proc::scheduler::scheduler();
}

#[panic_handler]
fn panic_wrapper(panic_info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = panic_info.location() {
        uprint!("kernel panic ({}): ", location.file());
    } else {
        uprint!("kernel panic: ");
    }

    if let Some(s) = panic_info.message() {
        uprintln!("{}", s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
        uprintln!("{}", s);
    } else if let Some(s) = panic_info.payload().downcast_ref::<&CStr>() {
        uprintln!("{:?}", s);
    } else {
        uprintln!("could not recover error message");
    }

    uprintln!("███████╗██╗   ██╗ ██████╗██╗  ██╗██╗██╗");
    uprintln!("██╔════╝██║   ██║██╔════╝██║ ██╔╝██║██║");
    uprintln!("█████╗  ██║   ██║██║     █████╔╝ ██║██║");
    uprintln!("██╔══╝  ██║   ██║██║     ██╔═██╗ ╚═╝╚═╝");
    uprintln!("██║     ╚██████╔╝╚██████╗██║  ██╗██╗██╗");
    uprintln!("╚═╝      ╚═════╝  ╚═════╝╚═╝  ╚═╝╚═╝╚═╝");

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
pub unsafe extern "C" fn panic(msg: *const c_char) -> ! {
    let mut message = [b' '; 32];
    let mut i = 0;
    loop {
        match *msg.add(i) {
            0 => break,
            c => message[i] = c as u8,
        }
        i += 1;
    }
    let message = core::str::from_raw_parts(addr_of!(message[0]), i);

    panic!("panic from c: {}", message);
}
