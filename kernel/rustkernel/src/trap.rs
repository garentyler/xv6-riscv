use crate::{
    printf::print,
    proc::{cpuid, wakeup, mycpu},
    riscv::*,
    sync::spinlock::Spinlock,
};
use core::{ffi::CStr, ptr::addr_of_mut};

extern "C" {
    pub fn kernelvec();
    pub fn usertrap();
    pub fn usertrapret();
    fn syscall();
    fn virtio_disk_intr();
}

#[no_mangle]
pub static mut tickslock: Spinlock = unsafe { Spinlock::uninitialized() };
#[no_mangle]
pub static mut ticks: u32 = 0;

#[no_mangle]
pub unsafe extern "C" fn trapinit() {
    tickslock = Spinlock::new(
        CStr::from_bytes_with_nul(b"time\0")
            .unwrap()
            .as_ptr()
            .cast_mut(),
    );
}

/// Set up to take exceptions and traps while in the kernel.
#[no_mangle]
pub unsafe extern "C" fn trapinithart() {
    w_stvec(kernelvec as usize as u64);
}

#[no_mangle]
pub unsafe extern "C" fn clockintr() {
    tickslock.lock();
    ticks += 1;
    wakeup(addr_of_mut!(ticks).cast());
    tickslock.unlock();
}

// /// Handle an interrupt, exception, or syscall from user space.
// /// Called from trampoline.S.
// #[no_mangle]
// pub unsafe extern "C" fn usertrap() {
//     if r_sstatus() & SSTATUS_SPP != 0 {
//         panic!("usertrap: not from user mode");
//     }
//
//     // Send interrupts and exceptions to kerneltrap(),
//     // since we're now in the kernel.
//     w_stvec(kernelvec as usize as u64);
//
//     let p = myproc();
//
//     // Save user program counter.
//     (*(*p).trapframe).epc = r_sepc();
//
//     if r_scause() == 8 {
//         // Syscall
//
//         if killed(p) > 0 {
//             exit(-1);
//         }
//
//         // sepc points to the ecall instruction,
//         // but we want to return to the next instruction.
//         (*(*p).trapframe).epc += 4;
//
//         // An interrupt will change sepc, scause, and sstatus,
//         // so enable only now that we're done with those registers.
//         intr_on();
//
//         syscall();
//     } else {
//         let which_dev = devintr();
//         if which_dev == 0 {
//             print!("usertrap(): unexpected scause {:#018x} pid={}\n", r_scause(), (*p).pid);
//             print!("            sepc={:#018x} stval={:#018x}\n", r_sepc(), r_stval());
//             setkilled(p);
//         }
//         if killed(p) > 0 {
//             exit(-1);
//         }
//
//         // Give up the CPU if this is a timer interrupt.
//         if which_dev == 2 {
//             r#yield();
//         }
//
//         usertrapret();
//     }
// }

/// Check if it's an external interrupt or software interrupt and handle it.
///
/// Returns 2 if timer interrupt, 1 if other device, 0 if not recognized.
#[no_mangle]
pub unsafe extern "C" fn devintr() -> i32 {
    let scause = r_scause();

    if (scause & 0x8000000000000000 > 0) && (scause & 0xff) == 9 {
        // This is a supervisor external interrupt, via PLIC.

        // IRQ indicates which device interrupted.
        let irq = plic::plic_claim();

        if irq == UART0_IRQ {
            crate::uart::uartintr();
        } else if irq == VIRTIO0_IRQ {
            virtio_disk_intr();
        } else if irq > 0 {
            print!("unexpected interrupt irq={}\n", irq);
        }

        // The PLIC allows each device to raise at most one
        // interrupt at a time; tell the PLIC the device is
        // now allowed to interrupt again.
        if irq > 0 {
            plic::plic_complete(irq);
        }

        1
    } else if scause == 0x8000000000000001 {
        // Software interrupt from a machine-mode timer interrupt,
        // forwarded by timervec in kernelvec.S.

        if cpuid() == 0 {
            clockintr();
        }

        // Acknowledge the software interrupt by
        // clearing the SSIP bit in sip.
        w_sip(r_sip() & !2);

        2
    } else {
        0
    }
}

pub struct InterruptBlocker;
impl InterruptBlocker {
    pub fn new() {
        unsafe {
            let interrupts_before = intr_get();
            let cpu = mycpu();
            
            intr_off();

            if (*cpu).interrupt_disable_layers == 0 {
                (*cpu).previous_interrupts_enabled = interrupts_before;
            }
            (*cpu).interrupt_disable_layers += 1;
            // crate::sync::spinlock::push_off();
        }
    }
}
impl core::ops::Drop for InterruptBlocker {
    fn drop(&mut self) {
        unsafe {
            let cpu = mycpu();

            if intr_get() == 1 || (*cpu).interrupt_disable_layers < 1 {
                // panic!("pop_off mismatched");
                return;
            }

            (*cpu).interrupt_disable_layers -= 1;

            if (*cpu).interrupt_disable_layers == 0 && (*cpu).previous_interrupts_enabled == 1 {
                intr_on();
            }
            // crate::sync::spinlock::pop_off();
        }
    }
}
impl !Send for InterruptBlocker {}
