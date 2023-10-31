pub const KIND_DIR: i16 = 1;
pub const KIND_FILE: i16 = 2;
pub const KIND_DEVICE: i16 = 3;

#[repr(C)]
#[derive(Default)]
pub struct Stat {
    /// FS's disk device.
    pub device: i32,
    /// Inode number.
    pub inode: u32,
    /// Type of file.
    pub kind: i16,
    /// Number of links to file.
    pub num_links: i16,
    /// Size of file in bytes.
    pub size: u64,
}
