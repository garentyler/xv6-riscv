use super::{
    lock::{Lock, LockGuard},
    LockStrategy,
};
use core::{
    cell::UnsafeCell,
    convert::{AsMut, AsRef},
    ops::{Deref, DerefMut, Drop},
};

pub struct Mutex<T> {
    lock: Lock,
    inner: UnsafeCell<T>,
}
impl<T> Mutex<T> {
    pub const fn new(value: T) -> Mutex<T> {
        Mutex {
            lock: Lock::new(),
            inner: UnsafeCell::new(value),
        }
    }
    pub unsafe fn as_inner(&self) -> *mut T {
        self.inner.get()
    }
    pub unsafe fn lock_unguarded(&self, lock_strategy: LockStrategy) {
        self.lock.lock_unguarded(lock_strategy);
    }
    pub fn lock(&self, lock_strategy: LockStrategy) -> MutexGuard<'_, T> {
        unsafe {
            self.lock_unguarded(lock_strategy);
        }
        MutexGuard { mutex: self }
    }
    pub fn lock_spinning(&self) -> MutexGuard<'_, T> {
        self.lock(LockStrategy::Spin)
    }
    pub fn lock_sleeping(&self) -> MutexGuard<'_, T> {
        self.lock(LockStrategy::Sleep)
    }
    pub unsafe fn unlock(&self) {
        self.lock.unlock();
    }
}
unsafe impl<T> Sync for Mutex<T> where T: Send {}
impl<T> Clone for Mutex<T> where T: Clone {
    fn clone(&self) -> Self {
        let value: T = self.lock_spinning().as_ref().clone();
        Mutex::new(value)
    }
}

pub struct MutexGuard<'m, T> {
    pub mutex: &'m Mutex<T>,
}
impl<'m, T> MutexGuard<'m, T> {
    /// Sleep until `wakeup(chan)` is called somewhere else, yielding access to the mutex until then.
    pub unsafe fn sleep(&mut self, chan: *mut core::ffi::c_void) {
        let guard = LockGuard {
            lock: &self.mutex.lock,
        };
        guard.sleep(chan);
        core::mem::forget(guard);
    }
}
impl<'m, T> Deref for MutexGuard<'m, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.as_inner() }
    }
}
impl<'m, T> DerefMut for MutexGuard<'m, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.as_inner() }
    }
}
impl<'m, T> AsRef<T> for MutexGuard<'m, T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}
impl<'m, T> AsMut<T> for MutexGuard<'m, T> {
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}
impl<'m, T> Drop for MutexGuard<'m, T> {
    fn drop(&mut self) {
        unsafe { self.mutex.unlock() }
    }
}
