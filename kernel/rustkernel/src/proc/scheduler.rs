use super::{
    context::Context,
    cpu::Cpu,
    process::{proc, Process, ProcessState},
};
use crate::{
    arch,
    sync::spinlock::{Spinlock, SpinlockGuard},
};
use core::{
    ffi::c_void,
    ptr::{addr_of, addr_of_mut, null_mut},
};

extern "C" {
    // pub fn wakeup(chan: *const c_void);
    // pub fn scheduler() -> !;
    pub fn swtch(a: *mut Context, b: *mut Context);
}

/// Give up the CPU for one scheduling round.
pub unsafe fn r#yield() {
    let p = Process::current().unwrap();
    let _guard = p.lock.lock();
    p.state = ProcessState::Runnable;
    sched();
}

// Per-CPU process scheduler.
// Each CPU calls scheduler() after setting itself up.
// Scheduler never returns.  It loops, doing:
//  - choose a process to run.
//  - swtch to start running that process.
//  - eventually that process transfers control
//    via swtch back to the scheduler.
pub unsafe fn scheduler() -> ! {
    let cpu = Cpu::current();
    cpu.proc = null_mut();

    loop {
        // Avoid deadlock by ensuring that devices can interrupt.
        arch::interrupt::enable_interrupts();

        for p in &mut proc {
            let _guard = p.lock.lock();
            if p.state == ProcessState::Runnable {
                // Switch to the chosen process. It's the process's job
                // to release its lock and then reacquire it before
                // jumping back to us.
                p.state = ProcessState::Running;
                cpu.proc = addr_of!(*p).cast_mut();

                // Run the process.
                swtch(addr_of_mut!(cpu.context), addr_of_mut!(p.context));

                // Process is done running for now.
                // It should have changed its state before coming back.
                cpu.proc = null_mut();
            }
        }
    }
}

/// Switch to scheduler.  Must hold only p->lock
/// and have changed proc->state. Saves and restores
/// previous_interrupts_enabled because previous_interrupts_enabled is a property of this
/// kernel thread, not this CPU. It should
/// be proc->previous_interrupts_enabled and proc->interrupt_disable_layers, but that would
/// break in the few places where a lock is held but
/// there's no process.
pub unsafe fn sched() {
    let p = Process::current().unwrap();
    let cpu = Cpu::current();

    if cpu.interrupt_disable_layers != 1 {
        panic!("sched locks");
    } else if p.state == ProcessState::Running {
        panic!("sched running");
    } else if arch::interrupt::interrupts_enabled() > 0 {
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

/// Wake up all processes sleeping on chan.
/// Must be called without any p.lock.
#[no_mangle]
pub unsafe extern "C" fn wakeup(chan: *mut c_void) {
    for p in &mut proc {
        if !p.is_current() {
            let _guard = p.lock.lock();
            if p.state == ProcessState::Sleeping && p.chan == chan {
                p.state = ProcessState::Runnable;
            }
        }
    }
}
