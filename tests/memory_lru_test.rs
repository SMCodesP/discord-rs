//! Deterministic tests for the generic MemoryLru cache.

use discord_rs::protocol::{MemoryLru, MemoryLruStats};

#[test]
fn lru_evicts_by_entry_limit_and_tracks_hits_misses() {
    let mut cache: MemoryLru<String, Vec<u8>> = MemoryLru::new(2, 1024);

    assert_eq!(cache.put("a".into(), vec![1], 1), None);
    assert_eq!(cache.put("b".into(), vec![2], 1), None);

    // Touch A so B becomes least recently used.
    assert_eq!(cache.get(&"a".to_string()), Some(&vec![1]));
    assert_eq!(cache.get(&"missing".to_string()), None);

    assert_eq!(cache.put("c".into(), vec![3], 1), None);

    assert_eq!(cache.get(&"a".to_string()), Some(&vec![1]));
    assert_eq!(cache.get(&"c".to_string()), Some(&vec![3]));
    assert_eq!(cache.get(&"b".to_string()), None);

    let stats = cache.stats();
    assert_eq!(
        stats,
        MemoryLruStats {
            hits: 3,
            misses: 2,
            evictions: 1,
            entries: 2,
            bytes: 2,
            max_entries: 2,
            max_bytes: 1024,
        }
    );
}

#[test]
fn lru_evicts_by_byte_budget() {
    let mut cache: MemoryLru<&'static str, Vec<u8>> = MemoryLru::new(10, 10);

    cache.put("a", vec![0; 4], 4);
    cache.put("b", vec![0; 4], 4);
    cache.put("c", vec![0; 4], 4); // should evict "a" (bytes 12 > 10)

    assert!(!cache.contains_key(&"a"));
    assert!(cache.contains_key(&"b"));
    assert!(cache.contains_key(&"c"));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.current_bytes(), 8);
    assert_eq!(cache.evictions(), 1);
    assert_eq!(
        cache.keys_lru_to_mru().copied().collect::<Vec<_>>(),
        vec!["b", "c"]
    );
}

#[test]
fn oversized_entry_does_not_stay_in_cache() {
    let mut cache: MemoryLru<String, Vec<u8>> = MemoryLru::new(5, 8);
    cache.put("small".into(), vec![1, 2, 3], 3);

    let previous = cache.put("huge".into(), vec![0; 16], 16);
    assert_eq!(previous, None);
    assert!(!cache.contains_key(&"huge".to_string()));
    assert_eq!(cache.get(&"small".to_string()), Some(&vec![1, 2, 3]));
    assert_eq!(cache.current_bytes(), 3);
    assert_eq!(cache.evictions(), 0);
}

#[test]
fn replace_updates_bytes_and_recency_without_eviction_count() {
    let mut cache: MemoryLru<String, String> = MemoryLru::new(2, 100);
    cache.put("a".into(), "old".into(), 10);
    cache.put("b".into(), "b".into(), 10);

    let prev = cache.put("a".into(), "new".into(), 20);
    assert_eq!(prev.as_deref(), Some("old"));
    assert_eq!(cache.evictions(), 0);
    assert_eq!(cache.current_bytes(), 30);

    // "a" was touched by replacement, so inserting "c" evicts "b".
    cache.put("c".into(), "c".into(), 10);
    assert!(cache.contains_key(&"a".to_string()));
    assert!(!cache.contains_key(&"b".to_string()));
    assert!(cache.contains_key(&"c".to_string()));
    assert_eq!(cache.evictions(), 1);
}

#[test]
fn peek_does_not_affect_recency_or_metrics() {
    let mut cache: MemoryLru<&'static str, i32> = MemoryLru::new(2, 100);
    cache.put("a", 1, 1);
    cache.put("b", 2, 1);

    assert_eq!(cache.peek(&"a"), Some(&1));
    assert_eq!(cache.hits(), 0);
    assert_eq!(cache.misses(), 0);

    // "a" was only peeked, so it remains LRU and gets evicted.
    cache.put("c", 3, 1);
    assert!(!cache.contains_key(&"a"));
    assert!(cache.contains_key(&"b"));
    assert!(cache.contains_key(&"c"));
}

#[test]
fn deterministic_60_channels_1000_images_stays_within_budget() {
    // Simulates 60 channels with 1,000 synthetic image entries total.
    // Budget: 8 channels cached, 256 "bytes" of decoded/media budget.
    let mut channels: MemoryLru<String, Vec<u32>> = MemoryLru::new(8, 8 * 1024);
    let mut images: MemoryLru<String, Vec<u8>> = MemoryLru::new(256, 256 * 16);
    let active_channel = "channel-00".to_string();
    let mut reloads_from_disk = 0u64;

    for cycle in 0..10u32 {
        for ch in 0..60u32 {
            let channel_id = format!("channel-{ch:02}");
            let msg_ids: Vec<u32> = (0..20).map(|i| cycle * 1000 + ch * 20 + i).collect();
            let bytes = msg_ids.len() * std::mem::size_of::<u32>();
            channels.put(channel_id.clone(), msg_ids, bytes);

            for img in 0..16u32 {
                let image_id = format!("img-{ch:02}-{img:02}");
                let payload = vec![((ch + img + cycle) % 255) as u8; 16];
                images.put(image_id, payload, 16);
            }
        }

        // Pin/keep active channel hot: if evicted, simulate disk reload.
        if channels.get(&active_channel).is_none() {
            reloads_from_disk += 1;
            channels.put(active_channel.clone(), vec![u32::MAX], 4);
        }
        // Draft pin simulation: active channel must remain addressable.
        assert!(channels.contains_key(&active_channel));

        assert!(channels.len() <= channels.max_entries());
        assert!(channels.current_bytes() <= channels.max_bytes());
        assert!(images.len() <= images.max_entries());
        assert!(images.current_bytes() <= images.max_bytes());
    }

    // After 10 cycles, active channel is preserved and old data was evicted/reloadable.
    assert!(channels.contains_key(&active_channel));
    assert!(channels.evictions() > 0);
    assert!(images.evictions() > 0);
    assert!(reloads_from_disk <= 10);

    // Simulate old image reload from disk: not present in RAM, but reinsertable.
    let old_id = "img-59-15".to_string();
    if images.get(&old_id).is_none() {
        images.put(old_id.clone(), vec![9; 16], 16);
    }
    assert_eq!(images.get(&old_id).map(|v| v.len()), Some(16));
    assert!(images.current_bytes() <= images.max_bytes());
}

#[test]
fn stats_reflect_current_state() {
    let mut cache: MemoryLru<i32, &'static str> = MemoryLru::new(3, 30);
    cache.put(1, "a", 10);
    cache.put(2, "b", 10);
    let _ = cache.get(&1);
    let _ = cache.get(&99);

    let stats = cache.stats();
    assert_eq!(stats.entries, 2);
    assert_eq!(stats.bytes, 20);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.evictions, 0);
    assert_eq!(stats.max_entries, 3);
    assert_eq!(stats.max_bytes, 30);
}
