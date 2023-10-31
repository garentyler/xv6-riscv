pub mod lock;
pub mod mutex;

// These have to stick around until the entire program is in rust =(
pub mod sleeplock;
pub mod spinlock;

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum LockStrategy {
    #[default]
    Spin,
    Sleep,
}
