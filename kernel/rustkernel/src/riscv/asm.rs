use super::*;
use core::arch::asm;

/// Which hart (core) is this?
#[inline(always)]
pub unsafe fn r_mhartid() -> u64 {
    let x: u64;
    asm!("csrr {}, mhartid", out(reg) x);
    x
}

// Machine Status Register, mstatus
#[inline(always)]
pub unsafe fn r_mstatus() -> u64 {
    let x: u64;
    asm!("csrr {}, mstatus", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_mstatus(x: u64) {
    asm!("csrw mstatus, {}", in(reg) x);
}

// Machine Exception Program Counter
// MEPC holds the instruction address to which a return from exception will go.
#[inline(always)]
pub unsafe fn w_mepc(x: u64) {
    asm!("csrw mepc, {}", in(reg) x);
}

// Supervisor Status Register, sstatus
#[inline(always)]
pub unsafe fn r_sstatus() -> u64 {
    let x: u64;
    asm!("csrr {}, sstatus", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_sstatus(x: u64) {
    asm!("csrw sstatus, {}", in(reg) x);
}

// Supervisor Interrupt Pending
#[inline(always)]
pub unsafe fn r_sip() -> u64 {
    let x: u64;
    asm!("csrr {}, sip", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_sip(x: u64) {
    asm!("csrw sip, {}", in(reg) x);
}

// Supervisor Interrupt Enable
#[inline(always)]
pub unsafe fn r_sie() -> u64 {
    let x: u64;
    asm!("csrr {}, sie", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_sie(x: u64) {
    asm!("csrw sie, {}", in(reg) x);
}

// Machine-mode Interrupt Enable
#[inline(always)]
pub unsafe fn r_mie() -> u64 {
    let x: u64;
    asm!("csrr {}, mie", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_mie(x: u64) {
    asm!("csrw mie, {}", in(reg) x);
}

// Supervisor Exception Program Counter
// SEPC holds the instruction address to which a return from exception will go.
#[inline(always)]
pub unsafe fn r_sepc() -> u64 {
    let x: u64;
    asm!("csrr {}, sepc", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_sepc(x: u64) {
    asm!("csrw sepc, {}", in(reg) x);
}

// Machine Exception Delegation
#[inline(always)]
pub unsafe fn r_medeleg() -> u64 {
    let x: u64;
    asm!("csrr {}, medeleg", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_medeleg(x: u64) {
    asm!("csrw medeleg, {}", in(reg) x);
}

// Machine Interrupt Delegation
#[inline(always)]
pub unsafe fn r_mideleg() -> u64 {
    let x: u64;
    asm!("csrr {}, mideleg", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_mideleg(x: u64) {
    asm!("csrw mideleg, {}", in(reg) x);
}

// Supervisor Trap-Vector Base Address
#[inline(always)]
pub unsafe fn r_stvec() -> u64 {
    let x: u64;
    asm!("csrr {}, stvec", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_stvec(x: u64) {
    asm!("csrw stvec, {}", in(reg) x);
}

// Machine-mode Interrupt Vector
#[inline(always)]
pub unsafe fn w_mtvec(x: u64) {
    asm!("csrw mtvec, {}", in(reg) x);
}

// Physical Memory Protection
#[inline(always)]
pub unsafe fn w_pmpcfg0(x: u64) {
    asm!("csrw pmpcfg0, {}", in(reg) x);
}
#[inline(always)]
pub unsafe fn w_pmpaddr0(x: u64) {
    asm!("csrw pmpaddr0, {}", in(reg) x);
}

// Supervisor Address Translation and Protection
// SATP holds the address of the page table.
#[inline(always)]
pub unsafe fn r_satp() -> u64 {
    let x: u64;
    asm!("csrr {}, satp", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_satp(x: u64) {
    asm!("csrw satp, {}", in(reg) x);
}

#[inline(always)]
pub unsafe fn w_mscratch(x: u64) {
    asm!("csrw mscratch, {}", in(reg) x);
}


// Supervisor Trap Cause
#[inline(always)]
pub unsafe fn r_scause() -> u64 {
    let x: u64;
    asm!("csrr {}, scause", out(reg) x);
    x
}

// Supervisor Trap Value
#[inline(always)]
pub unsafe fn r_stval() -> u64 {
    let x: u64;
    asm!("csrr {}, stval", out(reg) x);
    x
}

// Machine-mode Counter-Enable
#[inline(always)]
pub unsafe fn r_mcounteren() -> u64 {
    let x: u64;
    asm!("csrr {}, mcounteren", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_mcounteren(x: u64) {
    asm!("csrw mcounteren, {}", in(reg) x);
}

// Machine-mode cycle counter
#[inline(always)]
pub unsafe fn r_time() -> u64 {
    let x: u64;
    asm!("csrr {}, time", out(reg) x);
    x
}

// Enable device interrupts
#[inline(always)]
pub unsafe fn intr_on() {
    w_sstatus(r_sstatus() | SSTATUS_SIE);
}

// Disable device interrupts
#[inline(always)]
pub unsafe fn intr_off() {
    w_sstatus(r_sstatus() & !SSTATUS_SIE);
}

// Are device interrupts enabled?
#[inline(always)]
pub unsafe fn intr_get() -> i32 {
    if (r_sstatus() & SSTATUS_SIE) > 0 {
        1
    } else {
        0
    }
}

#[inline(always)]
pub unsafe fn r_sp() -> u64 {
    let x: u64;
    asm!("mv {}, sp", out(reg) x);
    x
}

// Read and write TP (thread pointer), which xv6 uses
// to hold this core's hartid, the index into cpus[].
// pub fn rv_r_tp() -> u64;
#[inline(always)]
pub unsafe fn r_tp() -> u64 {
    let x: u64;
    asm!("mv {}, tp", out(reg) x);
    x
}
#[inline(always)]
pub unsafe fn w_tp(x: u64) {
    asm!("mv tp, {}", in(reg) x);
}

#[inline(always)]
pub unsafe fn r_ra() -> u64 {
    let x: u64;
    asm!("mv {}, ra", out(reg) x);
    x
}

// Flush the TLB.
#[inline(always)]
pub unsafe fn sfence_vma() {
    // The "zero, zero" means flush all TLB entries.
    asm!("sfence.vma zero, zero");
}
