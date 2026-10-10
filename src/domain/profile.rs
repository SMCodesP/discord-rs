//! Discord RS - User Profile Domain Models
//! Canonical domain structures representing Discord User Profiles (/users/{id}/profile).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::domain::user::User;

/// Converts a Discord Snowflake ID string to its corresponding account creation DateTime in UTC.
/// Discord Epoch is 2015-01-01T00:00:00Z (1420070400000 ms).
/// Returns None if the string cannot be parsed as a valid u64 Snowflake integer.
pub fn snowflake_to_datetime(snowflake_str: &str) -> Option<DateTime<Utc>> {
    let snowflake = snowflake_str.trim().parse::<u64>().ok()?;
    // Top 42 bits are milliseconds since Discord Epoch
    let ms_since_epoch = snowflake >> 22;
    let discord_epoch_ms = 1_420_070_400_000u64;
    let timestamp_ms = discord_epoch_ms.checked_add(ms_since_epoch)? as i64;
    DateTime::from_timestamp_millis(timestamp_ms)
}

/// Profile badge earned by user (Nitro, HypeSquad, Early Supporter, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileBadge {
    pub id: String,
    pub description: String,
    pub icon: String,
    #[serde(default)]
    pub link: Option<String>,
}

impl ProfileBadge {
    pub fn icon_url(&self) -> String {
        if self.icon.starts_with("http://") || self.icon.starts_with("https://") {
            self.icon.clone()
        } else {
            format!("https://cdn.discordapp.com/badge-icons/{}.png", self.icon)
        }
    }
}

/// Connected third-party account (Spotify, Steam, GitHub, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedAccount {
    #[serde(rename = "type")]
    pub account_type: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub verified: bool,
}

/// Mutual guild shared between current user and profile target
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutualGuild {
    pub id: String,
    #[serde(default)]
    pub nick: Option<String>,
}

/// Navigation tabs in the profile popout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProfileTab {
    #[default]
    MutualGuilds,
    MutualFriends,
}

/// Interactive state of the opened profile popout in `AppState`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilePopoutState {
    pub user_id: String,
    pub guild_id: Option<String>,
    pub data: Option<UserProfileData>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub active_tab: ProfileTab,
    pub quick_message: String,
}

/// Complete official Discord User Profile data structure (GET /users/{id}/profile)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfileData {
    pub user: User,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub banner_color: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    #[serde(default)]
    pub profile_effect_id: Option<String>,
    #[serde(default)]
    pub badges: Vec<ProfileBadge>,
    #[serde(default)]
    pub connected_accounts: Vec<ConnectedAccount>,
    #[serde(default)]
    pub mutual_guilds: Vec<MutualGuild>,
    #[serde(default)]
    pub mutual_friends_count: u32,
    /// Account creation date derived from snowflake (None if not a valid snowflake ID)
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    /// Timestamp when member joined guild (when queried with guild context)
    #[serde(default)]
    pub joined_server_at: Option<DateTime<Utc>>,
}

impl UserProfileData {
    /// Resolved banner image URL
    pub fn banner_image_url(&self) -> Option<String> {
        self.banner.as_ref().map(|b| {
            if b.starts_with("http://") || b.starts_with("https://") {
                b.clone()
            } else {
                format!(
                    "https://cdn.discordapp.com/banners/{}/{}.png?size=600",
                    self.user.id, b
                )
            }
        })
    }

    /// Resolved banner background color integer
    pub fn display_banner_color(&self) -> Option<u32> {
        if let Some(ref hex_str) = self.banner_color {
            let hex = hex_str.trim_start_matches('#');
            if let Ok(val) = u32::from_str_radix(hex, 16) {
                return Some(val);
            }
        }
        self.accent_color
    }
}
