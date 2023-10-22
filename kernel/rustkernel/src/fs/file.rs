#[repr(C)]
pub struct Devsw {
    pub read: *const i32,
    pub write: *const i32,
}

extern "C" {
    pub static mut devsw: [Devsw; crate::NDEV];
    pub fn fileinit();
}

pub const CONSOLE: usize = 1;
