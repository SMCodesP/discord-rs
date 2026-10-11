use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::user::AvatarDecorationData;

/// The author of a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    /// Snowflake user ID.
    pub id: String,
    /// Discord username handle.
    pub username: String,
    /// Global display name.
    pub global_name: Option<String>,
    /// Avatar hash or URL.
    pub avatar: Option<String>,
    /// Whether the author is a bot.
    #[serde(default)]
    pub bot: bool,
    /// Active avatar decoration equipped by the author.
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

impl Author {
    pub fn new(id: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            username: username.into(),
            global_name: None,
            avatar: None,
            bot: false,
            avatar_decoration_data: None,
        }
    }

    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .unwrap_or(&self.username)
    }

    pub fn avatar_url(&self) -> Option<String> {
        self.avatar.as_ref().map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                hash.clone()
            } else {
                format!("https://cdn.discordapp.com/avatars/{}/{}.png", self.id, hash)
            }
        })
    }

    /// Returns the avatar decoration CDN URL if the author has an equipped decoration.
    pub fn avatar_decoration_url(&self) -> Option<String> {
        self.avatar_decoration_data.as_ref().map(|d| d.url())
    }
}

/// A file attachment uploaded with a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Snowflake ID of attachment.
    pub id: String,
    /// File name (e.g. "screenshot.png", "main.rs").
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// Direct URL to the asset on Discord CDN.
    pub url: String,
    /// Proxied URL for secure delivery.
    pub proxy_url: Option<String>,
    /// MIME type (e.g. "image/png", "application/json").
    pub content_type: Option<String>,
    /// Width in pixels if media is an image/video.
    pub width: Option<u32>,
    /// Height in pixels if media is an image/video.
    pub height: Option<u32>,
}

impl Attachment {
    pub fn is_image(&self) -> bool {
        self.content_type
            .as_ref()
            .map(|ct| ct.starts_with("image/"))
            .unwrap_or_else(|| {
                let lower = self.filename.to_lowercase();
                lower.ends_with(".png")
                    || lower.ends_with(".jpg")
                    || lower.ends_with(".jpeg")
                    || lower.ends_with(".gif")
                    || lower.ends_with(".webp")
            })
    }
}

/// A reaction applied to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    /// Unicode emoji character (e.g. "🦀", "👍") or custom emoji name/id.
    pub emoji: String,
    /// Total count of users who reacted with this emoji.
    pub count: u32,
    /// Whether the current authenticated user has reacted with this emoji.
    pub me: bool,
}

impl Reaction {
    pub fn new(emoji: impl Into<String>, count: u32, me: bool) -> Self {
        Self {
            emoji: emoji.into(),
            count,
            me,
        }
    }
}

/// Author field inside a rich embed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EmbedAuthor {
    pub name: String,
    pub url: Option<String>,
    pub icon_url: Option<String>,
}

/// Field inside a rich embed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}

/// Footer field inside a rich embed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EmbedFooter {
    pub text: String,
    pub icon_url: Option<String>,
}

/// Rich embed content inside a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Embed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub author: Option<EmbedAuthor>,
    #[serde(default)]
    pub fields: Vec<EmbedField>,
    pub footer: Option<EmbedFooter>,
    pub image_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Represents a sticker item in a Discord message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageStickerItem {
    pub id: String,
    pub name: String,
    pub format_type: u8,
}

impl MessageStickerItem {
    pub fn new(id: impl Into<String>, name: impl Into<String>, format_type: u8) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            format_type,
        }
    }

    pub fn cdn_url(&self) -> String {
        if self.format_type == 2 || self.format_type == 4 {
            format!("https://media.discordapp.net/stickers/{}.gif?size=160", self.id)
        } else {
            format!("https://media.discordapp.net/stickers/{}.webp?size=160&quality=lossless", self.id)
        }
    }
}

/// Core message entity representing a chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Snowflake ID of the message.
    pub id: String,
    /// Channel where the message was sent.
    pub channel_id: String,
    /// Guild where the message was sent, if applicable.
    pub guild_id: Option<String>,
    /// Author who sent the message.
    pub author: Author,
    /// Raw markdown content of the message.
    pub content: String,
    /// Creation timestamp in UTC.
    pub timestamp: DateTime<Utc>,
    /// List of file attachments.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// List of emoji reactions.
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    /// List of rich embeds.
    #[serde(default)]
    pub embeds: Vec<Embed>,
    /// List of stickers attached to the message.
    #[serde(default)]
    pub sticker_items: Vec<MessageStickerItem>,
    /// Timestamp of last edit, if edited.
    pub edited_timestamp: Option<DateTime<Utc>>,
    /// Whether the message is pinned in the channel.
    #[serde(default)]
    pub pinned: bool,
    /// Whether the message is pending confirmation from the Discord gateway/API.
    #[serde(default)]
    pub is_pending: bool,
}

impl Message {
    pub fn new(
        id: impl Into<String>,
        channel_id: impl Into<String>,
        author: Author,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            channel_id: channel_id.into(),
            guild_id: None,
            author,
            content: content.into(),
            timestamp: Utc::now(),
            attachments: Vec::new(),
            reactions: Vec::new(),
            embeds: Vec::new(),
            sticker_items: Vec::new(),
            edited_timestamp: None,
            pinned: false,
            is_pending: false,
        }
    }

    pub fn with_sticker_item(mut self, sticker: MessageStickerItem) -> Self {
        self.sticker_items.push(sticker);
        self
    }

    pub fn is_sticker(&self) -> bool {
        !self.sticker_items.is_empty()
            || self.content.starts_with(":sticker[")
            || self.content.starts_with("sticker:")
    }

    pub fn sticker_id(&self) -> Option<String> {
        self.sticker_items.first().map(|s| s.id.clone())
    }

    pub fn sticker_name(&self) -> Option<String> {
        if let Some(item) = self.sticker_items.first() {
            return Some(item.name.clone());
        }
        if self.content.starts_with(":sticker[") {
            if let Some(end) = self.content.find("]:") {
                let name = &self.content[":sticker[".len()..end];
                return Some(name.to_string());
            }
        }
        if self.content.starts_with("sticker:") {
            let name = &self.content["sticker:".len()..];
            return Some(name.to_string());
        }
        None
    }

    pub fn sticker_format_type(&self) -> Option<u8> {
        self.sticker_items.first().map(|s| s.format_type)
    }

    pub fn sticker_cdn_url(&self) -> Option<String> {
        self.sticker_items.first().map(|s| s.cdn_url())
    }

    pub fn with_pending(mut self, pending: bool) -> Self {
        self.is_pending = pending;
        self
    }

    pub fn with_is_pending(mut self, is_pending: bool) -> Self {
        self.is_pending = is_pending;
        self
    }

    pub fn with_guild_id(mut self, guild_id: impl Into<String>) -> Self {
        self.guild_id = Some(guild_id.into());
        self
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn with_attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    pub fn with_reactions(mut self, reactions: Vec<Reaction>) -> Self {
        self.reactions = reactions;
        self
    }

    pub fn with_embeds(mut self, embeds: Vec<Embed>) -> Self {
        self.embeds = embeds;
        self
    }

    /// Adds a reaction to the message. If emoji already exists, increments count.
    pub fn add_reaction(&mut self, emoji: &str, is_me: bool) {
        if let Some(existing) = self.reactions.iter_mut().find(|r| r.emoji == emoji) {
            existing.count += 1;
            if is_me {
                existing.me = true;
            }
        } else {
            self.reactions.push(Reaction {
                emoji: emoji.to_string(),
                count: 1,
                me: is_me,
            });
        }
    }

    /// Removes a reaction from the message. If count hits zero, reaction is removed.
    pub fn remove_reaction(&mut self, emoji: &str, is_me: bool) {
        if let Some(pos) = self.reactions.iter().position(|r| r.emoji == emoji) {
            let reaction = &mut self.reactions[pos];
            if reaction.count > 1 {
                reaction.count -= 1;
                if is_me {
                    reaction.me = false;
                }
            } else {
                self.reactions.remove(pos);
            }
        }
    }
}
