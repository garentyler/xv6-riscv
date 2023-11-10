pub mod kalloc;

#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut u8, data: i32, max_bytes: u32) -> *mut u8 {
    for i in 0..max_bytes {
        *dst.add(i as usize) = data as u8;
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(mut a: *const u8, mut b: *const u8, max_bytes: u32) -> i32 {
    for _ in 0..max_bytes {
        if *a != *b {
            return (*a - *b) as i32;
        } else {
            a = a.add(1);
            b = b.add(1);
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn memmove(mut dst: *mut u8, mut src: *const u8, max_bytes: u32) -> *mut u8 {
    if max_bytes == 0 {
        return dst;
    }

    // If src starts before dst and src + max_bytes
    // is after d, the memory regions overlap.
    if src < dst && src.add(max_bytes as usize) > dst {
        dst = dst.add(max_bytes as usize);
        src = src.add(max_bytes as usize);

        for _ in 0..max_bytes {
            dst = dst.sub(1);
            src = src.sub(1);
            *dst = *src;
        }
    } else {
        for _ in 0..max_bytes {
            *dst = *src;
            dst = dst.add(1);
            src = src.add(1);
        }
    }

    dst
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, max_bytes: u32) -> *mut u8 {
    memmove(dst, src, max_bytes)
}
