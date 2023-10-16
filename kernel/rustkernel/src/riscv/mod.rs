pub mod asm;
pub mod memlayout;
pub mod plic;

pub use asm::*;
pub use memlayout::*;

pub type Pte = u64;
pub type Pagetable = *mut [Pte; 512];

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

pub const SATP_SV39: u64 = 8 << 60;

/// Bytes per page
pub const PGSIZE: u64 = 4096;
/// Bits of offset within a page
pub const PGSHIFT: u64 = 12;

pub fn pg_round_up(sz: u64) -> u64 {
    (sz + PGSIZE - 1) & !(PGSIZE - 1)
}
pub fn pg_round_down(a: u64) -> u64 {
    a & !(PGSIZE - 1)
}

// Valid.
pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;
// User can access.
pub const PTE_U: u64 = 1 << 4;

/*
// shift a physical address to the right place for a PTE.
#define PA2PTE(pa) ((((uint64)pa) >> 12) << 10)

#define PTE2PA(pte) (((pte) >> 10) << 12)

#define PTE_FLAGS(pte) ((pte) & 0x3FF)

// extract the three 9-bit page table indices from a virtual address.
#define PXMASK          0x1FF // 9 bits
#define PXSHIFT(level)  (PGSHIFT+(9*(level)))
#define PX(level, va) ((((uint64) (va)) >> PXSHIFT(level)) & PXMASK)
*/

/// One beyond the highest possible virtual address.
///
/// MAXVA is actually one bit less than the max allowed by
/// Sv39, to avoid having to sign-extend virtual addresses
/// that have the high bit set.
pub const MAXVA: u64 = 1u64 << (9 + 9 + 9 + 12 - 1);
