use crate::protocol::media_cache::StickerFormatType;

pub const CDN_BASE: &str = "https://cdn.discordapp.com";
pub const MEDIA_BASE: &str = "https://media.discordapp.net";

pub const DEFAULT_GUILD_ICON_SIZE: u16 = 96;
pub const DEFAULT_AVATAR_SIZE: u16 = 96;
pub const DEFAULT_BANNER_SIZE: u16 = 600;
pub const DEFAULT_EMOJI_SIZE: u16 = 64;
pub const DEFAULT_STICKER_SIZE: u16 = 160;
pub const DEFAULT_DECORATION_SIZE: u16 = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Gif,
    Webp,
    Json,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Json => "json",
        }
    }
}

pub fn is_animated_hash(hash: &str) -> bool {
    hash.starts_with("a_")
}

pub fn hash_extension(hash: &str) -> &'static str {
    if is_animated_hash(hash) {
        "gif"
    } else {
        "png"
    }
}

fn clean(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn clean_required(value: &str) -> Option<&str> {
    clean(Some(value))
}

fn direct_url(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.starts_with("file:")
    {
        Some(value.to_string())
    } else {
        None
    }
}

fn strip_known_extension(hash: &str) -> &str {
    for extension in [".png", ".gif", ".webp", ".jpg", ".jpeg", ".avif"] {
        if hash.len() > extension.len() && hash.to_ascii_lowercase().ends_with(extension) {
            return &hash[..hash.len() - extension.len()];
        }
    }
    hash
}

fn hash_asset_url(prefix: &str, id: &str, asset: Option<&str>, size: u16) -> Option<String> {
    let asset = clean(asset)?;
    if let Some(url) = direct_url(asset) {
        return Some(url);
    }
    let id = clean_required(id)?;
    let hash = strip_known_extension(asset);
    Some(format!(
        "{CDN_BASE}/{prefix}/{id}/{hash}.{}?size={size}",
        hash_extension(hash)
    ))
}

pub fn guild_icon_url(guild_id: &str, icon: Option<&str>, size: u16) -> Option<String> {
    hash_asset_url("icons", guild_id, icon, size)
}

pub fn guild_icon(guild_id: &str, icon: Option<&str>) -> Option<String> {
    guild_icon_url(guild_id, icon, DEFAULT_GUILD_ICON_SIZE)
}

pub fn avatar_url(user_id: &str, avatar: Option<&str>, size: u16) -> Option<String> {
    hash_asset_url("avatars", user_id, avatar, size)
}

pub fn avatar(user_id: &str, avatar: Option<&str>) -> Option<String> {
    avatar_url(user_id, avatar, DEFAULT_AVATAR_SIZE)
}

pub fn banner_url(user_id: &str, banner: Option<&str>, size: u16) -> Option<String> {
    hash_asset_url("banners", user_id, banner, size)
}

pub fn banner(user_id: &str, banner: Option<&str>) -> Option<String> {
    banner_url(user_id, banner, DEFAULT_BANNER_SIZE)
}

pub fn emoji_url(emoji_id: &str, animated: bool, size: u16) -> Option<String> {
    let emoji_id = clean_required(emoji_id)?;
    if let Some(url) = direct_url(emoji_id) {
        return Some(url);
    }
    let emoji_id = strip_known_extension(emoji_id);
    let extension = if animated || is_animated_hash(emoji_id) {
        ImageFormat::Gif
    } else {
        ImageFormat::Png
    };
    Some(format!(
        "{CDN_BASE}/emojis/{emoji_id}.{}?size={size}&quality=lossless",
        extension.extension()
    ))
}

pub fn emoji(emoji_id: &str, animated: bool) -> Option<String> {
    emoji_url(emoji_id, animated, DEFAULT_EMOJI_SIZE)
}

pub fn custom_emoji_url(emoji_id: Option<&str>, animated: bool, size: u16) -> Option<String> {
    emoji_url(clean(emoji_id)?, animated, size)
}

pub fn sticker_url(sticker_id: &str, format: StickerFormatType, size: u16) -> Option<String> {
    let sticker_id = clean_required(sticker_id)?;
    if let Some(url) = direct_url(sticker_id) {
        return Some(url);
    }
    let sticker_id = strip_known_extension(sticker_id);
    let suffix = match format {
        StickerFormatType::Apng | StickerFormatType::Gif => format!("gif?size={size}"),
        StickerFormatType::Png => format!("webp?size={size}&quality=lossless"),
        StickerFormatType::Lottie => "json".to_string(),
    };
    Some(format!("{MEDIA_BASE}/stickers/{sticker_id}.{suffix}"))
}

pub fn sticker(sticker_id: &str, format: StickerFormatType) -> Option<String> {
    sticker_url(sticker_id, format, DEFAULT_STICKER_SIZE)
}

pub fn sticker_url_animated(sticker_id: &str, animated: bool, size: u16) -> Option<String> {
    let format = if animated {
        StickerFormatType::Gif
    } else {
        StickerFormatType::Png
    };
    sticker_url(sticker_id, format, size)
}

pub fn decoration_url(asset: Option<&str>, size: u16) -> Option<String> {
    let asset = clean(asset)?;
    if let Some(url) = direct_url(asset) {
        return Some(url);
    }
    let asset = strip_known_extension(asset);
    Some(format!(
        "{CDN_BASE}/avatar-decoration-presets/{asset}.png?size={size}&passthrough=true"
    ))
}

pub fn decoration(asset: Option<&str>) -> Option<String> {
    decoration_url(asset, DEFAULT_DECORATION_SIZE)
}

pub struct CdnUrl;

impl CdnUrl {
    pub fn guild_icon(guild_id: &str, icon: Option<&str>, size: u16) -> Option<String> {
        guild_icon_url(guild_id, icon, size)
    }

    pub fn avatar(user_id: &str, avatar: Option<&str>, size: u16) -> Option<String> {
        avatar_url(user_id, avatar, size)
    }

    pub fn banner(user_id: &str, banner: Option<&str>, size: u16) -> Option<String> {
        banner_url(user_id, banner, size)
    }

    pub fn emoji(emoji_id: &str, animated: bool, size: u16) -> Option<String> {
        emoji_url(emoji_id, animated, size)
    }

    pub fn sticker(sticker_id: &str, format: StickerFormatType, size: u16) -> Option<String> {
        sticker_url(sticker_id, format, size)
    }

    pub fn decoration(asset: Option<&str>, size: u16) -> Option<String> {
        decoration_url(asset, size)
    }
}
