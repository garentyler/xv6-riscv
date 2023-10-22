use core::{ffi::c_char, option::Option};

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
