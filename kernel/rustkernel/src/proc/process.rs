#![allow(clippy::comparison_chain)]

use super::{
    context::Context,
    cpu::Cpu,
    scheduler::{sched, wakeup},
    trapframe::Trapframe,
};
use crate::{
    arch::riscv::{
        memlayout::{TRAMPOLINE, TRAPFRAME},
        Pagetable, PGSIZE, PTE_R, PTE_W, PTE_X,
    },
    fs::{
        file::{fileclose, filedup, File, Inode},
        idup, iput,
        log::LogOperation,
    },
    mem::{
        kalloc::{kalloc, kfree},
        memset,
        virtual_memory::{
            copyout, mappages, uvmalloc, uvmcopy, uvmcreate, uvmdealloc, uvmfree, uvmunmap,
        },
    },
    sync::spinlock::Spinlock,
    uprintln,
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
    // pub fn exit(status: i32) -> !;
    pub fn wait(addr: u64) -> i32;
    // pub fn procdump();
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
    NoChildren,
    Killed,
    PageError,
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
    pub exit_status: i32,
    /// Process ID
    pub pid: i32,

    // wait_lock msut be held when using this:
    /// Parent process
    pub parent: *mut Process,

    // These are private to the process, so p->lock need not be held.
    /// Virtual address of kernel stack
    pub kernel_stack: u64,
    /// Size of process memory (bytes)
    pub memory_allocated: u64,
    /// User page table
    pub pagetable: Pagetable,
    /// Data page for trampoline.S
    pub trapframe: *mut Trapframe,
    /// swtch() here to run process
    pub context: Context,
    /// Open files
    pub open_files: [*mut File; crate::NOFILE],
    /// Current directory
    pub current_dir: *mut Inode,
}
impl Process {
    pub const fn new() -> Process {
        Process {
            lock: Spinlock::new(),
            state: ProcessState::Unused,
            chan: null_mut(),
            killed: 0,
            exit_status: 0,
            pid: 0,
            parent: null_mut(),
            kernel_stack: 0,
            memory_allocated: 0,
            pagetable: null_mut(),
            trapframe: null_mut(),
            context: Context::new(),
            open_files: [null_mut(); crate::NOFILE],
            current_dir: null_mut(),
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
    pub fn is_current(&self) -> bool {
        addr_of!(*self).cast_mut() == Cpu::current().proc
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
        memset(
            addr_of_mut!(p.context).cast(),
            0,
            core::mem::size_of::<Context>() as u32,
        );
        p.context.ra = forkret as usize as u64;
        p.context.sp = p.kernel_stack + PGSIZE;

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
            proc_freepagetable(self.pagetable, self.memory_allocated);
        }
        self.pagetable = null_mut();
        self.memory_allocated = 0;
        self.pid = 0;
        self.parent = null_mut();
        self.chan = null_mut();
        self.killed = 0;
        self.exit_status = 0;
        self.state = ProcessState::Unused;
    }

    /// Grow or shrink user memory.
    pub unsafe fn grow_memory(&mut self, num_bytes: i32) -> Result<(), ProcessError> {
        let mut size = self.memory_allocated;

        if num_bytes > 0 {
            size = uvmalloc(
                self.pagetable,
                size,
                size.wrapping_add(num_bytes as u64),
                PTE_W,
            );

            if size == 0 {
                return Err(ProcessError::Allocation);
            }
        } else if num_bytes < 0 {
            size = uvmdealloc(self.pagetable, size, size.wrapping_add(num_bytes as u64));
        }

        self.memory_allocated = size;
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
        if mappages(
            pagetable,
            TRAMPOLINE,
            PGSIZE,
            addr_of!(trampoline) as usize as u64,
            PTE_R | PTE_X,
        ) < 0
        {
            uvmfree(pagetable, 0);
            return Err(ProcessError::Allocation);
        }

        // Map the trapframe page just below the trampoline page for trampoline.S.
        if mappages(
            pagetable,
            TRAPFRAME,
            PGSIZE,
            self.trapframe as usize as u64,
            PTE_R | PTE_W,
        ) < 0
        {
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
        if uvmcopy(parent.pagetable, child.pagetable, parent.memory_allocated) < 0 {
            child.free();
            child.lock.unlock();
            return Err(ProcessError::Allocation);
        }
        child.memory_allocated = parent.memory_allocated;

        // Copy saved user registers.
        *child.trapframe = *parent.trapframe;

        // Cause fork to return 0 in the child.
        (*child.trapframe).a0 = 0;

        // Increment reference counts on open file descriptors.
        for (i, file) in parent.open_files.iter().enumerate() {
            if !file.is_null() {
                child.open_files[i] = filedup(parent.open_files[i]);
            }
        }
        child.current_dir = idup(parent.current_dir);

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

    /// Pass p's abandoned children to init.
    /// Caller must hold wait_lock.
    pub unsafe fn reparent(&self) {
        for p in proc.iter_mut() {
            if p.parent == addr_of!(*self).cast_mut() {
                p.parent = initproc;
                wakeup(initproc.cast());
            }
        }
    }

    /// Exit the current process. Does not return.
    /// An exited process remains in the zombie state
    /// until its parent calls wait().
    pub unsafe fn exit(&mut self, status: i32) -> ! {
        if addr_of_mut!(*self) == initproc {
            panic!("init exiting");
        }

        // Close all open files.
        for file in self.open_files.iter_mut() {
            if !file.is_null() {
                fileclose(*file);
                *file = null_mut();
            }
        }

        {
            let _operation = LogOperation::new();
            iput(self.current_dir);
        }
        self.current_dir = null_mut();

        {
            let _guard = wait_lock.lock();

            // Give any children to init.
            self.reparent();

            // Parent might be sleeping in wait().
            wakeup(self.parent.cast());

            self.lock.lock_unguarded();
            self.exit_status = status;
            self.state = ProcessState::Zombie;
        }

        // Jump into the scheduler, never to return.
        sched();
        unreachable!();
    }

    /// Wait for a child process to exit, and return its pid.
    pub unsafe fn wait_for_child(&mut self, addr: u64) -> Result<i32, ProcessError> {
        let guard = wait_lock.lock();

        loop {
            // Scan through the table looking for exited children.
            let mut has_children = false;

            for p in &mut proc {
                if p.parent == addr_of_mut!(*self) {
                    has_children = true;

                    // Ensure the child isn't still in exit() or swtch().
                    p.lock.lock_unguarded();

                    if p.state == ProcessState::Zombie {
                        // Found an exited child.
                        let pid = p.pid;

                        if addr != 0
                            && copyout(
                                self.pagetable,
                                addr,
                                addr_of_mut!(p.exit_status).cast(),
                                core::mem::size_of::<i32>() as u64,
                            ) < 0
                        {
                            p.lock.unlock();
                            return Err(ProcessError::PageError);
                        }

                        p.free();
                        p.lock.unlock();
                        return Ok(pid);
                    }

                    p.lock.unlock();
                }
            }

            if !has_children {
                return Err(ProcessError::NoChildren);
            } else if self.is_killed() {
                return Err(ProcessError::Killed);
            }

            // Wait for child to exit.
            // DOC: wait-sleep
            guard.sleep(addr_of_mut!(*self).cast());
        }
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
pub unsafe extern "C" fn allocproc() -> *mut Process {
    if let Ok(process) = Process::alloc() {
        process as *mut Process
    } else {
        null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn proc_pagetable(p: *mut Process) -> Pagetable {
    (*p).alloc_pagetable().unwrap_or(null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn proc_freepagetable(pagetable: Pagetable, size: u64) {
    Process::free_pagetable(pagetable, size as usize)
}

/// Print a process listing to console for debugging.
/// Runs when a user types ^P on console.
/// No lock to avoid wedging a stuck machine further.
pub unsafe fn procdump() {
    uprintln!("\nprocdump:");
    for p in &proc {
        if p.state != ProcessState::Unused {
            uprintln!("    {}: {:?}", p.pid, p.state);
        }
    }
}
