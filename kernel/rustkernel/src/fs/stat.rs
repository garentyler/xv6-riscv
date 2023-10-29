#[repr(C)]
pub enum StatType {
    Directory = 1,
    File,
    Device,
}

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
