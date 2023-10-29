use crate::{fs::file::File, sync::spinlock::Spinlock};
use core::ffi::c_char;

pub const PIPESIZE: usize = 512usize;

#[repr(C)]
pub struct Pipe {
    lock: Spinlock,
    data: [c_char; PIPESIZE],
    /// Number of bytes read.
    nread: u32,
    /// Number of bytes written.
    nwrite: u32,
    /// Read fd is still open.
    readopen: i32,
    /// Write fd is still open.
    writeopen: i32,
}

extern "C" {
    pub fn pipealloc(a: *mut *mut File, b: *mut *mut File) -> i32;
    pub fn pipeclose(pipe: *mut Pipe, writable: i32);
    pub fn pipewrite(pipe: *mut Pipe, addr: u64, n: i32) -> i32;
    pub fn piperead(pipe: *mut Pipe, addr: u64, n: i32) -> i32;
}
