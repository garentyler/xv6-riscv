use crate::{
    proc::{myproc, sched, ProcState},
    trap::{pop_intr_off, push_intr_off},
};
use core::{
    ffi::c_char,
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
};

#[repr(C)]
#[derive(Default)]
pub struct Spinlock {
    pub locked: AtomicBool,
}
impl Spinlock {
    /// Initializes a `Spinlock`.
    pub const fn new() -> Spinlock {
        Spinlock {
            locked: AtomicBool::new(false),
        }
    }
    pub unsafe fn lock_unguarded(&self) {
        push_intr_off();

        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
    }
    pub fn lock(&self) -> SpinlockGuard<'_> {
        unsafe {
            self.lock_unguarded();
        }
        SpinlockGuard { lock: self }
    }
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);

        pop_intr_off();
    }
}

pub struct SpinlockGuard<'l> {
    pub lock: &'l Spinlock,
}
impl<'l> SpinlockGuard<'l> {
    /// Sleep until `wakeup(chan)` is called somewhere else, yielding the lock until then.
    pub unsafe fn sleep(&self, chan: *mut core::ffi::c_void) {
        let p = myproc();
        let _guard = (*p).lock.lock();
        self.lock.unlock();

        // Put the process to sleep.
        (*p).chan = chan;
        (*p).state = ProcState::Sleeping;
        sched();

        // Tidy up and reacquire the lock.
        (*p).chan = null_mut();
        self.lock.lock_unguarded();
    }
}
impl<'l> Drop for SpinlockGuard<'l> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() }
    }
}

#[no_mangle]
pub unsafe extern "C" fn initlock(lock: *mut Spinlock, _name: *mut c_char) {
    *lock = Spinlock::new();
}

#[no_mangle]
pub unsafe extern "C" fn acquire(lock: *mut Spinlock) {
    (*lock).lock_unguarded();
}

#[no_mangle]
pub unsafe extern "C" fn release(lock: *mut Spinlock) {
    (*lock).unlock();
}
