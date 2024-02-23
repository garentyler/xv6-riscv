use crate::{
    hal::arch::riscv::{asm, MIE_MTIE, MSTATUS_MIE},
    NCPU,
};
use core::ptr::addr_of;

// Core Local Interrupter (CLINT), which contains the timer.
// I'm pretty sure the CLINT address is standardized to this location.
pub const CLINT: usize = 0x2000000;
const CLINT_MTIME: usize = CLINT + 0xbff8;

extern "C" {
    pub fn timervec();
}

#[no_mangle]
pub static mut timer_scratch: [[u64; 5]; NCPU] = [[0u64; 5]; NCPU];

fn clint_mtimecmp(hartid: usize) -> *mut u64 {
    (CLINT + 0x4000 + (8 * hartid)) as *mut u64
}

/// Arrange to receive timer interrupts.
///
/// They will arrive in machine mode at
/// at timervec in kernelvec.S,
/// which turns them into software interrupts for
/// devintr() in trap.c.
pub unsafe fn timerinit() {
    // Each CPU has a separate source of timer interrupts.
    let id = asm::r_mhartid() as usize;

    // Ask the CLINT for a timer interrupt.
    // cycles, about 1/10th second in qemu
    let interval = 1_000_000u64;
    *clint_mtimecmp(id) = *(CLINT_MTIME as *const u64) + interval;

    // Prepare information in scratch[] for timervec.
    // scratch[0..=2]: Space for timervec to save registers.
    // scratch[3]: Address of CLINT MTIMECMP register.
    // scratch[4]: Desired interval (in cycles) between timer interrupts.
    let scratch: &mut [u64; 5] = &mut timer_scratch[id];
    scratch[3] = clint_mtimecmp(id) as usize as u64;
    scratch[4] = interval;
    asm::w_mscratch(addr_of!(scratch[0]) as usize as u64);

    // Set the machine-mode trap handler.
    asm::w_mtvec(timervec as usize as u64);

    // Enable machine-mode interrupts.
    asm::w_mstatus(asm::r_mstatus() | MSTATUS_MIE);

    // Enable machine-mode timer interrupts.
    asm::w_mie(asm::r_mie() | MIE_MTIE);
}
