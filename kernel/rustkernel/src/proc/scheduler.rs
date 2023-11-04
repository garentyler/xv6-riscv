use super::{
    context::Context,
    cpu::Cpu,
    process::{Process, ProcessState},
};
use crate::{
    arch::riscv::intr_get,
    sync::spinlock::{Spinlock, SpinlockGuard},
};
use core::{
    ffi::c_void,
    ptr::{addr_of_mut, null_mut},
};

extern "C" {
    pub fn wakeup(chan: *const c_void);
    pub fn scheduler() -> !;
    pub fn swtch(a: *mut Context, b: *mut Context);
}

/// Give up the CPU for one scheduling round.
pub unsafe fn r#yield() {
    let p = Process::current().unwrap();
    let _guard = p.lock.lock();
    p.state = ProcessState::Runnable;
    sched();
}

/// Switch to scheduler.  Must hold only p->lock
/// and have changed proc->state. Saves and restores
/// previous_interrupts_enabled because previous_interrupts_enabled is a property of this
/// kernel thread, not this CPU. It should
/// be proc->previous_interrupts_enabled and proc->interrupt_disable_layers, but that would
/// break in the few places where a lock is held but
/// there's no process.
#[no_mangle]
pub unsafe extern "C" fn sched() {
    let p = Process::current().unwrap();
    let cpu = Cpu::current();

    if cpu.interrupt_disable_layers != 1 {
        panic!("sched locks");
    } else if p.state == ProcessState::Running {
        panic!("sched running");
    } else if intr_get() > 0 {
        panic!("sched interruptible");
    }

    let previous_interrupts_enabled = cpu.previous_interrupts_enabled;
    swtch(addr_of_mut!(p.context), addr_of_mut!(cpu.context));
    cpu.previous_interrupts_enabled = previous_interrupts_enabled;
}

/// The lock should already be locked.
/// Unsafely create a new guard for it so that we can call SpinlockGuard.sleep().
#[no_mangle]
pub unsafe extern "C" fn sleep_lock(chan: *mut c_void, lock: *mut Spinlock) {
    let lock: &Spinlock = &*lock;
    let guard = SpinlockGuard { lock };
    guard.sleep(chan);
    core::mem::forget(guard);
}

/// Sleep until `wakeup(chan)` is called somewhere else.
pub unsafe fn sleep(chan: *mut c_void) {
    let p = Process::current().unwrap();
    let _guard = p.lock.lock();

    // Go to sleep.
    p.chan = chan;
    p.state = ProcessState::Sleeping;

    sched();

    // Tidy up.
    p.chan = null_mut();
}
