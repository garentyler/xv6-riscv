//! Support functions for system calls that involve file descriptors.

use super::inode::{iput, readi, stati, writei, Inode, InodeLockGuard};
use crate::{
    fs::{log, stat::Stat},
    hal::arch::virtual_memory::copyout,
    io::pipe::Pipe,
    proc::process::Process,
    sync::mutex::Mutex,
};
use core::ptr::{addr_of_mut, null_mut};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Default)]
pub enum FileType {
    #[default]
    None,
    Pipe,
    Inode,
    Device,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct File {
    pub kind: FileType,
    /// Reference count.
    pub references: i32,
    pub readable: u8,
    pub writable: u8,
    /// FileType::Pipe
    pub pipe: *mut Pipe,
    /// FileType::Inode and FileType::Device
    pub ip: *mut Inode,
    /// FileType::Inode
    pub off: u32,
    /// FileType::Device
    pub major: i16,
}
unsafe impl Send for File {}
impl File {
    pub const fn uninitialized() -> File {
        File {
            kind: FileType::None,
            references: 0,
            readable: 0,
            writable: 0,
            pipe: null_mut(),
            ip: null_mut(),
            off: 0,
            major: 0,
        }
    }
}

/// Map major device number to device functions.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Devsw {
    pub read: Option<fn(i32, u64, i32) -> i32>,
    pub write: Option<fn(i32, u64, i32) -> i32>,
}
impl Devsw {
    pub const fn new() -> Devsw {
        Devsw {
            read: None,
            write: None,
        }
    }
}

#[no_mangle]
pub static mut devsw: [Devsw; crate::NDEV] = [Devsw::new(); crate::NDEV];
pub static FILES: Mutex<[File; crate::NFILE]> = Mutex::new([File::uninitialized(); crate::NFILE]);
pub const CONSOLE: usize = 1;

/// Allocate a file structure.
#[no_mangle]
pub unsafe extern "C" fn filealloc() -> *mut File {
    let mut files = FILES.lock_spinning();

    for file in files.as_mut() {
        if file.references == 0 {
            file.references = 1;
            return addr_of_mut!(*file);
        }
    }

    null_mut()
}

/// Increment reference count for file `file`.
pub unsafe fn filedup(file: *mut File) -> *mut File {
    let _guard = FILES.lock_spinning();

    if (*file).references < 1 {
        panic!("filedup");
    } else {
        (*file).references += 1;
    }

    file
}

/// Close file `file`.
///
/// Decrement reference count, and close when reaching 0.
#[no_mangle]
pub unsafe extern "C" fn fileclose(file: *mut File) {
    let guard = FILES.lock_spinning();

    if (*file).references < 1 {
        panic!("fileclose");
    }

    (*file).references -= 1;

    if (*file).references == 0 {
        let f = *file;
        (*file).references = 0;
        (*file).kind = FileType::None;
        core::mem::drop(guard);

        match f.kind {
            FileType::Pipe => (*f.pipe).close(f.writable as i32),
            FileType::Inode | FileType::Device => {
                let _operation = log::LogOperation::new();
                iput(f.ip);
            }
            FileType::None => {}
        }
    }
}

/// Get metadata about file `file`.
///
/// `addr` is a user virtual address, pointing to a Stat.
pub unsafe fn filestat(file: *mut File, addr: u64) -> i32 {
    let proc = Process::current().unwrap();
    let mut stat = Stat::default();

    if (*file).kind == FileType::Inode || (*file).kind == FileType::Device {
        {
            let _guard = InodeLockGuard::new((*file).ip.as_mut().unwrap());
            stati((*file).ip, addr_of_mut!(stat));
        }

        if copyout(
            proc.pagetable,
            addr as usize,
            addr_of_mut!(stat).cast(),
            core::mem::size_of::<Stat>(),
        ) < 0
        {
            return -1;
        } else {
            return 0;
        }
    }

    -1
}

/// Read from file `file`.
///
/// `addr` is a user virtual address.
pub unsafe fn fileread(file: *mut File, addr: u64, num_bytes: i32) -> i32 {
    if (*file).readable == 0 {
        return -1;
    }

    match (*file).kind {
        FileType::Pipe => (*(*file).pipe)
            .read(addr, num_bytes as usize)
            .map(|n| n as i32)
            .unwrap_or(-1i32),
        FileType::Device => {
            if (*file).major < 0 || (*file).major >= crate::NDEV as i16 {
                return -1;
            }
            let Some(read) = devsw[(*file).major as usize].read else {
                return -1;
            };

            read(1, addr, num_bytes)
        }
        FileType::Inode => {
            let _guard = InodeLockGuard::new((*file).ip.as_mut().unwrap());
            let r = readi((*file).ip, 1, addr, (*file).off, num_bytes as u32);
            if r > 0 {
                (*file).off += r as u32;
            }
            r
        }
        _ => panic!("fileread"),
    }
}

/// Write to file `file`.
///
/// `addr` is as user virtual address.
pub unsafe fn filewrite(file: *mut File, addr: u64, num_bytes: i32) -> i32 {
    if (*file).writable == 0 {
        return -1;
    }

    match (*file).kind {
        FileType::Pipe => (*(*file).pipe)
            .write(addr, num_bytes as usize)
            .map(|n| n as i32)
            .unwrap_or(-1i32),
        FileType::Device => {
            if (*file).major < 0 || (*file).major >= crate::NDEV as i16 {
                return -1;
            }
            let Some(write) = devsw[(*file).major as usize].write else {
                return -1;
            };

            write(1, addr, num_bytes)
        }
        FileType::Inode => {
            // Write a few blocks at a time to avoid exceeding
            // the maximum log transaction size, including
            // inode, indirect block, allocation blocks,
            // and 2 blocks of slop for non-aligned writes.
            // This really belongs lower down, since writei()
            // might be writing a device like the console.
            let max = ((crate::MAXOPBLOCKS - 1 - 1 - 2) / 2) * super::BSIZE as usize;
            let mut i = 0;
            while i < num_bytes {
                let mut n = num_bytes - i;
                if n > max as i32 {
                    n = max as i32;
                }

                let r = {
                    let _operation = log::LogOperation::new();
                    let _guard = InodeLockGuard::new((*file).ip.as_mut().unwrap());

                    let r = writei((*file).ip, 1, addr + i as u64, (*file).off, n as u32);
                    if r > 0 {
                        (*file).off += r as u32;
                    }
                    r
                };

                if r != n {
                    // Error from writei.
                    break;
                } else {
                    i += r;
                }
            }
            if i == num_bytes {
                num_bytes
            } else {
                -1
            }
        }
        _ => panic!("filewrite"),
    }
}
