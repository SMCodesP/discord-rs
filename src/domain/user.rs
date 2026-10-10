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
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
