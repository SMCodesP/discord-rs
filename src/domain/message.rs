use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::chat::polls::Poll;

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
    /// Unicode emoji character or custom emoji name.
    #[serde(alias = "emoji", default)]
    pub emoji_name: String,
    /// Custom emoji snowflake ID. None for unicode reactions.
    #[serde(default)]
    pub emoji_id: Option<String>,
    /// Whether the custom emoji is animated.
    #[serde(default)]
    pub emoji_animated: bool,
    /// Total count of users who reacted with this emoji.
    pub count: u32,
    /// Whether the current authenticated user has reacted with this emoji.
    pub me: bool,
}

impl Reaction {
    /// Creates a unicode reaction.
    pub fn new(emoji: impl Into<String>, count: u32, me: bool) -> Self {
        Self {
            emoji_name: emoji.into(),
            emoji_id: None,
            emoji_animated: false,
            count,
            me,
        }
    }

    /// Creates a reaction with full emoji identity.
    pub fn from_parts(
        emoji_name: impl Into<String>,
        emoji_id: Option<String>,
        emoji_animated: bool,
        count: u32,
        me: bool,
    ) -> Self {
        Self {
            emoji_name: emoji_name.into(),
            emoji_id,
            emoji_animated,
            count,
            me,
        }
    }

    pub fn is_custom(&self) -> bool {
        self.emoji_id.is_some()
    }

    /// Canonical identity key: `name:id` for custom, `name` for unicode.
    pub fn reaction_key(&self) -> String {
        reaction_key(&self.emoji_name, self.emoji_id.as_deref())
    }

    /// API path component for PUT/DELETE reaction endpoints.
    pub fn api_emoji(&self) -> String {
        self.reaction_key()
    }

    pub fn cdn_url(&self) -> Option<String> {
        self.emoji_id.as_ref().map(|id| {
            if self.emoji_animated {
                format!(
                    "https://cdn.discordapp.com/emojis/{}.gif?size=64&quality=lossless",
                    id
                )
            } else {
                format!(
                    "https://cdn.discordapp.com/emojis/{}.png?size=64&quality=lossless",
                    id
                )
            }
        })
    }
}

/// Canonical reaction identity key: `name:id` for custom, `name` for unicode.
pub fn reaction_key(emoji_name: &str, emoji_id: Option<&str>) -> String {
    match emoji_id {
        Some(id) if !id.is_empty() => format!("{}:{}", emoji_name, id),
        _ => emoji_name.to_string(),
    }
}

/// Parses a canonical reaction key into `(name, id)`.
pub fn parse_reaction_key(key: &str) -> (String, Option<String>) {
    if let Some((name, id)) = key.rsplit_once(':') {
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) {
            return (name.to_string(), Some(id.to_string()));
        }
    }
    (key.to_string(), None)
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
    /// Native Discord poll attached to the message, when present.
    #[serde(default)]
    pub poll: Option<Poll>,
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
            poll: None,
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

    /// Adds a reaction to the message by canonical reaction key.
    pub fn add_reaction(&mut self, key: &str, is_me: bool) {
        if let Some(existing) = self.reactions.iter_mut().find(|r| r.reaction_key() == key) {
            existing.count = existing.count.saturating_add(1);
            if is_me {
                existing.me = true;
            }
            return;
        }
        let (name, id) = parse_reaction_key(key);
        let animated = id.is_some() && key.starts_with("<a:");
        self.reactions.push(Reaction::from_parts(name, id, animated, 1, is_me));
    }

    /// Removes a reaction from the message by canonical reaction key.
    pub fn remove_reaction(&mut self, key: &str, is_me: bool) {
        if let Some(pos) = self.reactions.iter().position(|r| r.reaction_key() == key) {
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

    /// Toggles reaction by key. Returns `Some(true)` if added, `Some(false)` if removed.
    pub fn toggle_reaction(&mut self, key: &str) -> Option<bool> {
        let pos = self.reactions.iter().position(|r| r.reaction_key() == key)?;
        let reaction = &mut self.reactions[pos];
        if reaction.me {
            if reaction.count > 1 {
                reaction.count -= 1;
                reaction.me = false;
            } else {
                self.reactions.remove(pos);
            }
            Some(false)
        } else {
            reaction.count = reaction.count.saturating_add(1);
            reaction.me = true;
            Some(true)
        }
    }

    /// Applies authoritative reaction state from transport/API.
    pub fn set_reaction(&mut self, reaction: Reaction) {
        let key = reaction.reaction_key();
        if reaction.count == 0 {
            self.reactions.retain(|r| r.reaction_key() != key);
            return;
        }
        if let Some(existing) = self.reactions.iter_mut().find(|r| r.reaction_key() == key) {
            *existing = reaction;
        } else {
            self.reactions.push(reaction);
        }
    }
}
