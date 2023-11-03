/// Saved registers for kernel context switches.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Context {
    pub ra: u64,
    pub sp: u64,

    // callee-saved
    pub s0: u64,
    pub s1: u64,
    pub s2: u64,
    pub s3: u64,
    pub s4: u64,
    pub s5: u64,
    pub s6: u64,
    pub s7: u64,
    pub s8: u64,
    pub s9: u64,
    pub s10: u64,
    pub s11: u64,
}
impl Context {
    pub const fn new() -> Context {
        Context {
            ra: 0u64,
            sp: 0u64,
            s0: 0u64,
            s1: 0u64,
            s2: 0u64,
            s3: 0u64,
            s4: 0u64,
            s5: 0u64,
            s6: 0u64,
            s7: 0u64,
            s8: 0u64,
            s9: 0u64,
            s10: 0u64,
            s11: 0u64,
        }
    }
}
