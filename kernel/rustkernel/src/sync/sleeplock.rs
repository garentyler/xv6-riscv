use crate::{
    proc::{myproc, sleep, wakeup},
    sync::spinlock::{self, Spinlock},
};
use core::{ffi::c_char, ptr::addr_of_mut};

#[repr(C)]
pub struct Sleeplock {
    pub locked: u32,
    pub inner: Spinlock,
    pub name: *mut c_char,
    pub pid: i32,
}

#[no_mangle]
pub unsafe extern "C" fn initsleeplock(lock: *mut Sleeplock, name: *mut c_char) {
    spinlock::initlock(addr_of_mut!((*lock).inner), name);
    (*lock).name = name;
    (*lock).locked = 0;
    (*lock).pid = 0;
}

#[no_mangle]
pub unsafe extern "C" fn acquiresleep(lock: *mut Sleeplock) {
    (*lock).inner.lock();
    while (*lock).locked > 0 {
        sleep(lock.cast(), addr_of_mut!((*lock).inner));
    }
    (*lock).locked = 1;
    (*lock).pid = (*myproc()).pid;
    (*lock).inner.unlock()
}

#[no_mangle]
pub unsafe extern "C" fn releasesleep(lock: *mut Sleeplock) {
    (*lock).inner.lock();
    (*lock).locked = 0;
    (*lock).pid = 0;
    wakeup(lock.cast());
    (*lock).inner.unlock();
}

#[no_mangle]
pub unsafe extern "C" fn holdingsleep(lock: *mut Sleeplock) -> i32 {
    (*lock).inner.lock();
    let holding = ((*lock).locked > 0) && ((*lock).pid == (*myproc()).pid);
    (*lock).inner.unlock();
    if holding {
        1
    } else {
        0
    }
}
