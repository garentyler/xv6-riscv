use crate::{fs::Superblock, io::buf::Buffer, sync::spinlock::Spinlock};

#[repr(C)]
pub struct LogHeader {
    pub n: i32,
    pub blocks: [i32; crate::LOGSIZE],
}
#[repr(C)]
pub struct Log {
    lock: Spinlock,
    start: i32,
    size: i32,
    /// How many FS syscalls are executing.
    outstanding: i32,
    /// In commit(), please wait.
    committing: i32,
    dev: i32,
    header: LogHeader,
}

extern "C" {
    pub static mut log: Log;
    pub fn initlog(dev: i32, superblock: *mut Superblock);
    pub fn begin_op();
    pub fn end_op();
    pub fn log_write(buffer: *mut Buffer);
}
