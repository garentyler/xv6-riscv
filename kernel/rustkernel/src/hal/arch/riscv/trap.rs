use super::{asm, mem::make_satp, SSTATUS_SPIE, SSTATUS_SPP};
use crate::{
    hal::{
        arch::{
            interrupt,
            mem::{PAGE_SIZE, TRAMPOLINE},
        },
        platform::VIRTIO0_IRQ,
    },
    println,
    proc::{
        cpu::Cpu,
        process::{Process, ProcessState},
        scheduler::{r#yield, wakeup},
    },
    sync::mutex::Mutex,
    syscall::syscall,
};
use core::ptr::addr_of;

extern "C" {
    pub fn kernelvec();
    // pub fn usertrap();
    // pub fn usertrapret();
    // fn syscall();
    // pub fn userret(satp: u64);
    fn virtio_disk_intr();
    pub static mut trampoline: [u8; 0];
    pub static mut uservec: [u8; 0];
    pub static mut userret: [u8; 0];
}

pub static CLOCK_TICKS: Mutex<usize> = Mutex::new(0);

/// Set up to take exceptions and traps while in the kernel.
pub unsafe fn trapinithart() {
    asm::w_stvec(kernelvec as usize as u64);
}

pub fn clockintr() {
    let mut ticks = CLOCK_TICKS.lock_spinning();

    *ticks += 1;
    unsafe {
        wakeup(addr_of!(CLOCK_TICKS).cast_mut().cast());
    }
}

/// Check if it's an external interrupt or software interrupt and handle it.
///
/// Returns 2 if timer interrupt, 1 if other device, 0 if not recognized.
pub unsafe fn devintr() -> i32 {
    let scause = asm::r_scause();

    if (scause & 0x8000000000000000 > 0) && (scause & 0xff) == 9 {
        // This is a supervisor external interrupt, via PLIC.

        // IRQ indicates which device interrupted.
        let irq = interrupt::handle_interrupt();

        let mut uart_interrupt = false;
        for (uart_irq, uart) in &crate::hal::platform::UARTS {
            if irq == *uart_irq {
                uart_interrupt = true;
                uart.interrupt();
            }
        }

        if !uart_interrupt {
            if irq == VIRTIO0_IRQ {
                virtio_disk_intr();
            } else if irq > 0 {
                println!("unexpected interrupt irq={}", irq);
            }
        }

        // The PLIC allows each device to raise at most one
        // interrupt at a time; tell the PLIC the device is
        // now allowed to interrupt again.
        if irq > 0 {
            interrupt::complete_interrupt(irq);
        }

        1
    } else if scause == 0x8000000000000001 {
        // Software interrupt from a machine-mode timer interrupt,
        // forwarded by timervec in kernelvec.S.

        if Cpu::current_id() == 0 {
            clockintr();
        }

        // Acknowledge the software interrupt by
        // clearing the SSIP bit in sip.
        asm::w_sip(asm::r_sip() & !2);

        2
    } else {
        0
    }
}

/// Return to user space
#[no_mangle]
pub unsafe extern "C" fn usertrapret() -> ! {
    let proc = Process::current().unwrap();

    // We're about to switch the destination of traps from
    // kerneltrap() to usertrap(), so turn off interrupts until
    // we're back in user space, where usertrap() is correct.
    interrupt::disable_interrupts();

    // Send syscalls, interrupts, and exceptions to uservec in trampoline.S
    let trampoline_uservec =
        TRAMPOLINE + (addr_of!(uservec) as usize) - (addr_of!(trampoline) as usize);
    asm::w_stvec(trampoline_uservec as u64);

    // Set up trapframe values that uservec will need when
    // the process next traps into the kernel.
    // kernel page table
    (*proc.trapframe).kernel_satp = asm::r_satp();
    // process's kernel stack
    (*proc.trapframe).kernel_sp = proc.kernel_stack + PAGE_SIZE as u64;
    (*proc.trapframe).kernel_trap = usertrap as usize as u64;
    // hartid for Cpu::current_id()
    (*proc.trapframe).kernel_hartid = asm::r_tp();

    // Set up the registers that trampoline.S's
    // sret will use to get to user space.

    // Set S Previous Privelege mode to User.
    let mut x = asm::r_sstatus();
    // Clear SPP to 0 for user mode.
    x &= !SSTATUS_SPP;
    // Enable interrupts in user mode.
    x |= SSTATUS_SPIE;
    asm::w_sstatus(x);

    // Set S Exception Program Counter to the saved user pc.
    asm::w_sepc((*proc.trapframe).epc);

    // Tell trampoline.S the user page table to switch to.
    let satp = make_satp(proc.pagetable);

    // Jump to userret in trampoline.S at the top of memory, which
    // switches to the user page table, restores user registers,
    // and switches to user mode with sret.
    let trampoline_userret =
        TRAMPOLINE + (addr_of!(userret) as usize) - (addr_of!(trampoline) as usize);
    let trampoline_userret = trampoline_userret as *const ();
    // Rust's most dangerous function: core::mem::transmute
    let trampoline_userret = core::mem::transmute::<*const (), fn(u64) -> !>(trampoline_userret);
    trampoline_userret(satp)
}

/// Interrupts and exceptions from kernel code go here via kernelvec,
/// on whatever the current kernel stack is.
#[no_mangle]
pub unsafe extern "C" fn kerneltrap() {
    let sepc = asm::r_sepc();
    let sstatus = asm::r_sstatus();
    let scause = asm::r_scause();

    if sstatus & SSTATUS_SPP == 0 {
        panic!("kerneltrap: not from supervisor mode");
    } else if interrupt::interrupts_enabled() != 0 {
        panic!("kerneltrap: interrupts enabled");
    }

    let which_dev = devintr();
    if which_dev == 0 {
        println!(
            "scause {}\nsepc={} stval={}",
            scause,
            asm::r_sepc(),
            asm::r_stval()
        );
        panic!("kerneltrap");
    } else if which_dev == 2
        && Process::current().is_some()
        && Process::current().unwrap().state == ProcessState::Running
    {
        // Give up the CPU if this is a timer interrupt.
        r#yield();
    }

    // The yield() may have caused some traps to occur,
    // so restore trap registers for use by kernelvec.S's sepc instruction.
    asm::w_sepc(sepc);
    asm::w_sstatus(sstatus);
}

/// Handle an interrupt, exception, or system call from userspace.
///
/// Called from trampoline.S
#[no_mangle]
pub unsafe extern "C" fn usertrap() {
    if asm::r_sstatus() & SSTATUS_SPP != 0 {
        panic!("usertrap: not from user mode");
    }

    // Send interrupts and exceptions to kerneltrap(),
    // since we're now in the kernel.
    asm::w_stvec(kernelvec as usize as u64);

    let proc = Process::current().unwrap();

    // Save user program counter.
    (*proc.trapframe).epc = asm::r_sepc();

    if asm::r_scause() == 8 {
        // System call

        if proc.is_killed() {
            proc.exit(-1);
        }

        // sepc points to the ecall instruction, but
        // we want to return to the next instruction.
        (*proc.trapframe).epc += 4;

        // An interrupt will change sepc, scause, and sstatus,
        // so enable only now that we're done with those registers.
        interrupt::enable_interrupts();

        syscall();
    }

    let which_dev = devintr();
    if asm::r_scause() != 8 && which_dev == 0 {
        println!(
            "usertrap(): unexpected scause {} {}\n\tsepc={} stval={}",
            asm::r_scause(),
            proc.pid,
            asm::r_sepc(),
            asm::r_stval()
        );
        proc.set_killed(true);
    }

    if proc.is_killed() {
        proc.exit(-1);
    }

    // Give up the CPU if this is a timer interrupt.
    if which_dev == 2 {
        r#yield();
    }

    usertrapret();
}
