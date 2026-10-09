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
}

impl Activity {
    pub fn new(name: impl Into<String>, activity_type: ActivityType) -> Self {
        Self {
            name: name.into(),
            activity_type,
            details: None,
            state: None,
            created_at: None,
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
        }
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
