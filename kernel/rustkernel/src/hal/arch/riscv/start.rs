use crate::{
    hal::{
        arch::riscv::{asm, MSTATUS_MPP_MASK, MSTATUS_MPP_S, SIE_SEIE, SIE_SSIE, SIE_STIE},
        hardware::riscv::clint,
    },
    main, NCPU,
};
use core::arch::asm;

#[no_mangle]
pub static mut stack0: [u8; 4096 * NCPU] = [0u8; 4096 * NCPU];

// entry.S jumps here in machine mode on stack0
#[no_mangle]
pub unsafe extern "C" fn start() {
    // Set M Previous Privilege mode to Supervisor, for mret.
    let mut x = asm::r_mstatus();
    x &= !MSTATUS_MPP_MASK;
    x |= MSTATUS_MPP_S;
    asm::w_mstatus(x);

    // Set M Exception Program Counter to main, for mret.
    asm::w_mepc(main as usize as u64);

    // Disable paging for now.
    asm::w_satp(0);

    // Delegate all interrupts and exceptions to supervisor mode.
    asm::w_medeleg(0xffffu64);
    asm::w_mideleg(0xffffu64);
    asm::w_sie(asm::r_sie() | SIE_SEIE | SIE_STIE | SIE_SSIE);

    // Configure Physical Memory Protection to give
    // supervisor mode access to all of physical memory.
    asm::w_pmpaddr0(0x3fffffffffffffu64);
    asm::w_pmpcfg0(0xf);

    // Ask for clock interrupts.
    clint::timerinit();

    // Keep each CPU's hartid in its tp register, for Cpu::current_id().
    asm::w_tp(asm::r_mhartid());

    // Switch to supervisor mode and jump to main().
    asm!("mret");
}
