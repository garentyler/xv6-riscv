use crate::{
    riscv::{self, Pagetable},
    spinlock::Spinlock,
};
use core::ffi::c_char;

/// Saved registers for kernel context switches.
#[repr(C)]
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
pub enum ProcState {
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
    pub chan: *mut u8,
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

extern "C" {
    // pub fn cpuid() -> i32;
    /// Return this CPU's cpu struct.
    /// Interrupts must be disabled.
    pub fn mycpu() -> *mut Cpu;
}
