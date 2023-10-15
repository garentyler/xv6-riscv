use core::arch::asm;

pub type Pte = u64;
pub type Pagetable = *mut [Pte; 512];

/// Previous mode
pub const MSTATUS_MPP_MASK: u64 = 3 << 11;
pub const MSTATUS_MPP_M: u64 = 3 << 11;
pub const MSTATUS_MPP_S: u64 = 1 << 11;
pub const MSTATUS_MPP_U: u64 = 0 << 11;
/// Machine-mode interrupt enable.
pub const MSTATUS_MIE: u64 = 1 << 3;

/// Previous mode: 1 = Supervisor, 0 = User
pub const SSTATUS_SPP: u64 = 1 << 8;
/// Supervisor Previous Interrupt Enable
pub const SSTATUS_SPIE: u64 = 1 << 5;
/// User Previous Interrupt Enable
pub const SSTATUS_UPIE: u64 = 1 << 4;
/// Supervisor Interrupt Enable
pub const SSTATUS_SIE: u64 = 1 << 1;
/// User Interrupt Enable
pub const SSTATUS_UIE: u64 = 1 << 0;

/// Supervisor External Interrupt Enable
pub const SIE_SEIE: u64 = 1 << 9;
/// Supervisor Timer Interrupt Enable
pub const SIE_STIE: u64 = 1 << 5;
/// Supervisor Software Interrupt Enable
pub const SIE_SSIE: u64 = 1 << 1;

/// Machine-mode External Interrupt Enable
pub const MIE_MEIE: u64 = 1 << 11;
/// Machine-mode Timer Interrupt Enable
pub const MIE_MTIE: u64 = 1 << 7;
/// Machine-mode Software Interrupt Enable
pub const MIE_MSIE: u64 = 1 << 3;

pub const SATP_SV39: u64 = 8 << 60;

/// Bytes per page
pub const PGSIZE: u64 = 4096;
/// Bits of offset within a page
pub const PGSHIFT: u64 = 12;

pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;
pub const PTE_U: u64 = 1 << 4;

/// Which hart (core) is this?
#[inline(always)]
pub unsafe fn r_mhartid() -> u64 {
    let x: u64;
    asm!("csrr {}, mhartid", out(reg) x);
    x
}

#[inline(always)]
pub unsafe fn r_tp() -> u64 {
    let x: u64;
    asm!("mv {}, tp", out(reg) x);
    x
}

#[inline(always)]
pub unsafe fn w_sstatus(x: u64) {
    asm!("csrw sstatus, {}", in(reg) x);
}

#[inline(always)]
pub unsafe fn r_sstatus() -> u64 {
    let x: u64;
    asm!("csrr {}, sstatus", out(reg) x);
    x
}

#[inline(always)]
pub unsafe fn intr_on() {
    w_sstatus(r_sstatus() | SSTATUS_SIE);
}

#[inline(always)]
pub unsafe fn intr_off() {
    w_sstatus(r_sstatus() & !SSTATUS_SIE);
}

#[inline(always)]
pub unsafe fn intr_get() -> i32 {
    if (r_sstatus() & SSTATUS_SIE) > 0 {
        1
    } else {
        0
    }
}

extern "C" {
    /// Which hart (core) is this?
    pub fn rv_r_mhartid() -> u64;

    // Machine Status Register, mstatus
    pub fn r_mstatus() -> u64;
    pub fn w_mstatus(x: u64);

    // Machine Exception Program Counter
    // MEPC holds the instruction address to which a return from exception will go.
    pub fn w_mepc(x: u64);

    // Supervisor Status Register, sstatus
    pub fn rv_r_sstatus() -> u64;
    pub fn rv_w_sstatus(x: u64);

    // Supervisor Interrupt Pending
    pub fn r_sip() -> u64;
    pub fn w_sip(x: u64);

    // Supervisor Interrupt Enable
    pub fn r_sie() -> u64;
    pub fn w_sie(x: u64);

    // Machine-mode Interrupt Enable
    pub fn r_mie() -> u64;
    pub fn w_mie(x: u64);

    // Supervisor Exception Program Counter
    // SEPC holds the instruction address to which a return from exception will go.
    pub fn r_sepc() -> u64;
    pub fn w_sepc(x: u64);

    // Machine Exception Deletgation
    pub fn r_medeleg() -> u64;
    pub fn w_medeleg(x: u64);

    // Machine Interrupt Deletgation
    pub fn r_mideleg() -> u64;
    pub fn w_mideleg(x: u64);

    // Supervisor Trap-Vector Base Address
    pub fn r_stvec() -> u64;
    pub fn w_stvec(x: u64);

    // Machine-mode Interrupt Vector
    pub fn w_mtvec(x: u64);

    // Physical Memory Protection
    pub fn w_pmpcfg0(x: u64);
    pub fn w_pmpaddr0(x: u64);

    // Supervisor Address Translation and Protection
    // SATP holds the address of the page table.
    pub fn r_satp() -> u64;
    pub fn w_satp(x: u64);

    pub fn w_mscratch(x: u64);

    // Supervisor Trap Cause
    pub fn r_scause() -> u64;

    // Supervisor Trap Value
    pub fn r_stval() -> u64;

    // Machine-mode Counter-Enable
    pub fn r_mcounteren() -> u64;
    pub fn w_mcounteren(x: u64);

    // Machine-mode cycle counter
    pub fn r_time() -> u64;

    // /// Enable device interrupts
    // pub fn intr_on();

    // /// Disable device interrupts
    // pub fn intr_off();

    // // Are device interrupts enabled?
    // pub fn intr_get() -> i32;

    pub fn r_sp() -> u64;

    // Read and write TP (thread pointer), which xv6 uses
    // to hold this core's hartid, the index into cpus[].
    // pub fn rv_r_tp() -> u64;
    pub fn w_tp(x: u64);

    pub fn r_ra() -> u64;

    /// Flush the TLB.
    pub fn sfence_vma();
}
