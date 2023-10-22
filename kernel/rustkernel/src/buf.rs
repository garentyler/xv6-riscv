use crate::{fs::BSIZE, sync::sleeplock::Sleeplock};

#[repr(C)]
pub struct Buffer {
    /// Has data been read from disk?
    pub valid: i32,
    /// Does disk "own" buf?
    pub disk: i32,
    pub dev: u32,
    pub blockno: u32,
    pub lock: Sleeplock,
    pub refcnt: u32,
    pub prev: *mut Buffer,
    pub next: *mut Buffer,
    pub data: [u8; BSIZE as usize],
}
