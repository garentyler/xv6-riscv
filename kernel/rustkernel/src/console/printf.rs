use crate::sync::lock::Lock;
use core::ffi::{c_char, CStr};

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
        // use $crate::hardware::uart::{BufferedUart, Uart, UartWriter};
        use core::fmt::Write;

        // let mut uart: UartWriter = $crate::console::UART0.writer_unbuffered();

        // let uart: &BufferedUart = &$crate::console::UART0;
        // let uart: &Uart = &**uart;
        // let mut uart: UartWriter = uart.writer();
        //
        let _ = core::write!($crate::console::UART0.writer_unbuffered(), $($arg)*);

        // let _ = core::write!(uart, $($arg)*);
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
