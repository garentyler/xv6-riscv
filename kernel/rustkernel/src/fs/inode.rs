use super::stat::Stat;
use crate::sync::sleeplock::Sleeplock;

extern "C" {
    pub fn iinit();
    pub fn ialloc(dev: u32, kind: i16) -> *mut Inode;
    pub fn iupdate(ip: *mut Inode);
    pub fn idup(ip: *mut Inode) -> *mut Inode;
    pub fn ilock(ip: *mut Inode);
    pub fn iunlock(ip: *mut Inode);
    pub fn iput(ip: *mut Inode);
    pub fn iunlockput(ip: *mut Inode);
    pub fn itrunc(ip: *mut Inode);
    pub fn stati(ip: *mut Inode, st: *mut Stat);
    pub fn readi(ip: *mut Inode, user_dst: i32, dst: u64, off: u32, n: u32) -> i32;
    pub fn writei(ip: *mut Inode, user_src: i32, src: u64, off: u32, n: u32) -> i32;
    pub fn namei(path: *mut u8) -> *mut Inode;
    // pub fn namecmp()
}

#[repr(C)]
#[derive(Clone)]
pub struct Inode {
    /// Device number.
    pub device: u32,
    /// Inode number.
    pub inum: u32,
    /// Reference count.
    pub references: i32,

    pub lock: Sleeplock,
    /// Inode has been read from disk?
    pub valid: i32,

    // Copy of DiskInode
    pub kind: i16,
    pub major: i16,
    pub minor: i16,
    pub num_links: i16,
    pub size: u32,
    pub addresses: [u32; crate::fs::NDIRECT + 1],
}
impl Inode {
    pub fn lock(&mut self) -> InodeLockGuard<'_> {
        InodeLockGuard::new(self)
    }
}

pub struct InodeLockGuard<'i> {
    pub inode: &'i mut Inode,
}
impl<'i> InodeLockGuard<'i> {
    pub fn new(inode: &mut Inode) -> InodeLockGuard<'_> {
        unsafe {
            ilock(inode as *mut Inode);
        }
        InodeLockGuard { inode }
    }
}
impl<'i> core::ops::Drop for InodeLockGuard<'i> {
    fn drop(&mut self) {
        unsafe {
            iunlock(self.inode as *mut Inode);
        }
    }
}
