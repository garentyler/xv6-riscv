use crate::{main, arch::riscv::*, NCPU};
use core::{arch::asm, ptr::addr_of};

extern "C" {
    pub fn timervec();
}

#[no_mangle]
pub static mut timer_scratch: [[u64; 5]; NCPU] = [[0u64; 5]; NCPU];

// The original C has this aligned to 16 - hopefully that's not a problem.
#[no_mangle]
pub static mut stack0: [u8; 4096 * NCPU] = [0u8; 4096 * NCPU];

// entry.S jumps here in machine mode on stack0
#[no_mangle]
pub unsafe extern "C" fn start() {
    // Set M Previous Privilege mode to Supervisor, for mret.
    let mut x = r_mstatus();
    x &= !MSTATUS_MPP_MASK;
    x |= MSTATUS_MPP_S;
    w_mstatus(x);

    // Set M Exception Program Counter to main, for mret.
    w_mepc(main as usize as u64);

    // Disable paging for now.
    w_satp(0);

    // Delegate all interrupts and exceptions to supervisor mode.
    w_medeleg(0xffffu64);
    w_mideleg(0xffffu64);
    w_sie(r_sie() | SIE_SEIE | SIE_STIE | SIE_SSIE);

    // Configure Physical Memory Protection to give
    // supervisor mode access to all of physical memory.
    w_pmpaddr0(0x3fffffffffffffu64);
    w_pmpcfg0(0xf);

    // Ask for clock interrupts.
    timerinit();

    // Keep each CPU's hartid in its tp register, for cpuid().
    w_tp(r_mhartid());

    // Switch to supervisor mode and jump to main().
    asm!("mret");
}

/// Arrange to receive timer interrupts.
///
/// They will arrive in machine mode at
/// at timervec in kernelvec.S,
/// which turns them into software interrupts for
/// devintr() in trap.c.
#[no_mangle]
pub unsafe extern "C" fn timerinit() {
    // Each CPU has a separate source of timer interrupts.
    let id = r_mhartid();

    // Ask the CLINT for a timer interrupt.
    // cycles, about 1/10th second in qemu
    let interval = 1_000_000u64;
    *(clint_mtimecmp(id) as *mut u64) = *(CLINT_MTIME as *const u64) + interval;

    // Prepare information in scratch[] for timervec.
    // scratch[0..=2]: Space for timervec to save registers.
    // scratch[3]: Address of CLINT MTIMECMP register.
    // scratch[4]: Desired interval (in cycles) between timer interrupts.
    let scratch: &mut [u64; 5] = &mut timer_scratch[id as usize];
    scratch[3] = clint_mtimecmp(id);
    scratch[4] = interval;
    w_mscratch(addr_of!(scratch[0]) as usize as u64);

    // Set the machine-mode trap handler.
    w_mtvec(timervec as usize as u64);

    // Enable machine-mode interrupts.
    w_mstatus(r_mstatus() | MSTATUS_MIE);

    // Enable machine-mode timer interrupts.
    w_mie(r_mie() | MIE_MTIE);
}
