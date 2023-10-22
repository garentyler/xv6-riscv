// use core::{
//     cell::UnsafeCell,
//     ops::{Deref, DerefMut, Drop},
//     sync::atomic::{AtomicBool, Ordering},
//     ffi::c_void,
//     ptr::addr_of,
// };
// use crate::proc::{sleep, wakeup, sched, myproc, ProcState};
//
// pub struct Mutex<T> {
//     locked: AtomicBool,
//     inner: UnsafeCell<T>,
// }
// impl<T> Mutex<T> {
//     pub const fn new(value: T) -> Mutex<T> {
//         Mutex {
//             locked: AtomicBool::new(false),
//             inner: UnsafeCell::new(value),
//         }
//     }
//     pub unsafe fn get_inner(&self) -> *mut T {
//         self.inner.get()
//     }
//     /// Spin until the mutex is unlocked, acquiring afterwards.
//     pub fn spin_lock(&self) -> MutexGuard<'_, T> {
//         while self.locked.swap(true, Ordering::Acquire) {
//             core::hint::spin_loop();
//         }
//
//         MutexGuard { mutex: self }
//     }
//     /// Sleep until the mutex is unlocked, acquiring afterwards.
//     pub fn sleep_lock(&self) -> MutexGuard<'_, T> {
//         while self.locked.swap(true, Ordering::Acquire) {
//             unsafe {
//                 sleep(addr_of!(*self).cast_mut().cast());
//             }
//         }
//
//         MutexGuard { mutex: self }
//     }
//     pub unsafe fn unlock(&self) {
//         self.locked.store(false, Ordering::Release);
//         wakeup(addr_of!(*self).cast_mut().cast());
//     }
// }
// unsafe impl<T> Sync for Mutex<T> where T: Send {}
//
// pub struct MutexGuard<'m, T> {
//     pub mutex: &'m Mutex<T>,
// }
// impl<'m, T> MutexGuard<'m, T> {
//     pub unsafe fn sleep(&mut self, channel: *mut c_void){
//         let p = myproc();
//         let _guard = (*p).lock.lock();
//         self.mutex.unlock();
//
//         // Go to sleep.
//         (*p).chan = channel;
//         (*p).state = ProcState::Sleeping;
//         sched();
//
//         // Clean up.
//         let guard = self.mutex.spin_lock();
//         core::mem::forget(guard);
//     }
// }
// impl<'m, T> Deref for MutexGuard<'m, T> {
//     type Target = T;
//
//     fn deref(&self) -> &Self::Target {
//         unsafe { &*self.mutex.get_inner() }
//     }
// }
// impl<'m, T> DerefMut for MutexGuard<'m, T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         unsafe { &mut *self.mutex.get_inner() }
//     }
// }
// impl<'m, T> Drop for MutexGuard<'m, T> {
//     fn drop(&mut self) {
//         unsafe { self.mutex.unlock() }
//     }
// }
//
