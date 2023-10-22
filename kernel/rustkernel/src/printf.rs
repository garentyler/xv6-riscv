use crate::sync::spinlock::Spinlock;
use core::ffi::{c_char, CStr};

pub use crate::panic;

#[no_mangle]
pub static mut PRINT_LOCK: Spinlock = unsafe { Spinlock::uninitialized() };

#[repr(C)]
pub struct PrintLock {
    pub lock: Spinlock,
    pub locking: i32,
}

macro_rules! print {
    ($($arg:tt)*) => {{
        unsafe { $crate::printf::PRINT_LOCK.lock_unguarded() };

        // Allocate a page of memory as the buffer and release it when we're done.
        let buf = unsafe { $crate::kalloc::kalloc() as *mut [u8; 4096] };

        let s: &str = format_no_std::show(
            unsafe { buf.as_mut() }.unwrap(),
            format_args!($($arg)*),
        ).unwrap();

        for c in s.as_bytes() {
            $crate::console::consputc(*c);
        }

        unsafe { $crate::kalloc::kfree(buf.cast()) };
        unsafe { $crate::printf::PRINT_LOCK.unlock() };
    }};
}
pub(crate) use print;

#[no_mangle]
pub extern "C" fn printint(n: i32) {
    print!("{}", n);
}

#[no_mangle]
pub extern "C" fn printhex(n: i32) {
    print!("{:0x}", n);
}

#[no_mangle]
pub extern "C" fn printptr(p: u64) {
    print!("{:#018x}", p);
}

#[no_mangle]
pub unsafe extern "C" fn printstr(s: *const c_char) {
    let s = CStr::from_ptr(s).to_str().unwrap_or_default();
    print!("{}", s);
}

#[no_mangle]
pub unsafe extern "C" fn printfinit() {
    PRINT_LOCK = Spinlock::new(
        CStr::from_bytes_with_nul(b"pr\0")
            .unwrap()
            .as_ptr()
            .cast_mut(),
    );
}
