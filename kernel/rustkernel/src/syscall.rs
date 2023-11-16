use crate::{
    arch::{
        clock::CLOCK_TICKS,
        power::shutdown,
        virtual_memory::{copyin, copyinstr},
    },
    fs::{
        file::{self, File},
        inode::{ilock, iput, iunlock, namei},
        log::LogOperation,
        stat::KIND_DIR,
    },
    println,
    proc::process::Process,
    string::strlen,
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
            Syscall::Fork => Process::fork().unwrap_or(-1) as i64 as u64,
            Syscall::Exit => {
                let mut status = 0i32;
                argint(0, addr_of_mut!(status));
                Process::current().unwrap().exit(status)
            }
            Syscall::Wait => {
                let mut p = 0u64;
                argaddr(0, addr_of_mut!(p));
                Process::current().unwrap().wait_for_child(p).unwrap_or(-1) as i64 as u64
                // process::wait(p) as u64
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
                Process::kill(pid) as u64
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
                let proc = Process::current().unwrap();

                let _operation = LogOperation::new();

                if argstr(0, addr_of_mut!(path).cast(), path.len() as i32) < 0 {
                    return -1i64 as u64;
                }
                let inode = namei(addr_of_mut!(path).cast());
                if inode.is_null() {
                    return -1i64 as u64;
                }
                ilock(inode);
                if (*inode).kind != KIND_DIR {
                    iunlock(inode);
                    iput(inode);
                    return -1i64 as u64;
                }
                iunlock(inode);
                iput(proc.current_dir);
                proc.current_dir = inode;
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
            Syscall::Getpid => Process::current().unwrap().pid as u64,
            Syscall::Sbrk => {
                let mut n = 0i32;
                argint(0, addr_of_mut!(n));
                let proc = Process::current().unwrap();
                let addr = proc.memory_allocated;

                if unsafe { proc.grow_memory(n).is_ok() } {
                    addr
                } else {
                    -1i64 as u64
                }
            }
            Syscall::Sleep => {
                let mut n = 0i32;
                argint(0, addr_of_mut!(n));

                let mut ticks = CLOCK_TICKS.lock_spinning();

                while *ticks < *ticks + n as usize {
                    if Process::current().unwrap().is_killed() {
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
                    Process::current().unwrap().open_files[file_descriptor as usize] = null_mut();
                    file::fileclose(file);
                    0
                } else {
                    -1i64 as u64
                }
            }
            Syscall::Shutdown => unsafe { shutdown() },
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
    let proc = Process::current().unwrap();

    // Both tests needed, in case of overflow.
    if addr >= proc.memory_allocated
        || addr + size_of::<u64>() as u64 > proc.memory_allocated
        || copyin(
            proc.pagetable,
            ip.cast(),
            addr as usize,
            size_of::<*mut u64>(),
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
    let proc = Process::current().unwrap();

    if copyinstr(proc.pagetable, buf, addr as usize, max as u32 as usize) < 0 {
        -1
    } else {
        strlen(buf.cast())
    }
}

/// Allocate a file descriptor for the given file.
/// Takes over file reference from caller on success.
unsafe fn fdalloc(file: *mut File) -> Result<usize, ()> {
    let proc = Process::current().unwrap();

    for file_descriptor in 0..crate::NOFILE {
        if proc.open_files[file_descriptor].is_null() {
            proc.open_files[file_descriptor] = file;
            return Ok(file_descriptor);
        }
    }
    Err(())
}

unsafe fn argraw(argument_index: usize) -> u64 {
    let proc = Process::current().unwrap();

    match argument_index {
        0 => (*proc.trapframe).a0,
        1 => (*proc.trapframe).a1,
        2 => (*proc.trapframe).a2,
        3 => (*proc.trapframe).a3,
        4 => (*proc.trapframe).a4,
        5 => (*proc.trapframe).a5,
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

    let file: *mut File = Process::current().unwrap().open_files[file_descriptor];
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
    let proc = Process::current().unwrap();

    let num = (*proc.trapframe).a7;

    (*proc.trapframe).a0 = match TryInto::<Syscall>::try_into(num as usize) {
        Ok(syscall) => syscall.call(),
        Err(_) => {
            println!("{} unknown syscall {}", proc.pid, num);
            -1i64 as u64
        }
    };
}
