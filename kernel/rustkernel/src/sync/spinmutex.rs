use core::{cell::UnsafeCell, ops::{Deref, DerefMut, Drop}, sync::atomic::{AtomicBool, Ordering}};

pub struct SpinMutex<T> {
    locked: AtomicBool,
    pub inner: UnsafeCell<T>,
}
impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> SpinMutex<T> {
        SpinMutex {
            locked: AtomicBool::new(false),
            inner: UnsafeCell::new(value),
        }
    }
    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinMutexGuard { mutex: &self }
    }
    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}
unsafe impl<T> Sync for SpinMutex<T> where T: Send {}

pub struct SpinMutexGuard<'m, T> {
    pub mutex: &'m SpinMutex<T>,
}
impl<'m, T> Deref for SpinMutexGuard<'m, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { & *self.mutex.inner.get() }
    }
}
impl<'m, T> DerefMut for SpinMutexGuard<'m, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mutex.inner.get() }
    }
}
impl<'m, T> Drop for SpinMutexGuard<'m, T> {
    fn drop(&mut self) {
        unsafe { self.mutex.unlock() }
    }
}
