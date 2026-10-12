use crate::domain::{
    Activity, ActivityType, Attachment, Author, AvatarDecorationData, Channel, ChannelType,
    CustomStatus, CustomStatusEmoji, Embed, EmbedField, Guild, Message, MessageStickerItem,
    PresenceStatus, Reaction, Role, User, VoiceState,
};
use crate::protocol::auth::{AuthToken, TokenType};
use crate::protocol::cache_engine::DiscordCacheEngine;
use crate::protocol::media_cache::{
    DiscordEmoji, DiscordSticker, MediaCacheManager, SoundboardSound, StickerFormatType,
    StickerPack, TenorGif,
};
use crate::domain::profile::{
    snowflake_to_datetime, ConnectedAccount, MutualGuild, ProfileBadge, UserProfileData,
};
use crate::protocol::rate_limiter::RateLimitManager;
use crate::protocol::transport::{DiscordTransport, GuildFolder, Relationship, TransportEvent};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::protocol::{Message as WsMessage, WebSocketConfig};

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const DEFAULT_GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

/// Gateway Gateway payload schema.
#[derive(Debug, Serialize, Deserialize)]
struct GatewayPayload {
    op: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    d: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    s: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<String>,
}

/// Live Discord Client combining REST API v10 and Gateway WebSocket event loop.
#[derive(Clone)]
pub struct DiscordApiClient {
    token: AuthToken,
    http_client: reqwest::Client,
    rate_limiter: Arc<RateLimitManager>,
    cache_engine: Arc<DiscordCacheEngine>,
    media_cache: MediaCacheManager,
    event_sender: broadcast::Sender<TransportEvent>,
    is_connected: Arc<AtomicBool>,
    last_sequence: Arc<AtomicI64>,
    session_id: Arc<RwLock<Option<String>>>,
    resume_gateway_url: Arc<RwLock<Option<String>>>,
    gateway_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    outbound_gateway_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>>,
    current_user_id: Arc<RwLock<Option<String>>>,
    last_voice_session_id: Arc<RwLock<Option<String>>>,
    last_voice_server: Arc<RwLock<Option<crate::protocol::VoiceServerData>>>,
}

impl DiscordApiClient {
    /// Creates a new Discord API client with the provided authentication token.
    pub fn new(token: AuthToken) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("discord-rs/0.1.0 (https://github.com/smcodes/discord-rs)")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            token,
            http_client,
            rate_limiter: Arc::new(RateLimitManager::new()),
            cache_engine: Arc::new(DiscordCacheEngine::new()),
            media_cache: MediaCacheManager::default_path(),
            event_sender,
            is_connected: Arc::new(AtomicBool::new(false)),
            last_sequence: Arc::new(AtomicI64::new(-1)),
            session_id: Arc::new(RwLock::new(None)),
            resume_gateway_url: Arc::new(RwLock::new(None)),
            gateway_task: Arc::new(Mutex::new(None)),
            outbound_gateway_tx: Arc::new(RwLock::new(None)),
            current_user_id: Arc::new(RwLock::new(None)),
            last_voice_session_id: Arc::new(RwLock::new(None)),
            last_voice_server: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates a new Discord API client with custom cache engine and rate limiter.
    pub fn with_cache_and_limiter(
        token: AuthToken,
        cache_engine: Arc<DiscordCacheEngine>,
        rate_limiter: Arc<RateLimitManager>,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("discord-rs/0.1.0 (https://github.com/smcodes/discord-rs)")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            token,
            http_client,
            rate_limiter,
            cache_engine,
            media_cache: MediaCacheManager::default_path(),
            event_sender,
            is_connected: Arc::new(AtomicBool::new(false)),
            last_sequence: Arc::new(AtomicI64::new(-1)),
            session_id: Arc::new(RwLock::new(None)),
            resume_gateway_url: Arc::new(RwLock::new(None)),
            gateway_task: Arc::new(Mutex::new(None)),
            outbound_gateway_tx: Arc::new(RwLock::new(None)),
            current_user_id: Arc::new(RwLock::new(None)),
            last_voice_session_id: Arc::new(RwLock::new(None)),
            last_voice_server: Arc::new(RwLock::new(None)),
        }
    }

    /// Access token currently used by client.
    pub fn token(&self) -> &AuthToken {
        &self.token
    }

    /// Reference to rate limit manager.
    pub fn rate_limiter(&self) -> &Arc<RateLimitManager> {
        &self.rate_limiter
    }

    /// Reference to cache engine.
    pub fn cache_engine(&self) -> &Arc<DiscordCacheEngine> {
        &self.cache_engine
    }

    /// Checks if gateway connection is active.
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    // --- Internal REST Request Helpers ---

    fn auth_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", DISCORD_API_BASE, path);
        self.http_client
            .request(method, url)
            .header("Authorization", self.token.authorization_header())
            .header("Content-Type", "application/json")
    }

    fn auth_request_url(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.http_client
            .request(method, url)
            .header("Authorization", self.token.authorization_header())
            .header("Content-Type", "application/json")
    }

    async fn send_request(&self, path: &str, req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        const MAX_RETRIES: usize = 3;
        let mut retries = 0;

        let Some(template) = req.try_clone() else {
            // Se o request não pode ser clonado, executa uma única vez
            self.rate_limiter.acquire_permit(path).await;
            let resp = req.send().await.context("HTTP request failed")?;
            self.rate_limiter.update_from_headers(path, resp.headers()).await;
            let status = resp.status();
            if !status.is_success() && status != reqwest::StatusCode::NOT_MODIFIED {
                let error_text = resp.text().await.unwrap_or_default();
                bail!("Discord API returned HTTP {}: {}", status, error_text);
            }
            return Ok(resp);
        };

        loop {
            // 1. Adquire autorização de bucket antes de qualquer disparo HTTP (acquire_permit)
            self.rate_limiter.acquire_permit(path).await;

            let builder = template.try_clone().context("Failed to clone request builder")?;
            let resp = builder.send().await.context("HTTP request failed")?;

            // 2. Captura cabeçalhos X-RateLimit-* de todas as respostas e atualiza o bucket
            self.rate_limiter.update_from_headers(path, resp.headers()).await;

            let status = resp.status();

            // 3. Se receber 429, aguarda Retry-After e retenta automaticamente sem explodir com panic
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                retries += 1;
                if retries > MAX_RETRIES {
                    let error_text = resp.text().await.unwrap_or_default();
                    bail!("Discord API returned HTTP 429 after {} retries: {}", retries, error_text);
                }

                let wait_duration = self.rate_limiter.handle_429(path, resp.headers()).await;
                log::warn!(
                    "[DiscordApiClient] HTTP 429 em '{}'. Aguardando {:?} antes de retentar ({}/{})",
                    path, wait_duration, retries, MAX_RETRIES
                );
                tokio::time::sleep(wait_duration).await;
                continue;
            }

            // Permite 2xx e 304 Not Modified
            if !status.is_success() && status != reqwest::StatusCode::NOT_MODIFIED {
                let error_text = resp.text().await.unwrap_or_default();
                bail!("Discord API returned HTTP {}: {}", status, error_text);
            }

            return Ok(resp);
        }
    }

    /// Fetches the authenticated user profile from /users/@me
    pub async fn get_current_user(&self) -> Result<User> {
        let path = "/users/@me";
        let etag = self.cache_engine.get_etag(path).await;

        let mut req = self.auth_request(reqwest::Method::GET, path);
        if let Some(ref tag) = etag {
            req = req.header("If-None-Match", tag);
        }

        let resp = self.send_request(path, req).await?;

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            log::debug!("[DiscordApiClient] /users/@me retornou 304 Not Modified. Carregando do cache em disco.");
            if let Some(cached_user) = self.cache_engine.load_current_user() {
                *self.current_user_id.write().await = Some(cached_user.id.clone());
                return Ok(cached_user);
            }
        }

        if let Some(new_etag) = resp.headers().get("ETag").and_then(|v| v.to_str().ok()) {
            self.cache_engine.set_etag(path, new_etag).await;
        }

        let val: serde_json::Value = resp.json().await?;
        let user = parse_user_from_json(&val).context("Failed to parse user from /users/@me")?;
        *self.current_user_id.write().await = Some(user.id.clone());
        self.cache_engine.save_current_user(&user);
        Ok(user)
    }

    /// Fetches a user's full profile (/users/{user_id}/profile)
    pub async fn get_user_profile(&self, user_id: &str) -> Result<UserProfileData> {
        self.get_user_profile_with_guild(user_id, None).await
    }

    /// Fetches a user's full profile optionally within a guild context (/users/{user_id}/profile)
    pub async fn get_user_profile_with_guild(
        &self,
        user_id: &str,
        guild_id: Option<&str>,
    ) -> Result<UserProfileData> {
        let mut path = format!(
            "/users/{}/profile?with_mutual_guilds=true&with_mutual_friends_count=true",
            user_id
        );
        if let Some(gid) = guild_id {
            path.push_str(&format!("&guild_id={}", gid));
        }

        let req = self.auth_request(reqwest::Method::GET, &path);
        let resp = self.send_request(&path, req).await?;
        let val: serde_json::Value = resp.json().await?;
        let profile = parse_user_profile_from_json(&val)
            .context("Failed to parse user profile from API response")?;
        Ok(profile)
    }

    /// Fetches all direct message (DM) channels for the authenticated user (/users/@me/channels)
    pub async fn list_dm_channels(&self) -> Result<Vec<Channel>> {
        let path = "/users/@me/channels";
        let req = self.auth_request(reqwest::Method::GET, path);
        let resp = self.send_request(path, req).await?;
        let raw_channels: Vec<serde_json::Value> = resp.json().await?;

        let mut channels = Vec::new();
        for val in raw_channels {
            if let Some(ch) = parse_channel_from_json(&val, None) {
                channels.push(ch);
            }
        }
        Ok(channels)
    }

    /// Creates or opens an existing direct message (DM) channel with a specific user (/users/@me/channels)
    pub async fn create_or_get_dm_channel(&self, recipient_id: &str) -> Result<Channel> {
        let path = "/users/@me/channels";
        let body = serde_json::json!({
            "recipient_id": recipient_id
        });
        let req = self.auth_request(reqwest::Method::POST, path).json(&body);
        let resp = self.send_request(path, req).await?;
        let val: serde_json::Value = resp.json().await?;

        let channel = parse_channel_from_json(&val, None)
            .context("Failed to deserialize DM channel response from Discord API")?;

        Ok(channel)
    }

    /// Fetches all user relationships (friends, requests, blocked) (/users/@me/relationships)
    pub async fn list_relationships(&self) -> Result<Vec<Relationship>> {
        let path = "/users/@me/relationships";
        let req = self.auth_request(reqwest::Method::GET, path);
        let resp = self.send_request(path, req).await?;
        let raw_relationships: Vec<serde_json::Value> = resp.json().await?;

        let mut relationships = Vec::new();
        for r in raw_relationships {
            if let Some(rel) = parse_relationship_from_json(&r) {
                relationships.push(rel);
            }
        }
        Ok(relationships)
    }

    /// Sends a friend request to a user (/users/@me/relationships)
    pub async fn send_friend_request(&self, username: &str, discriminator: Option<&str>) -> Result<()> {
        let path = "/users/@me/relationships";
        let mut body = serde_json::json!({
            "username": username,
        });
        if let Some(disc) = discriminator {
            body["discriminator"] = serde_json::Value::String(disc.to_string());
        }
        let req = self.auth_request(reqwest::Method::POST, path).json(&body);
        self.send_request(path, req).await?;
        Ok(())
    }

    /// Accepts an incoming friend request (/users/@me/relationships/{user_id})
    pub async fn accept_friend_request(&self, user_id: &str) -> Result<()> {
        let path = format!("/users/@me/relationships/{}", user_id);
        let req = self.auth_request(reqwest::Method::PUT, &path).json(&serde_json::json!({}));
        self.send_request(&path, req).await?;
        Ok(())
    }

    /// Removes a relationship (decline, cancel, unfriend, or unblock) (/users/@me/relationships/{user_id})
    pub async fn remove_relationship(&self, user_id: &str) -> Result<()> {
        let path = format!("/users/@me/relationships/{}", user_id);
        let req = self.auth_request(reqwest::Method::DELETE, &path);
        self.send_request(&path, req).await?;
        Ok(())
    }

    /// Fetches user settings (specifically guild_folders) from /users/@me/settings
    pub async fn get_user_guild_settings(&self) -> Result<Vec<GuildFolder>> {
        let path = "/users/@me/settings";
        let req = self.auth_request(reqwest::Method::GET, path);
        let resp = self.send_request(path, req).await?;
        let val: serde_json::Value = resp.json().await?;

        let raw_folders = val
            .get("guild_folders")
            .or_else(|| val.get("user_settings").and_then(|us| us.get("guild_folders")))
            .and_then(|v| v.as_array());

        let mut guild_folders = Vec::new();
        if let Some(folders) = raw_folders {
            for f in folders {
                let id = f.get("id").and_then(|v| v.as_str().map(ToString::to_string));
                let name = f.get("name").and_then(|v| v.as_str().map(ToString::to_string));
                let color = f.get("color").and_then(|v| v.as_u64().map(|c| c as u32));
                let guild_ids = f
                    .get("guild_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|g| g.as_str().map(ToString::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                guild_folders.push(GuildFolder {
                    id,
                    name,
                    color,
                    guild_ids,
                });
            }
        }

        Ok(guild_folders)
    }


    /// Fetches all custom emojis for a guild (/guilds/{guild_id}/emojis) with local disk cache
    pub async fn get_guild_emojis(&self, guild_id: &str) -> Result<Vec<DiscordEmoji>> {
        if let Some(cached) = self.media_cache.load_guild_emojis_from_disk(guild_id) {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
        let path = format!("/guilds/{}/emojis", guild_id);
        let req = self.auth_request(reqwest::Method::GET, &path);
        let resp = self.send_request(&path, req).await?;
        let raw_emojis: Vec<serde_json::Value> = resp.json().await?;
        let emojis = parse_emojis_from_value_array(&raw_emojis, guild_id);
        let _ = self.media_cache.save_guild_emojis_to_disk(guild_id, &emojis);
        Ok(emojis)
    }

    /// Fetches all custom stickers for a guild (/guilds/{guild_id}/stickers) with local disk cache
    pub async fn get_guild_stickers(&self, guild_id: &str) -> Result<Vec<DiscordSticker>> {
        if let Some(cached) = self.media_cache.load_guild_stickers_from_disk(guild_id) {
            if !cached.is_empty() {
                return Ok(cached);
            }
        }
        let path = format!("/guilds/{}/stickers", guild_id);
        let req = self.auth_request(reqwest::Method::GET, &path);
        let resp = self.send_request(&path, req).await?;
        let raw_stickers: Vec<serde_json::Value> = resp.json().await?;
        let stickers = parse_stickers_from_value_array(&raw_stickers, guild_id);
        let _ = self.media_cache.save_guild_stickers_to_disk(guild_id, &stickers);
        Ok(stickers)
    }

    /// Fetches official sticker packs from Discord (/sticker-packs?locale=pt-BR) with local fallback
    pub async fn get_sticker_packs(&self) -> Result<Vec<StickerPack>> {
        let url = "https://discord.com/api/v9/sticker-packs?locale=pt-BR";
        let req = self.auth_request_url(reqwest::Method::GET, url);

        match self.send_request("/sticker-packs", req).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(val) = resp.json::<serde_json::Value>().await {
                        let mut packs = Vec::new();
                        if let Some(array) = val.get("sticker_packs").and_then(|v| v.as_array()) {
                            for pack_val in array {
                                if let Some(pack) = parse_sticker_pack_from_json(pack_val) {
                                    packs.push(pack);
                                }
                            }
                        }
                        if !packs.is_empty() {
                            return Ok(packs);
                        }
                    }
                }
                log::warn!("[DiscordApiClient] get_sticker_packs returned non-success or empty, falling back to local packs");
                Ok(MediaCacheManager::get_default_sticker_packs())
            }
            Err(e) => {
                log::warn!("[DiscordApiClient] get_sticker_packs failed ({}). Falling back to local packs", e);
                Ok(MediaCacheManager::get_default_sticker_packs())
            }
        }
    }

    /// Fetches trending GIFs from Discord official API (/gifs/trending?locale=pt-BR&media_format=webm) with Tenor/local fallback
    pub async fn get_trending_gifs(&self) -> Result<Vec<TenorGif>> {
        let url = "https://discord.com/api/v9/gifs/trending?locale=pt-BR&media_format=webm";
        let req = self.auth_request_url(reqwest::Method::GET, url);

        match self.send_request("/gifs/trending", req).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(val) = resp.json::<serde_json::Value>().await {
                        let gifs = MediaCacheManager::parse_gifs_response(&val, Some("trending"));
                        if !gifs.is_empty() {
                            return Ok(gifs);
                        }
                    }
                }
                log::warn!("[DiscordApiClient] get_trending_gifs returned non-success or empty, falling back to curated GIFs");
                Ok(MediaCacheManager::get_curated_tenor_gifs_for_category("trending"))
            }
            Err(e) => {
                log::warn!("[DiscordApiClient] get_trending_gifs request failed ({}). Falling back to curated GIFs", e);
                Ok(MediaCacheManager::get_curated_tenor_gifs_for_category("trending"))
            }
        }
    }

    /// Searches GIFs via Discord official API (/gifs/search?q={query}&locale=pt-BR&media_format=mp4) with fallback
    pub async fn search_gifs(&self, query: &str) -> Result<Vec<TenorGif>> {
        let q = query.trim();
        if q.is_empty() {
            return self.get_trending_gifs().await;
        }

        let mut url = reqwest::Url::parse("https://discord.com/api/v9/gifs/search")
            .map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
        url.query_pairs_mut()
            .append_pair("q", q)
            .append_pair("locale", "pt-BR")
            .append_pair("media_format", "mp4");

        let req = self.auth_request_url(reqwest::Method::GET, url.as_str());

        match self.send_request("/gifs/search", req).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(val) = resp.json::<serde_json::Value>().await {
                        let gifs = MediaCacheManager::parse_gifs_response(&val, Some(q));
                        if !gifs.is_empty() {
                            return Ok(gifs);
                        }
                    }
                }
                log::warn!("[DiscordApiClient] search_gifs returned non-success or empty, falling back to curated search");
                let fallback = MediaCacheManager::curated_tenor_gifs();
                let filtered: Vec<TenorGif> = fallback
                    .into_iter()
                    .filter(|g| {
                        g.title.to_lowercase().contains(&q.to_lowercase())
                            || g.category.as_deref().map(|c| c.to_lowercase() == q.to_lowercase()).unwrap_or(false)
                    })
                    .collect();
                if filtered.is_empty() {
                    Ok(MediaCacheManager::curated_tenor_gifs())
                } else {
                    Ok(filtered)
                }
            }
            Err(e) => {
                log::warn!("[DiscordApiClient] search_gifs request failed ({}). Falling back to curated search", e);
                let fallback = MediaCacheManager::curated_tenor_gifs();
                let filtered: Vec<TenorGif> = fallback
                    .into_iter()
                    .filter(|g| {
                        g.title.to_lowercase().contains(&q.to_lowercase())
                            || g.category.as_deref().map(|c| c.to_lowercase() == q.to_lowercase()).unwrap_or(false)
                    })
                    .collect();
                if filtered.is_empty() {
                    Ok(MediaCacheManager::curated_tenor_gifs())
                } else {
                    Ok(filtered)
                }
            }
        }
    }

    /// Fetches the default soundboard sounds provided by Discord (/soundboard-default-sounds)
    pub async fn get_soundboard_default_sounds(&self) -> Result<Vec<SoundboardSound>> {
        let path = "/soundboard-default-sounds";
        let req = self.auth_request(reqwest::Method::GET, path);
        let resp = self.send_request(path, req).await?;
        let raw_sounds: Vec<serde_json::Value> = resp.json().await?;

        let mut sounds = Vec::new();
        for val in raw_sounds {
            let sound_id = val
                .get("sound_id")
                .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|n| Box::leak(n.to_string().into_boxed_str()) as &str)))
                .unwrap_or("0")
                .to_string();
            let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("sound").to_string();
            let volume = val.get("volume").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let emoji_id = val.get("emoji_id").and_then(|v| v.as_str().map(ToString::to_string));
            let emoji_name = val.get("emoji_name").and_then(|v| v.as_str().map(ToString::to_string));
            let available = val.get("available").and_then(|v| v.as_bool()).unwrap_or(true);

            sounds.push(SoundboardSound {
                sound_id,
                name,
                volume,
                emoji_id,
                emoji_name,
                guild_id: None,
                available,
            });
        }
        Ok(sounds)
    }

    /// Fetches the profile effects collectibles catalog (/collectibles-categories/v2)
    pub async fn get_profile_effects_catalog(&self) -> Result<crate::components::ProfileEffectCatalog> {
        let path = "/collectibles-categories/v2?include_bundles=true&variants_return_style=2";
        let req = self.auth_request(reqwest::Method::GET, path);
        let resp = self.send_request(path, req).await?;
        let raw_json: serde_json::Value = resp.json().await?;
        Ok(crate::components::ProfileEffectCatalog::from_categories_json(&raw_json))
    }

    /// Sends a sticker message to a text channel (/channels/{channel_id}/messages)
    pub async fn send_sticker_message(&self, channel_id: &str, sticker_id: &str) -> Result<Message> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(e) => {
                    log::warn!("Falha ao resolver DM para envio de figurinha {}: {:?}", recipient_id, e);
                    channel_id.to_string()
                }
            }
        } else {
            channel_id.to_string()
        };

        let path = format!("/channels/{}/messages", resolved_id);
        let body = json!({
            "content": "",
            "sticker_ids": [sticker_id]
        });
        let req = self.auth_request(reqwest::Method::POST, &path).json(&body);
        let resp = self.send_request(&path, req).await?;
        let val: serde_json::Value = resp.json().await?;

        let msg = parse_message_from_json(&val)
            .context("Failed to deserialize sent sticker message response from Discord API")?;

        self.cache_engine.apply_gateway_message_create(&msg).await;

        Ok(msg)
    }

    /// Send Opcode 4 Voice State Update to Discord Gateway.
    pub async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        let payload = json!({
            "op": 4,
            "d": {
                "guild_id": guild_id,
                "channel_id": channel_id,
                "self_mute": self_mute,
                "self_deaf": self_deaf,
            }
        });
        self.send_gateway_payload(&payload).await
    }

    /// Send an arbitrary JSON payload to the main Gateway WebSocket.
    pub async fn send_gateway_payload(&self, payload: &serde_json::Value) -> Result<()> {
        let text = serde_json::to_string(payload)?;
        let lock = self.outbound_gateway_tx.read().await;
        if let Some(ref tx) = *lock {
            tx.send(WsMessage::Text(text.into()))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send gateway message: {}", e))?;
            Ok(())
        } else {
            bail!("Gateway is not connected");
        }
    }

    /// Ring participants in a DM / private channel via POST /channels/{id}/call/ring
    pub async fn ring_call(&self, channel_id: &str, recipients: Option<Vec<String>>) -> Result<()> {
        let path = format!("/channels/{}/call/ring", channel_id);
        let body = if let Some(recips) = recipients {
            json!({ "recipients": recips })
        } else {
            json!({})
        };
        let req = self.auth_request(reqwest::Method::POST, &path).json(&body);
        let _ = self.send_request(&path, req).await?;
        Ok(())
    }

    /// Stop ringing participants in a DM / private channel via POST /channels/{id}/call/stop-ringing
    pub async fn stop_ringing_call(&self, channel_id: &str, recipients: Option<Vec<String>>) -> Result<()> {
        let path = format!("/channels/{}/call/stop-ringing", channel_id);
        let body = if let Some(recips) = recipients {
            json!({ "recipients": recips })
        } else {
            json!({})
        };
        let req = self.auth_request(reqwest::Method::POST, &path).json(&body);
        let _ = self.send_request(&path, req).await?;
        Ok(())
    }

    /// Get the latest captured voice session_id for the current user.
    pub async fn current_voice_session_id(&self) -> Option<String> {
        self.last_voice_session_id.read().await.clone()
    }

    /// Get the latest captured voice server information (token, endpoint, guild_id/channel_id).
    pub async fn current_voice_server(&self) -> Option<crate::protocol::VoiceServerData> {
        self.last_voice_server.read().await.clone()
    }
}

#[async_trait]
impl DiscordTransport for DiscordApiClient {
    async fn connect(&self) -> Result<()> {
        if self.is_connected.load(Ordering::SeqCst) {
            return Ok(());
        }

        let token_clone = self.token.clone();
        let event_tx = self.event_sender.clone();
        let is_connected = self.is_connected.clone();
        let last_sequence = self.last_sequence.clone();
        let session_id_lock = self.session_id.clone();
        let resume_gateway_lock = self.resume_gateway_url.clone();
        let cache_engine_clone = self.cache_engine.clone();
        let outbound_tx_lock = self.outbound_gateway_tx.clone();
        let current_user_id_lock = self.current_user_id.clone();
        let voice_session_id_lock = self.last_voice_session_id.clone();
        let voice_server_lock = self.last_voice_server.clone();

        let handle = tokio::spawn(async move {
            run_gateway_loop(
                token_clone,
                event_tx,
                is_connected,
                last_sequence,
                session_id_lock,
                resume_gateway_lock,
                cache_engine_clone,
                outbound_tx_lock,
                current_user_id_lock,
                voice_session_id_lock,
                voice_server_lock,
            )
            .await;
        });

        let mut lock = self.gateway_task.lock().await;
        *lock = Some(handle);

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.is_connected.store(false, Ordering::SeqCst);
        let mut lock = self.gateway_task.lock().await;
        if let Some(task) = lock.take() {
            task.abort();
        }
        *self.outbound_gateway_tx.write().await = None;
        let _ = self
            .event_sender
            .send(TransportEvent::Disconnected("Disconnected by client".to_string()));
        Ok(())
    }

    async fn get_current_user(&self) -> Result<User> {
        self.get_current_user().await
    }

    async fn list_guilds(&self) -> Result<Vec<Guild>> {
        let path = "/users/@me/guilds";
        let etag = self.cache_engine.get_etag(path).await;

        let mut req = self.auth_request(reqwest::Method::GET, path);
        if let Some(ref tag) = etag {
            req = req.header("If-None-Match", tag);
        }

        let resp = self.send_request(path, req).await?;

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            log::debug!("[DiscordApiClient] /users/@me/guilds retornou 304 Not Modified. Carregando do cache em disco.");
            if let Some(cached_guilds) = self.cache_engine.load_guilds() {
                return Ok(cached_guilds);
            }
        }

        if let Some(new_etag) = resp.headers().get("ETag").and_then(|v| v.to_str().ok()) {
            self.cache_engine.set_etag(path, new_etag).await;
        }

        let raw_guilds: Vec<serde_json::Value> = resp.json().await?;

        let mut guilds = Vec::new();
        for val in raw_guilds {
            let id = val.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = val.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let icon = val.get("icon").and_then(|v| v.as_str()).map(|hash| {
                if hash.starts_with("http://") || hash.starts_with("https://") {
                    hash.to_string()
                } else {
                    format!("https://cdn.discordapp.com/icons/{}/{}.png?size=96", id, hash)
                }
            });
            let owner = val.get("owner").and_then(|v| v.as_bool()).unwrap_or(false);

            let mut guild = Guild::new(id, name, if owner { "current_user" } else { "" });
            guild.icon = icon;
            if let Some(cached_emojis) = self.media_cache.load_guild_emojis_from_disk(id) {
                guild.emojis = cached_emojis;
            }
            if let Some(cached_stickers) = self.media_cache.load_guild_stickers_from_disk(id) {
                guild.stickers = cached_stickers;
            }
            guilds.push(guild);
        }

        self.cache_engine.save_guilds(&guilds);

        Ok(guilds)
    }

    async fn get_guild(&self, guild_id: &str) -> Result<Option<Guild>> {
        let path = format!("/guilds/{}?with_counts=true", guild_id);
        let req = self.auth_request(reqwest::Method::GET, &path);
        let resp = match self.send_request(&path, req).await {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("{:#}", e);
                if err_msg.contains("HTTP 404") {
                    return Ok(None);
                }
                return Err(e);
            }
        };

        if !resp.status().is_success() {
            bail!("Failed to get guild: HTTP {}", resp.status());
        }

        let val: serde_json::Value = resp.json().await?;
        let channels = self.list_channels(guild_id).await.unwrap_or_default();
        let roles = parse_roles_from_json(&val);

        let id = val.get("id").and_then(|v| v.as_str()).unwrap_or(guild_id);
        let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown Guild");
        let owner_id = val.get("owner_id").and_then(|v| v.as_str()).unwrap_or_default();
        let icon = val.get("icon").and_then(|v| v.as_str()).map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                hash.to_string()
            } else {
                format!("https://cdn.discordapp.com/icons/{}/{}.png?size=96", id, hash)
            }
        });

        let mut guild = Guild::new(id, name, owner_id)
            .with_channels(channels)
            .with_roles(roles);
        guild.icon = icon;
        if let Some(raw_emojis) = val.get("emojis").and_then(|v| v.as_array()) {
            guild.emojis = parse_emojis_from_value_array(raw_emojis, id);
        }
        if let Some(raw_stickers) = val.get("stickers").and_then(|v| v.as_array()) {
            guild.stickers = parse_stickers_from_value_array(raw_stickers, id);
        }

        Ok(Some(guild))
    }

    async fn get_guild_emojis(&self, guild_id: &str) -> Result<Vec<DiscordEmoji>> {
        self.get_guild_emojis(guild_id).await
    }

    async fn get_guild_stickers(&self, guild_id: &str) -> Result<Vec<DiscordSticker>> {
        self.get_guild_stickers(guild_id).await
    }

    async fn list_channels(&self, guild_id: &str) -> Result<Vec<Channel>> {
        let path = format!("/guilds/{}/channels", guild_id);
        let req = self.auth_request(reqwest::Method::GET, &path);
        let resp = self.send_request(&path, req).await?;
        let raw_channels: Vec<serde_json::Value> = resp.json().await?;

        let mut channels = Vec::new();
        for val in raw_channels {
            if let Some(ch) = parse_channel_from_json(&val, Some(guild_id)) {
                channels.push(ch);
            }
        }

        channels.sort_by_key(|c| c.position);
        Ok(channels)
    }

    async fn list_dm_channels(&self) -> Result<Vec<Channel>> {
        self.list_dm_channels().await
    }

    async fn create_or_get_dm_channel(&self, recipient_id: &str) -> anyhow::Result<Channel> {
        self.create_or_get_dm_channel(recipient_id).await
    }

    async fn list_relationships(&self) -> Result<Vec<Relationship>> {
        self.list_relationships().await
    }

    async fn send_friend_request(&self, username: &str, discriminator: Option<&str>) -> anyhow::Result<()> {
        self.send_friend_request(username, discriminator).await
    }

    async fn accept_friend_request(&self, user_id: &str) -> anyhow::Result<()> {
        self.accept_friend_request(user_id).await
    }

    async fn remove_relationship(&self, user_id: &str) -> anyhow::Result<()> {
        self.remove_relationship(user_id).await
    }

    async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<Message>> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(e) => {
                    log::warn!("Falha ao resolver DM para {}: {:?}", recipient_id, e);
                    channel_id.to_string()
                }
            }
        } else {
            channel_id.to_string()
        };

        let path = format!("/channels/{}/messages?limit={}", resolved_id, limit.min(100));
        let endpoint_key = format!("/channels/{}/messages", resolved_id);
        let etag = self.cache_engine.get_etag(&endpoint_key).await;

        let mut req = self.auth_request(reqwest::Method::GET, &path);
        if let Some(ref tag) = etag {
            req = req.header("If-None-Match", tag);
        }

        let resp = self.send_request(&path, req).await?;

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            log::debug!(
                "[DiscordApiClient] Mensagens do canal {} retornaram 304 Not Modified. Carregando do cache em disco.",
                resolved_id
            );
            let cached = self.cache_engine.get_channel_messages(&resolved_id).await;
            if !cached.is_empty() {
                return Ok(cached);
            }
        }

        if let Some(new_etag) = resp.headers().get("ETag").and_then(|v| v.to_str().ok()) {
            self.cache_engine.set_etag(&endpoint_key, new_etag).await;
        }

        let raw_messages: Vec<serde_json::Value> = resp.json().await?;

        let mut messages = Vec::new();
        for val in raw_messages.into_iter().rev() {
            if let Some(msg) = parse_message_from_json(&val) {
                messages.push(msg);
            }
        }

        self.cache_engine.save_channel_messages(&resolved_id, &messages).await;

        Ok(messages)
    }

    async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(e) => {
                    log::warn!("Falha ao resolver DM para envio {}: {:?}", recipient_id, e);
                    channel_id.to_string()
                }
            }
        } else {
            channel_id.to_string()
        };

        let path = format!("/channels/{}/messages", resolved_id);
        let body = json!({ "content": content });
        let req = self.auth_request(reqwest::Method::POST, &path).json(&body);
        let resp = self.send_request(&path, req).await?;
        let val: serde_json::Value = resp.json().await?;

        let msg = parse_message_from_json(&val)
            .context("Failed to deserialize sent message response from Discord API")?;

        self.cache_engine.apply_gateway_message_create(&msg).await;

        Ok(msg)
    }

    async fn send_sticker(&self, channel_id: &str, sticker_id: &str) -> Result<Message> {
        self.send_sticker_message(channel_id, sticker_id).await
    }

    async fn add_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(_) => channel_id.to_string(),
            }
        } else {
            channel_id.to_string()
        };

        let encoded_emoji = urlencoding_encode(emoji);
        let path = format!(
            "/channels/{}/messages/{}/reactions/{}/@me",
            resolved_id, message_id, encoded_emoji
        );
        let req = self.auth_request(reqwest::Method::PUT, &path);
        self.send_request(&path, req).await?;
        Ok(())
    }

    async fn remove_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(_) => channel_id.to_string(),
            }
        } else {
            channel_id.to_string()
        };

        let encoded_emoji = urlencoding_encode(emoji);
        let path = format!(
            "/channels/{}/messages/{}/reactions/{}/@me",
            resolved_id, message_id, encoded_emoji
        );
        let req = self.auth_request(reqwest::Method::DELETE, &path);
        self.send_request(&path, req).await?;
        Ok(())
    }

    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> anyhow::Result<()> {
        self.send_voice_state_update(guild_id, channel_id, self_mute, self_deaf).await
    }

    async fn ring_call(
        &self,
        channel_id: &str,
        recipients: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        self.ring_call(channel_id, recipients).await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_sender.subscribe()
    }
}

// --- Background Gateway WebSocket Loop ---

async fn run_gateway_loop(
    token: AuthToken,
    event_tx: broadcast::Sender<TransportEvent>,
    is_connected: Arc<AtomicBool>,
    last_sequence: Arc<AtomicI64>,
    session_id: Arc<RwLock<Option<String>>>,
    resume_gateway_url: Arc<RwLock<Option<String>>>,
    cache_engine: Arc<DiscordCacheEngine>,
    outbound_gateway_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>>,
    current_user_id: Arc<RwLock<Option<String>>>,
    last_voice_session_id: Arc<RwLock<Option<String>>>,
    last_voice_server: Arc<RwLock<Option<crate::protocol::VoiceServerData>>>,
) {
    let mut backoff_seconds = 1;

    loop {
        let gateway_url = {
            let resume_url = resume_gateway_url.read().await;
            resume_url
                .clone()
                .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string())
        };

        log::info!("Connecting to Discord Gateway: {}", gateway_url);

        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = None;
        ws_config.max_frame_size = None;

        match connect_async_with_config(&gateway_url, Some(ws_config), false).await {
            Ok((ws_stream, _)) => {
                log::info!("Connected to Discord Gateway WebSocket");
                is_connected.store(true, Ordering::SeqCst);
                let _ = event_tx.send(TransportEvent::Connected);
                backoff_seconds = 1;

                let (mut ws_write, mut ws_read) = ws_stream.split();

                // Wait for Op 10 Hello
                let mut heartbeat_interval_ms = 41250u64;
                if let Some(Ok(WsMessage::Text(text))) = ws_read.next().await {
                    if let Ok(payload) = serde_json::from_str::<GatewayPayload>(&text) {
                        if payload.op == 10 {
                            if let Some(d) = payload.d {
                                if let Some(interval) = d.get("heartbeat_interval").and_then(|v| v.as_u64()) {
                                    heartbeat_interval_ms = interval;
                                }
                            }
                        }
                    }
                }

                // Send Op 2 Identify (or Op 6 Resume if session exists)
                let sess = session_id.read().await.clone();
                let seq = last_sequence.load(Ordering::SeqCst);

                let identify_payload = if let (Some(sid), true) = (&sess, seq >= 0) {
                    // Op 6 Resume
                    json!({
                        "op": 6,
                        "d": {
                            "token": token.secret(),
                            "session_id": sid,
                            "seq": seq
                        }
                    })
                } else if token.token_type == TokenType::User {
                    // Op 2 Identify para User Token: NUNCA enviar intents! Enviar propriedades canônicas do Discord Desktop
                    json!({
                        "op": 2,
                        "d": {
                            "token": token.secret(),
                            "capabilities": 30717,
                            "properties": {
                                "os": "Linux",
                                "browser": "Discord Client",
                                "release_channel": "stable",
                                "client_version": "1.0.9168",
                                "os_version": "6.6.0",
                                "os_arch": "x64",
                                "app_arch": "x64",
                                "system_locale": "pt-BR",
                                "browser_user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.9168 Chrome/128.0.6613.186 Electron/32.2.0 Safari/537.36",
                                "browser_version": "32.2.0",
                                "client_build_number": 331705,
                                "native_build_number": null,
                                "client_event_source": null
                            },
                            "presence": {
                                "status": "online",
                                "since": 0,
                                "activities": [],
                                "afk": false
                            },
                            "compress": false,
                            "client_state": {
                                "guild_versions": {},
                                "highest_last_message_id": "0",
                                "read_state_version": 0,
                                "user_guild_settings_version": -1,
                                "user_settings_version": -1,
                                "private_channels_version": "0",
                                "api_code_version": 0
                            }
                        }
                    })
                } else {
                    // Op 2 Identify para Bot Token
                    // Intents: GUILDS (1) | GUILD_MESSAGES (512) | MESSAGE_CONTENT (32768) | GUILD_VOICE_STATES (128)
                    let intents = 1 | 512 | 32768 | 128;
                    json!({
                        "op": 2,
                        "d": {
                            "token": token.secret(),
                            "intents": intents,
                            "properties": {
                                "os": "linux",
                                "browser": "discord-rs",
                                "device": "discord-rs"
                            }
                        }
                    })
                };

                let _ = ws_write
                    .send(WsMessage::Text(identify_payload.to_string().into()))
                    .await;

                // Spawn Heartbeat Task
                let (heartbeat_stop_tx, mut heartbeat_stop_rx) = tokio::sync::oneshot::channel();
                let seq_tracker = last_sequence.clone();
                let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<WsMessage>(64);
                *outbound_gateway_tx.write().await = Some(cmd_tx.clone());

                let hb_tx_clone = cmd_tx.clone();
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_millis(heartbeat_interval_ms));
                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                let s = seq_tracker.load(Ordering::SeqCst);
                                let d = if s >= 0 { json!(s) } else { json!(null) };
                                let hb_msg = json!({ "op": 1, "d": d });
                                if hb_tx_clone.send(WsMessage::Text(hb_msg.to_string().into())).await.is_err() {
                                    break;
                                }
                            }
                            _ = &mut heartbeat_stop_rx => {
                                break;
                            }
                        }
                    }
                });

                // WebSocket Processing Loop
                loop {
                    tokio::select! {
                        Some(msg_to_send) = cmd_rx.recv() => {
                            if ws_write.send(msg_to_send).await.is_err() {
                                break;
                            }
                        }
                        incoming = ws_read.next() => {
                            match incoming {
                                Some(Ok(WsMessage::Text(text))) => {
                                    if let Ok(payload) = serde_json::from_str::<GatewayPayload>(&text) {
                                        if let Some(s) = payload.s {
                                             last_sequence.store(s, Ordering::SeqCst);
                                        }

                                        match payload.op {
                                            0 => {
                                                // Dispatch
                                                if let (Some(event_name), Some(data)) = (payload.t.as_deref(), payload.d) {
                                                    handle_dispatch(
                                                        event_name,
                                                        data,
                                                        &event_tx,
                                                        &session_id,
                                                        &resume_gateway_url,
                                                        &cache_engine,
                                                        &current_user_id,
                                                        &last_voice_session_id,
                                                        &last_voice_server,
                                                    ).await;
                                                }
                                            }
                                            1 => {
                                                // Heartbeat requested by server
                                                let s = last_sequence.load(Ordering::SeqCst);
                                                let d = if s >= 0 { json!(s) } else { json!(null) };
                                                let hb = json!({ "op": 1, "d": d });
                                                let _ = ws_write.send(WsMessage::Text(hb.to_string().into())).await;
                                            }
                                            7 => {
                                                // Reconnect
                                                log::info!("Gateway requested reconnect (Op 7)");
                                                break;
                                            }
                                            9 => {
                                                // Invalid session
                                                log::warn!("Gateway invalid session (Op 9)");
                                                *session_id.write().await = None;
                                                last_sequence.store(-1, Ordering::SeqCst);
                                                break;
                                            }
                                            11 => {
                                                // Heartbeat ACK
                                                log::trace!("Heartbeat ACK received");
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                Some(Ok(WsMessage::Close(frame))) => {
                                    log::warn!("Gateway connection closed: {:?}", frame);
                                    break;
                                }
                                Some(Err(e)) => {
                                    log::error!("Gateway read error: {}", e);
                                    break;
                                }
                                None => {
                                    log::warn!("Gateway stream ended");
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                let _ = heartbeat_stop_tx.send(());
                *outbound_gateway_tx.write().await = None;
                is_connected.store(false, Ordering::SeqCst);
                let _ = event_tx.send(TransportEvent::Disconnected("Gateway disconnected".to_string()));
            }
            Err(err) => {
                log::error!("Failed to connect to Discord gateway: {}", err);
                *outbound_gateway_tx.write().await = None;
                is_connected.store(false, Ordering::SeqCst);
                let _ = event_tx.send(TransportEvent::Disconnected(format!("Falha ao conectar no Gateway: {}", err)));
            }
        }

        tokio::time::sleep(Duration::from_secs(backoff_seconds)).await;
        backoff_seconds = (backoff_seconds * 2).min(30);
    }
}

async fn handle_dispatch(
    event_name: &str,
    data: serde_json::Value,
    event_tx: &broadcast::Sender<TransportEvent>,
    session_id: &Arc<RwLock<Option<String>>>,
    resume_gateway_url: &Arc<RwLock<Option<String>>>,
    cache_engine: &Arc<DiscordCacheEngine>,
    current_user_id: &Arc<RwLock<Option<String>>>,
    last_voice_session_id: &Arc<RwLock<Option<String>>>,
    last_voice_server: &Arc<RwLock<Option<crate::protocol::VoiceServerData>>>,
) {
    match event_name {
        "READY" => {
            if let Some(sid) = data.get("session_id").and_then(|v| v.as_str()) {
                *session_id.write().await = Some(sid.to_string());
            }
            if let Some(r_url) = data.get("resume_gateway_url").and_then(|v| v.as_str()) {
                *resume_gateway_url.write().await = Some(r_url.to_string());
            }

            let user = data
                .get("user")
                .and_then(parse_user_from_json)
                .unwrap_or_else(|| User::new("me", "Authenticated User"));

            *current_user_id.write().await = Some(user.id.clone());

            let mut guilds = Vec::new();
            if let Some(raw_guilds) = data.get("guilds").and_then(|v| v.as_array()) {
                for g in raw_guilds {
                    let id = g.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("Server");
                    let icon = g.get("icon").and_then(|v| v.as_str()).map(|hash| {
                        if hash.starts_with("http://") || hash.starts_with("https://") {
                            hash.to_string()
                        } else {
                            format!("https://cdn.discordapp.com/icons/{}/{}.png?size=96", id, hash)
                        }
                    });
                    let mut guild = Guild::new(id, name, "");
                    guild.icon = icon;
                    if let Some(raw_emojis) = g.get("emojis").and_then(|v| v.as_array()) {
                        guild.emojis = parse_emojis_from_value_array(raw_emojis, id);
                    }
                    if let Some(raw_stickers) = g.get("stickers").and_then(|v| v.as_array()) {
                        guild.stickers = parse_stickers_from_value_array(raw_stickers, id);
                    }
                    guilds.push(guild);
                }
            }

            // Extrair pastas de servidores (guild_folders) de user_settings ou data
            let raw_folders = data
                .get("user_settings")
                .and_then(|us| us.get("guild_folders"))
                .or_else(|| data.get("guild_folders"))
                .and_then(|v| v.as_array());

            let mut guild_folders = Vec::new();
            if let Some(folders) = raw_folders {
                for f in folders {
                    let id = f.get("id").and_then(|v| v.as_str().map(ToString::to_string));
                    let name = f.get("name").and_then(|v| v.as_str().map(ToString::to_string));
                    let color = f.get("color").and_then(|v| v.as_u64().map(|c| c as u32));
                    let guild_ids = f
                        .get("guild_ids")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|g| g.as_str().map(ToString::to_string))
                                .collect()
                        })
                        .unwrap_or_default();

                    guild_folders.push(GuildFolder {
                        id,
                        name,
                        color,
                        guild_ids,
                    });
                }
            }

            // Atualiza cache em disco com o usuário autenticado e servidores
            cache_engine.save_current_user(&user);
            cache_engine.save_guilds(&guilds);

            // Discord web stores user settings / DM pins in binary Protobuf (PreloadedUserSettings); Gateway READY / REST doesn't expose them in JSON, so pinned DMs are managed locally in discord-rs AppState.
            let raw_pinned = data
                .get("user_settings")
                .and_then(|us| us.get("pinned_dms"))
                .or_else(|| data.get("pinned_dms"))
                .and_then(|v| v.as_array());

            let mut pinned_dms = Vec::new();
            if let Some(arr) = raw_pinned {
                for item in arr {
                    if let Some(id) = item.as_str() {
                        pinned_dms.push(id.to_string());
                    } else if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                        pinned_dms.push(id.to_string());
                    }
                }
            }

            // Extract appearance settings from user_settings_proto if present in READY
            let mut account_theme = None;
            let mut client_theme = None;
            if let Some(proto_b64) = data.get("user_settings_proto").and_then(|v| v.as_str()) {
                if let Ok(settings) = crate::protocol::user_settings_proto::decode_preloaded_user_settings(proto_b64) {
                    account_theme = settings.theme();
                    client_theme = settings.appearance.as_ref().and_then(|a| a.client_theme_settings.clone());
                }
            }

            let _ = event_tx.send(TransportEvent::Ready {
                user,
                guilds,
                guild_folders,
            });

            // Extrair relacionamentos iniciais do payload READY (Gateway)
            if let Some(raw_relationships) = data.get("relationships").and_then(|v| v.as_array()) {
                let mut relationships = Vec::new();
                for r in raw_relationships {
                    if let Some(rel) = parse_relationship_from_json(r) {
                        relationships.push(rel);
                    }
                }
                if !relationships.is_empty() {
                    let _ = event_tx.send(TransportEvent::RelationshipsUpdated(relationships));
                }
            }

            // Extrair presenças iniciais de amigos/contatos do payload READY
            if let Some(raw_presences) = data.get("presences").and_then(|v| v.as_array()) {
                for p in raw_presences {
                    if let Some(evt) = parse_presence_from_json(p) {
                        let _ = event_tx.send(evt);
                    }
                }
            }

            if !pinned_dms.is_empty() {
                let _ = event_tx.send(TransportEvent::PinnedDmsUpdated(pinned_dms));
            }

            if let Some(theme) = account_theme {
                let _ = event_tx.send(TransportEvent::AccountThemeUpdated {
                    theme,
                    client_theme_settings: client_theme,
                });
            }

            // Popula voice_states iniciais a partir do snapshot do READY
            if let Some(raw_guilds) = data.get("guilds").and_then(|v| v.as_array()) {
                for g in raw_guilds {
                    let g_id = g.get("id").and_then(|v| v.as_str());
                    if let Some(raw_states) = g.get("voice_states").and_then(|v| v.as_array()) {
                        for vs_val in raw_states {
                            if let Some(state) = parse_voice_state_from_json(vs_val, g_id) {
                                let _ = event_tx.send(TransportEvent::VoiceStateUpdate(state));
                            }
                        }
                    }
                }
            }
            if let Some(raw_states) = data.get("voice_states").and_then(|v| v.as_array()) {
                for vs_val in raw_states {
                    if let Some(state) = parse_voice_state_from_json(vs_val, None) {
                        let _ = event_tx.send(TransportEvent::VoiceStateUpdate(state));
                    }
                }
            }
        }
        "CHANNEL_CREATE" => {
            if let Some(ch) = parse_channel_from_json(&data, None) {
                if ch.channel_type == ChannelType::Dm || ch.channel_type == ChannelType::GroupDm {
                    let _ = event_tx.send(TransportEvent::DmChannelCreated(ch));
                } else {
                    let _ = event_tx.send(TransportEvent::ChannelCreate(ch));
                }
            }
        }
        "GUILD_CREATE" => {
            if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
                let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("Server");
                let owner_id = data.get("owner_id").and_then(|v| v.as_str()).unwrap_or_default();
                let icon = data.get("icon").and_then(|v| v.as_str()).map(|hash| {
                    if hash.starts_with("http://") || hash.starts_with("https://") {
                        hash.to_string()
                    } else {
                        format!("https://cdn.discordapp.com/icons/{}/{}.png?size=96", id, hash)
                    }
                });

                let mut channels = Vec::new();
                if let Some(raw_channels) = data.get("channels").and_then(|v| v.as_array()) {
                    for val in raw_channels {
                        if let Some(ch) = parse_channel_from_json(val, Some(id)) {
                            channels.push(ch);
                        }
                    }
                }
                let roles = parse_roles_from_json(&data);
                let mut guild = Guild::new(id, name, owner_id)
                    .with_channels(channels)
                    .with_roles(roles);
                guild.icon = icon;
                if let Some(raw_emojis) = data.get("emojis").and_then(|v| v.as_array()) {
                    guild.emojis = parse_emojis_from_value_array(raw_emojis, id);
                }
                if let Some(raw_stickers) = data.get("stickers").and_then(|v| v.as_array()) {
                    guild.stickers = parse_stickers_from_value_array(raw_stickers, id);
                }

                // Popula voice_states a partir do snapshot inicial de GUILD_CREATE
                if let Some(raw_states) = data.get("voice_states").and_then(|v| v.as_array()) {
                    let mut guild_members: std::collections::HashMap<String, (Option<String>, Option<String>, Option<AvatarDecorationData>)> = std::collections::HashMap::new();
                    if let Some(raw_members) = data.get("members").and_then(|v| v.as_array()) {
                        for m in raw_members {
                            if let Some(user) = m.get("user") {
                                if let Some(uid) = user.get("id").and_then(|v| v.as_str()) {
                                    let name = m.get("nick").and_then(|v| v.as_str())
                                        .or_else(|| user.get("global_name").and_then(|v| v.as_str()))
                                        .or_else(|| user.get("username").and_then(|v| v.as_str()))
                                        .map(ToString::to_string);
                                    let avatar = user.get("avatar").and_then(|v| v.as_str()).map(|hash| {
                                        if hash.starts_with("http") {
                                            hash.to_string()
                                        } else {
                                            format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=64", uid, hash)
                                        }
                                    });
                                    let decor = parse_avatar_decoration_data(m)
                                        .or_else(|| parse_avatar_decoration_data(user));
                                    guild_members.insert(uid.to_string(), (name, avatar, decor));
                                }
                            }
                        }
                    }

                    for vs_val in raw_states {
                        if let Some(mut state) = parse_voice_state_from_json(vs_val, Some(id)) {
                            if state.user_name.is_none() {
                                if let Some((name, avatar, decor)) = guild_members.get(&state.user_id) {
                                    state.user_name = name.clone();
                                    if state.avatar_url.is_none() {
                                        state.avatar_url = avatar.clone();
                                    }
                                    if state.avatar_decoration_data.is_none() {
                                        state.avatar_decoration_data = decor.clone();
                                    }
                                }
                            } else if state.avatar_decoration_data.is_none() {
                                if let Some((_, _, decor)) = guild_members.get(&state.user_id) {
                                    state.avatar_decoration_data = decor.clone();
                                }
                            }
                            let _ = event_tx.send(TransportEvent::VoiceStateUpdate(state));
                        }
                    }
                }

                let _ = event_tx.send(TransportEvent::GuildCreate(guild));
            }
        }
        "GUILD_EMOJIS_UPDATE" => {
            if let Some(guild_id) = data.get("guild_id").and_then(|v| v.as_str()) {
                if let Some(raw_emojis) = data.get("emojis").and_then(|v| v.as_array()) {
                    let emojis = parse_emojis_from_value_array(raw_emojis, guild_id);
                    let _ = event_tx.send(TransportEvent::GuildEmojisUpdated {
                        guild_id: guild_id.to_string(),
                        emojis,
                    });
                }
            }
        }
        "GUILD_STICKERS_UPDATE" => {
            if let Some(guild_id) = data.get("guild_id").and_then(|v| v.as_str()) {
                if let Some(raw_stickers) = data.get("stickers").and_then(|v| v.as_array()) {
                    let stickers = parse_stickers_from_value_array(raw_stickers, guild_id);
                    let _ = event_tx.send(TransportEvent::GuildStickersUpdated {
                        guild_id: guild_id.to_string(),
                        stickers,
                    });
                }
            }
        }
        "MESSAGE_CREATE" => {
            if let Some(msg) = parse_message_from_json(&data) {
                let cache = cache_engine.clone();
                let msg_clone = msg.clone();
                tokio::spawn(async move {
                    cache.apply_gateway_message_create(&msg_clone).await;
                });
                let _ = event_tx.send(TransportEvent::MessageCreate(msg));
            }
        }
        "MESSAGE_UPDATE" => {
            if let Some(msg) = parse_message_from_json(&data) {
                let cache = cache_engine.clone();
                let c_id = msg.channel_id.clone();
                let m_id = msg.id.clone();
                let content = msg.content.clone();
                tokio::spawn(async move {
                    cache.apply_gateway_message_update(&c_id, &m_id, &content).await;
                });
                let _ = event_tx.send(TransportEvent::MessageUpdate(msg));
            }
        }
        "MESSAGE_DELETE" => {
            if let (Some(id), Some(ch_id)) = (
                data.get("id").and_then(|v| v.as_str()),
                data.get("channel_id").and_then(|v| v.as_str()),
            ) {
                let cache = cache_engine.clone();
                let c_id = ch_id.to_string();
                let m_id = id.to_string();
                tokio::spawn(async move {
                    cache.apply_gateway_message_delete(&c_id, &m_id).await;
                });
                let _ = event_tx.send(TransportEvent::MessageDelete {
                    message_id: id.to_string(),
                    channel_id: ch_id.to_string(),
                });
            }
        }
        "MESSAGE_REACTION_ADD" => {
            if let (Some(ch_id), Some(msg_id), Some(user_id)) = (
                data.get("channel_id").and_then(|v| v.as_str()),
                data.get("message_id").and_then(|v| v.as_str()),
                data.get("user_id").and_then(|v| v.as_str()),
            ) {
                let emoji = data
                    .get("emoji")
                    .and_then(|e| e.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("👍")
                    .to_string();

                let _ = event_tx.send(TransportEvent::ReactionAdd {
                    channel_id: ch_id.to_string(),
                    message_id: msg_id.to_string(),
                    user_id: user_id.to_string(),
                    emoji,
                });
            }
        }
        "MESSAGE_REACTION_REMOVE" => {
            if let (Some(ch_id), Some(msg_id), Some(user_id)) = (
                data.get("channel_id").and_then(|v| v.as_str()),
                data.get("message_id").and_then(|v| v.as_str()),
                data.get("user_id").and_then(|v| v.as_str()),
            ) {
                let emoji = data
                    .get("emoji")
                    .and_then(|e| e.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("👍")
                    .to_string();

                let _ = event_tx.send(TransportEvent::ReactionRemove {
                    channel_id: ch_id.to_string(),
                    message_id: msg_id.to_string(),
                    user_id: user_id.to_string(),
                    emoji,
                });
            }
        }
        "VOICE_STATE_UPDATE" => {
            if let Some(state) = parse_voice_state_from_json(&data, None) {
                {
                    let my_id = current_user_id.read().await;
                    if my_id.as_deref() == Some(state.user_id.as_str()) && !state.session_id.is_empty() {
                        *last_voice_session_id.write().await = Some(state.session_id.clone());
                    }
                }

                let _ = event_tx.send(TransportEvent::VoiceStateUpdate(state));
            }
        }
        "VOICE_SERVER_UPDATE" => {
            let token = data.get("token").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let guild_id = data.get("guild_id").and_then(|v| v.as_str()).map(ToString::to_string);
            let channel_id = data.get("channel_id").and_then(|v| v.as_str()).map(ToString::to_string);
            let endpoint = data.get("endpoint").and_then(|v| v.as_str()).map(ToString::to_string);

            let server_data = crate::protocol::VoiceServerData {
                token: token.clone(),
                guild_id: guild_id.clone(),
                channel_id: channel_id.clone(),
                endpoint: endpoint.clone(),
            };
            *last_voice_server.write().await = Some(server_data);

            let _ = event_tx.send(TransportEvent::VoiceServerUpdate {
                token,
                guild_id,
                channel_id,
                endpoint,
            });
        }
        "TYPING_START" => {
            if let (Some(ch_id), Some(user_id)) = (
                data.get("channel_id").and_then(|v| v.as_str()),
                data.get("user_id").and_then(|v| v.as_str()),
            ) {
                let timestamp = data.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                let _ = event_tx.send(TransportEvent::TypingStart {
                    channel_id: ch_id.to_string(),
                    user_id: user_id.to_string(),
                    timestamp,
                });
            }
        }
        "RELATIONSHIP_ADD" => {
            if let Some(rel) = parse_relationship_from_json(&data) {
                let _ = event_tx.send(TransportEvent::RelationshipAdd(rel));
            }
        }
        "RELATIONSHIP_REMOVE" => {
            let id = data
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| data.get("user").and_then(|u| u.get("id")).and_then(|v| v.as_str()))
                .unwrap_or_default()
                .to_string();
            if !id.is_empty() {
                let _ = event_tx.send(TransportEvent::RelationshipRemove { id });
            }
        }
        "PRESENCE_UPDATE" => {
            if let Some(evt) = parse_presence_from_json(&data) {
                let _ = event_tx.send(evt);
            }
        }
        "USER_UPDATE" => {
            // Current-user profile changed (e.g. new avatar decoration equipped).
            // Persist to disk cache and propagate live to AppState::current_user
            // via TransportEvent::CurrentUserUpdated.
            if let Some(user) = parse_user_from_json(&data) {
                *current_user_id.write().await = Some(user.id.clone());
                cache_engine.save_current_user(&user);
                let _ = event_tx.send(TransportEvent::CurrentUserUpdated(user));
            }
        }
        "USER_SETTINGS_PROTO_UPDATE" => {
            // Live protobuf user settings update from Discord Gateway:
            // payload contains { "settings": "<base64_string>", "partial": bool }
            if let Some(settings_b64) = data.get("settings").and_then(|v| v.as_str()) {
                if let Ok(settings) = crate::protocol::user_settings_proto::decode_preloaded_user_settings(settings_b64) {
                    if let Some(theme) = settings.theme() {
                        let _ = event_tx.send(TransportEvent::AccountThemeUpdated {
                            theme,
                            client_theme_settings: settings.appearance.as_ref().and_then(|a| a.client_theme_settings.clone()),
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

// --- JSON Parsers for Discord Data Models ---

/// Deserializes a user relationship from Discord JSON payload.
pub fn parse_relationship_from_json(r: &serde_json::Value) -> Option<Relationship> {
    let id = r
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| r.get("user").and_then(|u| u.get("id")).and_then(|v| v.as_str()))?
        .to_string();
    let rel_type = r.get("type").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    let user_val = r.get("user");
    let username = user_val
        .and_then(|u| u.get("username"))
        .and_then(|v| v.as_str())
        .or_else(|| r.get("username").and_then(|v| v.as_str()))
        .unwrap_or("Desconhecido")
        .to_string();
    let global_name = user_val
        .and_then(|u| u.get("global_name"))
        .and_then(|v| v.as_str())
        .or_else(|| r.get("global_name").and_then(|v| v.as_str()))
        .map(ToString::to_string);
    let avatar_hash = user_val
        .and_then(|u| u.get("avatar"))
        .and_then(|v| v.as_str())
        .or_else(|| r.get("avatar").and_then(|v| v.as_str()));
    let avatar = avatar_hash.map(|hash| {
        if hash.starts_with("http://") || hash.starts_with("https://") {
            hash.to_string()
        } else {
            format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=96", id, hash)
        }
    });

    // Decoration comes from the nested `user` object of the relationship payload
    // (READY relationships, RELATIONSHIP_ADD dispatch, REST list). Absent/null -> None.
    let avatar_decoration_data = user_val
        .and_then(parse_avatar_decoration_data)
        .or_else(|| parse_avatar_decoration_data(r));

    Some(Relationship {
        id,
        rel_type,
        username,
        global_name,
        avatar,
        avatar_decoration_data,
    })
}

fn parse_user_from_json(val: &serde_json::Value) -> Option<User> {
    let id = val.get("id")?.as_str()?;
    let username = val.get("username")?.as_str()?;
    let mut user = User::new(id, username);
    user.discriminator = val.get("discriminator").and_then(|v| v.as_str()).map(ToString::to_string);
    user.global_name = val.get("global_name").and_then(|v| v.as_str()).map(ToString::to_string);
    user.avatar = val.get("avatar").and_then(|v| v.as_str()).map(ToString::to_string);
    user.bot = val.get("bot").and_then(|v| v.as_bool()).unwrap_or(false);
    user.avatar_decoration_data = parse_avatar_decoration_data(val);
    Some(user)
}

/// Parses an optional `avatar_decoration_data` object from any Discord user-like JSON value.
/// Returns None when the field is absent, null, or malformed.
fn parse_avatar_decoration_data(val: &serde_json::Value) -> Option<AvatarDecorationData> {
    let decor_val = val.get("avatar_decoration_data")?;
    if decor_val.is_null() {
        return None;
    }
    serde_json::from_value::<AvatarDecorationData>(decor_val.clone()).ok()
}

pub fn parse_user_profile_from_json(val: &serde_json::Value) -> Option<UserProfileData> {
    let user_val = val.get("user")?;
    let user = parse_user_from_json(user_val)?;

    let bio = val
        .get("user_profile")
        .and_then(|up| up.get("bio"))
        .and_then(|v| v.as_str())
        .or_else(|| user_val.get("bio").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let pronouns = val
        .get("user_profile")
        .and_then(|up| up.get("pronouns"))
        .and_then(|v| v.as_str())
        .or_else(|| user_val.get("pronouns").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);

    let banner = user_val
        .get("banner")
        .and_then(|v| v.as_str())
        .or_else(|| {
            val.get("user_profile")
                .and_then(|up| up.get("banner"))
                .and_then(|v| v.as_str())
        })
        .map(ToString::to_string);

    let banner_color = user_val
        .get("banner_color")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let accent_color = user_val
        .get("accent_color")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            val.get("user_profile")
                .and_then(|up| up.get("accent_color"))
                .and_then(|v| v.as_u64())
        })
        .map(|v| v as u32);

    let mut badges = Vec::new();
    if let Some(arr) = val.get("badges").and_then(|v| v.as_array()) {
        for b in arr {
            if let (Some(id), Some(desc), Some(icon)) = (
                b.get("id").and_then(|v| v.as_str()),
                b.get("description").and_then(|v| v.as_str()),
                b.get("icon").and_then(|v| v.as_str()),
            ) {
                badges.push(ProfileBadge {
                    id: id.to_string(),
                    description: desc.to_string(),
                    icon: icon.to_string(),
                    link: b.get("link").and_then(|v| v.as_str()).map(ToString::to_string),
                });
            }
        }
    }

    let mut connected_accounts = Vec::new();
    if let Some(arr) = val.get("connected_accounts").and_then(|v| v.as_array()) {
        for ca in arr {
            if let (Some(t), Some(id), Some(name)) = (
                ca.get("type").and_then(|v| v.as_str()),
                ca.get("id").and_then(|v| v.as_str()),
                ca.get("name").and_then(|v| v.as_str()),
            ) {
                connected_accounts.push(ConnectedAccount {
                    account_type: t.to_string(),
                    id: id.to_string(),
                    name: name.to_string(),
                    verified: ca.get("verified").and_then(|v| v.as_bool()).unwrap_or(false),
                });
            }
        }
    }

    let mut mutual_guilds = Vec::new();
    if let Some(arr) = val.get("mutual_guilds").and_then(|v| v.as_array()) {
        for mg in arr {
            if let Some(id) = mg.get("id").and_then(|v| v.as_str()) {
                mutual_guilds.push(MutualGuild {
                    id: id.to_string(),
                    nick: mg.get("nick").and_then(|v| v.as_str()).map(ToString::to_string),
                });
            }
        }
    }

    let mutual_friends_count = val
        .get("mutual_friends_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let created_at = snowflake_to_datetime(&user.id);

    let joined_server_at = val
        .get("guild_member")
        .and_then(|gm| gm.get("joined_at"))
        .or_else(|| val.get("joined_at"))
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let profile_effect_id = val
        .get("user_profile")
        .and_then(|up| up.get("profile_effect"))
        .and_then(|pe| pe.get("id").or_else(|| pe.get("sku_id")))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    Some(UserProfileData {
        user,
        bio,
        pronouns,
        banner,
        banner_color,
        accent_color,
        profile_effect_id,
        badges,
        connected_accounts,
        mutual_guilds,
        mutual_friends_count,
        created_at,
        joined_server_at,
    })
}

pub(crate) fn parse_voice_state_from_json(val: &serde_json::Value, default_guild_id: Option<&str>) -> Option<VoiceState> {
    let user_id = val.get("user_id")?.as_str()?;
    let session_id = val.get("session_id").and_then(|v| v.as_str()).unwrap_or_default();
    let mut state = VoiceState::new(user_id, session_id);
    state.guild_id = val.get("guild_id").and_then(|v| v.as_str()).or(default_guild_id).map(ToString::to_string);
    state.channel_id = val.get("channel_id").and_then(|v| v.as_str()).map(ToString::to_string);
    state.mute = val.get("mute").and_then(|v| v.as_bool()).unwrap_or(false);
    state.deaf = val.get("deaf").and_then(|v| v.as_bool()).unwrap_or(false);
    state.self_mute = val.get("self_mute").and_then(|v| v.as_bool()).unwrap_or(false);
    state.self_deaf = val.get("self_deaf").and_then(|v| v.as_bool()).unwrap_or(false);
    state.self_video = val.get("self_video").and_then(|v| v.as_bool()).unwrap_or(false);
    state.self_stream = val.get("self_stream").and_then(|v| v.as_bool()).unwrap_or(false);
    state.suppress = val.get("suppress").and_then(|v| v.as_bool()).unwrap_or(false);

    if let Some(member) = val.get("member") {
        if let Some(user) = member.get("user") {
            state.user_name = member.get("nick").and_then(|v| v.as_str())
                .or_else(|| user.get("global_name").and_then(|v| v.as_str()))
                .or_else(|| user.get("username").and_then(|v| v.as_str()))
                .map(ToString::to_string);
            state.avatar_url = user.get("avatar").and_then(|v| v.as_str()).map(|hash| {
                if hash.starts_with("http") {
                    hash.to_string()
                } else {
                    format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=64", user_id, hash)
                }
            });
            // Guild-level decoration overrides the global user one in guild context
            // (see Guild Member Structure: `avatar_decoration_data` is "data for the
            // member's guild avatar decoration").
            state.avatar_decoration_data = parse_avatar_decoration_data(member)
                .or_else(|| parse_avatar_decoration_data(user));
        } else {
            state.avatar_decoration_data = parse_avatar_decoration_data(member);
        }
    } else if let Some(user) = val.get("user") {
        state.user_name = user.get("global_name").and_then(|v| v.as_str())
            .or_else(|| user.get("username").and_then(|v| v.as_str()))
            .map(ToString::to_string);
        state.avatar_url = user.get("avatar").and_then(|v| v.as_str()).map(|hash| {
            if hash.starts_with("http") {
                hash.to_string()
            } else {
                format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=64", user_id, hash)
            }
        });
        state.avatar_decoration_data = parse_avatar_decoration_data(user);
    }
    Some(state)
}

fn parse_channel_from_json(val: &serde_json::Value, default_guild_id: Option<&str>) -> Option<Channel> {
    let id = val.get("id")?.as_str()?;
    let raw_type = val.get("type")?.as_u64()?;
    let channel_type = ChannelType::from_discord_code(raw_type)?;

    let mut recipient_ids: Vec<String> = Vec::new();
    let mut recipient_names: Vec<String> = Vec::new();
    let mut recipient_avatar_decorations: Vec<Option<AvatarDecorationData>> = Vec::new();

    if let Some(recipients) = val.get("recipients").and_then(|v| v.as_array()) {
        for r in recipients {
            if let Some(uid) = r.get("id").and_then(|v| v.as_str()) {
                recipient_ids.push(uid.to_string());
            }
            if let Some(name) = r
                .get("global_name")
                .and_then(|v| v.as_str())
                .or_else(|| r.get("username").and_then(|v| v.as_str()))
            {
                recipient_names.push(name.to_string());
            }
            recipient_avatar_decorations.push(parse_avatar_decoration_data(r));
        }
    }

    let name: String = if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
        n.to_string()
    } else if !recipient_names.is_empty() {
        recipient_names.join(", ")
    } else {
        "Mensagem Direta".to_string()
    };

    let mut ch = Channel::new(id, &name, channel_type);
    ch.recipient_ids = recipient_ids;
    ch.recipient_names = recipient_names;
    ch.recipient_avatar_decorations = recipient_avatar_decorations;
    ch.guild_id = val
        .get("guild_id")
        .and_then(|v| v.as_str())
        .or(default_guild_id)
        .map(ToString::to_string);
    ch.category_id = val.get("parent_id").and_then(|v| v.as_str()).map(ToString::to_string);
    ch.topic = val.get("topic").and_then(|v| v.as_str()).map(ToString::to_string);
    ch.position = val.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    ch.last_message_id = val.get("last_message_id").and_then(|v| v.as_str()).map(ToString::to_string);
    ch.user_limit = val.get("user_limit").and_then(|v| v.as_u64()).map(|l| l as u32);
    ch.is_pinned = val
        .get("is_pinned")
        .and_then(|v| v.as_bool())
        .or_else(|| val.get("pinned").and_then(|v| v.as_bool()))
        .unwrap_or(false);

    let icon_url: Option<String> = val
        .get("recipients")
        .and_then(|v| v.as_array())
        .and_then(|recipients| recipients.first())
        .and_then(|first_rec| {
            let rec_id = first_rec.get("id")?.as_str()?;
            let rec_avatar = first_rec.get("avatar")?.as_str()?;
            Some(format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png?size=96",
                rec_id, rec_avatar
            ))
        })
        .or_else(|| {
            val.get("icon").and_then(|v| v.as_str()).map(|icon_hash| {
                format!(
                    "https://cdn.discordapp.com/channel-icons/{}/{}.png?size=96",
                    id, icon_hash
                )
            })
        });
    ch.icon_url = icon_url;

    Some(ch)
}

fn parse_roles_from_json(guild_val: &serde_json::Value) -> Vec<Role> {
    let mut roles = Vec::new();
    if let Some(raw_roles) = guild_val.get("roles").and_then(|v| v.as_array()) {
        for r in raw_roles {
            let id = r.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = r.get("name").and_then(|v| v.as_str()).unwrap_or_default();
            let color = r.get("color").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let hoist = r.get("hoist").and_then(|v| v.as_bool()).unwrap_or(false);
            let position = r.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let permissions = r
                .get("permissions")
                .and_then(|v| v.as_str())
                .unwrap_or("0")
                .to_string();

            roles.push(Role {
                id: id.to_string(),
                name: name.to_string(),
                color,
                hoist,
                position,
                permissions,
            });
        }
    }
    roles
}

fn parse_emojis_from_value_array(raw_emojis: &[serde_json::Value], guild_id: &str) -> Vec<DiscordEmoji> {
    let mut emojis = Vec::new();
    for val in raw_emojis {
        let id = val.get("id").and_then(|v| v.as_str()).map(ToString::to_string);
        let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("emoji").to_string();
        let animated = val.get("animated").and_then(|v| v.as_bool()).unwrap_or(false);
        let available = val.get("available").and_then(|v| v.as_bool()).unwrap_or(true);
        let roles = val
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|r| r.as_str().map(ToString::to_string)).collect())
            .unwrap_or_default();

        emojis.push(DiscordEmoji {
            id,
            name,
            roles,
            user: val.get("user").cloned(),
            require_colons: val.get("require_colons").and_then(|v| v.as_bool()).unwrap_or(true),
            managed: val.get("managed").and_then(|v| v.as_bool()).unwrap_or(false),
            animated,
            available,
            guild_id: Some(guild_id.to_string()),
        });
    }
    emojis
}

fn parse_stickers_from_value_array(raw_stickers: &[serde_json::Value], guild_id: &str) -> Vec<DiscordSticker> {
    let mut stickers = Vec::new();
    for val in raw_stickers {
        let id = val.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("sticker").to_string();
        let description = val.get("description").and_then(|v| v.as_str().map(ToString::to_string));
        let tags = val.get("tags").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let pack_id = val.get("pack_id").and_then(|v| v.as_str()).map(ToString::to_string);
        let fmt_code = val.get("format_type").and_then(|v| v.as_u64()).unwrap_or(1);
        let format_type = match fmt_code {
            2 => StickerFormatType::Apng,
            3 => StickerFormatType::Lottie,
            4 => StickerFormatType::Gif,
            _ => StickerFormatType::Png,
        };
        let available = val.get("available").and_then(|v| v.as_bool()).unwrap_or(true);

        stickers.push(DiscordSticker {
            id,
            pack_id,
            name,
            description,
            tags,
            format_type,
            available,
            guild_id: Some(guild_id.to_string()),
        });
    }
    stickers
}

fn parse_sticker_pack_from_json(val: &serde_json::Value) -> Option<StickerPack> {
    let id = val.get("id").and_then(|v| v.as_str())?.to_string();
    let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("Sticker Pack").to_string();
    let sku_id = val.get("sku_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let description = val.get("description").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let banner_asset_id = val.get("banner_asset_id").and_then(|v| v.as_str()).map(ToString::to_string);

    let mut stickers = Vec::new();
    if let Some(arr) = val.get("stickers").and_then(|v| v.as_array()) {
        for s in arr {
            let s_id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let s_name = s.get("name").and_then(|v| v.as_str()).unwrap_or("sticker").to_string();
            let s_desc = s.get("description").and_then(|v| v.as_str()).map(ToString::to_string);
            let s_tags = s.get("tags").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let s_pack_id = s.get("pack_id").and_then(|v| v.as_str()).map(ToString::to_string).or_else(|| Some(id.clone()));
            let fmt_code = s.get("format_type").and_then(|v| v.as_u64()).unwrap_or(1);
            let format_type = match fmt_code {
                2 => StickerFormatType::Apng,
                3 => StickerFormatType::Lottie,
                4 => StickerFormatType::Gif,
                _ => StickerFormatType::Png,
            };
            let available = s.get("available").and_then(|v| v.as_bool()).unwrap_or(true);

            stickers.push(DiscordSticker {
                id: s_id,
                pack_id: s_pack_id,
                name: s_name,
                description: s_desc,
                tags: s_tags,
                format_type,
                available,
                guild_id: None,
            });
        }
    }

    Some(StickerPack {
        id,
        name,
        sku_id,
        description,
        stickers,
        banner_asset_id,
    })
}

fn parse_message_from_json(val: &serde_json::Value) -> Option<Message> {
    let id = val.get("id")?.as_str()?;
    let channel_id = val.get("channel_id")?.as_str()?;
    let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("");

    let author_val = val.get("author")?;
    let author_id = author_val.get("id")?.as_str()?;
    let author_username = author_val.get("username")?.as_str()?;
    let mut author = Author::new(author_id, author_username);
    author.global_name = author_val.get("global_name").and_then(|v| v.as_str()).map(ToString::to_string);
    author.avatar = author_val.get("avatar").and_then(|v| v.as_str()).map(ToString::to_string);
    author.bot = author_val.get("bot").and_then(|v| v.as_bool()).unwrap_or(false);
    author.avatar_decoration_data = parse_avatar_decoration_data(author_val);

    let mut message = Message::new(id, channel_id, author, content);
    message.guild_id = val.get("guild_id").and_then(|v| v.as_str()).map(ToString::to_string);
    message.pinned = val.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);

    if let Some(ts_str) = val.get("timestamp").and_then(|v| v.as_str()) {
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
            message.timestamp = ts.with_timezone(&chrono::Utc);
        }
    }

    if let Some(raw_att) = val.get("attachments").and_then(|v| v.as_array()) {
        for a in raw_att {
            if let (Some(a_id), Some(filename), Some(url)) = (
                a.get("id").and_then(|v| v.as_str()),
                a.get("filename").and_then(|v| v.as_str()),
                a.get("url").and_then(|v| v.as_str()),
            ) {
                message.attachments.push(Attachment {
                    id: a_id.to_string(),
                    filename: filename.to_string(),
                    size: a.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
                    url: url.to_string(),
                    proxy_url: a.get("proxy_url").and_then(|v| v.as_str()).map(ToString::to_string),
                    content_type: a.get("content_type").and_then(|v| v.as_str()).map(ToString::to_string),
                    width: a.get("width").and_then(|v| v.as_u64()).map(|w| w as u32),
                    height: a.get("height").and_then(|v| v.as_u64()).map(|h| h as u32),
                });
            }
        }
    }

    if let Some(raw_reactions) = val.get("reactions").and_then(|v| v.as_array()) {
        for r in raw_reactions {
            let emoji_name = r
                .get("emoji")
                .and_then(|e| e.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("👍");
            let count = r.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            let me = r.get("me").and_then(|v| v.as_bool()).unwrap_or(false);
            message.reactions.push(Reaction::new(emoji_name, count, me));
        }
    }

    if let Some(raw_embeds) = val.get("embeds").and_then(|v| v.as_array()) {
        for e in raw_embeds {
            let title = e.get("title").and_then(|v| v.as_str()).map(ToString::to_string);
            let description = e.get("description").and_then(|v| v.as_str()).map(ToString::to_string);
            let url = e.get("url").and_then(|v| v.as_str()).map(ToString::to_string);
            let color = e.get("color").and_then(|v| v.as_u64()).map(|c| c as u32);

            let mut fields = Vec::new();
            if let Some(raw_fields) = e.get("fields").and_then(|v| v.as_array()) {
                for f in raw_fields {
                    if let (Some(name), Some(value)) = (
                        f.get("name").and_then(|v| v.as_str()),
                        f.get("value").and_then(|v| v.as_str()),
                    ) {
                        let inline = f.get("inline").and_then(|v| v.as_bool()).unwrap_or(false);
                        fields.push(EmbedField {
                            name: name.to_string(),
                            value: value.to_string(),
                            inline,
                        });
                    }
                }
            }

            message.embeds.push(Embed {
                title,
                description,
                url,
                color,
                author: None,
                fields,
                footer: None,
                image_url: e.get("image").and_then(|img| img.get("url")).and_then(|v| v.as_str()).map(ToString::to_string),
                thumbnail_url: e.get("thumbnail").and_then(|th| th.get("url")).and_then(|v| v.as_str()).map(ToString::to_string),
                timestamp: None,
            });
        }
    }

    if let Some(raw_stickers) = val.get("sticker_items").and_then(|v| v.as_array()) {
        for s in raw_stickers {
            if let (Some(s_id), Some(s_name)) = (
                s.get("id").and_then(|v| v.as_str()),
                s.get("name").and_then(|v| v.as_str()),
            ) {
                let fmt = s.get("format_type").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                message.sticker_items.push(MessageStickerItem::new(s_id, s_name, fmt));
            }
        }
    }

    Some(message)
}

fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

pub(crate) fn parse_presence_from_json(val: &serde_json::Value) -> Option<TransportEvent> {
    let user_id = val
        .get("user")
        .and_then(|u| {
            if let Some(id) = u.get("id").and_then(|v| v.as_str()) {
                Some(id.to_string())
            } else if let Some(s) = u.as_str() {
                Some(s.to_string())
            } else {
                None
            }
        })
        .or_else(|| val.get("user_id").and_then(|v| v.as_str()).map(ToString::to_string))?;

    let status_str = val.get("status").and_then(|v| v.as_str()).unwrap_or("offline");
    let status = match status_str.to_lowercase().as_str() {
        "online" => PresenceStatus::Online,
        "idle" => PresenceStatus::Idle,
        "dnd" => PresenceStatus::DoNotDisturb,
        _ => PresenceStatus::Offline,
    };

    let mut primary_activity: Option<Activity> = None;
    let mut custom_status: Option<CustomStatus> = None;

    if let Some(activities) = val.get("activities").and_then(|v| v.as_array()) {
        for act in activities {
            let type_num = act.get("type").and_then(|v| v.as_u64());
            let name = act.get("name").and_then(|v| v.as_str()).unwrap_or("");

            // Discord Activity Type 4 is Custom Status
            let is_custom = type_num == Some(4) || name.eq_ignore_ascii_case("Custom Status");
            if is_custom {
                let state_text = act.get("state").and_then(|v| v.as_str()).map(ToString::to_string);
                let emoji = act.get("emoji").and_then(|e| {
                    let emoji_name = e.get("name").and_then(|v| v.as_str())?;
                    let emoji_id = e.get("id").and_then(|v| v.as_str()).map(ToString::to_string);
                    let animated = e.get("animated").and_then(|v| v.as_bool()).unwrap_or(false);
                    Some(CustomStatusEmoji {
                        name: emoji_name.to_string(),
                        id: emoji_id,
                        animated,
                    })
                });

                if state_text.is_some() || emoji.is_some() {
                    custom_status = Some(CustomStatus {
                        text: state_text,
                        emoji,
                    });
                }
            } else if primary_activity.is_none() {
                let act_type = match type_num {
                    Some(0) => ActivityType::Playing,
                    Some(1) => ActivityType::Streaming,
                    Some(2) => ActivityType::Listening,
                    Some(3) => ActivityType::Watching,
                    Some(5) => ActivityType::Competing,
                    _ => ActivityType::Playing,
                };
                let mut activity = Activity::new(name, act_type);
                activity.details = act.get("details").and_then(|v| v.as_str()).map(ToString::to_string);
                activity.state = act.get("state").and_then(|v| v.as_str()).map(ToString::to_string);
                activity.created_at = act.get("created_at").and_then(|v| v.as_i64());
                primary_activity = Some(activity);
            }
        }
    }

    Some(TransportEvent::PresenceUpdate {
        user_id,
        status,
        activity: primary_activity,
        custom_status,
    })
}

#[cfg(test)]
mod presence_tests {
    use super::*;

    #[test]
    fn test_parse_presence_update_with_custom_status() {
        let json = serde_json::json!({
            "user": { "id": "100000000000000001" },
            "status": "online",
            "activities": [
                {
                    "name": "Custom Status",
                    "type": 4,
                    "state": "I found her... ( A )",
                    "emoji": {
                        "name": "🌸",
                        "id": null,
                        "animated": false
                    }
                },
                {
                    "name": "VALORANT",
                    "type": 0
                }
            ]
        });

        let event = parse_presence_from_json(&json).expect("should parse presence");
        match event {
            TransportEvent::PresenceUpdate { user_id, status, activity, custom_status } => {
                assert_eq!(user_id, "100000000000000001");
                assert_eq!(status, PresenceStatus::Online);
                let cs = custom_status.expect("custom status should be present");
                assert_eq!(cs.text, Some("I found her... ( A )".to_string()));
                assert_eq!(cs.emoji.unwrap().name, "🌸");
                let act = activity.expect("activity should be present");
                assert_eq!(act.name, "VALORANT");
            }
            _ => panic!("Expected PresenceUpdate event"),
        }
    }

    #[test]
    fn test_parse_presence_custom_emoji_and_idle() {
        let json = serde_json::json!({
            "user_id": "999888777",
            "status": "idle",
            "activities": [
                {
                    "name": "Custom Status",
                    "type": 4,
                    "state": "Studying",
                    "emoji": {
                        "name": "pepe_study",
                        "id": "12345678",
                        "animated": true
                    }
                }
            ]
        });

        let event = parse_presence_from_json(&json).expect("should parse presence");
        match event {
            TransportEvent::PresenceUpdate { user_id, status, activity, custom_status } => {
                assert_eq!(user_id, "999888777");
                assert_eq!(status, PresenceStatus::Idle);
                assert!(activity.is_none());
                let cs = custom_status.unwrap();
                assert_eq!(cs.text, Some("Studying".to_string()));
                let emoji = cs.emoji.unwrap();
                assert_eq!(emoji.name, "pepe_study");
                assert_eq!(emoji.id, Some("12345678".to_string()));
                assert!(emoji.animated);
                assert_eq!(
                    emoji.image_url(),
                    Some("https://cdn.discordapp.com/emojis/12345678.gif?size=48".to_string())
                );
            }
            _ => panic!("Expected PresenceUpdate"),
        }
    }
}

#[cfg(test)]
mod avatar_decoration_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_user_avatar_decoration_present_absent_null() {
        let with = serde_json::json!({
            "id": "100000000000000011",
            "username": "synth_deco_user",
            "avatar_decoration_data": { "asset": "a_synth_user_deco", "sku_id": "111" }
        });
        let user = parse_user_from_json(&with).expect("should parse user");
        let decor = user.avatar_decoration_data.as_ref().expect("decoration must be present");
        assert_eq!(decor.asset, "a_synth_user_deco");
        assert_eq!(decor.sku_id.as_deref(), Some("111"));

        let absent = serde_json::json!({ "id": "100000000000000012", "username": "synth_plain" });
        let plain = parse_user_from_json(&absent).expect("should parse user");
        assert_eq!(plain.avatar_decoration_data, None);

        let null = serde_json::json!({
            "id": "100000000000000013",
            "username": "synth_null",
            "avatar_decoration_data": null
        });
        let nulled = parse_user_from_json(&null).expect("should parse user");
        assert_eq!(nulled.avatar_decoration_data, None);
    }

    #[test]
    fn test_parse_message_author_avatar_decoration() {
        let payload = serde_json::json!({
            "id": "200000000000000021",
            "channel_id": "300000000000000031",
            "content": "hello",
            "timestamp": "2026-10-01T12:00:00.000Z",
            "author": {
                "id": "100000000000000014",
                "username": "synth_author",
                "avatar_decoration_data": { "asset": "a_synth_msg_deco", "sku_id": "114" }
            }
        });
        let msg = parse_message_from_json(&payload).expect("should parse message");
        let decor = msg.author.avatar_decoration_data.as_ref().expect("author decoration must be present");
        assert_eq!(decor.asset, "a_synth_msg_deco");
        assert_eq!(decor.sku_id.as_deref(), Some("114"));

        let bare = serde_json::json!({
            "id": "200000000000000022",
            "channel_id": "300000000000000031",
            "content": "hi",
            "author": { "id": "100000000000000015", "username": "synth_bare" }
        });
        let msg_bare = parse_message_from_json(&bare).expect("should parse message");
        assert_eq!(msg_bare.author.avatar_decoration_data, None);
    }

    #[test]
    fn test_parse_dm_channel_recipient_decorations() {
        let payload = serde_json::json!({
            "id": "300000000000000032",
            "type": 1,
            "recipients": [
                {
                    "id": "100000000000000016",
                    "username": "synth_recip",
                    "avatar_decoration_data": { "asset": "a_synth_dm_deco", "sku_id": "116" }
                }
            ]
        });
        let ch = parse_channel_from_json(&payload, None).expect("should parse DM channel");
        assert_eq!(ch.recipient_avatar_decorations.len(), 1);
        let decor = ch.recipient_avatar_decorations[0].as_ref().expect("recipient decoration must be present");
        assert_eq!(decor.asset, "a_synth_dm_deco");

        let bare = serde_json::json!({
            "id": "300000000000000033",
            "type": 1,
            "recipients": [{ "id": "100000000000000017", "username": "synth_plain_recip" }]
        });
        let ch_bare = parse_channel_from_json(&bare, None).expect("should parse DM channel");
        assert_eq!(ch_bare.recipient_avatar_decorations, vec![None]);
    }

    #[test]
    fn test_parse_voice_state_guild_decoration_takes_precedence() {
        // Guild-level member.avatar_decoration_data wins over member.user.avatar_decoration_data
        // (Guild Member Structure: `avatar_decoration_data` is "data for the member's
        // guild avatar decoration" — https://discord.com/developers/docs/resources/guild#guild-member-object).
        let payload = serde_json::json!({
            "user_id": "100000000000000018",
            "session_id": "sess-synth-1",
            "channel_id": "400000000000000041",
            "member": {
                "nick": "SynthMember",
                "avatar_decoration_data": { "asset": "a_synth_guild_deco", "sku_id": "118" },
                "user": {
                    "id": "100000000000000018",
                    "username": "synth_voice_user",
                    "avatar_decoration_data": { "asset": "a_synth_global_deco", "sku_id": "218" }
                }
            }
        });
        let state = parse_voice_state_from_json(&payload, Some("500000000000000051"))
            .expect("should parse voice state");
        let decor = state.avatar_decoration_data.as_ref().expect("voice decoration must be present");
        assert_eq!(decor.asset, "a_synth_guild_deco");

        // Falls back to the nested user object when the member has no guild decoration.
        let fallback = serde_json::json!({
            "user_id": "100000000000000019",
            "session_id": "sess-synth-2",
            "member": {
                "user": {
                    "id": "100000000000000019",
                    "username": "synth_fallback",
                    "avatar_decoration_data": { "asset": "a_synth_user_deco", "sku_id": "119" }
                }
            }
        });
        let fb = parse_voice_state_from_json(&fallback, None).expect("should parse voice state");
        assert_eq!(
            fb.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
            Some("a_synth_user_deco")
        );

        // Absent everywhere -> None.
        let bare = serde_json::json!({
            "user_id": "100000000000000020",
            "session_id": "sess-synth-3"
        });
        let bare_state = parse_voice_state_from_json(&bare, None).expect("should parse voice state");
        assert_eq!(bare_state.avatar_decoration_data, None);
    }
}
