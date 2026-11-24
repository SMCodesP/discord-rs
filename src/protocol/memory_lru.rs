//! Generic LRU cache bounded by entry count and byte budget.
//!
//! Designed for long-running sessions where message/media/AST caches must stay
//! within deterministic memory limits. Tracks hit/miss/eviction metrics so
//! callers can report cache effectiveness.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Snapshot of cache metrics and occupancy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryLruStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub bytes: usize,
    pub max_entries: usize,
    pub max_bytes: usize,
}

#[derive(Debug)]
struct Entry<V> {
    value: V,
    bytes: usize,
}

/// Generic LRU cache with byte and entry limits.
///
/// `get`/`get_mut` update recency and hit/miss counters. `peek`/`peek_mut`
/// inspect without affecting recency or metrics.
#[derive(Debug)]
pub struct MemoryLru<K, V> {
    map: HashMap<K, Entry<V>>,
    // front = least recently used, back = most recently used
    order: VecDeque<K>,
    max_entries: usize,
    max_bytes: usize,
    current_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl<K, V> MemoryLru<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Creates a cache with at least 1 entry and 1 byte of capacity.
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
            current_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    /// Returns the cached value and marks it as most recently used.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            self.touch(key);
            self.hits += 1;
            self.map.get(key).map(|entry| &entry.value)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Returns a mutable cached value and marks it as most recently used.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        if self.map.contains_key(key) {
            self.touch(key);
            self.hits += 1;
            self.map.get_mut(key).map(|entry| &mut entry.value)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Returns a cached value without affecting recency or metrics.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|entry| &entry.value)
    }

    /// Returns a mutable cached value without affecting recency or metrics.
    pub fn peek_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key).map(|entry| &mut entry.value)
    }

    /// Returns whether the key is present without affecting recency or metrics.
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Inserts or replaces a value.
    ///
    /// Existing keys are replaced and moved to most-recently-used position.
    /// Replacing a key does not count as eviction.
    ///
    /// If the entry cannot fit within `max_bytes` even after evicting other
    /// entries, it is not inserted and any previous value for the key is
    /// removed. The returned value is the previous value for that key, if any.
    pub fn put(&mut self, key: K, value: V, bytes: usize) -> Option<V> {
        let previous = self.remove_entry(&key);

        if bytes > self.max_bytes {
            // Entry can never fit; keep cache within budget.
            self.evict_until_within(self.max_entries, self.max_bytes);
            return previous;
        }

        self.map.insert(key.clone(), Entry { value, bytes });
        self.order.push_back(key);
        self.current_bytes = self.current_bytes.saturating_add(bytes);
        self.evict_until_within(self.max_entries, self.max_bytes);
        previous
    }

    /// Removes a key, returning its value if present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.remove_entry(key)
    }

    /// Clears all entries and bytes, preserving limits and metrics.
    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.current_bytes = 0;
    }

    /// Clears entries, bytes, and metrics.
    pub fn reset(&mut self) {
        self.clear();
        self.hits = 0;
        self.misses = 0;
        self.evictions = 0;
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    pub fn stats(&self) -> MemoryLruStats {
        MemoryLruStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            entries: self.map.len(),
            bytes: self.current_bytes,
            max_entries: self.max_entries,
            max_bytes: self.max_bytes,
        }
    }

    /// Iterates keys from least-recently-used to most-recently-used.
    pub fn keys_lru_to_mru(&self) -> impl Iterator<Item = &K> {
        self.order.iter()
    }

    fn touch(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(pos);
            self.order.push_back(key.clone());
        }
    }

    fn remove_entry(&mut self, key: &K) -> Option<V> {
        let entry = self.map.remove(key)?;
        self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
        if let Some(pos) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(pos);
        }
        Some(entry.value)
    }

    fn evict_until_within(&mut self, max_entries: usize, max_bytes: usize) {
        while self.map.len() > max_entries || self.current_bytes > max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                self.current_bytes = 0;
                break;
            };
            if let Some(entry) = self.map.remove(&oldest) {
                self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
                self.evictions += 1;
            }
        }
    }
}

impl<K, V> Default for MemoryLru<K, V>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new(1024, 64 * 1024 * 1024)
    }
}
