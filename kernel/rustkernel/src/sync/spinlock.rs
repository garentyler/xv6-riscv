use crate::{
    proc::{mycpu, Cpu},
    trap::{push_intr_off, pop_intr_off},
};
use core::{
    ffi::c_char,
    ptr::{addr_of, null_mut},
    sync::atomic::{AtomicBool, Ordering},
};

#[repr(C)]
pub struct Spinlock {
    pub locked: AtomicBool,
    pub name: *mut c_char,
    pub cpu: *mut Cpu,
}
impl Spinlock {
    pub const unsafe fn uninitialized() -> Spinlock {
        Spinlock {
            locked: AtomicBool::new(false),
            name: null_mut(),
            cpu: null_mut(),
        }
    }
    /// Initializes a `Spinlock`.
    pub const fn new(name: *mut c_char) -> Spinlock {
        Spinlock {
            locked: AtomicBool::new(false),
            cpu: null_mut(),
            name,
        }
    }
    /// Check whether this cpu is holding the lock.
    ///
    /// Interrupts must be off.
    pub fn held_by_current_cpu(&self) -> bool {
        self.cpu == unsafe { mycpu() } && self.locked.load(Ordering::Relaxed)
    }
    pub unsafe fn lock_unguarded(&self) {
        push_intr_off();

        if self.held_by_current_cpu() {
            panic!("Attempt to acquire twice by the same CPU");
        }

        let this: &mut Self = &mut *addr_of!(*self).cast_mut();

        while this.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }

        // The lock is now locked and we can write our CPU info.
        this.cpu = mycpu();
    }
    pub fn lock(&self) -> SpinlockGuard<'_> {
        unsafe {
            self.lock_unguarded();
        }
        SpinlockGuard { lock: self }
    }
    pub unsafe fn unlock(&self) {
        if !self.held_by_current_cpu() {
            panic!("Attempt to release lock from different CPU");
        }

        let this: &mut Self = &mut *addr_of!(*self).cast_mut();

        this.cpu = null_mut();
        this.locked.store(false, Ordering::Release);

        pop_intr_off();
    }
}

pub struct SpinlockGuard<'l> {
    pub lock: &'l Spinlock,
}
impl<'l> Drop for SpinlockGuard<'l> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() }
    }
}

#[no_mangle]
pub unsafe extern "C" fn initlock(lock: *mut Spinlock, name: *mut c_char) {
    *lock = Spinlock::new(name);
}

#[no_mangle]
pub unsafe extern "C" fn holding(lock: *mut Spinlock) -> i32 {
    (*lock).held_by_current_cpu().into()
}

#[no_mangle]
pub unsafe extern "C" fn acquire(lock: *mut Spinlock) {
    (*lock).lock_unguarded();
}

#[no_mangle]
pub unsafe extern "C" fn release(lock: *mut Spinlock) {
    (*lock).unlock();
}
