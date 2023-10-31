use crate::{
    arch::riscv::memlayout::QEMU_POWER,
    fs::{
        self,
        file::{self, File},
        log::LogOperation,
        stat::KIND_DIR,
    },
    mem::virtual_memory::{copyin, copyinstr},
    println,
    proc::{self, myproc},
    string::strlen,
    trap::CLOCK_TICKS,
    NOFILE,
};
use core::{
    mem::size_of,
    ptr::{addr_of, addr_of_mut, null_mut},
};

extern "C" {
    fn sys_pipe() -> u64;
    fn sys_exec() -> u64;
    fn sys_fstat() -> u64;
    fn sys_chdir() -> u64;
    fn sys_open() -> u64;
    fn sys_mknod() -> u64;
    fn sys_unlink() -> u64;
    fn sys_link() -> u64;
    fn sys_mkdir() -> u64;
}

pub enum Syscall {
    Fork,
    Exit,
    Wait,
    Pipe,
    Read,
    Kill,
    Exec,
    Fstat,
    Chdir,
    Dup,
    Getpid,
    Sbrk,
    Sleep,
    Uptime,
    Open,
    Write,
    Mknod,
    Unlink,
    Link,
    Mkdir,
    Close,
    Shutdown,
}
impl Syscall {
    pub unsafe fn call(&self) -> u64 {
        match self {
            Syscall::Fork => proc::fork() as u64,
            Syscall::Exit => {
                let mut n = 0i32;
                argint(0, addr_of_mut!(n));
                proc::exit(n)
            }
            Syscall::Wait => {
                let mut p = 0u64;
                argaddr(0, addr_of_mut!(p));
                proc::wait(p) as u64
            }
            Syscall::Pipe => sys_pipe(),
            Syscall::Read => {
                let mut file: *mut File = null_mut();
                let mut num_bytes: i32 = 0;
                let mut ptr: u64 = 0;

                if argfd(0, null_mut(), addr_of_mut!(file)) >= 0 {
                    argaddr(1, addr_of_mut!(ptr));
                    argint(2, addr_of_mut!(num_bytes));
                    file::fileread(file, ptr, num_bytes) as i64 as u64
                } else {
                    -1i64 as u64
                }
            }
            Syscall::Kill => {
                let mut pid = 0i32;
                argint(0, addr_of_mut!(pid));
                proc::kill(pid) as u64
            }
            Syscall::Exec => sys_exec(),
            Syscall::Fstat => {
                let mut file: *mut File = null_mut();
                // User pointer to struct stat.
                let mut stat: u64 = 0;

                if argfd(0, null_mut(), addr_of_mut!(file)) >= 0 {
                    argaddr(1, addr_of_mut!(stat));
                    file::filestat(file, stat) as i64 as u64
                } else {
                    -1i64 as u64
                }
            }
            Syscall::Chdir => {
                let mut path = [0u8; crate::MAXPATH];
                let p = myproc();

                let _operation = LogOperation::new();

                if argstr(0, addr_of_mut!(path).cast(), path.len() as i32) < 0 {
                    return -1i64 as u64;
                }
                let inode = fs::namei(addr_of_mut!(path).cast());
                if inode.is_null() {
                    return -1i64 as u64;
                }
                fs::ilock(inode);
                if (*inode).kind != KIND_DIR {
                    fs::iunlock(inode);
                    fs::iput(inode);
                    return -1i64 as u64;
                }
                fs::iunlock(inode);
                fs::iput((*p).cwd);
                (*p).cwd = inode;
                0
            }
            Syscall::Dup => {
                let mut file: *mut File = null_mut();

                if argfd(0, null_mut(), addr_of_mut!(file)) < 0 {
                    return -1i64 as u64;
                }

                let Ok(file_descriptor) = fdalloc(file) else {
                    return -1i64 as u64;
                };

                file::filedup(file);
                file_descriptor as u64
            }
            Syscall::Getpid => (*myproc()).pid as u64,
            Syscall::Sbrk => {
                let mut n = 0i32;
                argint(0, addr_of_mut!(n));
                let addr = (*myproc()).sz;

                if proc::growproc(n) < 0 {
                    -1i64 as u64
                } else {
                    addr
                }
            }
            Syscall::Sleep => {
                let mut n = 0i32;
                argint(0, addr_of_mut!(n));

                let mut ticks = CLOCK_TICKS.lock_spinning();

                while *ticks < *ticks + n as usize {
                    if proc::killed(myproc()) > 0 {
                        return -1i64 as u64;
                    }
                    // Sleep until the value changes.
                    ticks.sleep(addr_of!(CLOCK_TICKS).cast_mut().cast());
                }
                0
            }
            // Returns how many clock tick interrupts have occured since start.
            Syscall::Uptime => *CLOCK_TICKS.lock_spinning() as u64,
            Syscall::Open => sys_open(),
            Syscall::Write => {
                let mut file: *mut File = null_mut();
                let mut num_bytes: i32 = 0;
                let mut ptr: u64 = 0;

                if argfd(0, null_mut(), addr_of_mut!(file)) >= 0 {
                    argaddr(1, addr_of_mut!(ptr));
                    argint(2, addr_of_mut!(num_bytes));
                    file::filewrite(file, ptr, num_bytes) as i64 as u64
                } else {
                    -1i64 as u64
                }
            }

            Syscall::Mknod => sys_mknod(),
            Syscall::Unlink => sys_unlink(),
            Syscall::Link => sys_link(),
            Syscall::Mkdir => sys_mkdir(),
            Syscall::Close => {
                let mut file_descriptor: i32 = 0;
                let mut file: *mut File = null_mut();

                if argfd(0, addr_of_mut!(file_descriptor), addr_of_mut!(file)) >= 0 {
                    (*myproc()).ofile[file_descriptor as usize] = null_mut();
                    file::fileclose(file);
                    0
                } else {
                    -1i64 as u64
                }
            }
            Syscall::Shutdown => {
                let qemu_power = QEMU_POWER as usize as *mut u32;
                qemu_power.write_volatile(0x5555u32);
                panic!("shutdown");
            }
        }
    }
}
impl TryFrom<usize> for Syscall {
    type Error = ();

    fn try_from(value: usize) -> core::result::Result<Self, Self::Error> {
        match value {
            1 => Ok(Syscall::Fork),
            2 => Ok(Syscall::Exit),
            3 => Ok(Syscall::Wait),
            4 => Ok(Syscall::Pipe),
            5 => Ok(Syscall::Read),
            6 => Ok(Syscall::Kill),
            7 => Ok(Syscall::Exec),
            8 => Ok(Syscall::Fstat),
            9 => Ok(Syscall::Chdir),
            10 => Ok(Syscall::Dup),
            11 => Ok(Syscall::Getpid),
            12 => Ok(Syscall::Sbrk),
            13 => Ok(Syscall::Sleep),
            14 => Ok(Syscall::Uptime),
            15 => Ok(Syscall::Open),
            16 => Ok(Syscall::Write),
            17 => Ok(Syscall::Mknod),
            18 => Ok(Syscall::Unlink),
            19 => Ok(Syscall::Link),
            20 => Ok(Syscall::Mkdir),
            21 => Ok(Syscall::Close),
            22 => Ok(Syscall::Shutdown),
            _ => Err(()),
        }
    }
}
impl From<Syscall> for usize {
    fn from(syscall: Syscall) -> usize {
        match syscall {
            Syscall::Fork => 1,
            Syscall::Exit => 2,
            Syscall::Wait => 3,
            Syscall::Pipe => 4,
            Syscall::Read => 5,
            Syscall::Kill => 6,
            Syscall::Exec => 7,
            Syscall::Fstat => 8,
            Syscall::Chdir => 9,
            Syscall::Dup => 10,
            Syscall::Getpid => 11,
            Syscall::Sbrk => 12,
            Syscall::Sleep => 13,
            Syscall::Uptime => 14,
            Syscall::Open => 15,
            Syscall::Write => 16,
            Syscall::Mknod => 17,
            Syscall::Unlink => 18,
            Syscall::Link => 19,
            Syscall::Mkdir => 20,
            Syscall::Close => 21,
            Syscall::Shutdown => 22,
        }
    }
}

/// Fetch the u64 at addr from the current process.
#[no_mangle]
pub unsafe extern "C" fn fetchaddr(addr: u64, ip: *mut u64) -> i32 {
    let p = myproc();

    // Both tests needed, in case of overflow.
    if addr >= (*p).sz
        || addr + size_of::<u64>() as u64 > (*p).sz
        || copyin(
            (*p).pagetable,
            ip.cast(),
            addr,
            size_of::<*mut u64>() as u64,
        ) != 0
    {
        -1
    } else {
        0
    }
}

/// Fetch the null-terminated string at addr from the current process.
///
/// Returns length of string, not including null, or -1 for error.
#[no_mangle]
pub unsafe extern "C" fn fetchstr(addr: u64, buf: *mut u8, max: i32) -> i32 {
    let p = myproc();
    if copyinstr((*p).pagetable, buf, addr, max as u64) < 0 {
        -1
    } else {
        strlen(buf.cast())
    }
}

/// Allocate a file descriptor for the given file.
/// Takes over file reference from caller on success.
unsafe fn fdalloc(file: *mut File) -> Result<usize, ()> {
    let p = myproc();
    for file_descriptor in 0..crate::NOFILE {
        if (*p).ofile[file_descriptor].is_null() {
            (*p).ofile[file_descriptor] = file;
            return Ok(file_descriptor);
        }
    }
    Err(())
}

unsafe fn argraw(argument_index: usize) -> u64 {
    let p = myproc();
    match argument_index {
        0 => (*(*p).trapframe).a0,
        1 => (*(*p).trapframe).a1,
        2 => (*(*p).trapframe).a2,
        3 => (*(*p).trapframe).a3,
        4 => (*(*p).trapframe).a4,
        5 => (*(*p).trapframe).a5,
        _ => panic!("argraw"),
    }
}

/// Fetch the n-th 32-bit syscall argument.
#[no_mangle]
pub unsafe extern "C" fn argint(n: i32, ip: *mut i32) {
    *ip = argraw(n as usize) as i32;
}

/// Retrieve an argument as a pointer.
///
/// Doesn't check for legality, since
/// copyin/copyout will do that.
#[no_mangle]
pub unsafe extern "C" fn argaddr(n: i32, ip: *mut u64) {
    *ip = argraw(n as usize);
}

/// Fetch the n-th word-sized syscall argument as a file descriptor
/// and return both the descriptor and the corresponding struct file.
#[no_mangle]
pub unsafe extern "C" fn argfd(
    n: i32,
    file_descriptor_out: *mut i32,
    file_out: *mut *mut File,
) -> i32 {
    let file_descriptor = argraw(n as usize) as usize;
    if file_descriptor >= NOFILE {
        return -1;
    }

    let file: *mut File = (*myproc()).ofile[file_descriptor];
    if file.is_null() {
        return -1;
    }

    if !file_descriptor_out.is_null() {
        *file_descriptor_out = file_descriptor as i32;
    }
    if !file_out.is_null() {
        *file_out = file;
    }
    0
}

/// Fetch the n-th word-sized syscall argument as a null-terminated string.
///
/// Copies into buf, at most max.
/// Returns string length if ok (including null), -1 if error.
#[no_mangle]
pub unsafe extern "C" fn argstr(n: i32, buf: *mut u8, max: i32) -> i32 {
    let mut addr = 0u64;
    argaddr(n, addr_of_mut!(addr));
    fetchstr(addr, buf, max)
}

pub unsafe fn syscall() {
    let p = myproc();
    let num = (*(*p).trapframe).a7;

    (*(*p).trapframe).a0 = match TryInto::<Syscall>::try_into(num as usize) {
        Ok(syscall) => syscall.call(),
        Err(_) => {
            println!("{} unknown syscall {}", (*p).pid, num);
            -1i64 as u64
        }
    };
}
