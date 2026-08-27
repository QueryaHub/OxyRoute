//! Thread-local buffer pool for request bodies to prevent heap fragmentation (issue #160).

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

const POOL_CAPACITY_LIMIT: usize = 128 * 1024;
const INITIAL_BUFFER_CAPACITY: usize = 16 * 1024;
const MAX_POOL_SIZE: usize = 64;

thread_local! {
    static BODY_POOL: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// RAII wrapper around a pooled `Vec<u8>` that automatically returns itself to the
/// thread-local buffer pool on `Drop` if capacity <= 128 KB.
pub struct PooledBuffer(Vec<u8>);

impl PooledBuffer {
    pub fn new() -> Self {
        let buf = BODY_POOL
            .with(|pool| pool.borrow_mut().pop())
            .unwrap_or_else(|| Vec::with_capacity(INITIAL_BUFFER_CAPACITY));
        Self(buf)
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}

impl Default for PooledBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for PooledBuffer {
    type Target = Vec<u8>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PooledBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if self.0.capacity() >= INITIAL_BUFFER_CAPACITY && self.0.capacity() <= POOL_CAPACITY_LIMIT {
            self.0.clear();
            let buf = std::mem::take(&mut self.0);
            BODY_POOL.with(|pool| {
                let mut p = pool.borrow_mut();
                if p.len() < MAX_POOL_SIZE {
                    p.push(buf);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pooled_buffer_recycle() {
        {
            let mut buf = PooledBuffer::new();
            buf.extend_from_slice(b"hello world");
            assert_eq!(&*buf, b"hello world");
        }
        // Dropped -> returned to pool.
        let buf2 = PooledBuffer::new();
        assert!(buf2.is_empty());
        assert!(buf2.capacity() >= INITIAL_BUFFER_CAPACITY);
    }
}
