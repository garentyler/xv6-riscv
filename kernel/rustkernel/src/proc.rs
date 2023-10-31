#![allow(clippy::comparison_chain)]

use crate::{
    mem::kalloc::kfree,
    arch::riscv::{Pagetable, PTE_W, intr_get, r_tp},
    sync::spinlock::{Spinlock, SpinlockGuard},
};
use core::{
    ffi::{c_char, c_void},
    ptr::{addr_of_mut, null_mut},
};

extern "C" {
    pub static mut cpus: [Cpu; crate::NCPU];
    pub static mut proc: [Proc; crate::NPROC];
    pub static mut initproc: *mut Proc;
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
    pub fn proc_pagetable(p: *mut Proc) -> Pagetable;
    pub fn proc_freepagetable(pagetable: Pagetable, sz: u64);
    pub fn wakeup(chan: *const c_void);
    pub fn allocproc() -> *mut Proc;
    // pub fn freeproc(p: *mut Proc);
    pub fn uvmalloc(pagetable: Pagetable, oldsz: u64, newsz: u64, xperm: i32) -> u64;
    pub fn uvmdealloc(pagetable: Pagetable, oldsz: u64, newsz: u64) -> u64;
    // pub fn sched();
    pub fn scheduler() -> !;
    pub fn swtch(a: *mut Context, b: *mut Context);
}

/// Saved registers for kernel context switches.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Context {
    pub ra: u64,
    pub sp: u64,

    // callee-saved
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
}
impl Context {
    pub const fn new() -> Context {
        Context {
            ra: 0u64,
            sp: 0u64,
            s0: 0u64,
            s1: 0u64,
            s2: 0u64,
            s3: 0u64,
            s4: 0u64,
            s5: 0u64,
            s6: 0u64,
            s7: 0u64,
            s8: 0u64,
            s9: 0u64,
            s10: 0u64,
            s11: 0u64,
        }
    }
}

/// Per-CPU state.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Cpu {
    pub proc: *mut Proc,
    /// swtch() here to enter scheduler()
    pub context: Context,
    /// Depth of push_off() nesting.
    pub interrupt_disable_layers: i32,
    /// Were interrupts enabled before push_off()?
    pub previous_interrupts_enabled: i32,
}
impl Cpu {
    pub const fn new() -> Cpu {
        Cpu {
            proc: null_mut(),
            // proc: None,
            context: Context::new(),
            interrupt_disable_layers: 0,
            previous_interrupts_enabled: 0,
        }
    }
}

/// Per-process data for the trap handling code in trampoline.S.
///
/// sits in a page by itself just under the trampoline page in the
/// user page table. not specially mapped in the kernel page table.
/// uservec in trampoline.S saves user registers in the trapframe,
/// then initializes registers from the trapframe's
/// kernel_sp, kernel_hartid, kernel_satp, and jumps to kernel_trap.
/// usertrapret() and userret in trampoline.S set up
/// the trapframe's kernel_*, restore user registers from the
/// trapframe, switch to the user page table, and enter user space.
/// the trapframe includes callee-saved user registers like s0-s11 because the
/// return-to-user path via usertrapret() doesn't return through
/// the entire kernel call stack.
#[repr(C)]
#[derive(Default)]
pub struct TrapFrame {
    /// Kernel page table.
    pub kernel_satp: u64,
    /// Top of process's kernel stack.
    pub kernel_sp: u64,
    /// usertrap()
    pub kernel_trap: u64,
    /// Saved user program counter.
    pub epc: u64,
    /// Saved kernel tp.
    pub kernel_hartid: u64,
    pub ra: u64,
    pub sp: u64,
    pub gp: u64,
    pub tp: u64,
    pub t0: u64,
    pub t1: u64,
    pub t2: u64,
    pub s0: u64,
    pub s1: u64,
    pub a0: u64,
    pub a1: u64,
    pub a2: u64,
    pub a3: u64,
    pub a4: u64,
    pub a5: u64,
    pub a6: u64,
    pub a7: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
    pub t3: u64,
    pub t4: u64,
    pub t5: u64,
    pub t6: u64,
}

#[repr(C)]
#[derive(PartialEq, Default)]
pub enum ProcState {
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
pub struct Proc {
    pub lock: Spinlock,

    // p->lock must be held when using these:
    /// Process state
    pub state: ProcState,
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
    pub parent: *mut Proc,

    // These are private to the process, so p->lock need not be held.
    /// Virtual address of kernel stack
    pub kstack: u64,
    /// Size of process memory (bytes)
    pub sz: u64,
    /// User page table
    pub pagetable: Pagetable,
    /// Data page for trampoline.S
    pub trapframe: *mut TrapFrame,
    /// swtch() here to run process
    pub context: Context,
    /// Open files
    pub ofile: *mut u8, // TODO: Change u8 ptr to File ptr.
    /// Current directory
    pub cwd: *mut u8, // TODO: Change u8 ptr to inode ptr.
    /// Process name (debugging)
    pub name: [c_char; 16],
}

/// Must be called with interrupts disabled
/// to prevent race with process being moved
/// to a different CPU.
#[no_mangle]
pub unsafe extern "C" fn cpuid() -> i32 {
    r_tp() as i32
}

/// Return this CPU's cpu struct.
/// Interrupts must be disabled.
#[no_mangle]
pub unsafe extern "C" fn mycpu() -> *mut Cpu {
    let id = cpuid();
    addr_of_mut!(cpus[id as usize])
}

/// Return the current struct proc *, or zero if none.
#[no_mangle]
pub unsafe extern "C" fn myproc() -> *mut Proc {
    let _ = crate::trap::InterruptBlocker::new();
    let c = mycpu();
    (*c).proc
}

#[no_mangle]
pub unsafe extern "C" fn allocpid() -> i32 {
    let _guard = pid_lock.lock();
    let pid = nextpid;
    nextpid += 1;
    pid
}

/// Free a proc structure and the data hanging from it, including user pages.
/// p->lock must be held.
#[no_mangle]
pub unsafe extern "C" fn freeproc(p: *mut Proc) {
    if !(*p).trapframe.is_null() {
        kfree((*p).trapframe.cast());
    }
    (*p).trapframe = null_mut();
    if !(*p).pagetable.is_null() {
        proc_freepagetable((*p).pagetable, (*p).sz);
    }
    (*p).pagetable = null_mut();
    (*p).sz = 0;
    (*p).pid = 0;
    (*p).parent = null_mut();
    (*p).name[0] = 0;
    (*p).chan = null_mut();
    (*p).killed = 0;
    (*p).xstate = 0;
    (*p).state = ProcState::Unused;
}

/// Pass p's abandoned children to init.
/// Caller must hold wait_lock.
#[no_mangle]
pub unsafe extern "C" fn reparent(p: *mut Proc) {
    for pp in proc.iter_mut().map(|p: &mut Proc| addr_of_mut!(*p)) {
        if (*pp).parent == p {
            (*pp).parent = initproc;
            wakeup(initproc.cast());
        }
    }
}

/// Grow or shrink user memory by n bytes.
/// Return 0 on success, -1 on failure.
#[no_mangle]
pub unsafe extern "C" fn growproc(n: i32) -> i32 {
    let p = myproc();
    let mut sz = (*p).sz;

    if n > 0 {
        sz = uvmalloc((*p).pagetable, sz, sz.wrapping_add(n as u64), PTE_W);
        if sz == 0 {
            return -1;
        }
    } else if n < 0 {
        sz = uvmdealloc((*p).pagetable, sz, sz.wrapping_add(n as u64));
    }
    (*p).sz = sz;
    0
}

/// Give up the CPU for one scheduling round.
#[no_mangle]
pub unsafe extern "C" fn r#yield() {
    let p = myproc();
    let _guard = (*p).lock.lock();
    (*p).state = ProcState::Runnable;
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
    let p = myproc();
    let c = mycpu();

    if (*c).interrupt_disable_layers != 1 {
        panic!("sched locks");
    } else if (*p).state == ProcState::Running {
        panic!("sched running");
    } else if intr_get() > 0 {
        panic!("sched interruptible");
    }

    let previous_interrupts_enabled = (*c).previous_interrupts_enabled;
    swtch(addr_of_mut!((*p).context), addr_of_mut!((*c).context));
    (*c).previous_interrupts_enabled = previous_interrupts_enabled;
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
    let p = myproc();
    let _guard = (*p).lock.lock();

    // Go to sleep.
    (*p).chan = chan;
    (*p).state = ProcState::Sleeping;

    sched();

    // Tidy up.
    (*p).chan = null_mut();
}

/// Kill the process with the given pid.
/// The victim won't exit until it tries to return
/// to user space (see usertrap() in trap.c).
#[no_mangle]
pub unsafe extern "C" fn kill(pid: i32) -> i32 {
    for p in &mut proc {
        let _guard = p.lock.lock();

        if p.pid == pid {
            p.killed = 1;

            if p.state == ProcState::Sleeping {
                // Wake process from sleep().
                p.state = ProcState::Runnable;
            }

            return 0;
        }
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn setkilled(p: *mut Proc) {
    let _guard = (*p).lock.lock();
    (*p).killed = 1;
}

#[no_mangle]
pub unsafe extern "C" fn killed(p: *mut Proc) -> i32 {
    let _guard = (*p).lock.lock();
    (*p).killed
}
