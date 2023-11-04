use crate::sync::lock::Lock;
use core::ffi::{c_char, CStr};

pub use crate::panic;

pub static PRINT_LOCK: Lock = Lock::new();

/// Print out formatted text to the console.
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

/// Print out formatted text to the UART.
/// Does not use any locks.
macro_rules! uprint {
    ($($arg:tt)*) => {{
        use $crate::console::uart::Uart;
        use core::fmt::Write;

        // Do some casts to get a mutable reference.
        // Safe because Uart's core::fmt::Write implementation
        // only uses the &mut reference immutably.
        let uart: *const Uart = &$crate::console::uart::UART0 as *const Uart;
        let uart: &mut Uart = unsafe { &mut *uart.cast_mut() };
        
        let _ = core::write!(uart, $($arg)*);
    }};
}
pub(crate) use uprint;

macro_rules! uprintln {
    ($($arg:tt)*) => {{
        use $crate::console::printf::uprint;
        uprint!($($arg)*);
        uprint!("\n");
    }};
}
pub(crate) use uprintln;


#[no_mangle]
pub extern "C" fn printint(n: i32) {
    print!("{}", n);
}

#[no_mangle]
pub unsafe extern "C" fn printstr(s: *const c_char) {
    let s = CStr::from_ptr(s).to_str().unwrap_or_default();
    print!("{}", s);
}
