pub mod asm;
pub mod cpu;
pub mod mem;
pub mod start;
pub mod trap;
pub mod virtual_memory;

/// Previous mode
pub const MSTATUS_MPP_MASK: u64 = 3 << 11;
pub const MSTATUS_MPP_M: u64 = 3 << 11;
pub const MSTATUS_MPP_S: u64 = 1 << 11;
pub const MSTATUS_MPP_U: u64 = 0 << 11;
/// Machine-mode interrupt enable.
pub const MSTATUS_MIE: u64 = 1 << 3;

/// Previous mode: 1 = Supervisor, 0 = User
pub const SSTATUS_SPP: u64 = 1 << 8;
/// Supervisor Previous Interrupt Enable
pub const SSTATUS_SPIE: u64 = 1 << 5;
/// User Previous Interrupt Enable
pub const SSTATUS_UPIE: u64 = 1 << 4;
/// Supervisor Interrupt Enable
pub const SSTATUS_SIE: u64 = 1 << 1;
/// User Interrupt Enable
pub const SSTATUS_UIE: u64 = 1 << 0;

/// Supervisor External Interrupt Enable
pub const SIE_SEIE: u64 = 1 << 9;
/// Supervisor Timer Interrupt Enable
pub const SIE_STIE: u64 = 1 << 5;
/// Supervisor Software Interrupt Enable
pub const SIE_SSIE: u64 = 1 << 1;

/// Machine-mode External Interrupt Enable
pub const MIE_MEIE: u64 = 1 << 11;
/// Machine-mode Timer Interrupt Enable
pub const MIE_MTIE: u64 = 1 << 7;
/// Machine-mode Software Interrupt Enable
pub const MIE_MSIE: u64 = 1 << 3;
