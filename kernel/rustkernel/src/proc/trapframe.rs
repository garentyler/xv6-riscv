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
pub struct Trapframe {
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
impl Trapframe {
    pub const fn new() -> Trapframe {
        Trapframe {
            kernel_satp: 0u64,
            kernel_sp: 0u64,
            kernel_trap: 0u64,
            epc: 0u64,
            kernel_hartid: 0u64,
            ra: 0u64,
            sp: 0u64,
            gp: 0u64,
            tp: 0u64,
            t0: 0u64,
            t1: 0u64,
            t2: 0u64,
            s0: 0u64,
            s1: 0u64,
            a0: 0u64,
            a1: 0u64,
            a2: 0u64,
            a3: 0u64,
            a4: 0u64,
            a5: 0u64,
            a6: 0u64,
            a7: 0u64,
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
            t3: 0u64,
            t4: 0u64,
            t5: 0u64,
            t6: 0u64,
        }
    }
}
