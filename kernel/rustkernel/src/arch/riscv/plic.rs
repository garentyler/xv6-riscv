//! The RISC-V Platform Level Interrupt Controller (PLIC)

use super::hardware::VIRTIO0_IRQ;
use crate::proc::cpu::Cpu;

// QEMU puts platform-level interrupt controller (PLIC) here.
pub const PLIC: usize = 0x0c000000;
const PLIC_PRIORITY: usize = PLIC;
const PLIC_PENDING: usize = PLIC + 0x1000;
const VIRTIO0_IRQ_ADDR: usize = PLIC + VIRTIO0_IRQ * 4;

/// Get a pointer to the CPU-specific machine-mode enable register.
fn plic_menable(hartid: usize) -> *mut u32 {
    (PLIC + 0x2000 + (0x100 * hartid)) as *mut u32
}
/// Get a pointer to the CPU-specific supervisor-mode enable register.
fn plic_senable(hartid: usize) -> *mut u32 {
    (PLIC + 0x2080 + (0x100 * hartid)) as *mut u32
}
/// Get a pointer to the CPU-specific machine-mode priority register.
fn plic_mpriority(hartid: usize) -> *mut u32 {
    (PLIC + 0x200000 + (0x2000 * hartid)) as *mut u32
}
/// Get a pointer to the CPU-specific supervisor-mode priority register.
fn plic_spriority(hartid: usize) -> *mut u32 {
    (PLIC + 0x201000 + (0x2000 * hartid)) as *mut u32
}
/// Get a pointer to the CPU-specific machine-mode claim register.
fn plic_mclaim(hartid: usize) -> *mut u32 {
    (PLIC + 0x200004 + (0x2000 * hartid)) as *mut u32
}
/// Get a pointer to the CPU-specific supervisor-mode claim register.
fn plic_sclaim(hartid: usize) -> *mut u32 {
    (PLIC + 0x201004 + (0x2000 * hartid)) as *mut u32
}

pub unsafe fn plicinit() {
    // Set desired IRQ priorities non-zero (otherwise disabled).
    for (uart_irq, _) in &crate::hardware::UARTS {
        *((PLIC + uart_irq * 4) as *mut u32) = 1;
    }
    *(VIRTIO0_IRQ_ADDR as *mut u32) = 1;
}

pub unsafe fn plicinithart() {
    let hart = Cpu::current_id();

    // Set enable bits for this hart's S-mode
    // for the UART and VIRTIO disk.
    let mut enable_bits = 0;
    for (uart_irq, _) in &crate::hardware::UARTS {
        enable_bits |= 1 << uart_irq;
    }
    enable_bits |= 1 << VIRTIO0_IRQ;
    *plic_senable(hart) = enable_bits;

    // Set this hart's S-mode priority threshold to 0.
    *plic_spriority(hart) = 0;
}

/// Ask the PLIC what interrupt we should serve.
pub unsafe fn plic_claim() -> usize {
    let hart = Cpu::current_id();
    (*plic_sclaim(hart)) as usize
}

/// Tell the PLIC we've served this IRQ.
pub unsafe fn plic_complete(irq: usize) {
    let hart = Cpu::current_id();
    *plic_sclaim(hart) = irq as u32;
}
