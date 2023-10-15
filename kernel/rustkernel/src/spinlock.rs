use crate::{
    proc::{mycpu, Cpu},
    riscv,
};
use core::{
    ffi::c_char,
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
};

#[repr(C)]
pub struct Spinlock {
    pub locked: AtomicBool,
    pub name: *mut c_char,
    pub cpu: *mut Cpu,
}
impl Spinlock {
    /// Initializes a `Spinlock`.
    pub fn new(name: *mut c_char) -> Spinlock {
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
    pub unsafe fn lock(&mut self) {
        push_off();

        if self.held_by_current_cpu() {
            panic!("Attempt to acquire twice by the same CPU");
        }

        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }

        // The lock is now locked and we can write our CPU info.

        self.cpu = mycpu();
    }
    pub unsafe fn unlock(&mut self) {
        if !self.held_by_current_cpu() {
            panic!("Attempt to release lock from different CPU");
        }

        self.cpu = null_mut();

        self.locked.store(false, Ordering::Release);

        pop_off();
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
    (*lock).lock();
}

#[no_mangle]
pub unsafe extern "C" fn release(lock: *mut Spinlock) {
    (*lock).unlock();
}

// push_off/pop_off are like intr_off()/intr_on() except that they are matched:
// it takes two pop_off()s to undo two push_off()s.  Also, if interrupts
// are initially off, then push_off, pop_off leaves them off.

#[no_mangle]
pub unsafe extern "C" fn push_off() {
    let old = riscv::intr_get();
    let cpu = mycpu();

    riscv::intr_off();
    if (*cpu).noff == 0 {
        (*cpu).intena = old;
    }
    (*cpu).noff += 1;
}
#[no_mangle]
pub unsafe extern "C" fn pop_off() {
    let cpu = mycpu();

    if riscv::intr_get() == 1 {
        panic!("pop_off - interruptible");
    } else if (*cpu).noff < 1 {
        panic!("pop_off");
    }

    (*cpu).noff -= 1;

    if (*cpu).noff == 0 && (*cpu).intena == 1 {
        riscv::intr_on();
    }
}
