use crate::{
    fs::file::{filealloc, fileclose, File, FileType},
    hal::arch::virtual_memory::{copyin, copyout},
    mem::kalloc::{kalloc, kfree},
    proc::{process::Process, scheduler::wakeup},
    sync::spinlock::Spinlock,
};
use core::ptr::{addr_of, addr_of_mut};

pub const PIPESIZE: usize = 512;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PipeError {
    Allocation,
    ProcessKilled,
}

pub type Result<T> = core::result::Result<T, PipeError>;

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
    #[allow(clippy::new_ret_no_self)]
    pub unsafe fn new(a: *mut *mut File, b: *mut *mut File) -> Result<()> {
        *a = filealloc();
        *b = filealloc();
        let pipe = kalloc() as *mut Pipe;

        // If any of them fail, close all and return an error.
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
            Err(PipeError::Allocation)
        } else {
            *pipe = Pipe::default();
            (**a).kind = FileType::Pipe;
            (**a).readable = 1;
            (**a).writable = 0;
            (**a).pipe = pipe;
            (**b).kind = FileType::Pipe;
            (**b).readable = 0;
            (**b).writable = 1;
            (**b).pipe = pipe;
            Ok(())
        }
    }
    /// Unsafely get a reference to `self`.
    ///
    /// `self.lock` must be held beforehand.
    #[allow(clippy::mut_from_ref)]
    unsafe fn as_mut(&self) -> &mut Self {
        &mut *addr_of!(*self).cast_mut()
    }
    pub unsafe fn close(&self, writable: i32) {
        let _guard = self.lock.lock();

        if writable > 0 {
            self.as_mut().is_write_open = 0;
            wakeup(addr_of!(self.bytes_read).cast_mut().cast());
        } else {
            self.as_mut().is_read_open = 0;
            wakeup(addr_of!(self.bytes_written).cast_mut().cast());
        }

        if self.is_read_open == 0 && self.is_write_open == 0 {
            kfree(addr_of!(*self).cast_mut().cast());
        }
    }
    pub unsafe fn write(&self, addr: u64, num_bytes: usize) -> Result<usize> {
        let mut i = 0;
        let proc = Process::current().unwrap();
        let guard = self.lock.lock();

        while i < num_bytes {
            if self.is_read_open == 0 || proc.is_killed() {
                return Err(PipeError::ProcessKilled);
            }
            if self.bytes_written == self.bytes_read + PIPESIZE as u32 {
                // DOC: pipewrite-full
                wakeup(addr_of!(self.bytes_read).cast_mut().cast());
                guard.sleep(addr_of!(self.bytes_written).cast_mut().cast());
            } else {
                let mut b = 0u8;
                if copyin(proc.pagetable, addr_of_mut!(b), addr as usize + i, 1) == -1 {
                    break;
                }
                let index = self.bytes_written as usize % PIPESIZE;
                self.as_mut().data[index] = b;
                self.as_mut().bytes_written += 1;
                i += 1;
            }
        }
        wakeup(addr_of!(self.bytes_read).cast_mut().cast());
        Ok(i)
    }
    #[allow(clippy::while_immutable_condition)]
    pub unsafe fn read(&self, addr: u64, num_bytes: usize) -> Result<usize> {
        let mut i = 0;
        let proc = Process::current().unwrap();
        let guard = self.lock.lock();

        // DOC: pipe-empty
        while self.bytes_read == self.bytes_written && self.is_write_open > 0 {
            if proc.is_killed() {
                return Err(PipeError::ProcessKilled);
            } else {
                // DOC: piperead-sleep
                guard.sleep(addr_of!(self.bytes_read).cast_mut().cast());
            }
        }

        // DOC: piperead-copy
        while i < num_bytes {
            if self.bytes_read == self.bytes_written {
                break;
            }
            let b = self.data[self.bytes_read as usize % PIPESIZE];
            self.as_mut().bytes_read += 1;
            if copyout(proc.pagetable, addr as usize + i, addr_of!(b).cast_mut(), 1) == -1 {
                break;
            }
            i += 1;
        }
        wakeup(addr_of!(self.bytes_written).cast_mut().cast());
        Ok(i)
    }
}
impl Default for Pipe {
    fn default() -> Pipe {
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

#[no_mangle]
pub unsafe extern "C" fn pipealloc(a: *mut *mut File, b: *mut *mut File) -> i32 {
    match Pipe::new(a, b) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
