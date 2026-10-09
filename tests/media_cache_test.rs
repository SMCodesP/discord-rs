use chrono::Utc;
use discord_rs::protocol::media_cache::{MediaCacheManager, StickerFormatType};
use std::fs;

#[test]
fn test_media_cache_cas_sharding() {
    let temp_dir = std::env::temp_dir().join(format!(
        "discord_rs_cache_test_{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let mgr = MediaCacheManager::with_root(&temp_dir);

    let test_bytes = b"discord-rs media cache CAS test content";
    let url = "https://cdn.discordapp.com/emojis/123456.png";
    let meta = mgr.store_media_for_url(url, test_bytes, "image/png").unwrap();

    assert_eq!(meta.size_bytes, test_bytes.len());
    assert_eq!(meta.url, url);

    let loaded = mgr.load_media_by_url(url).expect("Media should be in cache");
    assert_eq!(loaded, test_bytes);

    let (bin_path, meta_path) = mgr.asset_paths(&meta.sha256_hash);
    assert!(bin_path.exists());
    assert!(meta_path.exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_recent_emojis_recording() {
    let temp_dir = std::env::temp_dir().join(format!(
        "discord_rs_recents_test_{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let mgr = MediaCacheManager::with_root(&temp_dir);

    mgr.record_emoji_use("🦀", "ferris", false, None).await;
    mgr.record_emoji_use("🦀", "ferris", false, None).await;

    let recents = mgr.get_recent_emojis().await;
    let ferris = recents.iter().find(|i| i.emoji == "🦀").expect("Ferris must be in recents");
    assert!(ferris.use_count >= 2);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_soundboard_defaults() {
    let sounds = MediaCacheManager::get_default_soundboard_sounds();
    assert!(!sounds.is_empty());
    let names: Vec<&str> = sounds.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Quack"));
    assert!(names.contains(&"Airhorn"));
    assert!(names.contains(&"Cricket"));
    assert!(names.contains(&"Applause"));
}

#[test]
fn test_tenor_gif_categories() {
    let trending = MediaCacheManager::get_curated_tenor_gifs_for_category("trending");
    assert!(!trending.is_empty());

    let anime = MediaCacheManager::get_curated_tenor_gifs_for_category("anime");
    assert!(!anime.is_empty());
    assert!(anime.iter().any(|g| g.category.as_deref() == Some("anime")));

    let reactions = MediaCacheManager::get_curated_tenor_gifs_for_category("reactions");
    assert!(!reactions.is_empty());
    assert!(reactions.iter().any(|g| g.category.as_deref() == Some("reactions")));

    let memes = MediaCacheManager::get_curated_tenor_gifs_for_category("memes");
    assert!(!memes.is_empty());
    assert!(memes.iter().any(|g| g.category.as_deref() == Some("memes")));
}

#[test]
fn test_sticker_packs_defaults() {
    let packs = MediaCacheManager::get_default_sticker_packs();
    assert!(packs.len() >= 3);
    assert!(packs.iter().any(|p| p.name == "Wumpus Adventures"));
    assert!(packs.iter().any(|p| p.name == "Clyde Classic"));
    assert!(packs.iter().any(|p| p.name == "Nelly the Hamster"));

    let wumpus_pack = packs.iter().find(|p| p.name == "Wumpus Adventures").unwrap();
    assert!(wumpus_pack.stickers.len() >= 4);
    assert_eq!(wumpus_pack.stickers[0].format_type, StickerFormatType::Png);
}
