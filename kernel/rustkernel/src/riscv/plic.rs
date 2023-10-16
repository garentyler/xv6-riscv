//! The RISC-V Platform Level Interrupt Controller (PLIC)

use crate::{
    proc::cpuid,
    riscv::{plic_sclaim, plic_senable, plic_spriority, PLIC, UART0_IRQ, VIRTIO0_IRQ},
};

#[no_mangle]
pub unsafe extern "C" fn plicinit() {
    // Set desired IRQ priorities non-zero (otherwise disabled).
    *((PLIC + UART0_IRQ as u64 * 4) as *mut u32) = 1;
    *((PLIC + VIRTIO0_IRQ as u64 * 4) as *mut u32) = 1;
}

#[no_mangle]
pub unsafe extern "C" fn plicinithart() {
    let hart = cpuid() as u64;

    // Set enable bits for this hart's S-mode
    // for the UART and VIRTIO disk.
    *(plic_senable(hart) as *mut u32) = (1 << UART0_IRQ) | (1 << VIRTIO0_IRQ);

    // Set this hart's S-mode priority threshold to 0.
    *(plic_spriority(hart) as *mut u32) = 0;
}

/// Ask the PLIC what interrupt we should serve.
#[no_mangle]
pub unsafe extern "C" fn plic_claim() -> i32 {
    let hart = cpuid() as u64;
    *(plic_sclaim(hart) as *const i32)
}

/// Tell the PLIC we've served this IRQ.
#[no_mangle]
pub unsafe extern "C" fn plic_complete(irq: i32) {
    let hart = cpuid() as u64;
    *(plic_sclaim(hart) as *mut i32) = irq;
}
