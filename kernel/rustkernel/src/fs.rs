//! On-disk file system forma.
//! Both the kernel and user programs use this header file.

// Root inode
pub const ROOTINO: u64 = 1;
/// Block size.
pub const BSIZE: u32 = 1024;

// Disk layout:
// [ boot block | super block | log | inode blocks | free bit map | data blocks ]
//
// mkfs computes the super block and builds an initial file system.
// The super block describes the disk layout:
#[repr(C)]
pub struct Superblock {
    /// Must be FSMAGIC.
    pub magic: u32,
    /// Size of file system image (blocks).
    pub size: u32,
    /// Number of data blocks.
    pub nblocks: u32,
    /// Number of inodes.
    pub ninodes: u32,
    /// Number of log blocks.
    pub nlog: u32,
    /// Block number of first log block.
    pub logstart: u32,
    /// Block number of first inode block.
    pub inodestart: u32,
    /// Block number of first free map block.
    pub bmapstart: u32,
}

pub const FSMAGIC: u32 = 0x10203040;
pub const NDIRECT: u32 = 12;
pub const NINDIRECT: u32 = BSIZE / core::mem::size_of::<u32>() as u32;
pub const MAXFILE: u32 = NDIRECT + NINDIRECT;

// On-disk inode structure;
#[repr(C)]
pub struct DiskInode {
    /// File type.
    pub kind: i16,
    /// Major device number (T_DEVICE only).
    pub major: i16,
    /// Minor device number (T_DEVICE only).
    pub minor: i16,
    /// Number of links to inode in file system.
    pub nlink: i16,
    /// Size of file (bytes).
    pub size: u32,
    /// Data block addresses.
    pub addrs: [u32; NDIRECT as usize + 1],
}

/// Inodes per block.
pub const IPB: u32 = BSIZE / core::mem::size_of::<DiskInode>() as u32;

/// Block containing inode i.
pub fn iblock(inode: u32, superblock: &Superblock) -> u32 {
    inode / IPB + superblock.inodestart
}

/// Bitmap bits per block.
pub const BPB: u32 = BSIZE * 8;

/// Block of free map containing bit for block b.
pub fn bblock(block: u32, superblock: &Superblock) -> u32 {
    block / BPB + superblock.bmapstart
}

/// Directory is a file containing a sequence of DirectoryEntry structures.
pub const DIRSIZ: usize = 14;

#[repr(C)]
pub struct DirectoryEntry {
    pub inum: u16,
    pub name: [u8; DIRSIZ],
}
