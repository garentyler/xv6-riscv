use crate::proc::scheduler::{sleep, wakeup};
use core::{
    ffi::c_char,
    ptr::addr_of,
    sync::atomic::{AtomicBool, Ordering},
};

#[repr(C)]
#[derive(Default)]
pub struct Sleeplock {
    pub locked: AtomicBool,
}
impl Sleeplock {
    pub const fn new() -> Sleeplock {
        Sleeplock {
            locked: AtomicBool::new(false),
        }
    }
    #[allow(clippy::while_immutable_condition)]
    pub unsafe fn lock_unguarded(&self) {
        while self.locked.swap(true, Ordering::Acquire) {
            // Put the process to sleep until it gets released.
            sleep(addr_of!(*self).cast_mut().cast());
        }
    }
    pub fn lock(&self) -> SleeplockGuard<'_> {
        unsafe {
            self.lock_unguarded();
        }
        SleeplockGuard { lock: self }
    }
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
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
pub unsafe extern "C" fn initsleeplock(lock: *mut Sleeplock, _name: *mut c_char) {
    (*lock) = Sleeplock::new();
}

#[no_mangle]
pub unsafe extern "C" fn acquiresleep(lock: *mut Sleeplock) {
    (*lock).lock_unguarded();
}

#[no_mangle]
pub unsafe extern "C" fn releasesleep(lock: *mut Sleeplock) {
    (*lock).unlock();
}
