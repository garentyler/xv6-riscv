use crate::{
    proc::{exit, fork, growproc, kill, killed, myproc, sleep, wait},
    syscall::{argaddr, argint},
};
use core::ptr::addr_of_mut;

#[no_mangle]
pub unsafe extern "C" fn sys_exit() -> u64 {
    let mut n = 0i32;
    argint(0, addr_of_mut!(n));
    exit(n)
}

#[no_mangle]
pub unsafe extern "C" fn sys_getpid() -> u64 {
    (*myproc()).pid as u64
}

#[no_mangle]
pub unsafe extern "C" fn sys_fork() -> u64 {
    fork() as u64
}

#[no_mangle]
pub unsafe extern "C" fn sys_wait() -> u64 {
    let mut p = 0u64;
    argaddr(0, addr_of_mut!(p));
    wait(p) as u64
}

#[no_mangle]
pub unsafe extern "C" fn sys_sbrk() -> u64 {
    let mut n = 0i32;
    argint(0, addr_of_mut!(n));
    let addr = (*myproc()).sz;

    if growproc(n) < 0 {
        -1i64 as u64
    } else {
        addr
    }
}

#[no_mangle]
pub unsafe extern "C" fn sys_sleep() -> u64 {
    let mut n = 0i32;
    argint(0, addr_of_mut!(n));

    crate::trap::tickslock.lock_unguarded();
    let ticks = crate::trap::ticks;
    while crate::trap::ticks < ticks + n as u32 {
        if killed(myproc()) > 0 {
            crate::trap::tickslock.unlock();
            return -1i64 as u64;
        }
        sleep(
            addr_of_mut!(crate::trap::ticks).cast(),
            addr_of_mut!(crate::trap::tickslock).cast(),
        )
    }
    crate::trap::tickslock.unlock();
    0
}

#[no_mangle]
pub unsafe extern "C" fn sys_kill() -> u64 {
    let mut pid = 0i32;
    argint(0, addr_of_mut!(pid));
    kill(pid) as u64
}

/// Returns how many clock tick interrupts have occurred since start.
#[no_mangle]
pub unsafe extern "C" fn sys_uptime() -> u64 {
    crate::trap::tickslock.lock_unguarded();
    let ticks = crate::trap::ticks;
    crate::trap::tickslock.unlock();
    ticks as u64
}
