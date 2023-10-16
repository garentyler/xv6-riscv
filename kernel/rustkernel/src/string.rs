use core::{ffi::c_char, option::Option};

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

pub(crate) unsafe fn strlen_checked(s: *const c_char, max_chars: usize) -> Option<i32> {
    for len in 0..max_chars {
        if (*s.add(len)) == '\0' as i8 {
            return Some(len.try_into().unwrap_or(i32::MAX));
        }
    }
    None
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> i32 {
    strlen_checked(s, usize::MAX).unwrap_or(i32::MAX)
}

#[no_mangle]
pub unsafe extern "C" fn strncmp(mut a: *const u8, mut b: *const u8, mut max_chars: u32) -> i32 {
    while max_chars > 0 && *a != 0 && *a == *b {
        max_chars -= 1;
        a = a.add(1);
        b = b.add(1);
    }
    if max_chars == 0 {
        0
    } else {
        (*a - *b) as i32
    }
}

#[no_mangle]
pub unsafe extern "C" fn strncpy(
    mut a: *mut u8,
    mut b: *const u8,
    mut max_chars: i32,
) -> *const u8 {
    let original_a = a;
    while max_chars > 0 && *b != 0 {
        *a = *b;
        max_chars -= 1;
        a = a.add(1);
        b = b.add(1);
    }

    while max_chars > 0 {
        *a = 0;
        max_chars -= 1;
        a = a.add(1);
    }

    original_a
}

/// Like strncpy but guaranteed to null-terminate.
#[no_mangle]
pub unsafe extern "C" fn safestrcpy(
    mut a: *mut u8,
    mut b: *const u8,
    mut max_chars: i32,
) -> *const u8 {
    let original_a = a;

    if max_chars <= 0 {
        return a;
    } else {
        max_chars -= 1;
    }

    while max_chars > 0 && *b != 0 {
        *a = *b;
        max_chars -= 1;
        a = a.add(1);
        b = b.add(1);
    }

    *a = 0;

    original_a
}
