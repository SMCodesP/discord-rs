use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use super::permissions::PermissionOverwrite;
use super::user::AvatarDecorationData;

/// Supported Discord channel types, including modern guild/thread types.
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
    /// Announcement thread (Discord type 10).
    AnnouncementThread,
    /// Public thread (Discord type 11).
    PublicThread,
    /// Private thread (Discord type 12).
    PrivateThread,
    /// Stage voice channel (Discord type 13).
    Stage,
    /// Student hub / directory channel (Discord type 14).
    Directory,
    /// Forum channel (Discord type 15).
    Forum,
    /// Media channel (Discord type 16).
    Media,
}

#[allow(non_upper_case_globals)]
impl ChannelType {
    /// Alias matching Discord's official name for [`ChannelType::Stage`].
    pub const StageVoice: Self = Self::Stage;
    /// Alias matching Discord's official name for a guild directory channel.
    pub const GuildDirectory: Self = Self::Directory;
    /// Alias used by some call sites for a guild forum channel.
    pub const GuildForum: Self = Self::Forum;

    /// Discord integer code for channel type.
    pub fn discord_code(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Dm => 1,
            Self::Voice => 2,
            Self::GroupDm => 3,
            Self::Category => 4,
            Self::Announcement => 5,
            Self::AnnouncementThread => 10,
            Self::PublicThread => 11,
            Self::PrivateThread => 12,
            Self::Stage => 13,
            Self::Directory => 14,
            Self::Forum => 15,
            Self::Media => 16,
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
            10 => Some(Self::AnnouncementThread),
            11 => Some(Self::PublicThread),
            12 => Some(Self::PrivateThread),
            13 => Some(Self::Stage),
            14 => Some(Self::Directory),
            15 => Some(Self::Forum),
            16 => Some(Self::Media),
            _ => None,
        }
    }

    /// Display symbol/icon for the channel.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Text | Self::Forum | Self::Media => "#",
            Self::Voice | Self::Stage => "",
            Self::Announcement => "",
            Self::Category => "",
            Self::Directory => "",
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread => "",
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
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread => {
                Some(crate::tokens::icons::DiscordIcon::Chat)
            }
            Self::Stage => Some(crate::tokens::icons::DiscordIcon::Speaker),
            Self::Directory => Some(crate::tokens::icons::DiscordIcon::Compass),
            Self::Forum | Self::Media => Some(crate::tokens::icons::DiscordIcon::Chat),
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    pub fn is_voice(&self) -> bool {
        matches!(self, Self::Voice | Self::Stage)
    }

    pub fn is_announcement(&self) -> bool {
        matches!(self, Self::Announcement)
    }

    pub fn is_category(&self) -> bool {
        matches!(self, Self::Category)
    }

    pub fn is_thread(&self) -> bool {
        matches!(
            self,
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread
        )
    }

    pub fn is_forum(&self) -> bool {
        matches!(self, Self::Forum | Self::Media)
    }

    pub fn is_stage(&self) -> bool {
        matches!(self, Self::Stage)
    }

    pub fn is_directory(&self) -> bool {
        matches!(self, Self::Directory)
    }

    /// Whether the type is a guild channel shown in the channel sidebar.
    pub fn is_guild_channel(&self) -> bool {
        !matches!(self, Self::Dm | Self::GroupDm | Self::Directory | Self::Media)
            && !self.is_thread()
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
            Self::AnnouncementThread => write!(f, "announcement_thread"),
            Self::PublicThread => write!(f, "public_thread"),
            Self::PrivateThread => write!(f, "private_thread"),
            Self::Stage => write!(f, "stage_voice"),
            Self::Directory => write!(f, "directory"),
            Self::Forum => write!(f, "forum"),
            Self::Media => write!(f, "media"),
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
                "announcement_thread" | "guild_announcement_thread" | "thread_announcement" => {
                    Ok(ChannelType::AnnouncementThread)
                }
                "public_thread" | "guild_public_thread" | "thread_public" => {
                    Ok(ChannelType::PublicThread)
                }
                "private_thread" | "guild_private_thread" | "thread_private" => {
                    Ok(ChannelType::PrivateThread)
                }
                "stage" | "stage_voice" | "guild_stage_voice" | "guild_stage" => {
                    Ok(ChannelType::Stage)
                }
                "directory" | "guild_directory" => Ok(ChannelType::Directory),
                "forum" | "guild_forum" => Ok(ChannelType::Forum),
                "media" | "guild_media" => Ok(ChannelType::Media),
                other => Err(serde::de::Error::custom(format!(
                    "Unknown channel type string: {}",
                    other
                ))),
            },
        }
    }
}

/// Discord thread metadata attached to thread channels (types 10, 11 and 12).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThreadMetadata {
    /// Whether the thread is archived.
    #[serde(default)]
    pub archived: bool,
    /// Auto-archive duration in minutes chosen when the thread was created.
    #[serde(default)]
    pub auto_archive_duration: u32,
    /// ISO-8601 timestamp when the thread was archived, when archived.
    #[serde(default)]
    pub archive_timestamp: Option<String>,
    /// Whether the thread is locked.
    #[serde(default)]
    pub locked: bool,
    /// Whether non-moderators can add members to a private thread.
    #[serde(default)]
    pub invitable: Option<bool>,
    /// ISO-8601 timestamp when the thread was created.
    #[serde(default)]
    pub create_timestamp: Option<String>,
}

/// Represents a Discord Channel (Text, Voice, Announcement, Forum or Thread).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// Snowflake identifier of the channel.
    pub id: String,
    /// Associated Guild Snowflake ID (None for direct messages).
    pub guild_id: Option<String>,
    /// Name of the channel (e.g. "general" or "Lounge").
    pub name: String,
    /// Type of channel (Text, Voice, Announcement, Forum or Thread).
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    /// Parent category (or parent channel for threads) Snowflake ID.
    #[serde(rename = "parent_id", alias = "category_id", default)]
    pub category_id: Option<String>,
    /// Channel description or topic text.
    #[serde(default)]
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
    /// Thread metadata, present for thread channels.
    #[serde(default)]
    pub thread_metadata: Option<ThreadMetadata>,
    /// Role/member permission overwrites for this channel or category.
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwrite>,
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
            thread_metadata: None,
            permission_overwrites: Vec::new(),
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

    pub fn with_permission_overwrites(
        mut self,
        overwrites: Vec<PermissionOverwrite>,
    ) -> Self {
        self.permission_overwrites = overwrites;
        self
    }

    pub fn with_thread_metadata(mut self, metadata: ThreadMetadata) -> Self {
        self.thread_metadata = Some(metadata);
        self
    }

    /// Parent category/channel ID (Discord `parent_id`).
    pub fn parent_id(&self) -> Option<&str> {
        self.category_id.as_deref()
    }

    pub fn is_voice(&self) -> bool {
        matches!(
            self.channel_type,
            ChannelType::Voice | ChannelType::Stage
        )
    }

    pub fn is_text(&self) -> bool {
        matches!(self.channel_type, ChannelType::Text)
    }

    pub fn is_announcement(&self) -> bool {
        matches!(self.channel_type, ChannelType::Announcement)
    }

    pub fn is_thread(&self) -> bool {
        self.channel_type.is_thread()
    }

    pub fn is_forum(&self) -> bool {
        self.channel_type.is_forum()
    }

    pub fn icon_symbol(&self) -> &'static str {
        self.channel_type.symbol()
    }
}
