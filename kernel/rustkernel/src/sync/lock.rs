use super::LockStrategy;
use crate::proc::{
    process::{Process, ProcessState},
    scheduler::{sched, sleep, wakeup},
};
use core::{
    cell::UnsafeCell,
    ops::Drop,
    ptr::{addr_of, null_mut},
    sync::atomic::{AtomicBool, Ordering},
};

pub struct Lock {
    locked: AtomicBool,
    lock_strategy: UnsafeCell<LockStrategy>,
}
impl Lock {
    pub const fn new() -> Lock {
        Lock {
            locked: AtomicBool::new(false),
            lock_strategy: UnsafeCell::new(LockStrategy::Spin),
        }
    }
    pub fn lock_strategy(&self) -> LockStrategy {
        unsafe { *self.lock_strategy.get() }
    }

    pub unsafe fn lock_unguarded(&self, lock_strategy: LockStrategy) {
        // Lock it first, then store the lock strategy.

        match lock_strategy {
            LockStrategy::Spin => {
                crate::trap::push_intr_off();

                while self.locked.swap(true, Ordering::Acquire) {
                    core::hint::spin_loop();
                }
            }
            LockStrategy::Sleep => {
                while self.locked.swap(true, Ordering::Acquire) {
                    // Put the process to sleep until the mutex gets released.
                    sleep(addr_of!(*self).cast_mut().cast());
                }
            }
        };

        *self.lock_strategy.get() = lock_strategy;
    }
    pub fn lock(&self, lock_strategy: LockStrategy) -> LockGuard<'_> {
        unsafe {
            self.lock_unguarded(lock_strategy);
        }
        LockGuard { lock: self }
    }
    pub fn lock_spinning(&self) -> LockGuard<'_> {
        self.lock(LockStrategy::Spin)
    }
    pub fn lock_sleeping(&self) -> LockGuard<'_> {
        self.lock(LockStrategy::Sleep)
    }
    pub unsafe fn unlock(&self) {
        let lock_strategy = self.lock_strategy();
        self.locked.store(false, Ordering::Release);

        match lock_strategy {
            LockStrategy::Spin => {
                crate::trap::pop_intr_off();
            }
            LockStrategy::Sleep => {
                wakeup(addr_of!(*self).cast_mut().cast());
            }
        }
    }
}
impl Default for Lock {
    fn default() -> Lock {
        Lock::new()
    }
}
unsafe impl Sync for Lock {}

pub struct LockGuard<'l> {
    pub lock: &'l Lock,
}
impl<'l> LockGuard<'l> {
    /// Sleep until `wakeup(chan)` is called somewhere
    /// else, yielding access to the lock until then.
    pub unsafe fn sleep(&self, chan: *mut core::ffi::c_void) {
        let proc = Process::current().unwrap();
        let _guard = proc.lock.lock();
        let strategy = self.lock.lock_strategy();
        self.lock.unlock();

        // Put the process to sleep.
        proc.chan = chan;
        proc.state = ProcessState::Sleeping;
        sched();

        // Tidy up and reacquire the lock.
        proc.chan = null_mut();
        self.lock.lock_unguarded(strategy);
    }
}
impl<'l> Drop for LockGuard<'l> {
    fn drop(&mut self) {
        unsafe { self.lock.unlock() }
    }
}
