use super::*;
use serde::{Deserialize, Serialize};

/// Disk cache structure for persisting guilds and folder structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildsDiskCache {
    pub guilds: Vec<GuildItemState>,
    pub guild_folders: Vec<crate::protocol::GuildFolder>,
}

/// Path to the guilds cache file on disk (~/.config/discord-rs/guilds_cache.json)
pub fn guilds_cache_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("discord-rs")
        .join("guilds_cache.json")
}

/// Persists the list of guilds and guild folders to disk in JSON format
pub fn save_guilds_to_disk(guilds: &[GuildItemState], guild_folders: &[crate::protocol::GuildFolder]) {
    let path = guilds_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = GuildsDiskCache {
        guilds: guilds.to_vec(),
        guild_folders: guild_folders.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(&path, json);
    }
}

/// Loads cached guilds and guild folders from disk if available and non-empty
pub fn load_guilds_from_disk() -> Option<GuildsDiskCache> {
    let path = guilds_cache_path();
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    if let Ok(cache) = serde_json::from_str::<GuildsDiskCache>(&data) {
        if !cache.guilds.is_empty() {
            return Some(cache);
        }
    }
    if let Ok(guilds) = serde_json::from_str::<Vec<GuildItemState>>(&data) {
        if !guilds.is_empty() {
            return Some(GuildsDiskCache {
                guilds,
                guild_folders: Vec::new(),
            });
        }
    }
    None
}
