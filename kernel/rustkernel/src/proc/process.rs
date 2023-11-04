#![allow(clippy::comparison_chain)]

use super::{context::Context, cpu::Cpu, scheduler::wakeup, trapframe::Trapframe};
use crate::{
    arch::riscv::{Pagetable, PTE_W, PTE_R, PTE_X, PGSIZE, memlayout::{TRAMPOLINE, TRAPFRAME}},
    fs::{file::{File, Inode, filedup}, idup},
    mem::{
        kalloc::{kfree, kalloc},
        memset,
        virtual_memory::{uvmcreate, uvmfree, uvmunmap, mappages, uvmalloc, uvmdealloc, uvmcopy},
    },
    sync::spinlock::Spinlock,
    string::safestrcpy,
};
use core::{
    ffi::{c_char, c_void},
    ptr::{addr_of, addr_of_mut, null_mut},
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
    // pub fn fork() -> i32;
    pub fn exit(status: i32) -> !;
    pub fn wait(addr: u64) -> i32;
    pub fn procdump();
    pub fn proc_mapstacks(kpgtbl: Pagetable);
}

pub static NEXT_PID: AtomicI32 = AtomicI32::new(1);

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum ProcessState {
    #[default]
    Unused,
    Used,
    Sleeping,
    Runnable,
    Running,
    Zombie,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ProcessError {
    MaxProcesses,
    Allocation,
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

    pub fn alloc_pid() -> i32 {
        NEXT_PID.fetch_add(1, Ordering::SeqCst)
    }
    /// Look in the process table for an UNUSED proc.
    /// If found, initialize state required to run in the kernel,
    /// and return with p.lock held.
    /// If there are no free procs, or a memory allocation fails, return an error.
    pub unsafe fn alloc() -> Result<&'static mut Process, ProcessError> {
        let mut index: Option<usize> = None;
        for (i, p) in &mut proc.iter_mut().enumerate() {
            p.lock.lock_unguarded();
            if p.state == ProcessState::Unused {
                index = Some(i);
                break;
            } else {
                p.lock.unlock();
            }
        }
        let Some(index) = index else {
            return Err(ProcessError::MaxProcesses);
        };

        let p: &mut Process = &mut proc[index];
        p.pid = Process::alloc_pid();
        p.state = ProcessState::Used;

        // Allocate a trapframe page.
        p.trapframe = kalloc() as *mut Trapframe;
        if p.trapframe.is_null() {
            p.free();
            p.lock.unlock();
            return Err(ProcessError::Allocation);
        }

        // An empty user page table.
        p.pagetable = proc_pagetable(addr_of_mut!(*p));
        if p.pagetable.is_null() {
            p.free();
            p.lock.unlock();
            return Err(ProcessError::Allocation);
        }

        // Set up new context to start executing at forkret,
        // which returns to userspace.
        memset(addr_of_mut!(p.context).cast(), 0, core::mem::size_of::<Context>() as u32);
        p.context.ra = forkret as usize as u64;
        p.context.sp = p.kstack + PGSIZE;

        Ok(p)
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

    /// Grow or shrink user memory.
    pub unsafe fn grow_memory(&mut self, num_bytes: i32) -> Result<(), ProcessError> {
        let mut size = self.sz;

        if num_bytes > 0 {
            size = uvmalloc(self.pagetable, size, size.wrapping_add(num_bytes as u64), PTE_W);

            if size == 0 {
                return Err(ProcessError::Allocation);
            }
        } else if num_bytes < 0 {
            size = uvmdealloc(self.pagetable, size, size.wrapping_add(num_bytes as u64));
        }

        self.sz = size;
        Ok(())
    }

    /// Create a user page table for a given process,
    /// with no user memory, but with trampoline and trapframe pages.
    pub unsafe fn alloc_pagetable(&mut self) -> Result<Pagetable, ProcessError> {
        // Create an empty page table.
        let pagetable: Pagetable = uvmcreate();
        if pagetable.is_null() {
            return Err(ProcessError::Allocation);
        }
        
        // Map the trampoline code (for syscall return)
        // at the highest user virtual address.
        // Only the supervisor uses it on the way
        // to and from user space, so not PTE_U.
        if mappages(pagetable, TRAMPOLINE, PGSIZE, addr_of!(trampoline) as usize as u64, PTE_R | PTE_X) < 0 {
            uvmfree(pagetable, 0);
            return Err(ProcessError::Allocation);
        }

        // Map the trapframe page just below the trampoline page for trampoline.S.
        if mappages(pagetable, TRAPFRAME, PGSIZE, self.trapframe as usize as u64, PTE_R | PTE_W) < 0 {
            uvmunmap(pagetable, TRAMPOLINE, 1, 0);
            uvmfree(pagetable, 0);
            return Err(ProcessError::Allocation);
        }
        
        Ok(pagetable)
    }
    /// Free a process's pagetable and free the physical memory it refers to.
    pub unsafe fn free_pagetable(pagetable: Pagetable, size: usize) {
        uvmunmap(pagetable, TRAMPOLINE, 1, 0);
        uvmunmap(pagetable, TRAPFRAME, 1, 0);
        uvmfree(pagetable, size as u64)
    }

    /// Create a new process, copying the parent.
    /// Sets up child kernel stack to return as if from fork() syscall.
    pub unsafe fn fork() -> Result<i32, ProcessError> {
        let parent = Process::current().unwrap();
        let child = Process::alloc()?;

        // Copy user memory from parent to child.
        if uvmcopy(parent.pagetable, child.pagetable, parent.sz) < 0 {
            child.free();
            child.lock.unlock();
            return Err(ProcessError::Allocation);
        }
        child.sz = parent.sz;

        // Copy saved user registers.
        *child.trapframe = *parent.trapframe;

        // Cause fork to return 0 in the child.
        (*child.trapframe).a0 = 0;

        // Increment reference counts on open file descriptors.
        for (i, file) in parent.ofile.iter().enumerate() {
            if !file.is_null() {
                child.ofile[i] = filedup(parent.ofile[i]);
            }
        }
        child.cwd = idup(parent.cwd);

        safestrcpy(addr_of!(child.name[0]).cast_mut().cast(), addr_of!(parent.name[0]).cast_mut().cast(), parent.name.len() as i32);
        
        let pid = child.pid;

        child.lock.unlock();
        {
            let _guard = wait_lock.lock();
            child.parent = addr_of!(*parent).cast_mut();
        }
        {
            let _guard = child.lock.lock();
            child.state = ProcessState::Runnable;
        }

        Ok(pid)
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
    Process::alloc_pid()
}

#[no_mangle]
pub unsafe extern "C" fn allocproc() -> *mut Process {
    if let Ok(process) = Process::alloc() {
        process as *mut Process
    } else {
        null_mut()
    }
}

/// Free a proc structure and the data hanging from it, including user pages.
/// p->lock must be held.
#[no_mangle]
pub unsafe extern "C" fn freeproc(p: *mut Process) {
    (*p).free();
}

#[no_mangle]
pub unsafe extern "C" fn proc_pagetable(p: *mut Process) -> Pagetable {
    (*p).alloc_pagetable().unwrap_or(null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn proc_freepagetable(pagetable: Pagetable, size: u64) {
    Process::free_pagetable(pagetable, size as usize)
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
