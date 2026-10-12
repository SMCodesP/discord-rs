use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the presence status of a Discord user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PresenceStatus {
    /// User is active and online.
    Online,
    /// User is idle or away from keyboard.
    Idle,
    /// User has set Do Not Disturb (does not receive desktop notifications).
    #[serde(rename = "dnd")]
    DoNotDisturb,
    /// User is offline or invisible.
    #[default]
    Offline,
}

impl PresenceStatus {
    /// Returns the canonical string representation for API and display.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Idle => "idle",
            Self::DoNotDisturb => "dnd",
            Self::Offline => "offline",
        }
    }

    /// Returns a human-friendly label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Idle => "Idle",
            Self::DoNotDisturb => "Do Not Disturb",
            Self::Offline => "Offline",
        }
    }

    /// Hex color associated with this status for UI indicator rendering.
    pub fn color_rgb(&self) -> u32 {
        match self {
            Self::Online => 0x23A55A,       // Discord Green
            Self::Idle => 0xF0B232,         // Discord Amber / Yellow
            Self::DoNotDisturb => 0xF23F43, // Discord Red
            Self::Offline => 0x80848E,      // Discord Muted Grey
        }
    }
}

impl fmt::Display for PresenceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Type of activity being performed by a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    #[default]
    Playing,
    Streaming,
    Listening,
    Watching,
    Custom,
    Competing,
}

impl ActivityType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Streaming => "Streaming",
            Self::Listening => "Listening to",
            Self::Watching => "Watching",
            Self::Custom => "Status",
            Self::Competing => "Competing in",
        }
    }
}

/// Activity timestamps (start and end in milliseconds or seconds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ActivityTimestamps {
    #[serde(default)]
    pub start: Option<i64>,
    #[serde(default)]
    pub end: Option<i64>,
}

/// Rich Presence assets (large/small images and hover texts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ActivityAssets {
    #[serde(default)]
    pub large_image: Option<String>,
    #[serde(default)]
    pub large_text: Option<String>,
    #[serde(default)]
    pub small_image: Option<String>,
    #[serde(default)]
    pub small_text: Option<String>,
}

impl ActivityAssets {
    /// Resolves the large image key into an absolute URL.
    pub fn large_image_url(&self, application_id: Option<&str>) -> Option<String> {
        self.large_image
            .as_deref()
            .and_then(|img| resolve_activity_asset_url(img, application_id))
    }

    /// Resolves the small image key into an absolute URL.
    pub fn small_image_url(&self, application_id: Option<&str>) -> Option<String> {
        self.small_image
            .as_deref()
            .and_then(|img| resolve_activity_asset_url(img, application_id))
    }
}

/// Resolves a Discord activity asset key into a loadable CDN or proxy URL.
///
/// Official Discord URL resolution rules:
/// 1. `mp:external/...`: Discord Media Proxy URL `https://media.discordapp.net/external/...`
///    (used when an external URL was registered/cached via Discord's Media Proxy)
/// 2. `spotify:...`: Spotify CDN URL `https://i.scdn.co/image/{id}`
///    (used by Discord's native Spotify integration for album artwork)
/// 3. Direct HTTP/HTTPS URL: used as-is.
/// 4. Application Asset ID: `https://cdn.discordapp.com/app-assets/{application_id}/{asset_id}.png`
///    (assets uploaded to Discord Developer Portal under Rich Presence)
pub fn resolve_activity_asset_url(asset: &str, application_id: Option<&str>) -> Option<String> {
    let trimmed = asset.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("mp:external/") {
        return Some(format!("https://media.discordapp.net/external/{}", rest));
    }
    if let Some(rest) = trimmed.strip_prefix("mp:") {
        return Some(format!("https://media.discordapp.net/{}", rest));
    }
    if let Some(rest) = trimmed.strip_prefix("spotify:") {
        return Some(format!("https://i.scdn.co/image/{}", rest));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    if let Some(app_id) = application_id {
        let app_id_trimmed = app_id.trim();
        if !app_id_trimmed.is_empty() {
            return Some(format!(
                "https://cdn.discordapp.com/app-assets/{}/{}.png",
                app_id_trimmed, trimmed
            ));
        }
    }
    None
}

/// Rich activity information for a user (Rich Presence or Custom Status).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Activity {
    /// Name of the activity, e.g. "Visual Studio Code" or game title.
    pub name: String,
    /// Type of activity (Playing, Listening, etc.)
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    /// Activity details (e.g. "Editing discord-rs/src/domain/user.rs").
    pub details: Option<String>,
    /// State of the activity (e.g. "In a Party").
    pub state: Option<String>,
    /// Unix timestamp of when the activity started.
    pub created_at: Option<i64>,
    /// Optional emoji for custom status.
    #[serde(default)]
    pub emoji: Option<CustomStatusEmoji>,
    /// Application ID (snowflake) for Rich Presence / Game.
    #[serde(default)]
    pub application_id: Option<String>,
    /// Timestamps (start/end) for activities like Spotify or elapsed game timer.
    #[serde(default)]
    pub timestamps: Option<ActivityTimestamps>,
    /// Assets containing large/small images.
    #[serde(default)]
    pub assets: Option<ActivityAssets>,
    /// Sync ID (e.g. Spotify track ID).
    #[serde(default)]
    pub sync_id: Option<String>,
}

impl Activity {
    pub fn new(name: impl Into<String>, activity_type: ActivityType) -> Self {
        Self {
            name: name.into(),
            activity_type,
            details: None,
            state: None,
            created_at: None,
            emoji: None,
            application_id: None,
            timestamps: None,
            assets: None,
            sync_id: None,
        }
    }

    pub fn playing(name: impl Into<String>) -> Self {
        Self::new(name, ActivityType::Playing)
    }

    pub fn custom(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            name: "Custom Status".to_string(),
            activity_type: ActivityType::Custom,
            state: Some(text),
            details: None,
            created_at: None,
            emoji: None,
            application_id: None,
            timestamps: None,
            assets: None,
            sync_id: None,
        }
    }

    pub fn custom_with_emoji(text: Option<String>, emoji: Option<CustomStatusEmoji>) -> Self {
        Self {
            name: "Custom Status".to_string(),
            activity_type: ActivityType::Custom,
            state: text,
            details: None,
            created_at: None,
            emoji,
            application_id: None,
            timestamps: None,
            assets: None,
            sync_id: None,
        }
    }

    /// Resolves the large image key into an absolute URL.
    pub fn large_image_url(&self) -> Option<String> {
        self.assets
            .as_ref()
            .and_then(|a| a.large_image_url(self.application_id.as_deref()))
    }

    /// Resolves the small image key into an absolute URL.
    pub fn small_image_url(&self) -> Option<String> {
        self.assets
            .as_ref()
            .and_then(|a| a.small_image_url(self.application_id.as_deref()))
    }
}

/// Custom emoji or unicode emoji for user custom status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomStatusEmoji {
    /// Name of the emoji (or unicode character if native unicode emoji).
    pub name: String,
    /// Snowflake ID if custom guild emoji, None if native unicode emoji.
    #[serde(default)]
    pub id: Option<String>,
    /// Whether the emoji is animated.
    #[serde(default)]
    pub animated: bool,
}

impl CustomStatusEmoji {
    pub fn unicode(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: None,
            animated: false,
        }
    }

    pub fn custom(id: impl Into<String>, name: impl Into<String>, animated: bool) -> Self {
        Self {
            id: Some(id.into()),
            name: name.into(),
            animated,
        }
    }

    /// CDN URL if custom emoji, or None if unicode.
    pub fn image_url(&self) -> Option<String> {
        self.id.as_ref().map(|id| {
            let ext = if self.animated { "gif" } else { "png" };
            format!("https://cdn.discordapp.com/emojis/{}.{}?size=48", id, ext)
        })
    }
}

/// Custom status phrase and optional emoji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomStatus {
    /// Custom status phrase/text.
    pub text: Option<String>,
    /// Optional emoji (unicode or custom Discord emoji).
    pub emoji: Option<CustomStatusEmoji>,
}

impl CustomStatus {
    pub fn new(text: Option<String>, emoji: Option<CustomStatusEmoji>) -> Self {
        Self { text, emoji }
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            emoji: None,
        }
    }

    pub fn with_emoji(mut self, emoji: CustomStatusEmoji) -> Self {
        self.emoji = Some(emoji);
        self
    }
}

/// Real-time presence information for a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Presence {
    pub user_id: String,
    pub status: PresenceStatus,
    pub activity: Option<Activity>,
    pub custom_status: Option<CustomStatus>,
}

impl Presence {
    pub fn new(user_id: impl Into<String>, status: PresenceStatus) -> Self {
        Self {
            user_id: user_id.into(),
            status,
            activity: None,
            custom_status: None,
        }
    }

    pub fn with_custom_status(mut self, custom_status: CustomStatus) -> Self {
        self.custom_status = Some(custom_status);
        self
    }

    pub fn with_activity(mut self, activity: Activity) -> Self {
        self.activity = Some(activity);
        self
    }
}

/// Data representing an avatar decoration equipped by a user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvatarDecorationData {
    /// Asset hash or identifier for the decoration preset.
    pub asset: String,
    /// Optional SKU Snowflake identifier for the store collectible.
    #[serde(default)]
    pub sku_id: Option<String>,
}

impl AvatarDecorationData {
    /// Creates a new decoration with just the asset identifier.
    pub fn new(asset: impl Into<String>) -> Self {
        Self {
            asset: asset.into(),
            sku_id: None,
        }
    }

    /// Creates a decoration with an asset and SKU ID.
    pub fn with_sku(asset: impl Into<String>, sku_id: impl Into<String>) -> Self {
        Self {
            asset: asset.into(),
            sku_id: Some(sku_id.into()),
        }
    }

    /// Canonical CDN URL for this decoration preset.
    pub fn url(&self) -> String {
        format!(
            "https://cdn.discordapp.com/avatar-decoration-presets/{}.png?size=96&passthrough=true",
            self.asset
        )
    }

    /// Returns true if the decoration is an animated APNG (`a_` prefix).
    pub fn is_animated(&self) -> bool {
        self.asset.starts_with("a_")
    }
}

/// Core domain representation of a Discord User.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// Unique Snowflake identifier.
    pub id: String,
    /// Discord username handle.
    pub username: String,
    /// Legacy 4-digit discriminator (e.g., "#0001") if present.
    pub discriminator: Option<String>,
    /// User's chosen display name (Discord global name).
    pub global_name: Option<String>,
    /// User avatar hash or asset URL.
    pub avatar: Option<String>,
    /// Whether this user account is an automated bot.
    #[serde(default)]
    pub bot: bool,
    /// Real-time presence status of the user.
    #[serde(default)]
    pub presence: PresenceStatus,
    /// Active Rich Presence or custom status activity.
    pub activity: Option<Activity>,
    /// Custom status phrase and emoji.
    #[serde(default)]
    pub custom_status: Option<CustomStatus>,
    /// Active avatar decoration equipped by the user.
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

impl User {
    pub fn new(id: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            discriminator: None,
            global_name: None,
            avatar: None,
            bot: false,
            presence: PresenceStatus::Offline,
            activity: None,
            custom_status: None,
            avatar_decoration_data: None,
        }
    }

    /// Best available display name for UI rendering (prefers global_name, falls back to username).
    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .unwrap_or(&self.username)
    }

    /// Generates the standard Discord CDN avatar URL if avatar hash exists.
    pub fn avatar_url(&self) -> Option<String> {
        self.avatar.as_ref().map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                hash.clone()
            } else {
                format!("https://cdn.discordapp.com/avatars/{}/{}.png", self.id, hash)
            }
        })
    }

    /// Returns the avatar decoration CDN URL if the user has an equipped decoration.
    pub fn avatar_decoration_url(&self) -> Option<String> {
        self.avatar_decoration_data.as_ref().map(|d| d.url())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avatar_decoration_data_url_and_animation() {
        let static_decor = AvatarDecorationData::new("b456abc");
        assert_eq!(
            static_decor.url(),
            "https://cdn.discordapp.com/avatar-decoration-presets/b456abc.png?size=96&passthrough=true"
        );
        assert!(!static_decor.is_animated());
        assert_eq!(static_decor.sku_id, None);

        let animated_decor = AvatarDecorationData::with_sku("a_def123", "sku-999");
        assert_eq!(
            animated_decor.url(),
            "https://cdn.discordapp.com/avatar-decoration-presets/a_def123.png?size=96&passthrough=true"
        );
        assert!(animated_decor.is_animated());
        assert_eq!(animated_decor.sku_id.as_deref(), Some("sku-999"));
    }

    #[test]
    fn test_user_serde_with_avatar_decoration_data() {
        // Payload without decoration field (should deserialize with None due to #[serde(default)])
        let json_no_decor = r#"{"id":"1","username":"alice"}"#;
        let user: User = serde_json::from_str(json_no_decor).unwrap();
        assert_eq!(user.avatar_decoration_data, None);
        assert_eq!(user.avatar_decoration_url(), None);

        // Payload with null decoration field
        let json_null_decor = r#"{"id":"2","username":"bob","avatar_decoration_data":null}"#;
        let user_null: User = serde_json::from_str(json_null_decor).unwrap();
        assert_eq!(user_null.avatar_decoration_data, None);

        // Payload with populated decoration
        let json_with_decor = r#"{
            "id":"3",
            "username":"charlie",
            "avatar_decoration_data":{
                "asset":"a_fire_ring",
                "sku_id":"sku_fire"
            }
        }"#;
        let user_decor: User = serde_json::from_str(json_with_decor).unwrap();
        let decor = user_decor.avatar_decoration_data.as_ref().unwrap();
        assert_eq!(decor.asset, "a_fire_ring");
        assert_eq!(decor.sku_id.as_deref(), Some("sku_fire"));
        assert!(decor.is_animated());
        assert_eq!(
            user_decor.avatar_decoration_url().unwrap(),
            "https://cdn.discordapp.com/avatar-decoration-presets/a_fire_ring.png?size=96&passthrough=true"
        );
    }
    #[test]
    fn test_custom_status_and_emoji_models() {
        let unicode_emoji = CustomStatusEmoji::unicode("🦀");
        assert_eq!(unicode_emoji.name, "🦀");
        assert_eq!(unicode_emoji.id, None);
        assert_eq!(unicode_emoji.image_url(), None);

        let custom_emoji = CustomStatusEmoji::custom("123456789", "ferris", false);
        assert_eq!(custom_emoji.id, Some("123456789".to_string()));
        assert_eq!(
            custom_emoji.image_url(),
            Some("https://cdn.discordapp.com/emojis/123456789.png?size=48".to_string())
        );

        let cs = CustomStatus::from_text("Coding in Rust").with_emoji(unicode_emoji);
        assert_eq!(cs.text, Some("Coding in Rust".to_string()));
        assert_eq!(cs.emoji.as_ref().unwrap().name, "🦀");

        let presence = Presence::new("user-1", PresenceStatus::Online)
            .with_custom_status(cs.clone())
            .with_activity(Activity::playing("Rust"));
        assert_eq!(presence.status, PresenceStatus::Online);
        assert_eq!(presence.custom_status, Some(cs));
        assert_eq!(presence.activity.unwrap().name, "Rust");
    }

    #[test]
    fn test_activity_assets_and_url_resolution() {
        // 1. mp:external/... -> https://media.discordapp.net/external/...
        let mp_url = resolve_activity_asset_url("mp:external/abc1234/https/example.com/art.png", None);
        assert_eq!(
            mp_url,
            Some("https://media.discordapp.net/external/abc1234/https/example.com/art.png".to_string())
        );

        // 2. spotify:... -> https://i.scdn.co/image/...
        let spotify_url = resolve_activity_asset_url("spotify:ab67616d0000b2731234567890abcdef", None);
        assert_eq!(
            spotify_url,
            Some("https://i.scdn.co/image/ab67616d0000b2731234567890abcdef".to_string())
        );

        // 3. Application asset ID with application_id
        let app_asset_url = resolve_activity_asset_url("382910398472918273", Some("383508436472004608"));
        assert_eq!(
            app_asset_url,
            Some("https://cdn.discordapp.com/app-assets/383508436472004608/382910398472918273.png".to_string())
        );

        // 4. Direct HTTP(S) URL
        let direct_url = resolve_activity_asset_url("https://cdn.example.com/image.png", None);
        assert_eq!(
            direct_url,
            Some("https://cdn.example.com/image.png".to_string())
        );

        // 5. Without application_id, raw numeric id returns None
        assert_eq!(resolve_activity_asset_url("382910398472918273", None), None);
        // Empty string returns None
        assert_eq!(resolve_activity_asset_url("", Some("123")), None);

        // Activity with assets
        let mut act = Activity::playing("VS Code");
        act.application_id = Some("383508436472004608".to_string());
        act.assets = Some(ActivityAssets {
            large_image: Some("mp:external/123/img.png".to_string()),
            large_text: Some("Editing user.rs".to_string()),
            small_image: Some("spotify:cover123".to_string()),
            small_text: Some("Spotify small".to_string()),
        });
        assert_eq!(
            act.large_image_url(),
            Some("https://media.discordapp.net/external/123/img.png".to_string())
        );
        assert_eq!(
            act.small_image_url(),
            Some("https://i.scdn.co/image/cover123".to_string())
        );
    }
}
