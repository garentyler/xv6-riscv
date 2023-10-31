use crate::sync::lock::Lock;
use core::ffi::{c_char, CStr};

pub use crate::panic;

pub static PRINT_LOCK: Lock = Lock::new();

/// Print out formatted text to the UART.
/// Spins to acquire the lock.
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;

        let _guard = $crate::console::printf::PRINT_LOCK.lock_spinning();
        let mut cons = $crate::console::cons.lock_spinning();

        let _ = core::write!(cons.as_mut(), $($arg)*);
    }};
}
pub(crate) use print;

macro_rules! println {
    ($($arg:tt)*) => {{
        use $crate::console::printf::print;
        print!($($arg)*);
        print!("\n");
    }};
}
pub(crate) use println;

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
