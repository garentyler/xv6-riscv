use crate::{
    fs::file::{filealloc, fileclose, File, FileType},
    mem::{
        kalloc::{kalloc, kfree},
        virtual_memory::{copyin, copyout},
    },
    proc::{killed, myproc, wakeup},
    sync::spinlock::Spinlock,
};
use core::ptr::{addr_of, addr_of_mut};

pub const PIPESIZE: usize = 512;

#[repr(C)]
pub struct Pipe {
    pub lock: Spinlock,
    pub data: [u8; PIPESIZE],
    /// Number of bytes read.
    pub bytes_read: u32,
    /// Number of bytes written.
    pub bytes_written: u32,
    /// Read fd is still open.
    pub is_read_open: i32,
    /// Write fd is still open.
    pub is_write_open: i32,
}
impl Pipe {
    pub fn new() -> Pipe {
        Pipe {
            lock: Spinlock::new(),
            data: [0u8; PIPESIZE],
            bytes_read: 0,
            bytes_written: 0,
            is_read_open: 1,
            is_write_open: 1,
        }
    }
}
impl Default for Pipe {
    fn default() -> Pipe {
        Pipe::new()
    }
}

extern "C" {
    // pub fn pipealloc(a: *mut *mut File, b: *mut *mut File) -> i32;
    // pub fn pipeclose(pipe: *mut Pipe, writable: i32);
    // pub fn pipewrite(pipe: *mut Pipe, addr: u64, n: i32) -> i32;
    // pub fn piperead(pipe: *mut Pipe, addr: u64, n: i32) -> i32;
}

#[no_mangle]
pub unsafe extern "C" fn pipealloc(a: *mut *mut File, b: *mut *mut File) -> i32 {
    *a = filealloc();
    *b = filealloc();
    let pipe = kalloc() as *mut Pipe;

    // If any of them fail, close and return -1.
    if a.is_null() || b.is_null() || pipe.is_null() {
        if !pipe.is_null() {
            kfree(pipe as *mut u8);
        }
        if !a.is_null() {
            fileclose(*a);
        }
        if !b.is_null() {
            fileclose(*b);
        }
        -1
    } else {
        *pipe = Pipe::new();
        (**a).kind = FileType::Pipe;
        (**a).readable = 1;
        (**a).writable = 0;
        (**a).pipe = pipe;
        (**b).kind = FileType::Pipe;
        (**b).readable = 0;
        (**b).writable = 1;
        (**b).pipe = pipe;
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn pipeclose(pipe: *mut Pipe, writable: i32) {
    let _guard = (*pipe).lock.lock();

    if writable > 0 {
        (*pipe).is_write_open = 0;
        wakeup(addr_of!((*pipe).bytes_read).cast_mut().cast());
    } else {
        (*pipe).is_read_open = 0;
        wakeup(addr_of!((*pipe).bytes_written).cast_mut().cast());
    }

    if (*pipe).is_read_open == 0 && (*pipe).is_write_open == 0 {
        kfree(pipe.cast());
    }
}

#[no_mangle]
pub unsafe extern "C" fn pipewrite(pipe: *mut Pipe, addr: u64, n: i32) -> i32 {
    let mut i = 0;
    let p = myproc();
    let lock = (*pipe).lock.lock();

    while i < n {
        if (*pipe).is_read_open == 0 || killed(p) > 0 {
            return -1;
        }
        if (*pipe).bytes_written == (*pipe).bytes_read + PIPESIZE as u32 {
            // DOC: pipewrite-full
            wakeup(addr_of!((*pipe).bytes_read).cast_mut().cast());
            lock.sleep(addr_of!((*pipe).bytes_written).cast_mut().cast());
        } else {
            let mut b = 0u8;
            if copyin((*p).pagetable, addr_of_mut!(b), addr + i as u64, 1) == -1 {
                break;
            }
            (*pipe).data[(*pipe).bytes_written as usize % PIPESIZE] = b;
            (*pipe).bytes_written += 1;
            i += 1;
        }
    }
    wakeup(addr_of!((*pipe).bytes_read).cast_mut().cast());
    i
}

#[no_mangle]
#[allow(clippy::while_immutable_condition)]
pub unsafe extern "C" fn piperead(pipe: *mut Pipe, addr: u64, n: i32) -> i32 {
    let mut i = 0;
    let p = myproc();
    let lock = (*pipe).lock.lock();

    // DOC: pipe-empty
    while (*pipe).bytes_read == (*pipe).bytes_written && (*pipe).is_write_open > 0 {
        if killed(p) > 0 {
            return -1;
        } else {
            // DOC: piperead-sleep
            lock.sleep(addr_of!((*pipe).bytes_read).cast_mut().cast());
        }
    }

    // DOC: piperead-copy
    while i < n {
        if (*pipe).bytes_read == (*pipe).bytes_written {
            break;
        }
        let b = (*pipe).data[(*pipe).bytes_read as usize % PIPESIZE];
        (*pipe).bytes_read += 1;
        if copyout((*p).pagetable, addr + i as u64, addr_of!(b).cast_mut(), 1) == -1 {
            break;
        }
        i += 1;
    }
    wakeup(addr_of!((*pipe).bytes_written).cast_mut().cast());
    i
}
