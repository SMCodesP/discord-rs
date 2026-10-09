use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::domain::message::Message;
use crate::domain::{Channel, Guild, User};

/// Metadata stored in ~/.config/discord-rs/cache/metadata.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub etags: HashMap<String, String>,
    pub last_sync: HashMap<String, DateTime<Utc>>,
    pub client_version: String,
}

/// Channel cache container in ~/.config/discord-rs/cache/channels/{channel_id}/messages.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessageCache {
    pub channel_id: String,
    pub last_message_id: Option<String>,
    pub messages: Vec<Message>,
    pub last_updated: DateTime<Utc>,
}

/// Initial state restored on application startup (0ms cold start)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialCachedState {
    pub current_user: Option<User>,
    pub guilds: Vec<Guild>,
    pub channels: Vec<Channel>,
    pub cached_messages_by_channel: HashMap<String, Vec<Message>>,
}

/// Canonical Discord Cache Engine matching the official Discord Desktop storage & revalidation lifecycle
#[derive(Clone)]
pub struct DiscordCacheEngine {
    base_dir: PathBuf,
    metadata: Arc<RwLock<CacheMetadata>>,
    memory_messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl DiscordCacheEngine {
    /// Creates or opens the cache engine at ~/.config/discord-rs/cache
    pub fn new() -> Self {
        let base_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("discord-rs")
            .join("cache");

        let _ = fs::create_dir_all(base_dir.join("guilds"));
        let _ = fs::create_dir_all(base_dir.join("channels"));
        let _ = fs::create_dir_all(base_dir.join("users"));
        let _ = fs::create_dir_all(base_dir.join("assets").join("icons"));
        let _ = fs::create_dir_all(base_dir.join("assets").join("avatars"));
        let _ = fs::create_dir_all(base_dir.join("settings"));

        let meta_file = base_dir.join("metadata.json");
        let metadata = if meta_file.exists() {
            fs::read_to_string(&meta_file)
                .ok()
                .and_then(|content| serde_json::from_str::<CacheMetadata>(&content).ok())
                .unwrap_or_default()
        } else {
            CacheMetadata {
                etags: HashMap::new(),
                last_sync: HashMap::new(),
                client_version: "0.1.0".to_string(),
            }
        };

        Self {
            base_dir,
            metadata: Arc::new(RwLock::new(metadata)),
            memory_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    // =========================================================================
    // 1. ETag & HTTP 304 CONDITIONAL REQUEST SUPPORT
    // =========================================================================

    /// Returns the cached ETag for an endpoint if present
    pub async fn get_etag(&self, endpoint: &str) -> Option<String> {
        self.metadata.read().await.etags.get(endpoint).cloned()
    }

    /// Stores a new ETag and saves metadata to disk
    pub async fn set_etag(&self, endpoint: &str, etag: &str) {
        let mut meta = self.metadata.write().await;
        meta.etags.insert(endpoint.to_string(), etag.to_string());
        meta.last_sync.insert(endpoint.to_string(), Utc::now());
        self.persist_metadata(&meta);
    }

    fn persist_metadata(&self, meta: &CacheMetadata) {
        let meta_path = self.base_dir.join("metadata.json");
        let tmp_path = self.base_dir.join("metadata.json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(meta) {
            if fs::write(&tmp_path, json).is_ok() {
                let _ = fs::rename(&tmp_path, &meta_path);
            }
        }
    }

    // =========================================================================
    // 2. ATOMIC DISK CACHE ACCESS (USERS, GUILDS, CHANNELS)
    // =========================================================================

    /// Saves the current user profile to ~/.config/discord-rs/cache/users/me.json
    pub fn save_current_user(&self, user: &User) {
        let path = self.base_dir.join("users").join("me.json");
        let tmp = self.base_dir.join("users").join("me.json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(user) {
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }

    /// Loads the current user from disk cache
    pub fn load_current_user(&self) -> Option<User> {
        let path = self.base_dir.join("users").join("me.json");
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Saves guilds list to ~/.config/discord-rs/cache/guilds/guilds_index.json
    pub fn save_guilds(&self, guilds: &[Guild]) {
        let path = self.base_dir.join("guilds").join("guilds_index.json");
        let tmp = self.base_dir.join("guilds").join("guilds_index.json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(guilds) {
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }

    /// Loads cached guilds from disk
    pub fn load_guilds(&self) -> Option<Vec<Guild>> {
        let path = self.base_dir.join("guilds").join("guilds_index.json");
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    // =========================================================================
    // 3. CHANNEL MESSAGES CACHE (SLIDING WINDOW & DISK PERSISTENCE)
    // =========================================================================

    /// Loads channel messages: first checks memory LRU, then falls back to disk
    pub async fn get_channel_messages(&self, channel_id: &str) -> Vec<Message> {
        // Fast-path: RAM Cache
        {
            let mem = self.memory_messages.read().await;
            if let Some(msgs) = mem.get(channel_id) {
                return msgs.clone();
            }
        }

        // Slow-path: Disk Cache
        let path = self.base_dir.join("channels").join(channel_id).join("messages.json");
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(cache) = serde_json::from_str::<ChannelMessageCache>(&data) {
                let mut mem = self.memory_messages.write().await;
                mem.insert(channel_id.to_string(), cache.messages.clone());
                return cache.messages;
            }
        }

        Vec::new()
    }

    /// Saves messages for a channel with sliding window truncation (max 100 messages)
    pub async fn save_channel_messages(&self, channel_id: &str, messages: &[Message]) {
        const MAX_MESSAGES_PER_CHANNEL: usize = 100;
        let bounded: Vec<Message> = if messages.len() > MAX_MESSAGES_PER_CHANNEL {
            messages[messages.len() - MAX_MESSAGES_PER_CHANNEL..].to_vec()
        } else {
            messages.to_vec()
        };

        // 1. Update RAM Cache
        {
            let mut mem = self.memory_messages.write().await;
            mem.insert(channel_id.to_string(), bounded.clone());
        }

        // 2. Persist to Disk Asynchronously
        let channel_dir = self.base_dir.join("channels").join(channel_id);
        let _ = fs::create_dir_all(&channel_dir);

        let cache = ChannelMessageCache {
            channel_id: channel_id.to_string(),
            last_message_id: bounded.last().map(|m| m.id.clone()),
            messages: bounded,
            last_updated: Utc::now(),
        };

        let path = channel_dir.join("messages.json");
        let tmp = channel_dir.join("messages.json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(&cache) {
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }

    // =========================================================================
    // 4. GATEWAY DELTA REVALIDATION (ZERO POLLING!)
    // =========================================================================

    /// Ingests a new MESSAGE_CREATE Gateway event into the cache
    pub async fn apply_gateway_message_create(&self, msg: &Message) {
        let mut msgs = self.get_channel_messages(&msg.channel_id).await;
        // Deduplicate
        if !msgs.iter().any(|m| m.id == msg.id) {
            msgs.push(msg.clone());
            self.save_channel_messages(&msg.channel_id, &msgs).await;
            debug!("[CacheEngine] Mensagem '{}' persistida no cache do canal {}", msg.id, msg.channel_id);
        }
    }

    /// Ingests a MESSAGE_UPDATE Gateway event into the cache
    pub async fn apply_gateway_message_update(&self, channel_id: &str, msg_id: &str, new_content: &str) {
        let mut msgs = self.get_channel_messages(channel_id).await;
        if let Some(target) = msgs.iter_mut().find(|m| m.id == msg_id) {
            target.content = new_content.to_string();
            target.edited_timestamp = Some(Utc::now());
            self.save_channel_messages(channel_id, &msgs).await;
            debug!("[CacheEngine] Mensagem '{}' atualizada in-place no cache", msg_id);
        }
    }

    /// Ingests a MESSAGE_DELETE Gateway event into the cache
    pub async fn apply_gateway_message_delete(&self, channel_id: &str, msg_id: &str) {
        let mut msgs = self.get_channel_messages(channel_id).await;
        let before_len = msgs.len();
        msgs.retain(|m| m.id != msg_id);
        if msgs.len() < before_len {
            self.save_channel_messages(channel_id, &msgs).await;
            debug!("[CacheEngine] Mensagem '{}' removida do cache do canal {}", msg_id, channel_id);
        }
    }

    // =========================================================================
    // 5. COLD START RESTORATION (0ms BLANK SCREEN)
    // =========================================================================

    /// Restores cached state immediately on application startup before network connection
    pub fn restore_cold_start(&self) -> InitialCachedState {
        let current_user = self.load_current_user();
        let guilds = self.load_guilds().unwrap_or_default();
        let mut cached_messages_by_channel = HashMap::new();

        let channels_dir = self.base_dir.join("channels");
        if let Ok(entries) = fs::read_dir(channels_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let cid = entry.file_name().to_string_lossy().to_string();
                    let msg_file = entry.path().join("messages.json");
                    if let Ok(data) = fs::read_to_string(msg_file) {
                        if let Ok(cache) = serde_json::from_str::<ChannelMessageCache>(&data) {
                            cached_messages_by_channel.insert(cid, cache.messages);
                        }
                    }
                }
            }
        }

        info!(
            "[CacheEngine] Cold start restaurado: {} guilds, {} canais com mensagens cacheadas",
            guilds.len(),
            cached_messages_by_channel.len()
        );

        InitialCachedState {
            current_user,
            guilds,
            channels: Vec::new(),
            cached_messages_by_channel,
        }
    }
}

impl Default for DiscordCacheEngine {
    fn default() -> Self {
        Self::new()
    }
}
