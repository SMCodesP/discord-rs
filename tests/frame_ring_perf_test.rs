use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use discord_rs::capture::{FrameRingBuffer, PixelFormat, PushError, VideoFrame};

fn make_frame(seq: u64, payload_bytes: usize) -> VideoFrame {
    let width = 64u32;
    let height = 64u32;
    let mut data = Vec::with_capacity(payload_bytes.max(16));
    data.resize(payload_bytes.max(16), (seq % 251) as u8);
    VideoFrame::new(
        width,
        height,
        PixelFormat::Rgba8,
        data,
        Duration::from_micros(seq),
        seq,
    )
}

#[test]
fn bounded_ring_drops_stale_frames_under_backpressure() {
    let ring = FrameRingBuffer::new(3);
    for seq in 1..=10u64 {
        ring.try_push(make_frame(seq, 256)).expect("push should not contend");
    }

    assert_eq!(ring.try_len(), Some(3));
    let latest = ring.try_pop_latest().expect("latest frame");
    assert_eq!(latest.sequence, 10);
    assert!(ring.try_pop().is_none(), "stale frames must be dropped");

    let stats = ring.try_stats().expect("stats");
    assert_eq!(stats.queued, 0);
    assert_eq!(stats.capacity, 3);
    assert!(stats.pooled_buffers >= 1, "dropped frames should recycle buffers");
}

#[test]
fn try_pop_latest_prefers_newest_and_recycles_stale_buffers() {
    let ring = FrameRingBuffer::new(8);
    for seq in 0..5u64 {
        ring.try_push(make_frame(seq, 128)).unwrap();
    }

    let latest = ring.try_pop_latest().unwrap();
    assert_eq!(latest.sequence, 4);
    assert_eq!(ring.try_len(), Some(0));

    // Recycled stale buffers should be available for reacquisition.
    let mut reused = 0;
    for _ in 0..4 {
        let buf = ring.try_acquire_buffer(128);
        if buf.capacity() >= 128 {
            reused += 1;
        }
    }
    assert_eq!(reused, 4);
}

#[test]
fn buffer_reuse_avoids_unbounded_growth_under_steady_state() {
    let ring = FrameRingBuffer::new(2);
    let mut observed_caps: Vec<usize> = Vec::new();

    for seq in 0..200u64 {
        let mut data = ring.try_acquire_buffer(1024);
        // simulate writer filling buffer
        if data.len() < 1024 {
            data.resize(1024, 0);
        }
        data[0] = (seq % 255) as u8;
        let frame = VideoFrame::new(16, 16, PixelFormat::Rgba8, data, Duration::from_micros(seq), seq);

        match ring.try_push(frame) {
            Ok(()) => {}
            Err(PushError::Contended(f)) => {
                // Contention should be rare/non-blocking; recycle and continue.
                ring.recycle_frame(f);
            }
        }

        // Consumer drains latest occasionally, recycling buffer.
        if seq % 3 == 0 {
            if let Some(f) = ring.try_pop_latest() {
                observed_caps.push(f.data.capacity());
                ring.recycle_frame(f);
            }
        }
    }

    // Drain remaining
    while let Some(f) = ring.try_pop_latest() {
        observed_caps.push(f.data.capacity());
        ring.recycle_frame(f);
    }

    let stats = ring.try_stats().unwrap();
    assert!(stats.pooled_buffers <= stats.capacity * 2);
    assert!(!observed_caps.is_empty());
    let max_cap = observed_caps.iter().copied().max().unwrap_or(0);
    assert!(max_cap >= 1024);
    assert!(max_cap < 1024 * 16, "buffer capacities should stay bounded, got max {max_cap}");
}

#[test]
fn producer_consumer_is_non_blocking_and_makes_progress() {
    let ring = Arc::new(FrameRingBuffer::new(4));
    let stop = Arc::new(AtomicBool::new(false));
    let produced = Arc::new(AtomicU64::new(0));
    let consumed = Arc::new(AtomicU64::new(0));
    let max_push_ns = Arc::new(AtomicU64::new(0));
    let max_pop_ns = Arc::new(AtomicU64::new(0));
    let barrier = Arc::new(Barrier::new(3));

    let producer_ring = Arc::clone(&ring);
    let producer_stop = Arc::clone(&stop);
    let producer_count = Arc::clone(&produced);
    let producer_max_ns = Arc::clone(&max_push_ns);
    let producer_barrier = Arc::clone(&barrier);
    let producer = thread::spawn(move || {
        producer_barrier.wait();
        let mut seq = 0u64;
        while !producer_stop.load(Ordering::Relaxed) {
            let mut data = producer_ring.try_acquire_buffer(512);
            if data.len() < 512 {
                data.resize(512, 0);
            }
            let frame = make_frame(seq, 512);
            let start = Instant::now();
            match producer_ring.try_push(frame) {
                Ok(()) => {
                    producer_count.fetch_add(1, Ordering::Relaxed);
                    seq += 1;
                }
                Err(PushError::Contended(f)) => {
                    producer_ring.recycle_frame(f);
                }
            }
            let elapsed_ns = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            producer_max_ns.fetch_max(elapsed_ns, Ordering::Relaxed);
            if seq % 32 == 0 {
                thread::yield_now();
            }
        }
    });

    let consumer_ring = Arc::clone(&ring);
    let consumer_stop = Arc::clone(&stop);
    let consumer_count = Arc::clone(&consumed);
    let consumer_max_ns = Arc::clone(&max_pop_ns);
    let consumer_barrier = Arc::clone(&barrier);
    let consumer = thread::spawn(move || {
        consumer_barrier.wait();
        while !consumer_stop.load(Ordering::Relaxed) {
            let start = Instant::now();
            if let Some(frame) = consumer_ring.try_pop_latest() {
                consumer_count.fetch_add(1, Ordering::Relaxed);
                consumer_ring.recycle_frame(frame);
            }
            let elapsed_ns = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            consumer_max_ns.fetch_max(elapsed_ns, Ordering::Relaxed);
            thread::yield_now();
        }
        // drain
        while let Some(frame) = consumer_ring.try_pop_latest() {
            consumer_count.fetch_add(1, Ordering::Relaxed);
            consumer_ring.recycle_frame(frame);
        }
    });

    barrier.wait();
    thread::sleep(Duration::from_millis(250));
    stop.store(true, Ordering::Relaxed);
    producer.join().expect("producer join");
    consumer.join().expect("consumer join");

    let produced_total = produced.load(Ordering::Relaxed);
    let consumed_total = consumed.load(Ordering::Relaxed);
    assert!(produced_total > 0, "producer must make progress");
    assert!(consumed_total > 0, "consumer must make progress");

    // Non-blocking sanity: single operations should not take unbounded time.
    // Generous bound for CI; lock-free-ish try_lock operations should be far below.
    let max_push_ms = max_push_ns.load(Ordering::Relaxed) / 1_000_000;
    let max_pop_ms = max_pop_ns.load(Ordering::Relaxed) / 1_000_000;
    assert!(max_push_ms < 50, "push took too long: {max_push_ms} ms");
    assert!(max_pop_ms < 50, "pop took too long: {max_pop_ms} ms");
}
