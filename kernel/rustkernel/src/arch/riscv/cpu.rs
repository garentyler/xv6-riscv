use super::asm::r_tp;

pub fn cpu_id() -> usize {
    unsafe { r_tp() as usize }
}
