use crate::domain::channel::Channel;
use serde::{Deserialize, Serialize};

/// Represents a Discord role within a Guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// Unique Snowflake identifier for the role.
    pub id: String,
    /// Name of the role (e.g. "Admin", "Moderator", "@everyone").
    pub name: String,
    /// RGB integer representation of the role color (e.g. 0x5865F2). 0 means default/no color.
    #[serde(default)]
    pub color: u32,
    /// Whether this role is pinned in the member list (hoisted).
    #[serde(default)]
    pub hoist: bool,
    /// Hierarchical position of the role.
    #[serde(default)]
    pub position: i32,
    /// Permission bitfield representation as a string.
    #[serde(default)]
    pub permissions: String,
}

impl Role {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            color: 0,
            hoist: false,
            position: 0,
            permissions: "0".to_string(),
        }
    }

    pub fn with_color(mut self, color: u32) -> Self {
        self.color = color;
        self
    }

    pub fn with_hoist(mut self, hoist: bool) -> Self {
        self.hoist = hoist;
        self
    }

    pub fn with_position(mut self, position: i32) -> Self {
        self.position = position;
        self
    }
}

/// Category that groups channels within a Guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    /// Unique Snowflake identifier for the category.
    pub id: String,
    /// Name of the category (e.g. "TEXT CHANNELS", "VOICE CHANNELS").
    pub name: String,
    /// Sorting position in the channel hierarchy.
    #[serde(default)]
    pub position: i32,
}

impl Category {
    pub fn new(id: impl Into<String>, name: impl Into<String>, position: i32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            position,
        }
    }
}

/// Represents a Discord Server / Guild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guild {
    /// Unique Snowflake identifier for the Guild.
    pub id: String,
    /// Name of the server (e.g. "Rust Brasil").
    pub name: String,
    /// Icon hash or URL.
    pub icon: Option<String>,
    /// Snowflake ID of the server owner.
    pub owner_id: String,
    /// List of channel categories.
    #[serde(default)]
    pub categories: Vec<Category>,
    /// List of channels within this server.
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// List of roles configured in this server.
    #[serde(default)]
    pub roles: Vec<Role>,
    /// List of custom emojis configured in this server.
    #[serde(default)]
    pub emojis: Vec<crate::protocol::media_cache::DiscordEmoji>,
    /// List of custom stickers configured in this server.
    #[serde(default)]
    pub stickers: Vec<crate::protocol::media_cache::DiscordSticker>,
}

impl Guild {
    pub fn new(id: impl Into<String>, name: impl Into<String>, owner_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            owner_id: owner_id.into(),
            categories: Vec::new(),
            channels: Vec::new(),
            roles: Vec::new(),
            emojis: Vec::new(),
            stickers: Vec::new(),
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_categories(mut self, categories: Vec<Category>) -> Self {
        self.categories = categories;
        self
    }

    pub fn with_channels(mut self, channels: Vec<Channel>) -> Self {
        self.channels = channels;
        self
    }

    pub fn with_roles(mut self, roles: Vec<Role>) -> Self {
        self.roles = roles;
        self
    }

    pub fn with_emojis(mut self, emojis: Vec<crate::protocol::media_cache::DiscordEmoji>) -> Self {
        self.emojis = emojis;
        self
    }

    pub fn with_stickers(mut self, stickers: Vec<crate::protocol::media_cache::DiscordSticker>) -> Self {
        self.stickers = stickers;
        self
    }

    /// Generates CDN icon URL if icon hash exists.
    pub fn icon_url(&self) -> Option<String> {
        self.icon.as_ref().map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                hash.clone()
            } else {
                format!("https://cdn.discordapp.com/icons/{}/{}.png", self.id, hash)
            }
        })
    }

    /// Filter channels that belong to a specific category (or uncategorized if None).
    pub fn channels_in_category(&self, category_id: Option<&str>) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter(|c| c.category_id.as_deref() == category_id)
            .collect()
    }

    /// Finds a channel by its ID.
    pub fn get_channel(&self, channel_id: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == channel_id)
    }

    /// Finds a role by its ID.
    pub fn get_role(&self, role_id: &str) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == role_id)
    }
}
