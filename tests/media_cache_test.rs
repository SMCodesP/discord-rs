use chrono::Utc;
use discord_rs::protocol::media_cache::MediaCacheManager;
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

    // Sem histórico de uso, a lista deve vir vazia (zero mocks)
    let initial = mgr.get_recent_emojis().await;
    assert!(initial.is_empty(), "Sem histórico de uso, recentes deve vir vazio");

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
    assert!(
        sounds.is_empty(),
        "Sem resposta da rede/API, lista de sons deve vir vazia (zero mocks)"
    );
}

#[test]
fn test_tenor_gif_categories() {
    let trending = MediaCacheManager::get_curated_tenor_gifs_for_category("trending");
    assert!(
        trending.is_empty(),
        "Sem rede/resposta real da API, lista de GIFs deve vir vazia (zero mocks)"
    );

    let anime = MediaCacheManager::get_curated_tenor_gifs_for_category("anime");
    assert!(anime.is_empty());

    let reactions = MediaCacheManager::get_curated_tenor_gifs_for_category("reactions");
    assert!(reactions.is_empty());

    let memes = MediaCacheManager::get_curated_tenor_gifs_for_category("memes");
    assert!(memes.is_empty());
}

#[test]
fn test_sticker_packs_defaults() {
    let packs = MediaCacheManager::get_default_sticker_packs();
    assert!(
        packs.is_empty(),
        "Sem resposta da rede/API, pacotes de figurinhas devem vir vazios (zero mocks)"
    );
}
