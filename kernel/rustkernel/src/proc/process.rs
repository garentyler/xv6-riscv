#![allow(clippy::comparison_chain)]

use super::{
    context::Context,
    cpu::Cpu,
    scheduler::{sched, wakeup},
    trapframe::Trapframe,
};
use crate::{
    fs::{
        file::{fileclose, filedup, File},
        fsinit,
        inode::{idup, iput, namei, Inode},
        log::LogOperation,
        FS_INITIALIZED,
    },
    hal::arch::{
        mem::{kstack, Pagetable, PAGE_SIZE, PTE_R, PTE_W, PTE_X, TRAMPOLINE, TRAPFRAME},
        trap::{usertrapret, InterruptBlocker},
        virtual_memory::{
            copyout, mappages, uvmalloc, uvmcopy, uvmcreate, uvmdealloc, uvmfirst, uvmfree,
            uvmunmap,
        },
    },
    mem::{
        kalloc::{kalloc, kfree},
        memset,
    },
    sync::spinlock::Spinlock,
    uprintln,
};
use arrayvec::ArrayVec;
use core::{
    ffi::{c_char, c_void, CStr},
    ptr::{addr_of, addr_of_mut, null_mut},
    sync::atomic::{AtomicI32, Ordering},
};

extern "C" {
    // trampoline.S
    pub static mut trampoline: *mut c_char;
}

pub static NEXT_PID: AtomicI32 = AtomicI32::new(1);
/// Helps ensure that wakeups of wait()ing
/// parents are not lost. Helps obey the
/// memory model when using p->parent.
/// Must be acquired before any p->lock.
pub static mut WAIT_LOCK: Spinlock = Spinlock::new();
pub static mut INITPROC: usize = 0;
pub static mut PROCESSES: ArrayVec<Process, { crate::NPROC }> = ArrayVec::new_const();

/// Initialize the proc table.
pub unsafe fn procinit() {
    let mut i = 0;
    let processes_iter = core::iter::repeat_with(|| {
        let mut p = Process::new();
        p.state = ProcessState::Unused;
        p.kernel_stack = kstack(i) as u64;
        i += 1;
        p
    });
    PROCESSES = processes_iter.take(crate::NPROC).collect();
}
/// Set up the first user process.
pub unsafe fn userinit() {
    let p = Process::alloc().unwrap();
    INITPROC = addr_of_mut!(*p) as usize;

    let initcode: &[u8] = &[
        0x17, 0x05, 0x00, 0x00, 0x13, 0x05, 0x45, 0x02, 0x97, 0x05, 0x00, 0x00, 0x93, 0x85, 0x35,
        0x02, 0x93, 0x08, 0x70, 0x00, 0x73, 0x00, 0x00, 0x00, 0x93, 0x08, 0x20, 0x00, 0x73, 0x00,
        0x00, 0x00, 0xef, 0xf0, 0x9f, 0xff, 0x2f, 0x69, 0x6e, 0x69, 0x74, 0x00, 0x00, 0x24, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    // Allocate one user page and copy initcode's
    // instructions and data into it.
    uvmfirst(p.pagetable, initcode.as_ptr().cast_mut(), initcode.len());
    p.memory_allocated = PAGE_SIZE as u64;

    // Prepare for the very first "return" from kernel to user.
    // User program counter
    (*p.trapframe).epc = 0;
    // User stack pointer
    (*p.trapframe).sp = PAGE_SIZE as u64;

    p.current_dir = namei(
        CStr::from_bytes_with_nul(b"/\0")
            .unwrap()
            .as_ptr()
            .cast_mut()
            .cast(),
    );
    p.state = ProcessState::Runnable;
    p.lock.unlock();
}

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
#[derive(Clone)]
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

    // WAIT_LOCK must be held when using this:
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
        let _ = InterruptBlocker::new();
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
    pub fn is_initproc(&self) -> bool {
        addr_of!(*self) as usize == unsafe { INITPROC }
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
        for (i, p) in PROCESSES.iter_mut().enumerate() {
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

        let p: &mut Process = &mut PROCESSES[index];
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
            core::mem::size_of::<Context>(),
        );
        p.context.ra = Process::forkret as usize as u64;
        p.context.sp = p.kernel_stack + PAGE_SIZE as u64;

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
                size as usize,
                size.wrapping_add(num_bytes as u64) as usize,
                PTE_W,
            );

            if size == 0 {
                return Err(ProcessError::Allocation);
            }
        } else if num_bytes < 0 {
            size = uvmdealloc(
                self.pagetable,
                size as usize,
                size.wrapping_add(num_bytes as u64) as usize,
            );
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
            PAGE_SIZE,
            addr_of!(trampoline) as usize,
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
            PAGE_SIZE,
            self.trapframe as usize,
            PTE_R | PTE_W,
        ) < 0
        {
            uvmunmap(pagetable, TRAMPOLINE, 1, false);
            uvmfree(pagetable, 0);
            return Err(ProcessError::Allocation);
        }

        Ok(pagetable)
    }
    /// Free a process's pagetable and free the physical memory it refers to.
    pub unsafe fn free_pagetable(pagetable: Pagetable, size: usize) {
        uvmunmap(pagetable, TRAMPOLINE, 1, false);
        uvmunmap(pagetable, TRAPFRAME, 1, false);
        uvmfree(pagetable, size)
    }

    /// Create a new process, copying the parent.
    /// Sets up child kernel stack to return as if from fork() syscall.
    pub unsafe fn fork() -> Result<i32, ProcessError> {
        let parent = Process::current().unwrap();
        let child = Process::alloc()?;

        // Copy user memory from parent to child.
        if uvmcopy(
            parent.pagetable,
            child.pagetable,
            parent.memory_allocated as usize,
        ) < 0
        {
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
            let _guard = WAIT_LOCK.lock();
            child.parent = addr_of!(*parent).cast_mut();
        }
        {
            let _guard = child.lock.lock();
            child.state = ProcessState::Runnable;
        }

        Ok(pid)
    }

    /// A fork child's very first scheduling by
    /// scheduler() will swtch to forkret.
    pub unsafe fn forkret() -> ! {
        // Still holding p->lock from scheduler.
        Process::current().unwrap().lock.unlock();

        if !FS_INITIALIZED {
            // File system initialization must be run in the context of a
            // regular process (e.g., because it calls sleep), and thus
            // cannot be run from main().
            FS_INITIALIZED = true;
            fsinit(crate::ROOTDEV as i32);
        }

        usertrapret()
    }

    /// Pass p's abandoned children to init.
    /// Caller must hold WAIT_LOCK.
    pub unsafe fn reparent(&self) {
        for p in PROCESSES.iter_mut() {
            if p.parent == addr_of!(*self).cast_mut() {
                p.parent = INITPROC as *mut Process;
                wakeup((INITPROC as *mut Process).cast());
            }
        }
    }

    /// Exit the current process. Does not return.
    /// An exited process remains in the zombie state
    /// until its parent calls wait().
    pub unsafe fn exit(&mut self, status: i32) -> ! {
        if self.is_initproc() {
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
            let _guard = WAIT_LOCK.lock();

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
        let guard = WAIT_LOCK.lock();

        loop {
            // Scan through the table looking for exited children.
            let mut has_children = false;

            for p in PROCESSES.iter_mut() {
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
                                addr as usize,
                                addr_of_mut!(p.exit_status).cast(),
                                core::mem::size_of::<i32>(),
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
        for p in PROCESSES.iter_mut() {
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
    for p in PROCESSES.iter() {
        if p.state != ProcessState::Unused {
            uprintln!("    {}: {:?}", p.pid, p.state);
        }
    }
}
