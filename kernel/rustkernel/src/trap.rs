use crate::{
    arch::riscv::*,
    println,
    proc::{
        cpu::Cpu,
        process::{exit, r#yield, wakeup, Process, ProcessState},
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
    w_stvec(kernelvec as usize as u64);
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
    let scause = r_scause();

    if (scause & 0x8000000000000000 > 0) && (scause & 0xff) == 9 {
        // This is a supervisor external interrupt, via PLIC.

        // IRQ indicates which device interrupted.
        let irq = plic::plic_claim();

        if irq == UART0_IRQ {
            crate::console::uart::UART0.interrupt();
        } else if irq == VIRTIO0_IRQ {
            virtio_disk_intr();
        } else if irq > 0 {
            println!("unexpected interrupt irq={}", irq);
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

        if Cpu::current_id() == 0 {
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

#[derive(Default)]
pub struct InterruptBlocker;
impl InterruptBlocker {
    pub fn new() -> InterruptBlocker {
        unsafe {
            let interrupts_before = intr_get();
            let cpu = Cpu::current();

            intr_off();

            if cpu.interrupt_disable_layers == 0 {
                cpu.previous_interrupts_enabled = interrupts_before;
            }
            cpu.interrupt_disable_layers += 1;
            // crate::sync::spinlock::push_off();
        }
        InterruptBlocker
    }
}
impl core::ops::Drop for InterruptBlocker {
    fn drop(&mut self) {
        unsafe {
            let cpu = Cpu::current();

            if intr_get() == 1 || cpu.interrupt_disable_layers < 1 {
                // panic!("pop_off mismatched");
                return;
            }

            cpu.interrupt_disable_layers -= 1;

            if cpu.interrupt_disable_layers == 0 && cpu.previous_interrupts_enabled == 1 {
                intr_on();
            }
            // crate::sync::spinlock::pop_off();
        }
    }
}
impl !Send for InterruptBlocker {}

/// Return to user space
#[no_mangle]
pub unsafe extern "C" fn usertrapret() {
    let proc = Process::current().unwrap();

    // We're about to switch the destination of traps from
    // kerneltrap() to usertrap(), so turn off interrupts until
    // we're back in user space, where usertrap() is correct.
    intr_off();

    // Send syscalls, interrupts, and exceptions to uservec in trampoline.S
    let trampoline_uservec =
        TRAMPOLINE + (addr_of!(uservec) as usize as u64) - (addr_of!(trampoline) as usize as u64);
    w_stvec(trampoline_uservec);

    // Set up trapframe values that uservec will need when
    // the process next traps into the kernel.
    // kernel page table
    (*proc.trapframe).kernel_satp = r_satp();
    // process's kernel stack
    (*proc.trapframe).kernel_sp = proc.kstack + PGSIZE;
    (*proc.trapframe).kernel_trap = usertrap as usize as u64;
    // hartid for Cpu::current_id()
    (*proc.trapframe).kernel_hartid = r_tp();

    // Set up the registers that trampoline.S's
    // sret will use to get to user space.

    // Set S Previous Privelege mode to User.
    let mut x = r_sstatus();
    // Clear SPP to 0 for user mode.
    x &= !SSTATUS_SPP;
    // Enable interrupts in user mode.
    x |= SSTATUS_SPIE;
    w_sstatus(x);

    // Set S Exception Program Counter to the saved user pc.
    w_sepc((*proc.trapframe).epc);

    // Tell trampoline.S the user page table to switch to.
    let satp = make_satp(proc.pagetable);

    // Jump to userret in trampoline.S at the top of memory, which
    // switches to the user page table, restores user registers,
    // and switches to user mode with sret.
    let trampoline_userret = (TRAMPOLINE + (addr_of!(userret) as usize as u64)
        - (addr_of!(trampoline) as usize as u64)) as usize;
    let trampoline_userret = trampoline_userret as *const ();
    // Rust's most dangerous function: core::mem::transmute
    let trampoline_userret = core::mem::transmute::<*const (), fn(u64)>(trampoline_userret);
    trampoline_userret(satp)
}

/// Interrupts and exceptions from kernel code go here via kernelvec,
/// on whatever the current kernel stack is.
#[no_mangle]
pub unsafe extern "C" fn kerneltrap() {
    let sepc = r_sepc();
    let sstatus = r_sstatus();
    let scause = r_scause();

    if sstatus & SSTATUS_SPP == 0 {
        panic!("kerneltrap: not from supervisor mode");
    } else if intr_get() != 0 {
        panic!("kerneltrap: interrupts enabled");
    }

    let which_dev = devintr();
    if which_dev == 0 {
        println!("scause {}\nsepc={} stval={}", scause, r_sepc(), r_stval());
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
    w_sepc(sepc);
    w_sstatus(sstatus);
}

/// Handle an interrupt, exception, or system call from userspace.
///
/// Called from trampoline.S
#[no_mangle]
pub unsafe extern "C" fn usertrap() {
    if r_sstatus() & SSTATUS_SPP != 0 {
        panic!("usertrap: not from user mode");
    }

    // Send interrupts and exceptions to kerneltrap(),
    // since we're now in the kernel.
    w_stvec(kernelvec as usize as u64);

    let proc = Process::current().unwrap();

    // Save user program counter.
    (*proc.trapframe).epc = r_sepc();

    if r_scause() == 8 {
        // System call

        if proc.is_killed() {
            exit(-1);
        }

        // sepc points to the ecall instruction, but
        // we want to return to the next instruction.
        (*proc.trapframe).epc += 4;

        // An interrupt will change sepc, scause, and sstatus,
        // so enable only now that we're done with those registers.
        intr_on();

        syscall();
    }

    let which_dev = devintr();
    if r_scause() != 8 && which_dev == 0 {
        println!(
            "usertrap(): unexpected scause {} {}\n\tsepc={} stval={}",
            r_scause(),
            proc.pid,
            r_sepc(),
            r_stval()
        );
        proc.set_killed(true);
    }

    if proc.is_killed() {
        exit(-1);
    }

    // Give up the CPU if this is a timer interrupt.
    if which_dev == 2 {
        r#yield();
    }

    usertrapret();
}

// push_intr_off/pop_intr_off are like intr_off()/intr_on() except that they are matched:
// it takes two pop_intr_off()s to undo two push_intr_off()s.  Also, if interrupts
// are initially off, then push_intr_off, pop_intr_off leaves them off.

pub unsafe fn push_intr_off() {
    let old = intr_get();
    let cpu = Cpu::current();

    intr_off();
    if cpu.interrupt_disable_layers == 0 {
        cpu.previous_interrupts_enabled = old;
    }
    cpu.interrupt_disable_layers += 1;
}
pub unsafe fn pop_intr_off() {
    let cpu = Cpu::current();

    if intr_get() == 1 {
        // crate::panic_byte(b'0');
        panic!("pop_intr_off - interruptible");
    } else if cpu.interrupt_disable_layers < 1 {
        // crate::panic_byte(b'1');
        panic!("pop_intr_off");
    }

    cpu.interrupt_disable_layers -= 1;

    if cpu.interrupt_disable_layers == 0 && cpu.previous_interrupts_enabled == 1 {
        intr_on();
    }
}
