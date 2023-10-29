use crate::{
    proc::{myproc, sched, ProcState},
    sync::spinlock::Spinlock,
};
use core::{
    cell::UnsafeCell,
    convert::{AsMut, AsRef},
    ops::{Deref, DerefMut, Drop},
    ptr::null_mut,
};

pub struct SpinMutex<T> {
    lock: Spinlock,
    inner: UnsafeCell<T>,
}
impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> SpinMutex<T> {
        SpinMutex {
            lock: Spinlock::new(),
            inner: UnsafeCell::new(value),
        }
    }
    pub unsafe fn as_inner(&self) -> *mut T {
        self.inner.get()
    }
    pub unsafe fn lock_unguarded(&self) {
        self.lock.lock_unguarded();
    }
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        unsafe {
            self.lock_unguarded();
        }
        SpinMutexGuard { mutex: self }
    }
    pub unsafe fn unlock(&self) {
        self.lock.unlock();
    }
}
unsafe impl<T> Sync for SpinMutex<T> where T: Send {}

pub struct SpinMutexGuard<'m, T> {
    pub mutex: &'m SpinMutex<T>,
}
impl<'m, T> SpinMutexGuard<'m, T> {
    /// Sleep until `wakeup(chan)` is called somewhere else, yielding access to the mutex until then.
    pub unsafe fn sleep(&mut self, chan: *mut core::ffi::c_void) {
        let p = myproc();
        let _guard = (*p).lock.lock();
        self.mutex.unlock();

        // Put the process to sleep.
        (*p).chan = chan;
        (*p).state = ProcState::Sleeping;
        sched();

        // Tidy up and reacquire the mutex.
        (*p).chan = null_mut();
        self.mutex.lock_unguarded();
    }
}
impl<'m, T> Deref for SpinMutexGuard<'m, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mutex.as_inner() }
    }
}
impl<'m, T> DerefMut for SpinMutexGuard<'m, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.as_inner() }
    }
}
impl<'m, T> AsRef<T> for SpinMutexGuard<'m, T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}
impl<'m, T> AsMut<T> for SpinMutexGuard<'m, T> {
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}
impl<'m, T> Drop for SpinMutexGuard<'m, T> {
    fn drop(&mut self) {
        unsafe { self.mutex.unlock() }
    }
}
