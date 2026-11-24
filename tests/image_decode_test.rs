use std::io::Cursor;
use std::time::Instant;

use discord_rs::protocol::image_decode::{
    decode_and_resize, decode_and_resize_off_thread, DecodeRequest, DecodedImage, ImageDecodeCache,
    MAX_IMAGE_DIMENSION,
};

fn make_rgba_png(width: u32, height: u32) -> Vec<u8> {
    let mut img = image::RgbaImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = ((x * 31 + y * 7) % 255) as u8;
        let g = ((x * 13 + y * 17) % 255) as u8;
        let b = ((x * 3 + y * 29) % 255) as u8;
        *pixel = image::Rgba([r, g, b, 255]);
    }
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode png");
    bytes
}

#[test]
fn decode_and_resize_clamps_to_max_dimension() {
    let bytes = make_rgba_png(5000, 2500);
    let decoded = decode_and_resize(&bytes, None, MAX_IMAGE_DIMENSION).expect("decode");
    assert!(decoded.width <= MAX_IMAGE_DIMENSION);
    assert!(decoded.height <= MAX_IMAGE_DIMENSION);
    assert_eq!(decoded.width, MAX_IMAGE_DIMENSION);
    assert_eq!(decoded.height, MAX_IMAGE_DIMENSION / 2);
    assert_eq!(
        decoded.pixels.len(),
        decoded.width as usize * decoded.height as usize * 4
    );
}

#[test]
fn decode_and_resize_respects_target_dimensions() {
    let bytes = make_rgba_png(640, 480);
    let decoded = decode_and_resize(&bytes, Some((128, 96)), MAX_IMAGE_DIMENSION).expect("decode");
    assert_eq!(decoded.width, 128);
    assert_eq!(decoded.height, 96);
    assert_eq!(decoded.pixels.len(), 128 * 96 * 4);
}

#[tokio::test]
async fn off_thread_decode_matches_sync_decode() {
    let bytes = make_rgba_png(320, 200);
    let sync = decode_and_resize(&bytes, Some((64, 40)), MAX_IMAGE_DIMENSION).expect("sync decode");
    let off_thread = decode_and_resize_off_thread(bytes, Some((64, 40)), MAX_IMAGE_DIMENSION)
        .await
        .expect("off-thread decode");
    assert_eq!(sync.width, off_thread.width);
    assert_eq!(sync.height, off_thread.height);
    assert_eq!(sync.pixels.as_ref(), off_thread.pixels.as_ref());
}

#[tokio::test]
async fn lru_cache_evicts_least_recently_used() {
    // Small cache: 2 entries, enough bytes for 2 tiny images.
    let cache = ImageDecodeCache::new(2, 1024 * 1024);
    let img_a = make_rgba_png(16, 16);
    let img_b = make_rgba_png(16, 16);
    let img_c = make_rgba_png(16, 16);

    let a = cache
        .get_or_decode(DecodeRequest::new("a", img_a).with_target(16, 16))
        .await
        .expect("decode a");
    let _b = cache
        .get_or_decode(DecodeRequest::new("b", img_b).with_target(16, 16))
        .await
        .expect("decode b");

    // Touch A so B becomes LRU.
    let a2 = cache.get("a").await.expect("a cached");
    assert_eq!(a.pixels.as_ref(), a2.pixels.as_ref());

    let _c = cache
        .get_or_decode(DecodeRequest::new("c", img_c).with_target(16, 16))
        .await
        .expect("decode c");

    assert!(cache.get("a").await.is_some(), "A should remain (recently used)");
    assert!(cache.get("c").await.is_some(), "C should be cached");
    assert!(cache.get("b").await.is_none(), "B should be evicted as LRU");
    assert_eq!(cache.len().await, 2);
}

#[tokio::test]
async fn cache_hit_is_fast_enough_for_120fps_budget() {
    // 120fps frame budget is ~8.33ms. Cached lookups must be well under this.
    let cache = ImageDecodeCache::new(64, 64 * 1024 * 1024);
    let bytes = make_rgba_png(256, 256);
    let key = "perf:256x256:64x64";

    let decoded = cache
        .get_or_decode(DecodeRequest::new(key, bytes).with_target(64, 64))
        .await
        .expect("initial decode");
    assert_eq!(decoded.width, 64);
    assert_eq!(decoded.height, 64);

    let iterations = 500usize;
    let start = Instant::now();
    for _ in 0..iterations {
        let hit = cache.get(key).await.expect("cache hit");
        std::hint::black_box(&hit.pixels[0]);
    }
    let elapsed = start.elapsed();
    let per_lookup = elapsed / iterations as u32;

    // Cached retrieval should be far below 8.33ms/frame budget; allow generous CI margin.
    assert!(
        per_lookup.as_micros() < 2_000,
        "cached image lookup too slow for 120fps target: {:?} per lookup ({iterations} iterations, total {:?})",
        per_lookup,
        elapsed
    );
}

#[tokio::test]
async fn get_or_decode_reuses_cached_entry() {
    let cache = ImageDecodeCache::new(8, 8 * 1024 * 1024);
    let bytes = make_rgba_png(128, 128);
    let req1 = DecodeRequest::new("same", bytes.clone()).with_target(32, 32);
    let req2 = DecodeRequest::new("same", bytes).with_target(32, 32);

    let a: std::sync::Arc<DecodedImage> = cache.get_or_decode(req1).await.expect("first");
    let b = cache.get_or_decode(req2).await.expect("second");
    assert!(std::sync::Arc::ptr_eq(&a, &b), "cache should return same Arc for same key");
    assert_eq!(cache.len().await, 1);
}
