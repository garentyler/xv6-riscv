use crate::{
    proc::{myproc, sleep, wakeup},
    sync::spinlock::{self, Spinlock},
};
use core::{ffi::c_char, ptr::{addr_of, null_mut}};

#[repr(C)]
pub struct Sleeplock {
    pub locked: u32,
    pub inner: Spinlock,
    pub name: *mut c_char,
    pub pid: i32,
}
impl Sleeplock {
    pub const unsafe fn uninitialized() -> Sleeplock {
        Sleeplock {
            locked: 0,
            inner: Spinlock::uninitialized(),
            name: null_mut(),
            pid: 0,
        }
    }
    /// Initializes a `Sleeplock`.
    pub const fn new(name: *mut c_char) -> Sleeplock {
        Sleeplock {
            locked: 0,
            inner: Spinlock::new(name),
            name,
            pid: 0,
        }
    }
    /// Check whether this proc is holding the lock.
    pub fn held_by_current_proc(&self) -> bool {
        self.locked > 0 && self.pid == unsafe { (*myproc()).pid } 
    } 
    pub unsafe fn lock_unguarded(&self) {
        let _guard = self.inner.lock();
        while self.locked > 0 {
            sleep(addr_of!(*self).cast_mut().cast(), addr_of!(self.inner).cast_mut().cast());
        }
        let this: &mut Self = &mut *addr_of!(*self).cast_mut();
        this.locked = 1;
        this.pid = (*myproc()).pid;
    }
    pub fn lock(&self) -> SleeplockGuard<'_> {
        unsafe {
            self.lock_unguarded();
        }
        SleeplockGuard { lock: self }
    }
    pub unsafe fn unlock(&self) {
        let _guard = self.inner.lock();
        let this: &mut Self = &mut *addr_of!(*self).cast_mut();
        this.locked = 0;
        this.pid = 0;
        wakeup(addr_of!(*self).cast_mut().cast());
    }
}

pub struct SleeplockGuard<'l> {
    pub lock: &'l Sleeplock,
}
impl<'l> Drop for SleeplockGuard<'l> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() }
    }
}

#[no_mangle]
pub unsafe extern "C" fn initsleeplock(lock: *mut Sleeplock, name: *mut c_char) {
    (*lock) = Sleeplock::new(name);
}

#[no_mangle]
pub unsafe extern "C" fn acquiresleep(lock: *mut Sleeplock) {
    (*lock).lock_unguarded();
}

#[no_mangle]
pub unsafe extern "C" fn releasesleep(lock: *mut Sleeplock) {
    (*lock).unlock();
}

#[no_mangle]
pub unsafe extern "C" fn holdingsleep(lock: *mut Sleeplock) -> i32 {
    if (*lock).held_by_current_proc() {
        1
    } else {
        0
    }
}
