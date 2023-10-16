#![allow(clippy::comparison_chain)]

use crate::{
    kalloc::kfree,
    param::*,
    riscv::{self, Pagetable, PTE_W},
    spinlock::{pop_off, push_off, Spinlock},
};
use core::{
    ffi::{c_char, c_void},
    ptr::{addr_of_mut, null_mut},
};

extern "C" {
    pub static mut cpus: [Cpu; NCPU];
    pub static mut proc: [Proc; NPROC];
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

    pub fn forkret();
    pub fn fork() -> i32;
    pub fn exit(status: i32) -> !;
    pub fn wait(addr: u64) -> i32;
    pub fn proc_pagetable(p: *mut Proc) -> Pagetable;
    pub fn proc_freepagetable(pagetable: Pagetable, sz: u64);
    pub fn wakeup(chan: *mut c_void);
    pub fn allocproc() -> *mut Proc;
    // pub fn freeproc(p: *mut Proc);
    pub fn uvmalloc(pagetable: Pagetable, oldsz: u64, newsz: u64, xperm: i32) -> u64;
    pub fn uvmdealloc(pagetable: Pagetable, oldsz: u64, newsz: u64) -> u64;
    // pub fn sched();
    pub fn swtch(a: *mut Context, b: *mut Context);
}

/// Saved registers for kernel context switches.
#[repr(C)]
#[derive(Default)]
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

/// Per-CPU state.
#[repr(C)]
pub struct Cpu {
    /// The process running on this cpu, or null.
    pub proc: *mut Proc,
    /// swtch() here to enter scheduler()
    pub context: Context,
    /// Depth of push_off() nesting.
    pub noff: i32,
    /// Were interrupts enabled before push_off()?
    pub intena: i32,
}
impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            proc: null_mut(),
            context: Context::default(),
            noff: 0,
            intena: 0,
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
    riscv::r_tp() as i32
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
    push_off();
    let c = mycpu();
    let p = (*c).proc;
    pop_off();
    p
}

#[no_mangle]
pub unsafe extern "C" fn allocpid() -> i32 {
    let lock = addr_of_mut!(pid_lock);
    (*lock).lock();
    let pid = nextpid;
    nextpid += 1;
    (*lock).unlock();
    pid
}

/*

/// Look in the process table for an UNUSED proc.
/// If found, initialize state required to run in the kernel,
/// and return with p->lock held. If there are no free procs,
/// or a memory allocation fails, return 0.
#[no_mangle]
pub unsafe extern "C" fn allocproc() -> *mut Proc {
    for p in &mut proc {
        let lock = addr_of_mut!(p.lock);
        (*lock).lock();

        if p.state != ProcState::Unused {
            (*lock).unlock();
            continue;
        }

        let p = addr_of_mut!(*p);
        (*p).pid = allocpid();
        (*p).state = ProcState::Used;

        // Allocate a trapframe page and
        // create an empty user page table.
        (*p).trapframe = kalloc().cast();
        (*p).pagetable = proc_pagetable(p);

        if (*p).trapframe.is_null() || (*p).pagetable.is_null() {
            freeproc(p);
            (*p).lock.unlock();
            return null_mut();
        }

        // Set up new context to start executing
        // at forkret which returns to user space.
        memset(addr_of_mut!((*p).context).cast(), 0, size_of::<Context>() as u32);
        // TODO: convert fn pointer to u64
        (*p).context.ra = forkret as usize as u64;
        (*p).context.sp = (*p).kstack + PGSIZE;

        return p;
    }

    null_mut()
}
*/

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

// /// Wake up all processes sleeping on chan.
// /// Must be called without any p->lock.
// #[no_mangle]
// pub unsafe extern "C" fn wakeup(chan: *mut c_void) {
//     for p in &mut proc {
//         let p: *mut Proc = addr_of_mut!(*p);
//
//         if p != myproc() {
//             (*p).lock.lock();
//             if (*p).state == ProcState::Sleeping && (*p).chan == chan {
//                 (*p).state = ProcState::Runnable;
//             }
//             (*p).lock.unlock();
//         }
//     }
// }

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
        sz = uvmalloc((*p).pagetable, sz, sz.wrapping_add(n as u64), PTE_W as i32);
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
    (*p).lock.lock();
    (*p).state = ProcState::Runnable;
    sched();
    (*p).lock.unlock();
}

/// Switch to scheduler.  Must hold only p->lock
/// and have changed proc->state. Saves and restores
/// intena because intena is a property of this
/// kernel thread, not this CPU. It should
/// be proc->intena and proc->noff, but that would
/// break in the few places where a lock is held but
/// there's no process.
#[no_mangle]
pub unsafe extern "C" fn sched() {
    let p = myproc();
    let c = mycpu();

    if !(*p).lock.held_by_current_cpu() {
        panic!("sched p->lock");
    } else if (*c).noff != 1 {
        panic!("sched locks");
    } else if (*p).state == ProcState::Running {
        panic!("sched running");
    } else if riscv::intr_get() > 0 {
        panic!("sched interruptible");
    }

    let intena = (*c).intena;
    swtch(addr_of_mut!((*p).context), addr_of_mut!((*c).context));
    (*c).intena = intena;
}

/// Atomically release lock and sleep on chan.
/// Reacquires lock when awakened.
#[no_mangle]
pub unsafe extern "C" fn sleep(chan: *mut c_void, lock: *mut Spinlock) {
    let p = myproc();

    // Must acquire p->lock in order to
    // change p->state and then call sched.
    // Once we hold p->lock, we can be
    // guaranteed that we won't miss any wakeup
    // (wakeup locks p->lock),
    // so it's okay to release lk.

    (*p).lock.lock();
    (*lock).unlock();

    // Go to sleep.
    (*p).chan = chan;
    (*p).state = ProcState::Sleeping;

    sched();

    // Tidy up.
    (*p).chan = null_mut();

    // Reacquire original lock.
    (*p).lock.unlock();
    (*lock).lock();
}

/// Kill the process with the given pid.
/// The victim won't exit until it tries to return
/// to user space (see usertrap() in trap.c).
#[no_mangle]
pub unsafe extern "C" fn kill(pid: i32) -> i32 {
    for p in &mut proc {
        p.lock.lock();

        if p.pid == pid {
            p.killed = 1;

            if p.state == ProcState::Sleeping {
                // Wake process from sleep().
                p.state = ProcState::Runnable;
            }

            p.lock.unlock();
            return 0;
        }
        p.lock.unlock();
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn setkilled(p: *mut Proc) {
    (*p).lock.lock();
    (*p).killed = 1;
    (*p).lock.unlock();
}

#[no_mangle]
pub unsafe extern "C" fn killed(p: *mut Proc) -> i32 {
    (*p).lock.lock();
    let k = (*p).killed;
    (*p).lock.unlock();
    k
}
