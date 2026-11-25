//! O(1) LRU cache for parsed Markdown AST, keyed by `(message_id, content_hash)`.
//!
//! This eliminates re-parsing message markdown on every frame during 120 FPS
//! chat rendering. The cache stores parsed `MarkdownBlock` ASTs and evicts the
//! least-recently-used entry when capacity is exceeded.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};

use super::markdown::{parse_markdown, MarkdownBlock};

/// Default maximum number of cached message markdown entries.
pub const DEFAULT_MARKDOWN_CACHE_CAPACITY: usize = 2048;

/// Cache key: message identity plus content identity.
/// `content_hash` must change whenever message content changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarkdownCacheKey {
    pub message_id: String,
    pub content_hash: u64,
}

impl MarkdownCacheKey {
    pub fn new(message_id: impl Into<String>, content_hash: u64) -> Self {
        Self {
            message_id: message_id.into(),
            content_hash,
        }
    }
}

/// Compute a stable content hash for markdown cache keys.
pub fn content_hash(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone)]
struct Node {
    key: MarkdownCacheKey,
    value: Vec<MarkdownBlock>,
    prev: Option<usize>,
    next: Option<usize>,
}

/// Small O(1) LRU cache implemented with a HashMap index + slab doubly-linked list.
/// Most-recently-used entries are at the front; least-recently-used at the back.
#[derive(Debug)]
pub struct MarkdownLruCache {
    map: HashMap<MarkdownCacheKey, usize>,
    nodes: Vec<Option<Node>>,
    free: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    capacity: usize,
    hits: u64,
    misses: u64,
    inserts: u64,
    evictions: u64,
}

impl MarkdownLruCache {
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            map: HashMap::with_capacity(capacity),
            nodes: Vec::with_capacity(capacity),
            free: Vec::new(),
            head: None,
            tail: None,
            capacity,
            hits: 0,
            misses: 0,
            inserts: 0,
            evictions: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    #[inline]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    #[inline]
    pub fn inserts(&self) -> u64 {
        self.inserts
    }

    #[inline]
    pub fn evictions(&self) -> u64 {
        self.evictions
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.nodes.clear();
        self.free.clear();
        self.head = None;
        self.tail = None;
    }

    /// Returns a cloned AST for the key and marks it as most-recently-used.
    pub fn get_cloned(&mut self, key: &MarkdownCacheKey) -> Option<Vec<MarkdownBlock>> {
        let idx = match self.map.get(key).copied() {
            Some(idx) => idx,
            None => {
                self.misses += 1;
                return None;
            }
        };
        self.hits += 1;
        self.move_to_front(idx);
        self.nodes[idx].as_ref().map(|n| n.value.clone())
    }

    /// Returns true when key exists. Does not update recency/counters.
    pub fn contains(&self, key: &MarkdownCacheKey) -> bool {
        self.map.contains_key(key)
    }

    /// Inserts or updates an entry. Updated/existing entries become MRU.
    pub fn put(&mut self, key: MarkdownCacheKey, value: Vec<MarkdownBlock>) {
        if let Some(&idx) = self.map.get(&key) {
            if let Some(node) = self.nodes[idx].as_mut() {
                node.value = value;
            }
            self.move_to_front(idx);
            return;
        }

        self.evict_if_needed();
        let idx = self.alloc_node(key.clone(), value);
        self.map.insert(key, idx);
        self.push_front(idx);
        self.inserts += 1;
    }

    /// Gets parsed markdown from cache, or parses and caches on miss.
    pub fn get_or_parse(&mut self, message_id: &str, content: &str) -> Vec<MarkdownBlock> {
        let key = MarkdownCacheKey::new(message_id, content_hash(content));
        if let Some(cached) = self.get_cloned(&key) {
            return cached;
        }
        let parsed = parse_markdown(content);
        self.put(key, parsed.clone());
        parsed
    }

    fn evict_if_needed(&mut self) {
        while self.map.len() >= self.capacity {
            let Some(tail_idx) = self.tail else {
                break;
            };
            let key = self.nodes[tail_idx].as_ref().map(|n| n.key.clone());
            self.detach(tail_idx);
            if let Some(key) = key {
                self.map.remove(&key);
            }
            self.nodes[tail_idx] = None;
            self.free.push(tail_idx);
            self.evictions += 1;
        }
    }

    fn alloc_node(&mut self, key: MarkdownCacheKey, value: Vec<MarkdownBlock>) -> usize {
        let node = Node {
            key,
            value,
            prev: None,
            next: None,
        };
        if let Some(idx) = self.free.pop() {
            self.nodes[idx] = Some(node);
            idx
        } else {
            let idx = self.nodes.len();
            self.nodes.push(Some(node));
            idx
        }
    }

    fn push_front(&mut self, idx: usize) {
        if let Some(node) = self.nodes[idx].as_mut() {
            node.prev = None;
            node.next = self.head;
        }
        if let Some(old_head) = self.head {
            if let Some(h) = self.nodes[old_head].as_mut() {
                h.prev = Some(idx);
            }
        } else {
            self.tail = Some(idx);
        }
        self.head = Some(idx);
    }

    fn detach(&mut self, idx: usize) {
        let (prev, next) = match self.nodes[idx].as_ref() {
            Some(n) => (n.prev, n.next),
            None => return,
        };
        if let Some(p) = prev {
            if let Some(pn) = self.nodes[p].as_mut() {
                pn.next = next;
            }
        } else {
            self.head = next;
        }
        if let Some(n) = next {
            if let Some(nn) = self.nodes[n].as_mut() {
                nn.prev = prev;
            }
        } else {
            self.tail = prev;
        }
        if let Some(node) = self.nodes[idx].as_mut() {
            node.prev = None;
            node.next = None;
        }
    }

    fn move_to_front(&mut self, idx: usize) {
        if self.head == Some(idx) {
            return;
        }
        self.detach(idx);
        self.push_front(idx);
    }
}

static MESSAGE_MARKDOWN_CACHE: LazyLock<Mutex<MarkdownLruCache>> = LazyLock::new(|| {
    Mutex::new(MarkdownLruCache::with_capacity(
        DEFAULT_MARKDOWN_CACHE_CAPACITY,
    ))
});

fn with_global_cache<T>(f: impl FnOnce(&mut MarkdownLruCache) -> T) -> T {
    match MESSAGE_MARKDOWN_CACHE.lock() {
        Ok(mut cache) => f(&mut cache),
        Err(poisoned) => {
            let mut cache = poisoned.into_inner();
            f(&mut cache)
        }
    }
}

/// Returns cached parsed markdown for `(message_id, content_hash)`, or parses and caches it.
pub fn get_or_parse_message_markdown(message_id: &str, content: &str) -> Vec<MarkdownBlock> {
    if content.trim().is_empty() {
        return Vec::new();
    }
    with_global_cache(|cache| cache.get_or_parse(message_id, content))
}

/// Clears the global message markdown cache. Intended for tests.
pub fn clear_message_markdown_cache() {
    with_global_cache(|cache| cache.clear());
}

/// Returns `(len, capacity, hits, misses, inserts, evictions)` for the global cache.
pub fn message_markdown_cache_stats() -> (usize, usize, u64, u64, u64, u64) {
    with_global_cache(|cache| {
        (
            cache.len(),
            cache.capacity(),
            cache.hits(),
            cache.misses(),
            cache.inserts(),
            cache.evictions(),
        )
    })
}
