//! Off-thread image decoding, resizing, and LRU caching for UI rendering.
//!
//! Keeps decode/resize work off the UI thread and caches decoded RGBA frames so
//! image-heavy surfaces can sustain 120fps without repeated decode work.

use anyhow::{Context, Result};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Hard upper bound for decoded image dimensions.
/// Images larger than this in either axis are downscaled before caching.
pub const MAX_IMAGE_DIMENSION: u32 = 4096;

/// Default maximum number of decoded images kept in memory.
pub const DEFAULT_CACHE_MAX_ENTRIES: usize = 256;

/// Default maximum decoded pixel bytes kept in memory (~128 MiB of RGBA).
pub const DEFAULT_CACHE_MAX_BYTES: usize = 128 * 1024 * 1024;

/// Fully decoded image ready for GPU upload/UI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// RGBA8 pixels, row-major, len == width * height * 4.
    pub pixels: Arc<[u8]>,
}

impl DecodedImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .context("decoded image dimensions overflow")?;
        anyhow::ensure!(
            pixels.len() == expected,
            "decoded image byte length mismatch: expected {expected}, got {}",
            pixels.len()
        );
        Ok(Self {
            width,
            height,
            pixels: Arc::from(pixels.into_boxed_slice()),
        })
    }

    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }
}

/// Request parameters for decode + resize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeRequest {
    /// Cache key. Callers should include content identity and target size when relevant.
    pub key: String,
    pub bytes: Vec<u8>,
    /// Optional target size. If omitted, image is decoded at original size (clamped by max dimension).
    pub target: Option<(u32, u32)>,
    /// Maximum allowed dimension for either axis after resize/clamp.
    pub max_dimension: u32,
}

impl DecodeRequest {
    pub fn new(key: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            key: key.into(),
            bytes,
            target: None,
            max_dimension: MAX_IMAGE_DIMENSION,
        }
    }

    pub fn with_target(mut self, width: u32, height: u32) -> Self {
        self.target = Some((width.max(1), height.max(1)));
        self
    }

    pub fn with_max_dimension(mut self, max_dimension: u32) -> Self {
        self.max_dimension = max_dimension.max(1);
        self
    }
}

/// Synchronous decode + resize. Intended to run on a blocking worker thread.
pub fn decode_and_resize(
    bytes: &[u8],
    target: Option<(u32, u32)>,
    max_dimension: u32,
) -> Result<DecodedImage> {
    let max_dimension = max_dimension.max(1);
    let img = image::load_from_memory(bytes).context("failed to decode image bytes")?;
    let rgba = img.to_rgba8();
    let (src_w, src_h) = rgba.dimensions();

    let (mut dst_w, mut dst_h) = if let Some((tw, th)) = target {
        clamp_dimensions(tw.max(1), th.max(1), max_dimension)
    } else {
        clamp_dimensions(src_w.max(1), src_h.max(1), max_dimension)
    };

    // Avoid zero-sized outputs.
    dst_w = dst_w.max(1);
    dst_h = dst_h.max(1);

    let pixels = if dst_w == src_w && dst_h == src_h {
        rgba.into_raw()
    } else {
        let resized = image::imageops::resize(
            &rgba,
            dst_w,
            dst_h,
            image::imageops::FilterType::Triangle,
        );
        let (rw, rh) = resized.dimensions();
        dst_w = rw;
        dst_h = rh;
        resized.into_raw()
    };

    DecodedImage::new(dst_w, dst_h, pixels)
}

fn clamp_dimensions(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let max_dimension = max_dimension.max(1);
    let w = width.max(1);
    let h = height.max(1);
    let max_side = w.max(h);
    if max_side <= max_dimension {
        return (w, h);
    }

    let scale = max_dimension as f64 / max_side as f64;
    let mut nw = ((w as f64) * scale).round() as u32;
    let mut nh = ((h as f64) * scale).round() as u32;
    nw = nw.clamp(1, max_dimension);
    nh = nh.clamp(1, max_dimension);
    (nw, nh)
}

struct CacheEntry {
    image: Arc<DecodedImage>,
    bytes: usize,
}

/// LRU cache for decoded images. Bounded by entry count and decoded byte size.
pub struct ImageDecodeCache {
    entries: RwLock<ImageDecodeCacheState>,
}

struct ImageDecodeCacheState {
    map: HashMap<String, CacheEntry>,
    order: VecDeque<String>, // front = least recently used, back = most recently used
    max_entries: usize,
    max_bytes: usize,
    current_bytes: usize,
}

impl std::fmt::Debug for ImageDecodeCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageDecodeCache").finish_non_exhaustive()
    }
}

impl Default for ImageDecodeCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_MAX_ENTRIES, DEFAULT_CACHE_MAX_BYTES)
    }
}

impl ImageDecodeCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: RwLock::new(ImageDecodeCacheState {
                map: HashMap::new(),
                order: VecDeque::new(),
                max_entries: max_entries.max(1),
                max_bytes: max_bytes.max(1),
                current_bytes: 0,
            }),
        }
    }

    pub async fn get(&self, key: &str) -> Option<Arc<DecodedImage>> {
        let mut state = self.entries.write().await;
        if let Some(entry) = state.map.get(key) {
            let image = Arc::clone(&entry.image);
            touch_key(&mut state.order, key);
            Some(image)
        } else {
            None
        }
    }

    pub async fn put(&self, key: String, image: Arc<DecodedImage>) {
        let mut state = self.entries.write().await;
        insert_entry(&mut state, key, image);
    }

    pub async fn len(&self) -> usize {
        self.entries.read().await.map.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    pub async fn total_bytes(&self) -> usize {
        self.entries.read().await.current_bytes
    }

    pub async fn clear(&self) {
        let mut state = self.entries.write().await;
        state.map.clear();
        state.order.clear();
        state.current_bytes = 0;
    }

    /// Returns cached image if present; otherwise decodes/resizes on a blocking
    /// worker thread, stores the result, and returns it.
    pub async fn get_or_decode(&self, request: DecodeRequest) -> Result<Arc<DecodedImage>> {
        if let Some(existing) = self.get(&request.key).await {
            return Ok(existing);
        }

        let key = request.key.clone();
        let bytes = request.bytes;
        let target = request.target;
        let max_dimension = request.max_dimension;

        let decoded = tokio::task::spawn_blocking(move || {
            decode_and_resize(&bytes, target, max_dimension)
        })
        .await
        .context("image decode worker panicked or was cancelled")??;

        let image = Arc::new(decoded);

        let mut state = self.entries.write().await;
        if let Some(existing) = state.map.get(&key) {
            let existing = Arc::clone(&existing.image);
            touch_key(&mut state.order, &key);
            return Ok(existing);
        }
        insert_entry(&mut state, key, Arc::clone(&image));
        Ok(image)
    }
}

fn touch_key(order: &mut VecDeque<String>, key: &str) {
    if let Some(pos) = order.iter().position(|k| k == key) {
        order.remove(pos);
    }
    order.push_back(key.to_string());
}

fn insert_entry(state: &mut ImageDecodeCacheState, key: String, image: Arc<DecodedImage>) {
    let bytes = image.byte_len();

    if let Some(old) = state.map.remove(&key) {
        state.current_bytes = state.current_bytes.saturating_sub(old.bytes);
        if let Some(pos) = state.order.iter().position(|k| k == &key) {
            state.order.remove(pos);
        }
    }

    // If a single image is larger than max_bytes, keep only it if it fits max_entries==1? 
    // Prefer not caching impossible entries; but caller still gets decoded result from get_or_decode.
    if bytes > state.max_bytes {
        evict_until_within(state, state.max_entries.saturating_sub(1), state.max_bytes);
        return;
    }

    state.map.insert(key.clone(), CacheEntry { image, bytes });
    state.order.push_back(key);
    state.current_bytes = state.current_bytes.saturating_add(bytes);

    evict_until_within(state, state.max_entries, state.max_bytes);
}

fn evict_until_within(state: &mut ImageDecodeCacheState, max_entries: usize, max_bytes: usize) {
    while state.map.len() > max_entries || state.current_bytes > max_bytes {
        let Some(oldest) = state.order.pop_front() else {
            break;
        };
        if let Some(entry) = state.map.remove(&oldest) {
            state.current_bytes = state.current_bytes.saturating_sub(entry.bytes);
        }
    }
}

/// Convenience async wrapper for one-off off-thread decode without cache.
pub async fn decode_and_resize_off_thread(
    bytes: Vec<u8>,
    target: Option<(u32, u32)>,
    max_dimension: u32,
) -> Result<DecodedImage> {
    tokio::task::spawn_blocking(move || decode_and_resize(&bytes, target, max_dimension))
        .await
        .context("image decode worker panicked or was cancelled")?
}
