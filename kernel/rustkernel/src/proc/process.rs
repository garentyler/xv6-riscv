#![allow(clippy::comparison_chain)]

use super::{context::Context, cpu::Cpu, trapframe::Trapframe};
use crate::{
    arch::riscv::{intr_get, Pagetable, PTE_W},
    fs::file::{File, Inode},
    mem::kalloc::kfree,
    sync::spinlock::{Spinlock, SpinlockGuard},
};
use core::{
    ffi::{c_char, c_void},
    ptr::{addr_of_mut, null_mut},
    sync::atomic::{AtomicI32, Ordering},
};

extern "C" {
    pub static mut proc: [Process; crate::NPROC];
    pub static mut initproc: *mut Process;
    pub static mut nextpid: i32;
    pub static mut pid_lock: Spinlock;
    /// Helps ensure that wakeups of wait()ing
    /// parents are not lost. Helps obey the
    /// memory model when using p->parent.
    /// Must be acquired before any p->lock.
    pub static mut wait_lock: Spinlock;
    // trampoline.S
    pub static mut trampoline: *mut c_char;

    pub fn procinit();
    pub fn userinit();
    pub fn forkret();
    pub fn fork() -> i32;
    pub fn exit(status: i32) -> !;
    pub fn wait(addr: u64) -> i32;
    pub fn procdump();
    pub fn proc_mapstacks(kpgtbl: Pagetable);
    pub fn proc_pagetable(p: *mut Process) -> Pagetable;
    pub fn proc_freepagetable(pagetable: Pagetable, sz: u64);
    pub fn wakeup(chan: *const c_void);
    pub fn allocproc() -> *mut Process;
    // pub fn freeproc(p: *mut Process);
    pub fn uvmalloc(pagetable: Pagetable, oldsz: u64, newsz: u64, xperm: i32) -> u64;
    pub fn uvmdealloc(pagetable: Pagetable, oldsz: u64, newsz: u64) -> u64;
    // pub fn sched();
    pub fn scheduler() -> !;
    pub fn swtch(a: *mut Context, b: *mut Context);
}

pub static NEXT_PID: AtomicI32 = AtomicI32::new(1);

#[repr(C)]
#[derive(PartialEq, Default)]
pub enum ProcessState {
    #[default]
    Unused,
    Used,
    Sleeping,
    Runnable,
    Running,
    Zombie,
}

/// Per-process state.
#[repr(C)]
pub struct Process {
    pub lock: Spinlock,

    // p->lock must be held when using these:
    /// Process state
    pub state: ProcessState,
    /// If non-zero, sleeping on chan
    pub chan: *mut c_void,
    /// If non-zero, have been killed
    pub killed: i32,
    /// Exit status to be returned to parent's wait
    pub xstate: i32,
    /// Process ID
    pub pid: i32,

    // wait_lock msut be held when using this:
    /// Parent process
    pub parent: *mut Process,

    // These are private to the process, so p->lock need not be held.
    /// Virtual address of kernel stack
    pub kstack: u64,
    /// Size of process memory (bytes)
    pub sz: u64,
    /// User page table
    pub pagetable: Pagetable,
    /// Data page for trampoline.S
    pub trapframe: *mut Trapframe,
    /// swtch() here to run process
    pub context: Context,
    /// Open files
    pub ofile: [*mut File; crate::NOFILE],
    /// Current directory
    pub cwd: *mut Inode,
    /// Process name (debugging)
    pub name: [c_char; 16],
}
impl Process {
    pub const fn new() -> Process {
        Process {
            lock: Spinlock::new(),
            state: ProcessState::Unused,
            chan: null_mut(),
            killed: 0,
            xstate: 0,
            pid: 0,
            parent: null_mut(),
            kstack: 0,
            sz: 0,
            pagetable: null_mut(),
            trapframe: null_mut(),
            context: Context::new(),
            ofile: [null_mut(); crate::NOFILE],
            cwd: null_mut(),
            name: [0x00; 16],
        }
    }
    pub fn current() -> Option<&'static mut Process> {
        let _ = crate::trap::InterruptBlocker::new();
        let p = Cpu::current().proc;
        if p.is_null() {
            None
        } else {
            unsafe { Some(&mut *p) }
        }
    }

    /// Free a proc structure and the data hanging from it, including user pages.
    /// self.lock must be held.
    pub unsafe fn free(&mut self) {
        if !self.trapframe.is_null() {
            kfree(self.trapframe.cast());
        }
        self.trapframe = null_mut();
        if !self.pagetable.is_null() {
            proc_freepagetable(self.pagetable, self.sz);
        }
        self.pagetable = null_mut();
        self.sz = 0;
        self.pid = 0;
        self.parent = null_mut();
        self.name[0] = 0;
        self.chan = null_mut();
        self.killed = 0;
        self.xstate = 0;
        self.state = ProcessState::Unused;
    }

    /// Kill the process with the given pid.
    /// Returns true if the process was killed.
    /// The victim won't exit until it tries to return
    /// to user space (see usertrap() in trap.c).
    pub unsafe fn kill(pid: i32) -> bool {
        for p in &mut proc {
            let _guard = p.lock.lock();

            if p.pid == pid {
                p.killed = 1;

                if p.state == ProcessState::Sleeping {
                    // Wake process from sleep().
                    p.state = ProcessState::Runnable;
                }

                return true;
            }
        }

        false
    }
    pub fn is_killed(&self) -> bool {
        let _guard = self.lock.lock();
        self.killed > 0
    }
    pub fn set_killed(&mut self, killed: bool) {
        let _guard = self.lock.lock();
        if killed {
            self.killed = 1;
        } else {
            self.killed = 0;
        }
    }
}

/// Return the current struct proc *, or zero if none.
#[no_mangle]
pub extern "C" fn myproc() -> *mut Process {
    if let Some(p) = Process::current() {
        p as *mut Process
    } else {
        null_mut()
    }
}

#[no_mangle]
pub extern "C" fn allocpid() -> i32 {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

/// Free a proc structure and the data hanging from it, including user pages.
/// p->lock must be held.
#[no_mangle]
pub unsafe extern "C" fn freeproc(p: *mut Process) {
    (*p).free();
}

/// Pass p's abandoned children to init.
/// Caller must hold wait_lock.
#[no_mangle]
pub unsafe extern "C" fn reparent(p: *mut Process) {
    for pp in proc.iter_mut().map(|p: &mut Process| addr_of_mut!(*p)) {
        if (*pp).parent == p {
            (*pp).parent = initproc;
            wakeup(initproc.cast());
        }
    }
}

/// Grow or shrink user memory by n bytes.
/// Return 0 on success, -1 on failure.
pub unsafe fn growproc(n: i32) -> i32 {
    let p = Process::current().unwrap();
    let mut sz = p.sz;

    if n > 0 {
        sz = uvmalloc(p.pagetable, sz, sz.wrapping_add(n as u64), PTE_W);
        if sz == 0 {
            return -1;
        }
    } else if n < 0 {
        sz = uvmdealloc(p.pagetable, sz, sz.wrapping_add(n as u64));
    }
    p.sz = sz;
    0
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

/// Kill the process with the given pid.
/// The victim won't exit until it tries to return
/// to user space (see usertrap() in trap.c).
#[no_mangle]
pub unsafe extern "C" fn kill(pid: i32) -> i32 {
    if Process::kill(pid) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn setkilled(p: *mut Process) {
    (*p).set_killed(true);
}

#[no_mangle]
pub unsafe extern "C" fn killed(p: *mut Process) -> i32 {
    if (*p).is_killed() {
        1
    } else {
        0
    }
}
