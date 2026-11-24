use discord_rs::protocol::cdn::{
    avatar, avatar_url, banner, banner_url, custom_emoji_url, decoration, decoration_url, emoji,
    emoji_url, guild_icon, guild_icon_url, hash_extension, is_animated_hash, sticker, sticker_url,
    sticker_url_animated, CdnUrl, DEFAULT_AVATAR_SIZE, DEFAULT_BANNER_SIZE, DEFAULT_DECORATION_SIZE,
    DEFAULT_EMOJI_SIZE, DEFAULT_GUILD_ICON_SIZE, DEFAULT_STICKER_SIZE,
};
use discord_rs::protocol::media_cache::StickerFormatType;

const GUILD_ID: &str = "123456789012345678";
const USER_ID: &str = "987654321098765432";
const STATIC_HASH: &str = "0a1b2c3d4e5f60718293a4b5c6d7e8f9";
const ANIMATED_HASH: &str = "a_0a1b2c3d4e5f60718293a4b5c6d7e8f9";

#[test]
fn guild_icon_static_animated_and_sized() {
    assert_eq!(DEFAULT_GUILD_ICON_SIZE, 96);
    assert_eq!(
        guild_icon(GUILD_ID, Some(STATIC_HASH)).as_deref(),
        Some(
            "https://cdn.discordapp.com/icons/123456789012345678/0a1b2c3d4e5f60718293a4b5c6d7e8f9.png?size=96"
        )
    );
    assert_eq!(
        guild_icon(GUILD_ID, Some(ANIMATED_HASH)).as_deref(),
        Some(
            "https://cdn.discordapp.com/icons/123456789012345678/a_0a1b2c3d4e5f60718293a4b5c6d7e8f9.gif?size=96"
        )
    );
    assert_eq!(
        guild_icon_url(GUILD_ID, Some(STATIC_HASH), 128).as_deref(),
        Some(
            "https://cdn.discordapp.com/icons/123456789012345678/0a1b2c3d4e5f60718293a4b5c6d7e8f9.png?size=128"
        )
    );
    assert_eq!(
        CdnUrl::guild_icon(GUILD_ID, Some(ANIMATED_HASH), 64).as_deref(),
        Some(
            "https://cdn.discordapp.com/icons/123456789012345678/a_0a1b2c3d4e5f60718293a4b5c6d7e8f9.gif?size=64"
        )
    );
}

#[test]
fn guild_icon_accepts_full_urls_from_icon_or_icon_hash() {
    let full_url = "https://cdn.discordapp.com/icons/42/abcdef.webp?size=240";
    assert_eq!(guild_icon(GUILD_ID, Some(full_url)).as_deref(), Some(full_url));
    assert_eq!(
        guild_icon_url(GUILD_ID, Some("  https://cdn.discordapp.com/icons/42/abcdef.webp?size=240  "), 96)
            .as_deref(),
        Some(full_url)
    );
    assert_eq!(
        guild_icon("", Some(full_url)).as_deref(),
        Some(full_url)
    );
}

#[test]
fn guild_icon_missing_or_blank_falls_back_cleanly() {
    assert_eq!(guild_icon(GUILD_ID, None), None);
    assert_eq!(guild_icon(GUILD_ID, Some("")), None);
    assert_eq!(guild_icon(GUILD_ID, Some("   ")), None);
    assert_eq!(guild_icon("", Some(STATIC_HASH)), None);
    assert_eq!(guild_icon("   ", Some(STATIC_HASH)), None);
}

#[test]
fn guild_icon_hash_extensions_are_normalized() {
    assert_eq!(
        guild_icon(GUILD_ID, Some("abc123.png")).as_deref(),
        Some(
            "https://cdn.discordapp.com/icons/123456789012345678/abc123.png?size=96"
        )
    );
    assert_eq!(
        guild_icon(GUILD_ID, Some("a_abc123.PNG")).as_deref(),
        Some(
            "https://cdn.discordapp.com/icons/123456789012345678/a_abc123.gif?size=96"
        )
    );
}

#[test]
fn avatar_static_animated_full_url_and_missing() {
    assert_eq!(DEFAULT_AVATAR_SIZE, 96);
    assert_eq!(
        avatar(USER_ID, Some("deadbeef")).as_deref(),
        Some(
            "https://cdn.discordapp.com/avatars/987654321098765432/deadbeef.png?size=96"
        )
    );
    assert_eq!(
        avatar(USER_ID, Some("a_deadbeef")).as_deref(),
        Some(
            "https://cdn.discordapp.com/avatars/987654321098765432/a_deadbeef.gif?size=96"
        )
    );
    assert_eq!(
        avatar_url(USER_ID, Some("a_deadbeef"), 256).as_deref(),
        Some(
            "https://cdn.discordapp.com/avatars/987654321098765432/a_deadbeef.gif?size=256"
        )
    );
    let full_url = "https://cdn.discordapp.com/avatars/987654321098765432/full.webp?size=128";
    assert_eq!(avatar(USER_ID, Some(full_url)).as_deref(), Some(full_url));
    assert_eq!(avatar(USER_ID, None), None);
    assert_eq!(avatar(USER_ID, Some("  ")), None);
    assert_eq!(avatar("", Some("deadbeef")), None);
}

#[test]
fn banner_static_animated_full_url_and_missing() {
    assert_eq!(DEFAULT_BANNER_SIZE, 600);
    assert_eq!(
        banner(USER_ID, Some("cafebabe")).as_deref(),
        Some(
            "https://cdn.discordapp.com/banners/987654321098765432/cafebabe.png?size=600"
        )
    );
    assert_eq!(
        banner(USER_ID, Some("a_cafebabe")).as_deref(),
        Some(
            "https://cdn.discordapp.com/banners/987654321098765432/a_cafebabe.gif?size=600"
        )
    );
    assert_eq!(
        banner_url(USER_ID, Some("cafebabe"), 480).as_deref(),
        Some(
            "https://cdn.discordapp.com/banners/987654321098765432/cafebabe.png?size=480"
        )
    );
    let full_url = "https://cdn.discordapp.com/banners/987654321098765432/full.png?size=600";
    assert_eq!(banner(USER_ID, Some(full_url)).as_deref(), Some(full_url));
    assert_eq!(banner(USER_ID, None), None);
    assert_eq!(banner(USER_ID, Some("")), None);
}

#[test]
fn emoji_static_animated_and_full_url() {
    assert_eq!(DEFAULT_EMOJI_SIZE, 64);
    assert_eq!(
        emoji("112233445566778899", false).as_deref(),
        Some("https://cdn.discordapp.com/emojis/112233445566778899.png?size=64&quality=lossless")
    );
    assert_eq!(
        emoji("112233445566778899", true).as_deref(),
        Some("https://cdn.discordapp.com/emojis/112233445566778899.gif?size=64&quality=lossless")
    );
    assert_eq!(
        emoji_url("a_112233445566778899", false, 48).as_deref(),
        Some("https://cdn.discordapp.com/emojis/a_112233445566778899.gif?size=48&quality=lossless")
    );
    let full_url = "https://cdn.discordapp.com/emojis/112233445566778899.webp?size=96";
    assert_eq!(emoji(full_url, false).as_deref(), Some(full_url));
    assert_eq!(emoji("", false), None);
    assert_eq!(custom_emoji_url(None, false, 64), None);
    assert_eq!(
        custom_emoji_url(Some("112233445566778899"), true, 64).as_deref(),
        Some("https://cdn.discordapp.com/emojis/112233445566778899.gif?size=64&quality=lossless")
    );
}

#[test]
fn sticker_formats_full_url_and_missing() {
    assert_eq!(DEFAULT_STICKER_SIZE, 160);
    assert_eq!(
        sticker("555666777888999000", StickerFormatType::Png).as_deref(),
        Some("https://media.discordapp.net/stickers/555666777888999000.webp?size=160&quality=lossless")
    );
    assert_eq!(
        sticker("555666777888999000", StickerFormatType::Apng).as_deref(),
        Some("https://media.discordapp.net/stickers/555666777888999000.gif?size=160")
    );
    assert_eq!(
        sticker("555666777888999000", StickerFormatType::Gif).as_deref(),
        Some("https://media.discordapp.net/stickers/555666777888999000.gif?size=160")
    );
    assert_eq!(
        sticker("555666777888999000", StickerFormatType::Lottie).as_deref(),
        Some("https://media.discordapp.net/stickers/555666777888999000.json")
    );
    assert_eq!(
        sticker_url("555666777888999000", StickerFormatType::Gif, 320).as_deref(),
        Some("https://media.discordapp.net/stickers/555666777888999000.gif?size=320")
    );
    assert_eq!(
        sticker_url_animated("555666777888999000", true, 160).as_deref(),
        Some("https://media.discordapp.net/stickers/555666777888999000.gif?size=160")
    );
    let full_url = "https://media.discordapp.net/stickers/555666777888999000.png?size=160";
    assert_eq!(sticker(full_url, StickerFormatType::Png).as_deref(), Some(full_url));
    assert_eq!(sticker("", StickerFormatType::Png), None);
}

#[test]
fn decoration_static_animated_apng_full_url_and_missing() {
    assert_eq!(DEFAULT_DECORATION_SIZE, 96);
    assert_eq!(
        decoration(Some("b_ring")).as_deref(),
        Some("https://cdn.discordapp.com/avatar-decoration-presets/b_ring.png?size=96&passthrough=true")
    );
    assert_eq!(
        decoration(Some("a_fire_ring")).as_deref(),
        Some("https://cdn.discordapp.com/avatar-decoration-presets/a_fire_ring.png?size=96&passthrough=true")
    );
    assert_eq!(
        decoration_url(Some("a_fire_ring"), 64).as_deref(),
        Some("https://cdn.discordapp.com/avatar-decoration-presets/a_fire_ring.png?size=64&passthrough=true")
    );
    let full_url = "https://cdn.discordapp.com/avatar-decoration-presets/full.png?size=96&passthrough=true";
    assert_eq!(decoration(Some(full_url)).as_deref(), Some(full_url));
    assert_eq!(decoration(None), None);
    assert_eq!(decoration(Some("  ")), None);
    assert_eq!(decoration_url(Some(""), 96), None);
}

#[test]
fn hash_helpers_match_discord_animation_rule() {
    assert!(is_animated_hash(ANIMATED_HASH));
    assert!(!is_animated_hash(STATIC_HASH));
    assert_eq!(hash_extension(ANIMATED_HASH), "gif");
    assert_eq!(hash_extension(STATIC_HASH), "png");
}

#[test]
fn cdn_namespace_matches_free_functions() {
    assert_eq!(
        CdnUrl::avatar(USER_ID, Some("a_deadbeef"), 96),
        avatar(USER_ID, Some("a_deadbeef"))
    );
    assert_eq!(
        CdnUrl::banner(USER_ID, Some("cafebabe"), 600),
        banner(USER_ID, Some("cafebabe"))
    );
    assert_eq!(
        CdnUrl::emoji("112233445566778899", true, 64),
        emoji("112233445566778899", true)
    );
    assert_eq!(
        CdnUrl::sticker("555666777888999000", StickerFormatType::Gif, 160),
        sticker("555666777888999000", StickerFormatType::Gif)
    );
    assert_eq!(
        CdnUrl::decoration(Some("a_fire_ring"), 96),
        decoration(Some("a_fire_ring"))
    );
}
