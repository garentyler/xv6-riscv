#![no_std]
#![no_main]

use core::ffi::{c_char, CStr};

extern "C" {
    fn print(message: *const c_char);
    fn panic(panic_message: *const c_char) -> !;
}

#[no_mangle]
pub extern "C" fn rust_main() {
    unsafe {
        print(
            CStr::from_bytes_with_nul(b"Hello from Rust!\0")
                .unwrap()
                .as_ptr(),
        );
    }
}


#[panic_handler]
unsafe fn panic_wrapper(_panic_info: &core::panic::PanicInfo) -> ! {
    panic(CStr::from_bytes_with_nul(b"panic from rust\0").unwrap_or_default().as_ptr())
}
