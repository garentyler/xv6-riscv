//! Virtio device driver.
//! 
//! For both the MMIO interface, and virtio descriptors.
//! Only tested with qemu.
//! 
//! The virtio spec: https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.pdf
//! qemu ... -drive file=fs.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

use crate::{sync::spinlock::Spinlock, buf::Buffer};
use core::ffi::c_char;

// Virtio MMIO control registers, mapped starting at 0x10001000
// From qemu virtio_mmio.h

/// 0x74726976
pub const VIRTIO_MMIO_MAGIC_VALUE: u64 = 0x000u64;
/// Version - should be 2.
pub const VIRTIO_MMIO_VERSION: u64 = 0x004u64;
/// Device type.
/// 
/// 1: Network
/// 2: Disk
pub const VIRTIO_MMIO_DEVICE_ID: u64		= 0x008u64;
/// 0x554d4551
pub const VIRTIO_MMIO_VENDOR_ID: u64		= 0x00cu64;
pub const VIRTIO_MMIO_DEVICE_FEATURES: u64	= 0x010u64;
pub const VIRTIO_MMIO_DRIVER_FEATURES: u64	= 0x020u64;
/// Select queue, write-only.
pub const VIRTIO_MMIO_QUEUE_SEL: u64		= 0x030u64;
/// Max size of current queue, read-only.
pub const VIRTIO_MMIO_QUEUE_NUM_MAX: u64	= 0x034u64;
/// Size of current queue, write-only.
pub const VIRTIO_MMIO_QUEUE_NUM: u64		= 0x038u64;
/// Ready bit.
pub const VIRTIO_MMIO_QUEUE_READY: u64		= 0x044u64;
/// Write-only.
pub const VIRTIO_MMIO_QUEUE_NOTIFY: u64	= 0x050u64;
/// Read-only.
pub const VIRTIO_MMIO_INTERRUPT_STATUS: u64	= 0x060u64;
/// Write-only.
pub const VIRTIO_MMIO_INTERRUPT_ACK: u64	= 0x064u64;
/// Read/write.
pub const VIRTIO_MMIO_STATUS: u64		= 0x070u64;
/// Physical address for descriptor table, write-only.
pub const VIRTIO_MMIO_QUEUE_DESC_LOW: u64	= 0x080u64;
pub const VIRTIO_MMIO_QUEUE_DESC_HIGH: u64	= 0x084u64;
/// Physical address for available ring, write-only.
pub const VIRTIO_MMIO_DRIVER_DESC_LOW: u64	= 0x090u64;
pub const VIRTIO_MMIO_DRIVER_DESC_HIGH: u64	= 0x094u64;
/// Physical address for used ring, write-only.
pub const VIRTIO_MMIO_DEVICE_DESC_LOW: u64	= 0x0a0u64;
pub const VIRTIO_MMIO_DEVICE_DESC_HIGH: u64	= 0x0a4u64;

// Status register bits, from qemu virtio_config.h.
pub const VIRTIO_CONFIG_S_ACKNOWLEDGE: u8	= 0x01u8;
pub const VIRTIO_CONFIG_S_DRIVER: u8		= 0x02u8;
pub const VIRTIO_CONFIG_S_DRIVER_OK: u8	= 0x04u8;
pub const VIRTIO_CONFIG_S_FEATURES_OK: u8	= 0x08u8;

// Device feature bits
/// Disk is read-only.
pub const VIRTIO_BLK_F_RO: u8               = 5u8;
/// Supports SCSI command passthrough.
pub const VIRTIO_BLK_F_SCSI: u8             = 7u8;
/// Writeback mode available in config.
pub const VIRTIO_BLK_F_CONFIG_WCE: u8      = 11u8;
/// Support more than one vq.
pub const VIRTIO_BLK_F_MQ: u8              = 12u8;
pub const VIRTIO_F_ANY_LAYOUT: u8          = 27u8;
pub const VIRTIO_RING_F_INDIRECT_DESC: u8  = 28u8;
pub const VIRTIO_RING_F_EVENT_IDX: u8      = 29u8;

/// This many virtio descriptors.
/// 
/// Must be a power of two.
pub const NUM_DESCRIPTORS: usize = 8usize;

/// A single descriptor, from the spec.
#[repr(C)]
pub struct VirtqDescriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

/// Chained with another descriptor.
pub const VRING_DESC_F_NEXT: u16 = 1u16;
/// Device writes (vs read).
pub const VRING_DESC_F_WRITE: u16 = 2u16;

/// The entire avail ring, from the spec.
#[repr(C)]
pub struct VirtqAvailable {
    /// Always zero.
    pub flags: u16,
    /// Driver will write ring[idx] next.
    pub idx: u16,
    /// Descriptor numbers of chain heads.
    pub ring: [u16; NUM_DESCRIPTORS],
    pub unused: u16,
}

/// One entry in the "used" ring, with which the
/// device tells the driver about completed requests.
#[repr(C)]
pub struct VirtqUsedElement {
    /// Index of start of completed descriptor chain.
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
pub struct VirtqUsed {
    /// Always zero.
    pub flags: u16,
    /// Device increments it when it adds a ring[] entry.
    pub idx: u16,
    pub ring: [VirtqUsedElement; NUM_DESCRIPTORS],
}

// These are specific to virtio block devices (disks),
// Described in section 5.2 of the spec.

/// Read the disk.
pub const VIRTIO_BLK_T_IN: u32  = 0u32;
/// Write the disk.
pub const VIRTIO_BLK_T_OUT: u32 = 1u32;

/// The format of the first descriptor in a disk request.
/// 
/// To be followed by two more descriptors containing
/// the block, and a one-byte status.
#[repr(C)]
pub struct VirtioBlockRequest {
    /// 0: Write the disk.
    /// 1: Read the disk.
    pub kind: u32,
    pub reserved: u32,
    pub sector: u64,
}

#[repr(C)]
pub struct DiskInfo {
    pub b: *mut Buffer,
    pub status: c_char,
}

#[repr(C)]
pub struct Disk {
    /// A set (not a ring) of DMA descriptors, with which the
    /// driver tells the device where to read and write individual
    /// disk operations. There are NUM descriptors.
    /// 
    /// Most commands consist of a "chain" (linked list)
    /// of a couple of these descriptors.
    pub descriptors: *mut VirtqDescriptor,
    /// A ring in which the driver writes descriptor numbers
    /// that the driver would like the device to process. It
    /// only includes the head descriptor of each chain. The
    /// ring has NUM elements.
    pub available: *mut VirtqAvailable,
    /// A ring in which the device writes descriptor numbers
    /// that the device has finished processing (just the
    /// head of each chain). There are NUM used ring entries.
    pub used: *mut VirtqUsed,

    // Our own book-keeping.
    /// Is a descriptor free?
    pub free: [c_char; NUM_DESCRIPTORS],
    /// We've looked this far in used[2..NUM].
    pub used_idx: u16,

    /// Track info about in-flight operations,
    /// for use when completion interrupt arrives.
    /// 
    /// Indexed by first descriptor index of chain.
    pub info: [DiskInfo; NUM_DESCRIPTORS],


    /// Disk command headers.
    /// One-for-one with descriptors, for convenience.
    pub ops: [VirtioBlockRequest; NUM_DESCRIPTORS],

    pub vdisk_lock: Spinlock,
}

extern "C" {
    pub static mut disk: Disk;
    pub fn virtio_disk_init();
    pub fn virtio_disk_rw(buf: *mut Buffer, write: i32);
    pub fn virtio_disk_intr();
}
