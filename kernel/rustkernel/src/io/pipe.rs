use crate::sync::spinlock::Spinlock;
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
