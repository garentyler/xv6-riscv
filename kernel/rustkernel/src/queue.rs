use core::iter::*;

pub const QUEUE_SIZE: usize = 64;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum QueueError {
    NoSpace,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Queue<T> {
    inner: [Option<T>; QUEUE_SIZE],
    /// The index of the first item in the queue.
    queue_start: usize,
    /// The length of the queue.
    queue_len: usize,
}
impl<T: Copy> Queue<T> {
    pub const fn new() -> Queue<T> {
        Queue {
            inner: [None; QUEUE_SIZE],
            queue_start: 0,
            queue_len: 0,
        }
    }
}
impl<T> Queue<T> {
    /// Accessor method for the length of the queue.
    pub fn len(&self) -> usize {
        self.queue_len
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Returns how many items can currently be added to the queue.
    pub fn space_remaining(&self) -> usize {
        self.inner.len() - self.len()
    }
    /// Returns the index of the last item in the queue.
    fn queue_end(&self) -> usize {
        (self.queue_start + self.queue_len - 1) % self.inner.len()
    }

    /// Removes an item from the front of the queue.
    pub fn pop_front(&mut self) -> Option<T> {
        let item = self.inner[self.queue_start].take();
        if item.is_some() {
            self.queue_start += 1;
            self.queue_start %= self.inner.len();
            self.queue_len -= 1;
        }
        item
    }
    /// Adds an item to the front of the queue.
    pub fn push_front(&mut self, value: T) -> Result<(), QueueError> {
        if self.space_remaining() == 0 {
            return Err(QueueError::NoSpace);
        }

        if self.queue_start == 0 {
            self.queue_start = self.inner.len() - 1;
        } else {
            self.queue_start -= 1;
        }
        self.inner[self.queue_start] = Some(value);
        self.queue_len += 1;
        Ok(())
    }
    /// Removes an item from the end of the queue.
    pub fn pop_back(&mut self) -> Option<T> {
        let item = self.inner[self.queue_start].take();
        if item.is_some() {
            self.queue_len -= 1;
        }
        item
    }
    /// Adds an item to the end of the queue.
    pub fn push_back(&mut self, value: T) -> Result<(), QueueError> {
        if self.space_remaining() == 0 {
            return Err(QueueError::NoSpace);
        }

        self.queue_len += 1;
        self.inner[self.queue_end()] = Some(value);
        Ok(())
    }
}

impl<T> Iterator for Queue<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop_front()
    }
}
impl<T> DoubleEndedIterator for Queue<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.pop_back()
    }
}
impl<T> ExactSizeIterator for Queue<T> {
    fn len(&self) -> usize {
        self.len()
    }
}
