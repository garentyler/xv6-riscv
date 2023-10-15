/// Maximum number of processes
pub const NPROC: usize = 64;
/// Maximum number of CPUs
pub const NCPU: usize = 8;
/// Maximum number of open files per process
pub const NOFILE: usize = 16;
/// Maximum number of open files per system
pub const NFILE: usize = 100;
/// Maximum number of active inodes
pub const NINODE: usize = 50;
/// Maximum major device number
pub const NDEV: usize = 10;
/// Device number of file system root disk
pub const ROOTDEV: usize = 1;
/// Max exec arguments
pub const MAXARG: usize = 32;
/// Max num of blocks any FS op writes
pub const MAXOPBLOCKS: usize = 10;
/// Max data blocks in on-disk log
pub const LOGSIZE: usize = MAXOPBLOCKS * 3;
/// Size of disk block cache
pub const NBUF: usize = MAXOPBLOCKS * 3;
/// Size of file system in blocks
pub const FSSIZE: usize = 2000;
/// Maximum file path size
pub const MAXPATH: usize = 128;
