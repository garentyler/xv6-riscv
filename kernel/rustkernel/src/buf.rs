use crate::{fs::BSIZE, sleeplock::Sleeplock};

#[repr(C)]
pub struct Buf {
    /// Has data been read from disk?
    pub valid: i32,
    /// Does disk "own" buf?
    pub disk: i32,
    pub dev: u32,
    pub blockno: u32,
    pub lock: Sleeplock,
    pub refcnt: u32,
    pub prev: *mut Buf,
    pub next: *mut Buf,
    pub data: [u8; BSIZE as usize],
}
