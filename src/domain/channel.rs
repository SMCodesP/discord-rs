use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use super::permissions::PermissionOverwrite;
use super::user::AvatarDecorationData;

/// Metadata for a thread channel (types 10, 11, 12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ThreadMetadata {
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub auto_archive_duration: Option<u32>,
    #[serde(default)]
    pub archive_timestamp: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub invitable: Option<bool>,
    #[serde(default)]
    pub create_timestamp: Option<String>,
}

/// Tag available in a forum/media channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ForumTag {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub moderated: bool,
    #[serde(default)]
    pub emoji_id: Option<String>,
    #[serde(default)]
    pub emoji_name: Option<String>,
}

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
    /// Temporary sub-channel within an announcement channel (Discord type 10).
    AnnouncementThread,
    /// Temporary sub-channel within a text/forum channel (Discord type 11).
    PublicThread,
    /// Private thread, invite-only (Discord type 12).
    PrivateThread,
    /// Stage voice channel for hosted events (Discord type 13).
    StageVoice,
    /// Guild directory channel (Discord type 14).
    Directory,
    /// Forum channel containing posts/threads (Discord type 15).
    Forum,
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
            Self::AnnouncementThread => 10,
            Self::PublicThread => 11,
            Self::PrivateThread => 12,
            Self::StageVoice => 13,
            Self::Directory => 14,
            Self::Forum => 15,
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
            13 => Some(Self::StageVoice),
            14 => Some(Self::Directory),
            15 => Some(Self::Forum),
            _ => None,
        }
    }

    /// Display symbol/icon for the channel.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Text => "#",
            Self::Voice | Self::StageVoice => "",
            Self::Announcement => "",
            Self::Category => "",
            Self::Dm | Self::GroupDm => "@",
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread => "",
            Self::Directory => "",
            Self::Forum => "",
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
            Self::StageVoice => Some(crate::tokens::icons::DiscordIcon::Speaker),
            Self::Directory => Some(crate::tokens::icons::DiscordIcon::Folder),
            Self::Forum => Some(crate::tokens::icons::DiscordIcon::Chat),
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    pub fn is_voice(&self) -> bool {
        matches!(self, Self::Voice | Self::StageVoice)
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
        matches!(self, Self::Forum)
    }

    pub fn is_stage(&self) -> bool {
        matches!(self, Self::StageVoice)
    }

    pub fn is_directory(&self) -> bool {
        matches!(self, Self::Directory)
    }

    pub fn is_guild_selectable(&self) -> bool {
        matches!(
            self,
            Self::Text
                | Self::Voice
                | Self::Announcement
                | Self::StageVoice
                | Self::Forum
                | Self::Directory
                | Self::AnnouncementThread
                | Self::PublicThread
                | Self::PrivateThread
        )
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
            Self::StageVoice => write!(f, "stage_voice"),
            Self::Directory => write!(f, "directory"),
            Self::Forum => write!(f, "forum"),
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
                "announcement_thread" | "guild_news_thread" => Ok(ChannelType::AnnouncementThread),
                "public_thread" | "guild_public_thread" => Ok(ChannelType::PublicThread),
                "private_thread" | "guild_private_thread" => Ok(ChannelType::PrivateThread),
                "stage_voice" | "guild_stage_voice" => Ok(ChannelType::StageVoice),
                "directory" | "guild_directory" => Ok(ChannelType::Directory),
                "forum" | "guild_forum" => Ok(ChannelType::Forum),
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
    /// Raw Discord parent_id (category for channels, parent channel for threads).
    #[serde(default)]
    pub parent_id: Option<String>,
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
    /// Role/member permission overwrites for this channel or category.
    #[serde(default)]
    pub permission_overwrites: Vec<PermissionOverwrite>,
    /// Thread metadata for thread channels (10,11,12).
    #[serde(default)]
    pub thread_metadata: Option<ThreadMetadata>,
    /// Default forum tag IDs applied to new posts.
    #[serde(default)]
    pub default_tag_ids: Vec<String>,
    /// Available tags in forum/media channels.
    #[serde(default)]
    pub available_tags: Vec<ForumTag>,
    /// Forum channel flags bitfield.
    #[serde(default)]
    pub flags: u32,
}

impl Channel {
    pub fn new(id: impl Into<String>, name: impl Into<String>, channel_type: ChannelType) -> Self {
        Self {
            id: id.into(),
            guild_id: None,
            name: name.into(),
            channel_type,
            category_id: None,
            parent_id: None,
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
            permission_overwrites: Vec::new(),
            thread_metadata: None,
            default_tag_ids: Vec::new(),
            available_tags: Vec::new(),
            flags: 0,
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

    pub fn is_voice(&self) -> bool {
        self.channel_type.is_voice()
    }

    pub fn is_text(&self) -> bool {
        self.channel_type.is_text()
    }

    pub fn is_announcement(&self) -> bool {
        self.channel_type.is_announcement()
    }

    pub fn is_thread(&self) -> bool {
        self.channel_type.is_thread()
    }

    pub fn is_forum(&self) -> bool {
        self.channel_type.is_forum()
    }

    pub fn is_stage(&self) -> bool {
        self.channel_type.is_stage()
    }

    pub fn icon_symbol(&self) -> &'static str {
        self.channel_type.symbol()
    }
}
#[cfg(test)]
mod channel_type_t076_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn discord_channel_types_10_to_15_are_preserved() {
        let cases = [
            (10u64, ChannelType::AnnouncementThread, "announcement_thread"),
            (11, ChannelType::PublicThread, "public_thread"),
            (12, ChannelType::PrivateThread, "private_thread"),
            (13, ChannelType::StageVoice, "stage_voice"),
            (14, ChannelType::Directory, "directory"),
            (15, ChannelType::Forum, "forum"),
        ];
        for (code, expected, name) in cases {
            let ty = ChannelType::from_discord_code(code).expect("type should parse");
            assert_eq!(ty, expected);
            assert_eq!(ty.discord_code() as u64, code);
            assert_eq!(ty.to_string(), name);
            let roundtrip: ChannelType = serde_json::from_value(json!(code)).expect("serde int");
            assert_eq!(roundtrip, expected);
        }
    }

    #[test]
    fn forum_thread_and_stage_helpers_are_correct() {
        assert!(ChannelType::Forum.is_forum());
        assert!(ChannelType::StageVoice.is_stage());
        assert!(ChannelType::StageVoice.is_voice());
        assert!(ChannelType::PublicThread.is_thread());
        assert!(ChannelType::PrivateThread.is_thread());
        assert!(ChannelType::AnnouncementThread.is_thread());
        assert!(!ChannelType::Text.is_thread());
    }
}
