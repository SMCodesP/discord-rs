use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use super::user::AvatarDecorationData;

/// Supported Discord channel types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChannelType {
    /// Standard text chat channel (Guild Text, Discord type 0).
    #[default]
    Text,
    /// Direct message channel (DM, Discord type 1).
    Dm,
    /// Voice channel with audio/video streaming (Guild Voice, Discord type 2).
    Voice,
    /// Group DM channel (Discord type 3).
    GroupDm,
    /// Category header for organizing channels (Guild Category, Discord type 4).
    Category,
    /// Read-only announcement channel (Guild News / Announcement, Discord type 5).
    Announcement,
}

impl ChannelType {
    /// Discord integer code for channel type.
    pub fn discord_code(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Dm => 1,
            Self::Voice => 2,
            Self::GroupDm => 3,
            Self::Category => 4,
            Self::Announcement => 5,
        }
    }

    /// Converts from Discord integer channel type.
    pub fn from_discord_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Text),
            1 => Some(Self::Dm),
            2 => Some(Self::Voice),
            3 => Some(Self::GroupDm),
            4 => Some(Self::Category),
            5 => Some(Self::Announcement),
            _ => None,
        }
    }

    /// Display symbol/icon for the channel.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Text => "#",
            Self::Voice => "",
            Self::Announcement => "",
            Self::Category => "",
            Self::Dm | Self::GroupDm => "@",
        }
    }

    /// Discord vector icon token for the channel.
    pub fn discord_icon(&self) -> Option<crate::tokens::icons::DiscordIcon> {
        match self {
            Self::Text => Some(crate::tokens::icons::DiscordIcon::Hash),
            Self::Voice => Some(crate::tokens::icons::DiscordIcon::Speaker),
            Self::Announcement => Some(crate::tokens::icons::DiscordIcon::Megaphone),
            Self::Category => Some(crate::tokens::icons::DiscordIcon::Folder),
            Self::Dm => Some(crate::tokens::icons::DiscordIcon::User),
            Self::GroupDm => Some(crate::tokens::icons::DiscordIcon::Members),
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    pub fn is_voice(&self) -> bool {
        matches!(self, Self::Voice)
    }

    pub fn is_announcement(&self) -> bool {
        matches!(self, Self::Announcement)
    }

    pub fn is_category(&self) -> bool {
        matches!(self, Self::Category)
    }
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => write!(f, "text"),
            Self::Dm => write!(f, "dm"),
            Self::Voice => write!(f, "voice"),
            Self::GroupDm => write!(f, "group_dm"),
            Self::Category => write!(f, "category"),
            Self::Announcement => write!(f, "announcement"),
        }
    }
}

impl Serialize for ChannelType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ChannelType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ChannelTypeHelper {
            Int(u64),
            Str(String),
        }

        match ChannelTypeHelper::deserialize(deserializer)? {
            ChannelTypeHelper::Int(code) => {
                ChannelType::from_discord_code(code).ok_or_else(|| {
                    serde::de::Error::custom(format!("Unknown Discord channel type code: {}", code))
                })
            }
            ChannelTypeHelper::Str(s) => match s.to_lowercase().as_str() {
                "text" | "guild_text" => Ok(ChannelType::Text),
                "dm" => Ok(ChannelType::Dm),
                "voice" | "guild_voice" => Ok(ChannelType::Voice),
                "group_dm" => Ok(ChannelType::GroupDm),
                "category" | "guild_category" => Ok(ChannelType::Category),
                "announcement" | "news" | "guild_news" | "guild_announcement" => {
                    Ok(ChannelType::Announcement)
                }
                other => Err(serde::de::Error::custom(format!(
                    "Unknown channel type string: {}",
                    other
                ))),
            },
        }
    }
}

/// Represents a Discord Channel (Text, Voice, or Announcement).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// Snowflake identifier of the channel.
    pub id: String,
    /// Associated Guild Snowflake ID (None for direct messages).
    pub guild_id: Option<String>,
    /// Name of the channel (e.g. "general" or "Lounge").
    pub name: String,
    /// Type of channel (Text, Voice, Announcement).
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    /// Parent category Snowflake ID if grouped.
    pub category_id: Option<String>,
    /// Channel description or topic text.
    pub topic: Option<String>,
    /// Sorting position in channel list.
    #[serde(default)]
    pub position: i32,
    /// Local count of unread messages for UI badge display.
    #[serde(default)]
    pub unread_count: u32,
    /// Snowflake of the most recent message posted in this channel.
    pub last_message_id: Option<String>,
    /// Optional avatar or icon URL (for DMs / Group DMs).
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Whether the channel or DM is pinned.
    #[serde(default)]
    pub is_pinned: bool,
    /// List of recipient user IDs for direct message / group DM channels.
    #[serde(default)]
    pub recipient_ids: Vec<String>,
    /// List of recipient display names / usernames for DM / group DM channels.
    #[serde(default)]
    pub recipient_names: Vec<String>,
    /// Per-recipient avatar decorations, aligned by index with `recipient_ids`
    /// (parsed from each DM recipient user object). Empty for guild channels.
    #[serde(default)]
    pub recipient_avatar_decorations: Vec<Option<AvatarDecorationData>>,
    /// User limit for voice channels (0 or None means unlimited).
    #[serde(default)]
    pub user_limit: Option<u32>,
}

impl Channel {
    pub fn new(id: impl Into<String>, name: impl Into<String>, channel_type: ChannelType) -> Self {
        Self {
            id: id.into(),
            guild_id: None,
            name: name.into(),
            channel_type,
            category_id: None,
            topic: None,
            position: 0,
            unread_count: 0,
            last_message_id: None,
            icon_url: None,
            is_pinned: false,
            recipient_ids: Vec::new(),
            recipient_names: Vec::new(),
            recipient_avatar_decorations: Vec::new(),
            user_limit: None,
        }
    }

    pub fn text(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, ChannelType::Text)
    }

    pub fn voice(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, ChannelType::Voice)
    }

    pub fn announcement(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, ChannelType::Announcement)
    }

    pub fn category(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, ChannelType::Category)
    }

    pub fn with_guild_id(mut self, guild_id: impl Into<String>) -> Self {
        self.guild_id = Some(guild_id.into());
        self
    }

    pub fn with_category_id(mut self, category_id: impl Into<String>) -> Self {
        self.category_id = Some(category_id.into());
        self
    }

    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    pub fn with_position(mut self, pos: i32) -> Self {
        self.position = pos;
        self
    }

    pub fn with_recipient_ids(mut self, ids: Vec<String>) -> Self {
        self.recipient_ids = ids;
        self
    }

    pub fn with_user_limit(mut self, limit: Option<u32>) -> Self {
        self.user_limit = limit;
        self
    }

    pub fn is_voice(&self) -> bool {
        matches!(self.channel_type, ChannelType::Voice)
    }

    pub fn is_text(&self) -> bool {
        matches!(self.channel_type, ChannelType::Text)
    }

    pub fn is_announcement(&self) -> bool {
        matches!(self.channel_type, ChannelType::Announcement)
    }

    pub fn icon_symbol(&self) -> &'static str {
        self.channel_type.symbol()
    }
}
