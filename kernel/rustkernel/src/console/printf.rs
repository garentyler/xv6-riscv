use crate::sync::spinlock::Spinlock;
use core::ffi::{c_char, CStr};

pub use crate::panic;

#[no_mangle]
pub static mut PRINT_LOCK: Spinlock = Spinlock::new();

#[repr(C)]
pub struct PrintLock {
    pub lock: Spinlock,
    pub locking: i32,
}

macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;

        // Still unsafe because static mut.
        let _guard = unsafe { $crate::console::printf::PRINT_LOCK.lock() };

        let mut cons = $crate::console::cons.lock();
        let _ = core::write!(cons.as_mut(), $($arg)*);
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
    PRINT_LOCK = Spinlock::new();
}
