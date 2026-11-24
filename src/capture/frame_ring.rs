use std::collections::VecDeque;
use std::sync::Mutex;

use super::frame::VideoFrame;

/// Bounded latest-frame ring buffer.
///
/// Semantics:
/// - Bounded capacity: oldest frames are dropped when full (backpressure).
/// - Latest-frame oriented: `try_pop_latest()` returns the newest frame and
///   recycles stale intermediate buffers.
/// - Non-blocking: all operations use `try_lock` and never block the caller.
/// - Buffer reuse: dropped/consumed frame data buffers are kept in an internal
///   pool and can be reacquired with `try_acquire_buffer()`.
pub struct FrameRingBuffer {
    inner: Mutex<FrameRingInner>,
    capacity: usize,
    max_pool_buffers: usize,
}

struct FrameRingInner {
    queue: VecDeque<VideoFrame>,
    buffer_pool: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub enum PushError {
    /// The lock was contended; caller may retry or drop.
    Contended(VideoFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRingStats {
    pub queued: usize,
    pub capacity: usize,
    pub pooled_buffers: usize,
}

impl FrameRingBuffer {
    /// Creates a ring buffer with at least capacity 1.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            inner: Mutex::new(FrameRingInner {
                queue: VecDeque::with_capacity(capacity),
                buffer_pool: Vec::new(),
            }),
            capacity,
            max_pool_buffers: capacity.saturating_mul(2).max(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns current queue length, or None if lock is contended.
    pub fn try_len(&self) -> Option<usize> {
        self.inner.try_lock().ok().map(|g| g.queue.len())
    }

    /// Returns true if empty, or None if lock is contended.
    pub fn try_is_empty(&self) -> Option<bool> {
        self.inner.try_lock().ok().map(|g| g.queue.is_empty())
    }

    /// Returns stats snapshot, or None if lock is contended.
    pub fn try_stats(&self) -> Option<FrameRingStats> {
        self.inner.try_lock().ok().map(|g| FrameRingStats {
            queued: g.queue.len(),
            capacity: self.capacity,
            pooled_buffers: g.buffer_pool.len(),
        })
    }

    /// Acquires a reusable data buffer from the pool, or allocates a new one.
    /// Never blocks; returns a fresh Vec if lock is contended or pool is empty.
    pub fn try_acquire_buffer(&self, min_capacity: usize) -> Vec<u8> {
        if let Ok(mut guard) = self.inner.try_lock() {
            if let Some(mut buf) = guard.buffer_pool.pop() {
                if buf.capacity() < min_capacity {
                    buf.reserve(min_capacity - buf.capacity());
                }
                buf.clear();
                return buf;
            }
        }
        Vec::with_capacity(min_capacity)
    }

    /// Pushes a frame, dropping the oldest stale frame when at capacity.
    /// Non-blocking: returns `Err(PushError::Contended(frame))` if lock contended.
    pub fn try_push(&self, frame: VideoFrame) -> Result<(), PushError> {
        let mut guard = match self.inner.try_lock() {
            Ok(g) => g,
            Err(_) => return Err(PushError::Contended(frame)),
        };

        if guard.queue.len() >= self.capacity {
            if let Some(stale) = guard.queue.pop_front() {
                recycle_frame_data(&mut guard.buffer_pool, self.max_pool_buffers, stale.data);
            }
        }

        guard.queue.push_back(frame);
        Ok(())
    }

    /// Pops the oldest frame (FIFO). Non-blocking; returns None if empty/contended.
    pub fn try_pop(&self) -> Option<VideoFrame> {
        let mut guard = self.inner.try_lock().ok()?;
        guard.queue.pop_front()
    }

    /// Pops the newest frame and drops/recycles all older stale frames.
    /// Non-blocking; returns None if empty/contended.
    pub fn try_pop_latest(&self) -> Option<VideoFrame> {
        let mut guard = self.inner.try_lock().ok()?;
        let latest = guard.queue.pop_back()?;
        while let Some(stale) = guard.queue.pop_front() {
            recycle_frame_data(&mut guard.buffer_pool, self.max_pool_buffers, stale.data);
        }
        Some(latest)
    }

    /// Recycles a consumed frame's data buffer into the pool for reuse.
    /// Non-blocking; drops buffer if lock contended or pool is full.
    pub fn recycle_frame(&self, mut frame: VideoFrame) {
        let data = std::mem::take(&mut frame.data);
        if let Ok(mut guard) = self.inner.try_lock() {
            recycle_frame_data(&mut guard.buffer_pool, self.max_pool_buffers, data);
        }
    }

    /// Recycles a raw data buffer into the pool for reuse.
    /// Non-blocking; drops buffer if lock contended or pool is full.
    pub fn recycle_buffer(&self, data: Vec<u8>) {
        if let Ok(mut guard) = self.inner.try_lock() {
            recycle_frame_data(&mut guard.buffer_pool, self.max_pool_buffers, data);
        }
    }

    /// Clears queued frames and recycles their buffers. Non-blocking.
    /// Returns false if lock contended.
    pub fn try_clear(&self) -> bool {
        let mut guard = match self.inner.try_lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        while let Some(stale) = guard.queue.pop_front() {
            recycle_frame_data(&mut guard.buffer_pool, self.max_pool_buffers, stale.data);
        }
        true
    }
}

fn recycle_frame_data(pool: &mut Vec<Vec<u8>>, max_pool_buffers: usize, mut data: Vec<u8>) {
    if pool.len() >= max_pool_buffers {
        return;
    }
    data.clear();
    pool.push(data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::PixelFormat;
    use std::time::Duration;

    fn frame(seq: u64) -> VideoFrame {
        VideoFrame::new(2, 2, PixelFormat::Rgba8, vec![seq as u8; 16], Duration::from_millis(seq), seq)
    }

    #[test]
    fn push_pop_fifo_and_capacity_drop_oldest() {
        let ring = FrameRingBuffer::new(2);
        ring.try_push(frame(1)).unwrap();
        ring.try_push(frame(2)).unwrap();
        ring.try_push(frame(3)).unwrap(); // drops seq 1

        let a = ring.try_pop().unwrap();
        let b = ring.try_pop().unwrap();
        assert_eq!(a.sequence, 2);
        assert_eq!(b.sequence, 3);
        assert!(ring.try_pop().is_none());
    }

    #[test]
    fn pop_latest_drops_stale_and_returns_newest() {
        let ring = FrameRingBuffer::new(4);
        for seq in 1..=4 {
            ring.try_push(frame(seq)).unwrap();
        }
        let latest = ring.try_pop_latest().unwrap();
        assert_eq!(latest.sequence, 4);
        assert!(ring.try_pop().is_none());
    }

    #[test]
    fn buffer_reuse_pool() {
        let ring = FrameRingBuffer::new(2);
        let buf = ring.try_acquire_buffer(1024);
        assert!(buf.capacity() >= 1024);

        ring.try_push(frame(1)).unwrap();
        ring.try_push(frame(2)).unwrap();
        ring.try_push(frame(3)).unwrap(); // recycles seq 1 data

        let stats = ring.try_stats().unwrap();
        assert!(stats.pooled_buffers >= 1);

        let reused = ring.try_acquire_buffer(16);
        assert!(reused.capacity() >= 16);
    }
}
