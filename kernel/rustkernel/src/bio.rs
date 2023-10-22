//! Buffer cache.
//!
//! The buffer cache is a linked list of buf strctures holding
//! cached copies of disk block contents. Caching disk blocks
//! in memory reduces the number of disk reads and also provides
//! a synchronization point for disk blocks used by multiple processes.
//!
//! Interface:
//! - To get a buffer for a particular disk block, call bread.
//! - After changing buffer data, call bwrite to write it to disk.
//! - When done with the buffer, call brelse.
//! - Do not use the buffer after calling brelse.
//! - Only one process at a time can use a buffer,
//!   so do not keep them longer than necessary.

use crate::{buf::Buffer, param::NBUF, sync::spinlock::Spinlock};

pub struct BufferCache {
    pub buffers: [Buffer; NBUF],
}
impl BufferCache {
    /// Look through the buffer cache for block on device dev.
    ///
    /// If not found, allocate a buffer.
    /// In either case, return locked buffer.
    fn get(&mut self, dev: u32, blockno: u32) {
        for buf in &mut self.buffers {
            if buf.dev == dev && buf.blockno == blockno {
                buf.refcnt += 1;
            }
        }
    }
}

#[repr(C)]
pub struct BCache {
    pub lock: Spinlock,
    pub buf: [Buffer; NBUF],
    pub head: Buffer,
}

extern "C" {
    pub static mut bcache: BCache;
    pub fn binit();
    // pub fn bget(dev: u32, blockno: u32) -> *mut Buffer;
    pub fn bread(dev: u32, blockno: u32) -> *mut Buffer;
    pub fn bwrite(b: *mut Buffer);
    pub fn brelse(b: *mut Buffer);
    pub fn bpin(b: *mut Buffer);
    pub fn bunpin(b: *mut Buffer);
}

// pub static BUFFER_CACHE: Mutex<BufferCache> = Mutex::new();

// #[no_mangle]
// pub unsafe extern "C" fn bget(dev: u32, blockno: u32) -> *mut Buffer {
//     let mut b: *mut Buffer;
//     let _guard = bcache.lock.lock();
//
//     // Is the block already cached?
//     b = bcache.head.next;
//     while b != addr_of_mut!(bcache.head) {
//         if (*b).dev == dev && (*b).blockno == blockno {
//             (*b).refcnt += 1;
//             acquiresleep(addr_of_mut!((*b).lock));
//             // (*b).lock.lock_unguarded();
//             return b;
//         } else {
//             b = (*b).next;
//         }
//     }
//
//     // Not cached.
//     // Recycle the least recently used unused buffer.
//     b = bcache.head.prev;
//     while b != addr_of_mut!(bcache.head) {
//         if (*b).refcnt == 0 {
//             (*b).dev = dev;
//             (*b).blockno = blockno;
//             (*b).valid = 0;
//             (*b).refcnt = 1;
//             // (*b).lock.lock_unguarded();
//             acquiresleep(addr_of_mut!((*b).lock));
//             return b;
//         }
//     }
//
//     panic!("bget: no buffers");
// }

// /// Return a locked buffer with the contents of the indicated block.
// #[no_mangle]
// pub unsafe extern "C" fn bread(dev: u32, blockno: u32) -> *mut Buffer {
//     let b = bget(dev, blockno);
//
//     if (*b).valid == 0 {
//         virtio_disk_rw(b, 0);
//         (*b).valid = 1;
//     }
//
//     b
// }
//
// #[no_mangle]
// pub unsafe extern "C" fn bwrite(b: *mut Buffer) {
//     if holdingsleep(addr_of_mut!((*b).lock)) == 0 {
//     // if !(*b).lock.held_by_current_proc() {
//         panic!("bwrite");
//     }
//
//     virtio_disk_rw(b, 1);
// }

// #[no_mangle]
// pub unsafe extern "C" fn bpin(b: *mut Buffer) {
//     let _guard = bcache.lock.lock();
//     (*b).refcnt += 1;
//     // bcache.lock.unlock();
// }
//
// #[no_mangle]
// pub unsafe extern "C" fn bunpin(b: *mut Buffer) {
//     let _guard = bcache.lock.lock();
//     (*b).refcnt -= 1;
//     // bcache.lock.unlock();
// }
//
