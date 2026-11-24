use crate::domain::profile::{
    ConnectedAccount, MutualGuild, ProfileBadge, UserProfileData, snowflake_to_datetime,
};
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
use crate::protocol::rate_limit::{
    RateLimitCoordinator, RateLimitHeaders, RequestCoalescer, retry_after_from_headers,
};
use crate::protocol::transport::{DiscordTransport, GuildFolder, Relationship, TransportEvent};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock, broadcast, oneshot};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::protocol::{Message as WsMessage, WebSocketConfig};

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const DEFAULT_GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const GATEWAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const GATEWAY_HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const GATEWAY_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_INITIAL_GATEWAY_ATTEMPTS: u32 = 5;
const MAX_GATEWAY_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatewayDisconnectKind {
    Resume,
    Identify,
    Fatal(&'static str),
}

impl GatewayDisconnectKind {
    fn reason(self) -> &'static str {
        match self {
            Self::Resume | Self::Identify => "Gateway desconectado; tentando reconectar.",
            Self::Fatal(reason) => reason,
        }
    }
}

fn gateway_close_action(code: Option<u16>) -> GatewayDisconnectKind {
    match code {
        Some(4003) | Some(4004) => GatewayDisconnectKind::Fatal(
            "Falha na autenticação: token inválido ou expirado. Faça login novamente.",
        ),
        Some(4010) => GatewayDisconnectKind::Fatal("Gateway recusou o shard informado."),
        Some(4011) => GatewayDisconnectKind::Fatal(
            "Esta conta exige sharding no Gateway; a conexão foi encerrada.",
        ),
        Some(4012) => {
            GatewayDisconnectKind::Fatal("A versão da API do Gateway não é aceita pelo Discord.")
        }
        Some(4013) => GatewayDisconnectKind::Fatal("O Discord recusou as intents do Gateway."),
        Some(4014) => GatewayDisconnectKind::Fatal(
            "Intents privilegiadas não habilitadas ou aprovadas no Developer Portal.",
        ),
        Some(4007 | 4009) => GatewayDisconnectKind::Identify,
        _ => GatewayDisconnectKind::Resume,
    }
}

#[derive(Debug, Clone, Copy)]
struct GatewayReconnectPolicy {
    failures: u32,
    next_delay: Duration,
}

impl GatewayReconnectPolicy {
    fn new() -> Self {
        Self {
            failures: 0,
            next_delay: Duration::from_secs(1),
        }
    }

    fn next_backoff(&mut self) -> Duration {
        let delay = self.next_delay;
        self.failures = self.failures.saturating_add(1);
        self.next_delay = (self.next_delay * 2).min(MAX_GATEWAY_BACKOFF);
        delay
    }

    fn on_authenticated(&mut self) {
        self.failures = 0;
        self.next_delay = Duration::from_secs(1);
    }

    fn initial_attempts_exhausted(&self) -> bool {
        self.failures >= MAX_INITIAL_GATEWAY_ATTEMPTS
    }
}

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
    rate_limiter: Arc<RateLimitCoordinator>,
    request_coalescer: Arc<RequestCoalescer>,
    cache_engine: Arc<DiscordCacheEngine>,
    media_cache: MediaCacheManager,
    event_sender: broadcast::Sender<TransportEvent>,
    is_connected: Arc<AtomicBool>,
    auth_failed: Arc<AtomicBool>,
    last_sequence: Arc<AtomicI64>,
    session_id: Arc<RwLock<Option<String>>>,
    resume_gateway_url: Arc<RwLock<Option<String>>>,
    gateway_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    outbound_gateway_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>>,
    current_user_id: Arc<RwLock<Option<String>>>,
    last_voice_session_id: Arc<RwLock<Option<String>>>,
    last_voice_server: Arc<RwLock<Option<crate::protocol::VoiceServerData>>>,
}

const DISCORD_DESKTOP_CLIENT_VERSION: &str = "1.0.170";
const DISCORD_DESKTOP_BROWSER_VERSION: &str = "33.2.0";
const DISCORD_DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.170 Chrome/130.0.6723.191 Electron/33.2.0 Safari/537.36";
const DISCORD_BOT_USER_AGENT: &str = "DiscordBot (https://github.com/SMCodesP/discord-rs, 0.1.0)";

fn build_user_client_properties() -> serde_json::Value {
    json!({
        "os": "Linux",
        "browser": "Discord Client",
        "release_channel": "stable",
        "client_version": DISCORD_DESKTOP_CLIENT_VERSION,
        "os_version": "6.11.0",
        "os_arch": "x64",
        "app_arch": "x64",
        "system_locale": "pt-BR",
        "browser_user_agent": DISCORD_DESKTOP_USER_AGENT,
        "browser_version": DISCORD_DESKTOP_BROWSER_VERSION,
        "client_build_number": 356995,
        "native_build_number": null,
        "client_event_source": null
    })
}

fn encode_base64_standard(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn build_super_properties_b64() -> String {
    let payload = serde_json::to_vec(&build_user_client_properties()).unwrap_or_default();
    encode_base64_standard(&payload)
}

fn build_http_client(token: &AuthToken) -> reqwest::Client {
    let ua = match token.token_type {
        TokenType::User => DISCORD_DESKTOP_USER_AGENT,
        TokenType::Bot => DISCORD_BOT_USER_AGENT,
    };
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(ua)
        .build()
        .expect("Failed to create HTTP client")
}

impl DiscordApiClient {
    /// Creates a new Discord API client with the provided authentication token.
    pub fn new(token: AuthToken) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        let http_client = build_http_client(&token);

        Self {
            token,
            http_client,
            rate_limiter: Arc::new(RateLimitCoordinator::new()),
            request_coalescer: Arc::new(RequestCoalescer::new()),
            cache_engine: Arc::new(DiscordCacheEngine::new()),
            media_cache: MediaCacheManager::default_path(),
            event_sender,
            is_connected: Arc::new(AtomicBool::new(false)),
            auth_failed: Arc::new(AtomicBool::new(false)),
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
        rate_limiter: Arc<RateLimitCoordinator>,
    ) -> Self {
        let (event_sender, _) = broadcast::channel(256);
        let http_client = build_http_client(&token);

        Self {
            token,
            http_client,
            rate_limiter,
            request_coalescer: Arc::new(RequestCoalescer::new()),
            cache_engine,
            media_cache: MediaCacheManager::default_path(),
            event_sender,
            is_connected: Arc::new(AtomicBool::new(false)),
            auth_failed: Arc::new(AtomicBool::new(false)),
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
    pub fn rate_limiter(&self) -> &Arc<RateLimitCoordinator> {
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

    fn apply_auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let builder = builder
            .header("Authorization", self.token.authorization_header())
            .header("Content-Type", "application/json");
        if self.token.token_type == TokenType::User {
            builder
                .header("X-Super-Properties", build_super_properties_b64())
                .header("X-Discord-Locale", "pt-BR")
                .header("X-Discord-Timezone", "America/Sao_Paulo")
                .header("Origin", "https://discord.com")
                .header("Referer", "https://discord.com/channels/@me")
                .header("Accept-Language", "pt-BR,pt;q=0.9,en-US;q=0.8,en;q=0.7")
        } else {
            builder
        }
    }

    fn auth_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", DISCORD_API_BASE, path);
        self.apply_auth_headers(self.http_client.request(method, url))
    }

    fn auth_request_url(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.apply_auth_headers(self.http_client.request(method, url))
    }

    async fn send_request(
        &self,
        path: &str,
        req: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        const MAX_RETRIES: usize = 3;
        const AUTH_ERROR: &str =
            "Falha na autenticação: token inválido ou expirado. Faça login novamente.";

        if self.auth_failed.load(Ordering::SeqCst) {
            bail!(AUTH_ERROR);
        }

        let Some(template) = req.try_clone() else {
            return self.execute_request_once(path, req).await;
        };

        let coalescing_key = template
            .try_clone()
            .and_then(|request| request.build().ok())
            .as_ref()
            .and_then(request_coalescing_key);

        if let Some(key) = coalescing_key {
            let this = self.clone();
            let coalescer = this.request_coalescer.clone();
            let route = path.to_string();
            let response = coalescer
                .coalesce(key, move || async move {
                    let response = this.execute_request(&route, template, MAX_RETRIES).await?;
                    crate::protocol::rate_limit::CoalescedResponse::capture(response).await
                })
                .await?;
            Ok(response.into_response())
        } else {
            self.execute_request(path, template, MAX_RETRIES).await
        }
    }

    async fn execute_request(
        &self,
        path: &str,
        template: reqwest::RequestBuilder,
        max_retries: usize,
    ) -> Result<reqwest::Response> {
        for retry in 0..=max_retries {
            if self.auth_failed.load(Ordering::SeqCst) {
                bail!("Falha na autenticação: token inválido ou expirado. Faça login novamente.");
            }

            self.rate_limiter.pre_acquire(path).await;
            let builder = template
                .try_clone()
                .context("Failed to clone request builder")?;
            let response = self.send_http(path, builder).await?;
            let headers = RateLimitHeaders::from_reqwest_headers(response.headers());
            self.rate_limiter.post_response(path, &headers).await;

            let status = response.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let response_headers = response.headers().clone();
                let body = response.bytes().await.unwrap_or_default();
                let payload = serde_json::from_slice::<serde_json::Value>(&body).ok();
                if retry == max_retries {
                    bail!(
                        "Discord API returned HTTP 429: Limite de requisições excedido; tente novamente."
                    );
                }

                let retry_after = retry_after_from_headers(&response_headers)
                    .or_else(|| {
                        payload
                            .as_ref()
                            .and_then(|value| value.get("retry_after"))
                            .and_then(serde_json::Value::as_f64)
                            .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
                    })
                    .unwrap_or_else(|| Duration::from_secs(1));
                let is_global = headers.is_global
                    || payload
                        .as_ref()
                        .and_then(|value| value.get("global"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);
                let wait = self
                    .rate_limiter
                    .handle_429(path, retry_after, is_global)
                    .await;
                log::warn!(
                    "[DiscordApiClient] HTTP 429 em '{}'; bucket bloqueado por {:?} (tentativa {}/{}).",
                    RateLimitCoordinator::normalize_route(path),
                    wait,
                    retry + 1,
                    max_retries
                );
                // pre_acquire aguarda o Retry-After na próxima tentativa.
                continue;
            }

            if !status.is_success() && status != reqwest::StatusCode::NOT_MODIFIED {
                let body = response.bytes().await.unwrap_or_default();
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    self.auth_failed.store(true, Ordering::SeqCst);
                }
                bail!("{}", api_error_message(status, &body));
            }

            return Ok(response);
        }

        unreachable!("o laço de retry sempre retorna ou termina no último erro 429")
    }

    async fn execute_request_once(
        &self,
        path: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        self.rate_limiter.pre_acquire(path).await;
        let response = self.send_http(path, request).await?;
        let headers = RateLimitHeaders::from_reqwest_headers(response.headers());
        self.rate_limiter.post_response(path, &headers).await;

        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::NOT_MODIFIED {
            let body = response.bytes().await.unwrap_or_default();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                self.auth_failed.store(true, Ordering::SeqCst);
            }
            bail!("{}", api_error_message(status, &body));
        }
        Ok(response)
    }

    async fn send_http(
        &self,
        path: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response> {
        request.send().await.map_err(|error| {
            // `without_url` remove query strings e userinfo dos erros do reqwest.
            let detail = error.without_url();
            log::warn!(
                "[DiscordApiClient] Falha de rede em '{}': {}",
                RateLimitCoordinator::normalize_route(path),
                detail
            );
            anyhow::anyhow!("Falha de rede ao acessar o Discord: {detail}")
        })
    }
}

fn request_coalescing_key(request: &reqwest::Request) -> Option<String> {
    if request.method() != reqwest::Method::GET {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    request.url().as_str().hash(&mut hasher);
    if let Some(etag) = request.headers().get(reqwest::header::IF_NONE_MATCH) {
        etag.hash(&mut hasher);
    }
    Some(format!("{:016x}", hasher.finish()))
}

fn api_error_message(status: reqwest::StatusCode, body: &[u8]) -> String {
    let payload = serde_json::from_slice::<serde_json::Value>(body).ok();
    let code = payload
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(serde_json::Value::as_u64);
    let discord_message = payload
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let requires_2fa = code == Some(60003)
        || discord_message.contains("two-factor")
        || discord_message.contains("two factor")
        || discord_message.contains("2fa")
        || discord_message.contains("verification");

    let message = match status {
        reqwest::StatusCode::UNAUTHORIZED => {
            "token inválido ou expirado. Faça login novamente.".to_string()
        }
        reqwest::StatusCode::FORBIDDEN if requires_2fa => {
            "verificação em duas etapas (2FA) necessária. Conclua a verificação no Discord e tente novamente."
                .to_string()
        }
        reqwest::StatusCode::FORBIDDEN => "acesso negado pelo Discord.".to_string(),
        reqwest::StatusCode::NOT_FOUND => "recurso não encontrado no Discord.".to_string(),
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            "limite de requisições excedido; tente novamente.".to_string()
        }
        status if status.is_server_error() => {
            "serviço do Discord indisponível; tente novamente.".to_string()
        }
        _ => "requisição recusada pelo Discord.".to_string(),
    };

    format!("Discord API returned HTTP {status}: {message}")
}

impl DiscordApiClient {
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
            log::debug!(
                "[DiscordApiClient] /users/@me retornou 304 Not Modified. Carregando do cache em disco."
            );
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

    fn credential_verification_request(&self) -> reqwest::RequestBuilder {
        self.auth_request(reqwest::Method::GET, "/users/@me")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
    }

    /// Verifica credenciais sem depender de ETag/cache local.
    /// Usado no commit de login: um 304 nunca é prova de autenticação.
    pub async fn verify_current_user(&self) -> Result<User> {
        let path = "/users/@me";
        let request = self.credential_verification_request();
        let response = self.send_request(path, request).await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            bail!("A verificação de credenciais retornou 304 inesperadamente");
        }
        let value: serde_json::Value = response.json().await?;
        let user = parse_user_from_json(&value)
            .context("Failed to parse verified user from /users/@me")?;
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
    pub async fn send_friend_request(
        &self,
        username: &str,
        discriminator: Option<&str>,
    ) -> Result<()> {
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
        let req = self
            .auth_request(reqwest::Method::PUT, &path)
            .json(&serde_json::json!({}));
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
            .or_else(|| {
                val.get("user_settings")
                    .and_then(|us| us.get("guild_folders"))
            })
            .and_then(|v| v.as_array());

        let mut guild_folders = Vec::new();
        if let Some(folders) = raw_folders {
            for f in folders {
                let id = f
                    .get("id")
                    .and_then(|v| v.as_str().map(ToString::to_string));
                let name = f
                    .get("name")
                    .and_then(|v| v.as_str().map(ToString::to_string));
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

    /// Lê as configurações protobuf da conta em `/users/@me/settings-proto/1`.
    /// O payload é decodificado localmente e nunca registrado em log.
    pub async fn get_user_settings_proto(
        &self,
    ) -> Result<crate::protocol::user_settings_proto::PreloadedUserSettings> {
        let path = "/users/@me/settings-proto/1";
        let request = self.auth_request(reqwest::Method::GET, path);
        let response = self.send_request(path, request).await?;
        let payload: serde_json::Value = response.json().await?;
        let encoded = payload
            .get("settings")
            .and_then(serde_json::Value::as_str)
            .context("Resposta de settings-proto sem campo settings")?;
        crate::protocol::user_settings_proto::decode_preloaded_user_settings(encoded)
            .context("Falha ao decodificar settings-proto do Discord")
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
        let _ = self
            .media_cache
            .save_guild_emojis_to_disk(guild_id, &emojis);
        Ok(emojis)
    }

    /// Fetches all custom stickers for a guild (/guilds/{guild_id}/stickers) with local disk cache
    pub async fn get_guild_stickers(&self, guild_id: &str) -> Result<Vec<DiscordSticker>> {
        if let Some(cached) = self.media_cache.load_guild_stickers_from_disk(guild_id) {
            return Ok(cached);
        }
        let path = format!("/guilds/{}/stickers", guild_id);
        let req = self.auth_request(reqwest::Method::GET, &path);
        let resp = self.send_request(&path, req).await?;
        let raw_stickers: Vec<serde_json::Value> = resp.json().await?;
        let stickers = parse_stickers_from_value_array(&raw_stickers, guild_id);
        let _ = self
            .media_cache
            .save_guild_stickers_to_disk(guild_id, &stickers);
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
                log::warn!(
                    "[DiscordApiClient] get_sticker_packs returned non-success or empty, falling back to local packs"
                );
                Ok(MediaCacheManager::get_default_sticker_packs())
            }
            Err(e) => {
                log::warn!(
                    "[DiscordApiClient] get_sticker_packs failed ({}). Falling back to local packs",
                    e
                );
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
                log::warn!(
                    "[DiscordApiClient] get_trending_gifs returned non-success or empty, falling back to curated GIFs"
                );
                Ok(MediaCacheManager::get_curated_tenor_gifs_for_category(
                    "trending",
                ))
            }
            Err(e) => {
                log::warn!(
                    "[DiscordApiClient] get_trending_gifs request failed ({}). Falling back to curated GIFs",
                    e
                );
                Ok(MediaCacheManager::get_curated_tenor_gifs_for_category(
                    "trending",
                ))
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
                log::warn!(
                    "[DiscordApiClient] search_gifs returned non-success or empty, falling back to curated search"
                );
                let fallback = MediaCacheManager::curated_tenor_gifs();
                let filtered: Vec<TenorGif> = fallback
                    .into_iter()
                    .filter(|g| {
                        g.title.to_lowercase().contains(&q.to_lowercase())
                            || g.category
                                .as_deref()
                                .map(|c| c.to_lowercase() == q.to_lowercase())
                                .unwrap_or(false)
                    })
                    .collect();
                if filtered.is_empty() {
                    Ok(MediaCacheManager::curated_tenor_gifs())
                } else {
                    Ok(filtered)
                }
            }
            Err(e) => {
                log::warn!(
                    "[DiscordApiClient] search_gifs request failed ({}). Falling back to curated search",
                    e
                );
                let fallback = MediaCacheManager::curated_tenor_gifs();
                let filtered: Vec<TenorGif> = fallback
                    .into_iter()
                    .filter(|g| {
                        g.title.to_lowercase().contains(&q.to_lowercase())
                            || g.category
                                .as_deref()
                                .map(|c| c.to_lowercase() == q.to_lowercase())
                                .unwrap_or(false)
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
                .and_then(|v| {
                    v.as_str().or_else(|| {
                        v.as_i64()
                            .map(|n| Box::leak(n.to_string().into_boxed_str()) as &str)
                    })
                })
                .unwrap_or("0")
                .to_string();
            let name = val
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("sound")
                .to_string();
            let volume = val.get("volume").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let emoji_id = val
                .get("emoji_id")
                .and_then(|v| v.as_str().map(ToString::to_string));
            let emoji_name = val
                .get("emoji_name")
                .and_then(|v| v.as_str().map(ToString::to_string));
            let available = val
                .get("available")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

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
    pub async fn get_profile_effects_catalog(
        &self,
    ) -> Result<crate::components::ProfileEffectCatalog> {
        let path = "/collectibles-categories/v2?include_bundles=true&variants_return_style=2";
        let req = self.auth_request(reqwest::Method::GET, path);
        let resp = self.send_request(path, req).await?;
        let raw_json: serde_json::Value = resp.json().await?;
        Ok(crate::components::ProfileEffectCatalog::from_categories_json(&raw_json))
    }

    /// Sends a sticker message to a text channel (/channels/{channel_id}/messages)
    pub async fn send_sticker_message(
        &self,
        channel_id: &str,
        sticker_id: &str,
    ) -> Result<Message> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(e) => {
                    log::warn!("Falha ao resolver DM para envio de figurinha: {}", e);
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
        match self.send_request(&path, req).await {
            Ok(resp) => {
                log::info!(
                    "[VoiceDiag] Status HTTP do toque: status={}",
                    resp.status().as_u16()
                );
                Ok(())
            }
            Err(e) => {
                let err_str = e.to_string();
                if let Some(code) = err_str
                    .split("HTTP ")
                    .nth(1)
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|c| c.parse::<u16>().ok())
                {
                    log::info!("[VoiceDiag] Status HTTP do toque: status={}", code);
                } else {
                    log::info!("[VoiceDiag] Status HTTP do toque: status=falha");
                }
                Err(e)
            }
        }
    }

    /// Stop ringing participants in a DM / private channel via POST /channels/{id}/call/stop-ringing
    pub async fn stop_ringing_call(
        &self,
        channel_id: &str,
        recipients: Option<Vec<String>>,
    ) -> Result<()> {
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

    /// Clear the cached voice server information.
    pub async fn clear_voice_server(&self) {
        *self.last_voice_server.write().await = None;
    }

    /// Clear both cached voice session ID and voice server information.
    pub async fn clear_voice_session(&self) {
        *self.last_voice_session_id.write().await = None;
        *self.last_voice_server.write().await = None;
    }

    /// Deletes a message from a channel (/channels/{channel_id}/messages/{message_id})
    pub async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let path = format!("/channels/{}/messages/{}", channel_id, message_id);
        let req = self.auth_request(reqwest::Method::DELETE, &path);
        let _resp = self.send_request(&path, req).await?;
        Ok(())
    }

    /// Subscribes to lazy guild members / viewport for a guild and channel.
    /// User account sends Gateway Opcode 14 (Guild Subscriptions) with [[0, 99]].
    /// Bot account uses standard guild members chunk / request members.
    pub async fn subscribe_guild_members(&self, guild_id: &str, channel_id: &str) -> Result<()> {
        if self.token.token_type == TokenType::User {
            let coordinator = crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared();
            let payload = coordinator
                .focus_guild_and_channel(guild_id, channel_id)
                .await;
            self.send_gateway_payload(&payload).await
        } else {
            // For Bot token, Opcode 14 does not exist; send Opcode 8 (Request Guild Members)
            let payload = json!({
                "op": 8,
                "d": {
                    "guild_id": guild_id,
                    "query": "",
                    "limit": 100
                }
            });
            self.send_gateway_payload(&payload).await
        }
    }
}

#[async_trait]
impl DiscordTransport for DiscordApiClient {
    async fn connect(&self) -> Result<()> {
        if self.is_connected.load(Ordering::SeqCst) {
            return Ok(());
        }
        if self.auth_failed.load(Ordering::SeqCst) {
            bail!("Falha na autenticação: token inválido ou expirado. Faça login novamente.");
        }

        // Garante que exista apenas um loop de Gateway por cliente.
        {
            let mut tasks = self.gateway_task.lock().await;
            if let Some(previous) = tasks.take() {
                previous.abort();
            }
        }

        let (handshake_tx, handshake_rx) = oneshot::channel();
        let handle = tokio::spawn({
            let token = self.token.clone();
            let event_tx = self.event_sender.clone();
            let is_connected = self.is_connected.clone();
            let auth_failed = self.auth_failed.clone();
            let last_sequence = self.last_sequence.clone();
            let session_id = self.session_id.clone();
            let resume_gateway_url = self.resume_gateway_url.clone();
            let cache_engine = self.cache_engine.clone();
            let outbound_gateway_tx = self.outbound_gateway_tx.clone();
            let current_user_id = self.current_user_id.clone();
            let last_voice_session_id = self.last_voice_session_id.clone();
            let last_voice_server = self.last_voice_server.clone();

            async move {
                run_gateway_loop(
                    token,
                    event_tx,
                    is_connected,
                    auth_failed,
                    last_sequence,
                    session_id,
                    resume_gateway_url,
                    cache_engine,
                    outbound_gateway_tx,
                    current_user_id,
                    last_voice_session_id,
                    last_voice_server,
                    Some(handshake_tx),
                )
                .await;
            }
        });
        *self.gateway_task.lock().await = Some(handle);

        let handshake = tokio::time::timeout(GATEWAY_HANDSHAKE_TIMEOUT, handshake_rx).await;
        let result = match handshake {
            Ok(Ok(Ok(()))) => return Ok(()),
            Ok(Ok(Err(reason))) => reason,
            Ok(Err(_)) => "O loop do Gateway foi encerrado durante o handshake.".to_string(),
            Err(_) => "Tempo esgotado ao autenticar no Gateway do Discord.".to_string(),
        };

        if let Some(task) = self.gateway_task.lock().await.take() {
            task.abort();
        }
        *self.outbound_gateway_tx.write().await = None;
        self.is_connected.store(false, Ordering::SeqCst);
        *self.session_id.write().await = None;
        *self.resume_gateway_url.write().await = None;
        self.last_sequence.store(-1, Ordering::SeqCst);
        bail!(result)
    }

    async fn disconnect(&self) -> Result<()> {
        self.is_connected.store(false, Ordering::SeqCst);
        self.auth_failed.store(false, Ordering::SeqCst);
        let mut lock = self.gateway_task.lock().await;
        if let Some(task) = lock.take() {
            task.abort();
        }
        drop(lock);
        *self.outbound_gateway_tx.write().await = None;
        *self.session_id.write().await = None;
        *self.resume_gateway_url.write().await = None;
        self.last_sequence.store(-1, Ordering::SeqCst);
        crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared()
            .clear()
            .await;
        *self.last_voice_session_id.write().await = None;
        *self.last_voice_server.write().await = None;
        let _ = self.event_sender.send(TransportEvent::Disconnected(
            "Disconnected by client".to_string(),
        ));
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
            log::debug!(
                "[DiscordApiClient] /users/@me/guilds retornou 304 Not Modified. Carregando do cache em disco."
            );
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
                    format!(
                        "https://cdn.discordapp.com/icons/{}/{}.png?size=96",
                        id, hash
                    )
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
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Guild");
        let owner_id = val
            .get("owner_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let icon = val.get("icon").and_then(|v| v.as_str()).map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                hash.to_string()
            } else {
                format!(
                    "https://cdn.discordapp.com/icons/{}/{}.png?size=96",
                    id, hash
                )
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

    async fn send_friend_request(
        &self,
        username: &str,
        discriminator: Option<&str>,
    ) -> anyhow::Result<()> {
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
                    log::warn!("Falha ao resolver DM: {}", e);
                    channel_id.to_string()
                }
            }
        } else {
            channel_id.to_string()
        };

        let path = format!(
            "/channels/{}/messages?limit={}",
            resolved_id,
            limit.min(100)
        );
        let endpoint_key = format!("/channels/{}/messages", resolved_id);
        let etag = self.cache_engine.get_etag(&endpoint_key).await;

        let mut req = self.auth_request(reqwest::Method::GET, &path);
        if let Some(ref tag) = etag {
            req = req.header("If-None-Match", tag);
        }

        let resp = self.send_request(&path, req).await?;

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            log::debug!(
                "[DiscordApiClient] Mensagens retornaram 304 Not Modified. Carregando do cache em disco."
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
            if let Some(author_id) = val
                .get("author")
                .and_then(|a| a.get("id"))
                .and_then(|v| v.as_str())
            {
                let guild_id = val.get("guild_id").and_then(|v| v.as_str());
                if let (Some(gid), Some(member_obj)) = (guild_id, val.get("member")) {
                    if let Some(roles_arr) = member_obj.get("roles").and_then(|v| v.as_array()) {
                        let role_ids: Vec<String> = roles_arr
                            .iter()
                            .filter_map(|r| r.as_str().map(ToString::to_string))
                            .collect();
                        crate::protocol::cache::lazy_guilds::cache_author_roles(
                            gid, author_id, role_ids,
                        );
                    }
                }
            }
            if let Some(msg) = parse_message_from_json(&val) {
                messages.push(msg);
            }
        }

        self.cache_engine
            .save_channel_messages(&resolved_id, &messages)
            .await;

        Ok(messages)
    }

    async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(e) => {
                    log::warn!("Falha ao resolver DM para envio: {}", e);
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

    async fn send_typing(&self, channel_id: &str) -> Result<()> {
        let resolved_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => ch.id,
                Err(e) => {
                    log::warn!("Falha ao resolver DM para typing: {}", e);
                    channel_id.to_string()
                }
            }
        } else {
            channel_id.to_string()
        };

        let path = format!("/channels/{}/typing", resolved_id);
        let req = self.auth_request(reqwest::Method::POST, &path);
        self.send_request(&path, req).await?;
        Ok(())
    }

    async fn add_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
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

    async fn remove_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
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

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> anyhow::Result<()> {
        self.delete_message(channel_id, message_id).await
    }

    async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        content: &str,
    ) -> anyhow::Result<Message> {
        self.edit_message(channel_id, message_id, content).await
    }

    async fn pin_message(&self, channel_id: &str, message_id: &str) -> anyhow::Result<()> {
        self.pin_message(channel_id, message_id).await
    }

    async fn unpin_message(&self, channel_id: &str, message_id: &str) -> anyhow::Result<()> {
        self.unpin_message(channel_id, message_id).await
    }

    async fn request_guild_members(
        &self,
        guild_id: &str,
        user_ids: Vec<String>,
    ) -> anyhow::Result<()> {
        self.request_guild_members(guild_id, user_ids).await
    }

    async fn update_guild_member_viewport(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> anyhow::Result<()> {
        self.update_guild_member_viewport(guild_id, channel_id, ranges)
            .await
    }

    async fn subscribe_guild_members(
        &self,
        guild_id: &str,
        channel_id: &str,
    ) -> anyhow::Result<()> {
        self.subscribe_guild_members(guild_id, channel_id).await
    }

    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> anyhow::Result<()> {
        self.send_voice_state_update(guild_id, channel_id, self_mute, self_deaf)
            .await
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

fn sanitize_gateway_url(raw: &str) -> String {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return "URL de Gateway inválida".to_string();
    };
    let Some(host) = url.host_str() else {
        return "URL de Gateway sem host".to_string();
    };
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{}://{host}{port}{}", url.scheme(), url.path())
}

fn websocket_error_kind(error: &tokio_tungstenite::tungstenite::Error) -> String {
    use tokio_tungstenite::tungstenite::Error as WsError;
    match error {
        WsError::Io(_) => "I/O".to_string(),
        WsError::Tls(_) => "TLS".to_string(),
        WsError::Capacity(_) => "limite de capacidade".to_string(),
        WsError::Protocol(_) => "protocolo WebSocket".to_string(),
        WsError::WriteBufferFull(_) => "buffer de escrita cheio".to_string(),
        WsError::Utf8(_) => "UTF-8 inválido".to_string(),
        WsError::AttackAttempt => "tentativa de ataque detectada".to_string(),
        WsError::ConnectionClosed | WsError::AlreadyClosed => "conexão fechada".to_string(),
        WsError::Url(_) => "URL inválida".to_string(),
        WsError::Http(response) => format!("HTTP {}", response.status()),
        WsError::HttpFormat(_) => "formato HTTP inválido".to_string(),
    }
}

fn heartbeat_initial_delay(interval: Duration, seed: u64) -> Duration {
    let interval_ms = u64::try_from(interval.as_millis()).unwrap_or(u64::MAX);
    if interval_ms == 0 {
        Duration::ZERO
    } else {
        Duration::from_millis(seed % interval_ms)
    }
}

fn identify_or_resume_payload(
    token: &AuthToken,
    session_id: Option<&str>,
    sequence: i64,
) -> serde_json::Value {
    if let (Some(session_id), true) = (session_id, sequence >= 0) {
        return json!({
            "op": 6,
            "d": {
                "token": token.secret(),
                "session_id": session_id,
                "seq": sequence
            }
        });
    }

    if token.token_type == TokenType::User {
        json!({
            "op": 2,
            "d": {
                "token": token.secret(),
                "capabilities": 30717,
                "properties": build_user_client_properties(),
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
    }
}

struct GatewayConnectionOutcome {
    kind: GatewayDisconnectKind,
    authenticated: bool,
    connected_for: Duration,
}

async fn run_gateway_connection<S>(
    ws_stream: tokio_tungstenite::WebSocketStream<S>,
    token: &AuthToken,
    event_tx: &broadcast::Sender<TransportEvent>,
    is_connected: &AtomicBool,
    auth_failed: &AtomicBool,
    last_sequence: &AtomicI64,
    session_id: &RwLock<Option<String>>,
    resume_gateway_url: &RwLock<Option<String>>,
    cache_engine: &Arc<DiscordCacheEngine>,
    outbound_gateway_tx: &RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>,
    current_user_id: &RwLock<Option<String>>,
    last_voice_session_id: &RwLock<Option<String>>,
    last_voice_server: &RwLock<Option<crate::protocol::VoiceServerData>>,
    handshake_tx: &mut Option<oneshot::Sender<Result<(), String>>>,
) -> GatewayConnectionOutcome
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio::time::{Instant as TokioInstant, MissedTickBehavior, interval_at};

    let connected_at = std::time::Instant::now();
    let mut authenticated = false;
    #[allow(unused_assignments)]
    let mut disconnect_kind = GatewayDisconnectKind::Resume;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    // Op 10 Hello é obrigatório antes de Identify/Resume. Não utiliza mais
    // um único `next()` sem timeout nem intervalo default fabricado.
    let hello = tokio::time::timeout(GATEWAY_HELLO_TIMEOUT, async {
        loop {
            match ws_read.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    if let Ok(payload) = serde_json::from_str::<GatewayPayload>(&text) {
                        if payload.op == 10 {
                            return payload
                                .d
                                .as_ref()
                                .and_then(|data| data.get("heartbeat_interval"))
                                .and_then(serde_json::Value::as_u64)
                                .and_then(|millis| {
                                    let interval = Duration::from_millis(millis);
                                    (!interval.is_zero()).then_some(interval)
                                })
                                .filter(|interval| {
                                    TokioInstant::now().checked_add(*interval).is_some()
                                });
                        }
                    }
                }
                Some(Ok(WsMessage::Ping(_))) => continue,
                _ => return None,
            }
        }
    })
    .await
    .ok()
    .flatten();

    let Some(heartbeat_interval) = hello else {
        log::warn!("Gateway não enviou Op 10 Hello válido dentro do tempo limite.");
        *outbound_gateway_tx.write().await = None;
        is_connected.store(false, Ordering::SeqCst);
        return GatewayConnectionOutcome {
            kind: GatewayDisconnectKind::Identify,
            authenticated,
            connected_for: connected_at.elapsed(),
        };
    };

    let session = session_id.read().await.clone();
    let sequence = last_sequence.load(Ordering::SeqCst);
    let has_resume_url = resume_gateway_url.read().await.is_some();
    let can_resume = session.is_some() && sequence >= 0 && has_resume_url;
    let resume_session = if can_resume { session.as_deref() } else { None };
    let payload = identify_or_resume_payload(token, resume_session, sequence);
    if ws_write
        .send(WsMessage::Text(payload.to_string().into()))
        .await
        .is_err()
    {
        *outbound_gateway_tx.write().await = None;
        is_connected.store(false, Ordering::SeqCst);
        return GatewayConnectionOutcome {
            kind: GatewayDisconnectKind::Identify,
            authenticated,
            connected_for: connected_at.elapsed(),
        };
    }

    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel::<WsMessage>(64);
    *outbound_gateway_tx.write().await = Some(command_tx);
    is_connected.store(true, Ordering::SeqCst);
    let _ = event_tx.send(TransportEvent::Connected);
    log::info!(
        "Gateway WebSocket conectado; {}.",
        if can_resume {
            "RESUME enviado"
        } else {
            "IDENTIFY enviado"
        }
    );

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
        ^ (sequence.max(0) as u64);
    let mut heartbeat = interval_at(
        TokioInstant::now() + heartbeat_initial_delay(heartbeat_interval, seed),
        heartbeat_interval,
    );
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut heartbeat_ack_timeout: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                if heartbeat_ack_timeout.is_some() {
                    log::warn!("Heartbeat ACK não recebido entre tentativas; encerrando conexão zumbi.");
                    disconnect_kind = GatewayDisconnectKind::Resume;
                    break;
                }
                let seq = last_sequence.load(Ordering::SeqCst);
                let data = if seq >= 0 { json!(seq) } else { json!(null) };
                let heartbeat_payload = json!({ "op": 1, "d": data });
                if ws_write.send(WsMessage::Text(heartbeat_payload.to_string().into())).await.is_err() {
                    disconnect_kind = GatewayDisconnectKind::Resume;
                    break;
                }
                heartbeat_ack_timeout = Some(Box::pin(tokio::time::sleep(heartbeat_interval)));
            }
            Some(message) = command_rx.recv() => {
                if ws_write.send(message).await.is_err() {
                    disconnect_kind = GatewayDisconnectKind::Resume;
                    break;
                }
            }
            incoming = ws_read.next() => {
                match incoming {
                    Some(Ok(WsMessage::Text(text))) => {
                        let Ok(payload) = serde_json::from_str::<GatewayPayload>(&text) else {
                            log::warn!("Payload JSON inválido recebido do Gateway; reconectando.");
                            disconnect_kind = GatewayDisconnectKind::Resume;
                            break;
                        };
                        if let Some(seq) = payload.s {
                            last_sequence.store(seq, Ordering::SeqCst);
                        }

                        match payload.op {
                            0 => {
                                if payload.t.as_deref() == Some("RESUMED") {
                                    authenticated = true;
                                    log::info!("Gateway RESUME aceito (RESUMED).");
                                    if let Some(tx) = handshake_tx.take() {
                                        let _ = tx.send(Ok(()));
                                    }
                                } else if let (Some(event_name), Some(data)) = (payload.t.as_deref(), payload.d) {
                                    if let Err(error) = handle_dispatch(
                                        event_name,
                                        data,
                                        event_tx,
                                        session_id,
                                        resume_gateway_url,
                                        cache_engine,
                                        current_user_id,
                                        last_voice_session_id,
                                        last_voice_server,
                                    ).await {
                                        log::warn!("Evento Gateway '{}' inválido: {:#}", event_name, error);
                                        // Um READY inválido pode ter gravado session_id
                                        // antes de falhar ao validar o usuário. Não retome
                                        // uma sessão que nunca chegou a autenticar.
                                        *session_id.write().await = None;
                                        *resume_gateway_url.write().await = None;
                                        last_sequence.store(-1, Ordering::SeqCst);
                                        disconnect_kind = GatewayDisconnectKind::Identify;
                                        break;
                                    }
                                    if event_name == "READY" {
                                        authenticated = true;
                                        log::info!("Gateway IDENTIFY aceito (READY).");
                                        if let Some(tx) = handshake_tx.take() {
                                            let _ = tx.send(Ok(()));
                                        }
                                    }
                                }
                            }
                            1 => {
                                let seq = last_sequence.load(Ordering::SeqCst);
                                let data = if seq >= 0 { json!(seq) } else { json!(null) };
                                let heartbeat_payload = json!({ "op": 1, "d": data });
                                if ws_write.send(WsMessage::Text(heartbeat_payload.to_string().into())).await.is_err() {
                                    disconnect_kind = GatewayDisconnectKind::Resume;
                                    break;
                                }
                                heartbeat_ack_timeout = Some(Box::pin(tokio::time::sleep(heartbeat_interval)));
                            }
                            7 => {
                                log::info!("Gateway solicitou reconexão (Op 7).");
                                disconnect_kind = GatewayDisconnectKind::Resume;
                                break;
                            }
                            9 => {
                                let resumable = payload.d.as_ref().and_then(serde_json::Value::as_bool) == Some(true);
                                if resumable {
                                    log::warn!("Gateway retornou Invalid Session ressumível (Op 9).");
                                    disconnect_kind = GatewayDisconnectKind::Resume;
                                } else {
                                    log::warn!("Gateway invalidou a sessão (Op 9); será necessário IDENTIFY.");
                                    *session_id.write().await = None;
                                    *resume_gateway_url.write().await = None;
                                    last_sequence.store(-1, Ordering::SeqCst);
                                    disconnect_kind = GatewayDisconnectKind::Identify;
                                }
                                break;
                            }
                            11 => {
                                heartbeat_ack_timeout = None;
                                log::trace!("Heartbeat ACK recebido.");
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(WsMessage::Ping(payload))) => {
                        if ws_write.send(WsMessage::Pong(payload)).await.is_err() {
                            disconnect_kind = GatewayDisconnectKind::Resume;
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Close(frame))) => {
                        let code = frame.as_ref().map(|frame| u16::from(frame.code));
                        log::warn!("Gateway fechou a conexão: código {:?}.", code);
                        disconnect_kind = gateway_close_action(code);
                        if disconnect_kind == GatewayDisconnectKind::Identify {
                            *session_id.write().await = None;
                            *resume_gateway_url.write().await = None;
                            last_sequence.store(-1, Ordering::SeqCst);
                        }
                        if matches!(code, Some(4003) | Some(4004)) {
                            auth_failed.store(true, Ordering::SeqCst);
                        }
                        break;
                    }
                    Some(Err(error)) => {
                        log::error!("Erro de leitura do Gateway: {}.", websocket_error_kind(&error));
                        disconnect_kind = GatewayDisconnectKind::Resume;
                        break;
                    }
                    None => {
                        log::warn!("Stream do Gateway encerrada.");
                        disconnect_kind = GatewayDisconnectKind::Resume;
                        break;
                    }
                    _ => {}
                }
            }
            _ = async {
                match heartbeat_ack_timeout.as_mut() {
                    Some(timeout) => timeout.await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                log::warn!("Timeout de Heartbeat ACK; reconectando para retomar a sessão.");
                disconnect_kind = GatewayDisconnectKind::Resume;
                break;
            }
        }
    }

    *outbound_gateway_tx.write().await = None;
    is_connected.store(false, Ordering::SeqCst);
    GatewayConnectionOutcome {
        kind: disconnect_kind,
        authenticated,
        connected_for: connected_at.elapsed(),
    }
}

async fn run_gateway_loop(
    token: AuthToken,
    event_tx: broadcast::Sender<TransportEvent>,
    is_connected: Arc<AtomicBool>,
    auth_failed: Arc<AtomicBool>,
    last_sequence: Arc<AtomicI64>,
    session_id: Arc<RwLock<Option<String>>>,
    resume_gateway_url: Arc<RwLock<Option<String>>>,
    cache_engine: Arc<DiscordCacheEngine>,
    outbound_gateway_tx: Arc<RwLock<Option<tokio::sync::mpsc::Sender<WsMessage>>>>,
    current_user_id: Arc<RwLock<Option<String>>>,
    last_voice_session_id: Arc<RwLock<Option<String>>>,
    last_voice_server: Arc<RwLock<Option<crate::protocol::VoiceServerData>>>,
    mut handshake_tx: Option<oneshot::Sender<Result<(), String>>>,
) {
    let mut reconnect_policy = GatewayReconnectPolicy::new();

    loop {
        if auth_failed.load(Ordering::SeqCst) {
            let reason = "Falha na autenticação: token inválido ou expirado. Faça login novamente.";
            let _ = event_tx.send(TransportEvent::Disconnected(reason.to_string()));
            if let Some(tx) = handshake_tx.take() {
                let _ = tx.send(Err(reason.to_string()));
            }
            break;
        }

        let gateway_url = {
            let guard = resume_gateway_url.read().await;
            guard
                .clone()
                .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string())
        };
        log::info!(
            "Conectando ao Discord Gateway: {}",
            sanitize_gateway_url(&gateway_url)
        );

        let mut ws_config = WebSocketConfig::default();
        ws_config.max_message_size = None;
        ws_config.max_frame_size = None;

        let connection = tokio::time::timeout(
            GATEWAY_CONNECT_TIMEOUT,
            connect_async_with_config(&gateway_url, Some(ws_config), false),
        )
        .await;

        match connection {
            Ok(Ok((ws_stream, _))) => {
                let outcome = run_gateway_connection(
                    ws_stream,
                    &token,
                    &event_tx,
                    &is_connected,
                    &auth_failed,
                    &last_sequence,
                    &session_id,
                    &resume_gateway_url,
                    &cache_engine,
                    &outbound_gateway_tx,
                    &current_user_id,
                    &last_voice_session_id,
                    &last_voice_server,
                    &mut handshake_tx,
                )
                .await;

                // Só zera o backoff depois de uma sessão estável; READY/RESUMED
                // seguido imediatamente de desconexão não deve criar loop de 1s.
                if outcome.authenticated && outcome.connected_for >= Duration::from_secs(30) {
                    reconnect_policy.on_authenticated();
                }

                let reason = outcome.kind.reason().to_string();
                let _ = event_tx.send(TransportEvent::Disconnected(reason.clone()));

                if let GatewayDisconnectKind::Fatal(_) = outcome.kind {
                    if let Some(tx) = handshake_tx.take() {
                        let _ = tx.send(Err(reason));
                    }
                    break;
                }

                if handshake_tx.is_some() {
                    if let Some(tx) = handshake_tx.take() {
                        let _ = tx.send(Err(reason));
                    }
                    break;
                }
            }
            Ok(Err(error)) => {
                let reason = format!(
                    "Falha ao conectar no Gateway do Discord ({}).",
                    websocket_error_kind(&error)
                );
                log::error!("{}", reason);
                *outbound_gateway_tx.write().await = None;
                is_connected.store(false, Ordering::SeqCst);
                let _ = event_tx.send(TransportEvent::Disconnected(reason.clone()));
                if let Some(tx) = handshake_tx.take() {
                    let _ = tx.send(Err(reason));
                    break;
                }
            }
            Err(_) => {
                let reason = "Tempo esgotado ao conectar no Gateway do Discord.".to_string();
                log::error!("{}", reason);
                *outbound_gateway_tx.write().await = None;
                is_connected.store(false, Ordering::SeqCst);
                let _ = event_tx.send(TransportEvent::Disconnected(reason.clone()));
                if let Some(tx) = handshake_tx.take() {
                    let _ = tx.send(Err(reason));
                    break;
                }
            }
        }

        let delay = reconnect_policy.next_backoff();
        if reconnect_policy.initial_attempts_exhausted() {
            let reason =
                "Não foi possível restabelecer o Gateway após várias tentativas.".to_string();
            log::error!("{}", reason);
            let _ = event_tx.send(TransportEvent::Disconnected(reason));
            break;
        }
        log::info!("Próxima reconexão do Gateway em {:?}.", delay);
        tokio::time::sleep(delay).await;
    }

    *outbound_gateway_tx.write().await = None;
    is_connected.store(false, Ordering::SeqCst);
}

async fn handle_dispatch(
    event_name: &str,
    data: serde_json::Value,
    event_tx: &broadcast::Sender<TransportEvent>,
    session_id: &RwLock<Option<String>>,
    resume_gateway_url: &RwLock<Option<String>>,
    cache_engine: &Arc<DiscordCacheEngine>,
    current_user_id: &RwLock<Option<String>>,
    last_voice_session_id: &RwLock<Option<String>>,
    last_voice_server: &RwLock<Option<crate::protocol::VoiceServerData>>,
) -> Result<()> {
    let media_cache = MediaCacheManager::default_path();
    match event_name {
        "READY" => {
            let session = data
                .get("session_id")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .context("READY do Gateway sem session_id")?;
            let resume_url = data
                .get("resume_gateway_url")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .context("READY do Gateway sem resume_gateway_url")?;
            let user = data
                .get("user")
                .and_then(parse_user_from_json)
                .context("READY do Gateway sem usuário válido")?;

            // Só publica metadados de resumo depois que o READY inteiro foi
            // validado. Um usuário inválido não pode criar uma sessão resumível.
            *session_id.write().await = Some(session.to_string());
            *resume_gateway_url.write().await = Some(resume_url.to_string());
            *last_voice_server.write().await = None;

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
                            format!(
                                "https://cdn.discordapp.com/icons/{}/{}.png?size=96",
                                id, hash
                            )
                        }
                    });
                    let mut guild = Guild::new(id, name, "");
                    guild.icon = icon;
                    if let Some(raw_emojis) = g.get("emojis").and_then(|v| v.as_array()) {
                        guild.emojis = parse_emojis_from_value_array(raw_emojis, id);
                    }
                    if let Some(raw_stickers) = g.get("stickers").and_then(|v| v.as_array()) {
                        let parsed = parse_stickers_from_value_array(raw_stickers, id);
                        let _ = media_cache.save_guild_stickers_to_disk(id, &parsed);
                        guild.stickers = parsed;
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
                    let id = f
                        .get("id")
                        .and_then(|v| v.as_str().map(ToString::to_string));
                    let name = f
                        .get("name")
                        .and_then(|v| v.as_str().map(ToString::to_string));
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
                match crate::protocol::user_settings_proto::decode_preloaded_user_settings(
                    proto_b64,
                ) {
                    Ok(settings) => {
                        account_theme = settings.theme();
                        client_theme = settings
                            .appearance
                            .as_ref()
                            .and_then(|appearance| appearance.client_theme_settings.clone());
                    }
                    Err(_) => {
                        log::warn!("READY continha user_settings_proto inválido; ignorado.");
                    }
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
                let name = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Server");
                let owner_id = data
                    .get("owner_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let icon = data.get("icon").and_then(|v| v.as_str()).map(|hash| {
                    if hash.starts_with("http://") || hash.starts_with("https://") {
                        hash.to_string()
                    } else {
                        format!(
                            "https://cdn.discordapp.com/icons/{}/{}.png?size=96",
                            id, hash
                        )
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
                    let parsed = parse_stickers_from_value_array(raw_stickers, id);
                    let _ = media_cache.save_guild_stickers_to_disk(id, &parsed);
                    guild.stickers = parsed;
                }

                // Popula voice_states a partir do snapshot inicial de GUILD_CREATE
                if let Some(raw_states) = data.get("voice_states").and_then(|v| v.as_array()) {
                    let mut guild_members: std::collections::HashMap<
                        String,
                        (Option<String>, Option<String>, Option<AvatarDecorationData>),
                    > = std::collections::HashMap::new();
                    if let Some(raw_members) = data.get("members").and_then(|v| v.as_array()) {
                        for m in raw_members {
                            if let Some(user) = m.get("user") {
                                if let Some(uid) = user.get("id").and_then(|v| v.as_str()) {
                                    let name = m
                                        .get("nick")
                                        .and_then(|v| v.as_str())
                                        .or_else(|| {
                                            user.get("global_name").and_then(|v| v.as_str())
                                        })
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
                                if let Some((name, avatar, decor)) =
                                    guild_members.get(&state.user_id)
                                {
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
        "GUILD_MEMBER_LIST_UPDATE" => {
            if let Some(update) =
                crate::protocol::cache::lazy_guilds::parse_guild_member_list_update(&data)
            {
                let _ = event_tx.send(TransportEvent::GuildMemberListUpdate(update));
            }
        }
        "GUILD_MEMBERS_CHUNK" => {
            handle_guild_members_chunk_dispatch(&data, event_tx);
        }
        "MESSAGE_CREATE" => {
            if let Some(author_id) = data
                .get("author")
                .and_then(|a| a.get("id"))
                .and_then(|v| v.as_str())
            {
                let guild_id = data.get("guild_id").and_then(|v| v.as_str());
                if let (Some(gid), Some(member_obj)) = (guild_id, data.get("member")) {
                    if let Some(roles_arr) = member_obj.get("roles").and_then(|v| v.as_array()) {
                        let role_ids: Vec<String> = roles_arr
                            .iter()
                            .filter_map(|r| r.as_str().map(ToString::to_string))
                            .collect();
                        crate::protocol::cache::lazy_guilds::cache_author_roles(
                            gid, author_id, role_ids,
                        );
                    }
                }
            }
            if let Some(message) = parse_message_from_json(&data) {
                cache_engine.apply_gateway_message_create(&message).await;
                let _ = event_tx.send(TransportEvent::MessageCreate(message));
            }
        }
        "MESSAGE_UPDATE" => {
            if let Some(message) = parse_message_from_json(&data) {
                cache_engine
                    .apply_gateway_message_update(
                        &message.channel_id,
                        &message.id,
                        &message.content,
                    )
                    .await;
                let _ = event_tx.send(TransportEvent::MessageUpdate(message));
            }
        }
        "MESSAGE_DELETE" => {
            if let (Some(id), Some(ch_id)) = (
                data.get("id").and_then(|v| v.as_str()),
                data.get("channel_id").and_then(|v| v.as_str()),
            ) {
                cache_engine.apply_gateway_message_delete(ch_id, id).await;
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
                    if my_id.as_deref() == Some(state.user_id.as_str())
                        && !state.session_id.is_empty()
                    {
                        *last_voice_session_id.write().await = Some(state.session_id.clone());
                    }
                }

                let _ = event_tx.send(TransportEvent::VoiceStateUpdate(state));
            }
        }
        "VOICE_SERVER_UPDATE" => {
            let token = data
                .get("token")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let guild_id = data
                .get("guild_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let channel_id = data
                .get("channel_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let endpoint = data
                .get("endpoint")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

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
        "CALL_CREATE" => {
            if let Some(ch_id) = data.get("channel_id").and_then(|v| v.as_str()) {
                let message_id = data
                    .get("message_id")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let region = data
                    .get("region")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let ringing = data
                    .get("ringing")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = event_tx.send(TransportEvent::CallCreate {
                    channel_id: ch_id.to_string(),
                    message_id,
                    region,
                    ringing,
                });
            }
        }
        "CALL_UPDATE" => {
            if let Some(ch_id) = data.get("channel_id").and_then(|v| v.as_str()) {
                let region = data
                    .get("region")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let ringing = data
                    .get("ringing")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = event_tx.send(TransportEvent::CallUpdate {
                    channel_id: ch_id.to_string(),
                    region,
                    ringing,
                });
            }
        }
        "CALL_DELETE" => {
            if let Some(ch_id) = data.get("channel_id").and_then(|v| v.as_str()) {
                let _ = event_tx.send(TransportEvent::CallDelete {
                    channel_id: ch_id.to_string(),
                });
            }
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
                .or_else(|| {
                    data.get("user")
                        .and_then(|u| u.get("id"))
                        .and_then(|v| v.as_str())
                })
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
                match crate::protocol::user_settings_proto::decode_preloaded_user_settings(
                    settings_b64,
                ) {
                    Ok(settings) => {
                        let theme = settings.theme();
                        let client_theme = settings
                            .appearance
                            .as_ref()
                            .and_then(|appearance| appearance.client_theme_settings.clone());
                        if theme.is_some() || client_theme.is_some() {
                            let _ = event_tx.send(TransportEvent::AccountThemeUpdated {
                                theme: theme
                                    .unwrap_or(crate::protocol::user_settings_proto::Theme::Unset),
                                client_theme_settings: client_theme,
                            });
                        }
                    }
                    Err(_) => {
                        log::warn!("USER_SETTINGS_PROTO_UPDATE inválido; ignorado.");
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

// --- JSON Parsers for Discord Data Models ---

/// Deserializes a user relationship from Discord JSON payload.
pub fn parse_relationship_from_json(r: &serde_json::Value) -> Option<Relationship> {
    let id = r
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| {
            r.get("user")
                .and_then(|u| u.get("id"))
                .and_then(|v| v.as_str())
        })?
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
            format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png?size=96",
                id, hash
            )
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
    user.discriminator = val
        .get("discriminator")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    user.global_name = val
        .get("global_name")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    user.avatar = val
        .get("avatar")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
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
                    link: b
                        .get("link")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
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
                    verified: ca
                        .get("verified")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
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
                    nick: mg
                        .get("nick")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
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

pub(crate) fn parse_voice_state_from_json(
    val: &serde_json::Value,
    default_guild_id: Option<&str>,
) -> Option<VoiceState> {
    let user_id = val.get("user_id")?.as_str()?;
    let session_id = val
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let mut state = VoiceState::new(user_id, session_id);
    state.guild_id = val
        .get("guild_id")
        .and_then(|v| v.as_str())
        .or(default_guild_id)
        .map(ToString::to_string);
    state.channel_id = val
        .get("channel_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    state.mute = val.get("mute").and_then(|v| v.as_bool()).unwrap_or(false);
    state.deaf = val.get("deaf").and_then(|v| v.as_bool()).unwrap_or(false);
    state.self_mute = val
        .get("self_mute")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    state.self_deaf = val
        .get("self_deaf")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    state.self_video = val
        .get("self_video")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    state.self_stream = val
        .get("self_stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    state.suppress = val
        .get("suppress")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if let Some(member) = val.get("member") {
        if let Some(user) = member.get("user") {
            state.user_name = member
                .get("nick")
                .and_then(|v| v.as_str())
                .or_else(|| user.get("global_name").and_then(|v| v.as_str()))
                .or_else(|| user.get("username").and_then(|v| v.as_str()))
                .map(ToString::to_string);
            state.avatar_url = user.get("avatar").and_then(|v| v.as_str()).map(|hash| {
                if hash.starts_with("http") {
                    hash.to_string()
                } else {
                    format!(
                        "https://cdn.discordapp.com/avatars/{}/{}.png?size=64",
                        user_id, hash
                    )
                }
            });
            // Guild-level decoration overrides the global user one in guild context
            // (see Guild Member Structure: `avatar_decoration_data` is "data for the
            // member's guild avatar decoration").
            state.avatar_decoration_data =
                parse_avatar_decoration_data(member).or_else(|| parse_avatar_decoration_data(user));
        } else {
            state.avatar_decoration_data = parse_avatar_decoration_data(member);
        }
    } else if let Some(user) = val.get("user") {
        state.user_name = user
            .get("global_name")
            .and_then(|v| v.as_str())
            .or_else(|| user.get("username").and_then(|v| v.as_str()))
            .map(ToString::to_string);
        state.avatar_url = user.get("avatar").and_then(|v| v.as_str()).map(|hash| {
            if hash.starts_with("http") {
                hash.to_string()
            } else {
                format!(
                    "https://cdn.discordapp.com/avatars/{}/{}.png?size=64",
                    user_id, hash
                )
            }
        });
        state.avatar_decoration_data = parse_avatar_decoration_data(user);
    }
    Some(state)
}

fn parse_channel_from_json(
    val: &serde_json::Value,
    default_guild_id: Option<&str>,
) -> Option<Channel> {
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
    ch.category_id = val
        .get("parent_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    ch.topic = val
        .get("topic")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    ch.position = val.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    ch.last_message_id = val
        .get("last_message_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    ch.user_limit = val
        .get("user_limit")
        .and_then(|v| v.as_u64())
        .map(|l| l as u32);
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

fn parse_emojis_from_value_array(
    raw_emojis: &[serde_json::Value],
    guild_id: &str,
) -> Vec<DiscordEmoji> {
    let mut emojis = Vec::new();
    for val in raw_emojis {
        let id = val
            .get("id")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("emoji")
            .to_string();
        let animated = val
            .get("animated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let available = val
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let roles = val
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| r.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        emojis.push(DiscordEmoji {
            id,
            name,
            roles,
            user: val.get("user").cloned(),
            require_colons: val
                .get("require_colons")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            managed: val
                .get("managed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            animated,
            available,
            guild_id: Some(guild_id.to_string()),
        });
    }
    emojis
}

fn parse_stickers_from_value_array(
    raw_stickers: &[serde_json::Value],
    guild_id: &str,
) -> Vec<DiscordSticker> {
    let mut stickers = Vec::new();
    for val in raw_stickers {
        let id = val
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("sticker")
            .to_string();
        let description = val
            .get("description")
            .and_then(|v| v.as_str().map(ToString::to_string));
        let tags = val
            .get("tags")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let pack_id = val
            .get("pack_id")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let fmt_code = val.get("format_type").and_then(|v| v.as_u64()).unwrap_or(1);
        let format_type = match fmt_code {
            2 => StickerFormatType::Apng,
            3 => StickerFormatType::Lottie,
            4 => StickerFormatType::Gif,
            _ => StickerFormatType::Png,
        };
        let available = val
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

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
    let name = val
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Sticker Pack")
        .to_string();
    let sku_id = val
        .get("sku_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let description = val
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let banner_asset_id = val
        .get("banner_asset_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let mut stickers = Vec::new();
    if let Some(arr) = val.get("stickers").and_then(|v| v.as_array()) {
        for s in arr {
            let s_id = s
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let s_name = s
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("sticker")
                .to_string();
            let s_desc = s
                .get("description")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let s_tags = s
                .get("tags")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let s_pack_id = s
                .get("pack_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .or_else(|| Some(id.clone()));
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
    author.global_name = author_val
        .get("global_name")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    author.avatar = author_val
        .get("avatar")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    author.bot = author_val
        .get("bot")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    author.avatar_decoration_data = parse_avatar_decoration_data(author_val);

    let mut message = Message::new(id, channel_id, author, content);
    message.guild_id = val
        .get("guild_id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
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
                    proxy_url: a
                        .get("proxy_url")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                    content_type: a
                        .get("content_type")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
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
            let title = e
                .get("title")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let description = e
                .get("description")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let url = e
                .get("url")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
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
                image_url: e
                    .get("image")
                    .and_then(|img| img.get("url"))
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                thumbnail_url: e
                    .get("thumbnail")
                    .and_then(|th| th.get("url"))
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
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
                message
                    .sticker_items
                    .push(MessageStickerItem::new(s_id, s_name, fmt));
            }
        }
    }

    if let Some(poll_val) = val.get("poll") {
        if !poll_val.is_null() {
            message.poll = crate::chat::polls::parse_poll_from_json(poll_val);
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
        .or_else(|| {
            val.get("user_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })?;

    let status_str = val
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("offline");
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
                let state_text = act
                    .get("state")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                let emoji = act.get("emoji").and_then(|e| {
                    let emoji_name = e.get("name").and_then(|v| v.as_str())?;
                    let emoji_id = e
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
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
                activity.details = act
                    .get("details")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                activity.state = act
                    .get("state")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                activity.created_at = act.get("created_at").and_then(|v| v.as_i64());
                activity.application_id = act
                    .get("application_id")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                activity.sync_id = act
                    .get("sync_id")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);

                if let Some(ts) = act.get("timestamps") {
                    let start = ts.get("start").and_then(|v| v.as_i64());
                    let end = ts.get("end").and_then(|v| v.as_i64());
                    if start.is_some() || end.is_some() {
                        activity.timestamps =
                            Some(crate::domain::user::ActivityTimestamps { start, end });
                    }
                }

                if let Some(assets) = act.get("assets") {
                    let large_image = assets
                        .get("large_image")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    let large_text = assets
                        .get("large_text")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    let small_image = assets
                        .get("small_image")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    let small_text = assets
                        .get("small_text")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    if large_image.is_some()
                        || large_text.is_some()
                        || small_image.is_some()
                        || small_text.is_some()
                    {
                        activity.assets = Some(crate::domain::user::ActivityAssets {
                            large_image,
                            large_text,
                            small_image,
                            small_text,
                        });
                    }
                }
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
mod network_hardening_tests {
    use super::*;

    #[test]
    fn test_reconnect_backoff_is_exponential_capped_and_resettable() {
        let mut policy = GatewayReconnectPolicy::new();
        let delays: Vec<Duration> = (0..7).map(|_| policy.next_backoff()).collect();
        assert_eq!(
            delays,
            vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(16),
                Duration::from_secs(30),
                Duration::from_secs(30),
            ]
        );
        assert!(policy.initial_attempts_exhausted());
        policy.on_authenticated();
        assert!(!policy.initial_attempts_exhausted());
        assert_eq!(policy.next_backoff(), Duration::from_secs(1));
    }

    #[test]
    fn test_gateway_close_codes_stop_fatal_reconnect_loops() {
        assert!(matches!(
            gateway_close_action(Some(4004)),
            GatewayDisconnectKind::Fatal(_)
        ));
        assert!(matches!(
            gateway_close_action(Some(4014)),
            GatewayDisconnectKind::Fatal(_)
        ));
        assert_eq!(
            gateway_close_action(Some(4009)),
            GatewayDisconnectKind::Identify
        );
        assert_eq!(gateway_close_action(None), GatewayDisconnectKind::Resume);
    }

    #[test]
    fn test_identify_and_resume_payloads() {
        let token = AuthToken::user("synthetic.user.token.for-test-only");
        let identify = identify_or_resume_payload(&token, None, -1);
        assert_eq!(identify["op"], 2);
        assert!(identify["d"].get("intents").is_none());

        let resume = identify_or_resume_payload(&token, Some("synthetic-session"), 42);
        assert_eq!(resume["op"], 6);
        assert_eq!(resume["d"]["session_id"], "synthetic-session");
        assert_eq!(resume["d"]["seq"], 42);

        let bot = AuthToken::bot("synthetic.bot.token.for-test-only");
        let identify = identify_or_resume_payload(&bot, None, -1);
        assert_eq!(identify["op"], 2);
        assert_eq!(identify["d"]["intents"], 1 | 512 | 32768 | 128);
    }

    #[test]
    fn test_gateway_url_logging_removes_query_and_credentials() {
        let safe = sanitize_gateway_url("wss://user:secret@gateway.example/path?token=secret&v=10");
        assert_eq!(safe, "wss://gateway.example/path");
        assert!(!safe.contains("secret"));
        assert!(!safe.contains("token"));
    }

    #[test]
    fn test_auth_errors_are_clear_and_do_not_echo_response_body() {
        let invalid = api_error_message(
            reqwest::StatusCode::UNAUTHORIZED,
            br#"{"message":"secret personal detail","code":50014}"#,
        );
        assert!(invalid.contains("token inválido ou expirado"));
        assert!(!invalid.contains("secret personal detail"));

        let two_factor = api_error_message(
            reqwest::StatusCode::FORBIDDEN,
            br#"{"message":"Two factor is required","code":60003}"#,
        );
        assert!(two_factor.contains("2FA"));
    }

    #[test]
    fn test_credential_verification_bypasses_etag_cache() {
        let client = DiscordApiClient::new(AuthToken::bot("synthetic.bot.token.for-test-only"));
        let request = client
            .credential_verification_request()
            .build()
            .expect("synthetic verification request");
        assert_eq!(
            request
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .unwrap(),
            "no-cache"
        );
        assert!(
            !request
                .headers()
                .contains_key(reqwest::header::IF_NONE_MATCH)
        );
    }

    #[tokio::test]
    async fn test_invalid_ready_does_not_publish_resume_state() {
        let client = DiscordApiClient::new(AuthToken::bot("synthetic.bot.token.for-test-only"));
        let (events, _) = broadcast::channel(8);
        let cache = Arc::new(DiscordCacheEngine::new());
        let current_user_id = RwLock::new(None);
        let voice_session_id = RwLock::new(None);
        let voice_server = RwLock::new(None);

        let result = handle_dispatch(
            "READY",
            json!({
                "session_id": "synthetic-session",
                "resume_gateway_url": "wss://gateway.example/?token=synthetic",
                "user": { "id": "900000000000000001", "username": 42 }
            }),
            &events,
            &client.session_id,
            &client.resume_gateway_url,
            &cache,
            &current_user_id,
            &voice_session_id,
            &voice_server,
        )
        .await;

        assert!(result.is_err(), "READY inválido deve falhar");
        assert!(client.session_id.read().await.is_none());
        assert!(client.resume_gateway_url.read().await.is_none());
        assert!(current_user_id.read().await.is_none());
    }

    #[test]
    fn test_request_coalescing_key_ignores_non_get_and_includes_etag() {
        let request = reqwest::Request::new(
            reqwest::Method::GET,
            "https://discord.com/api/v10/users/@me".parse().unwrap(),
        );
        assert!(request_coalescing_key(&request).is_some());

        let post = reqwest::Request::new(
            reqwest::Method::POST,
            "https://discord.com/api/v10/channels/1/messages"
                .parse()
                .unwrap(),
        );
        assert!(request_coalescing_key(&post).is_none());
    }
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
            TransportEvent::PresenceUpdate {
                user_id,
                status,
                activity,
                custom_status,
            } => {
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
            TransportEvent::PresenceUpdate {
                user_id,
                status,
                activity,
                custom_status,
            } => {
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

    #[test]
    fn test_parse_presence_with_assets_and_timestamps() {
        let json = serde_json::json!({
            "user_id": "888777666",
            "status": "online",
            "activities": [
                {
                    "name": "Spotify",
                    "type": 2,
                    "details": "Song Title",
                    "state": "Artist Name",
                    "application_id": null,
                    "sync_id": "track_123",
                    "timestamps": {
                        "start": 1712850000000i64,
                        "end": 1712850230000i64
                    },
                    "assets": {
                        "large_image": "spotify:ab67616d0000b273abcdef",
                        "large_text": "Album Name"
                    }
                }
            ]
        });

        let event = parse_presence_from_json(&json).expect("should parse presence");
        match event {
            TransportEvent::PresenceUpdate {
                user_id,
                status,
                activity,
                custom_status,
            } => {
                assert_eq!(user_id, "888777666");
                assert_eq!(status, PresenceStatus::Online);
                assert!(custom_status.is_none());
                let act = activity.expect("activity should be present");
                assert_eq!(act.name, "Spotify");
                assert_eq!(act.details.as_deref(), Some("Song Title"));
                assert_eq!(act.state.as_deref(), Some("Artist Name"));
                assert_eq!(act.sync_id.as_deref(), Some("track_123"));
                let ts = act
                    .timestamps
                    .as_ref()
                    .expect("timestamps should be present");
                assert_eq!(ts.start, Some(1712850000000));
                assert_eq!(ts.end, Some(1712850230000));
                let assets = act.assets.as_ref().expect("assets should be present");
                assert_eq!(
                    assets.large_image.as_deref(),
                    Some("spotify:ab67616d0000b273abcdef")
                );
                assert_eq!(
                    act.large_image_url(),
                    Some("https://i.scdn.co/image/ab67616d0000b273abcdef".to_string())
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
        let decor = user
            .avatar_decoration_data
            .as_ref()
            .expect("decoration must be present");
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
        let decor = msg
            .author
            .avatar_decoration_data
            .as_ref()
            .expect("author decoration must be present");
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
        let decor = ch.recipient_avatar_decorations[0]
            .as_ref()
            .expect("recipient decoration must be present");
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
        let decor = state
            .avatar_decoration_data
            .as_ref()
            .expect("voice decoration must be present");
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
        let bare_state =
            parse_voice_state_from_json(&bare, None).expect("should parse voice state");
        assert_eq!(bare_state.avatar_decoration_data, None);
    }
}

impl DiscordApiClient {
    /// GET /api/v10/channels/{channel_id}/messages/search
    pub async fn search_channel_messages(
        &self,
        channel_id: &str,
        query: &crate::chat::search::SearchQuery,
        offset: u32,
        sort_order: Option<&str>,
    ) -> Result<crate::chat::search::SearchApiResponse> {
        let path = format!("/channels/{}/messages/search", channel_id);
        let mut params = query.to_api_query_params();
        if offset > 0 {
            params.push(("offset", offset.to_string()));
        }
        if let Some(order) = sort_order {
            params.push(("sort_by", "timestamp".to_string()));
            params.push(("sort_order", order.to_string()));
        }
        let mut qs = String::new();
        for (k, v) in &params {
            if !qs.is_empty() {
                qs.push('&');
            }
            let encoded: String = v
                .chars()
                .flat_map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                        vec![c]
                    } else {
                        format!("%{:02X}", c as u8).chars().collect()
                    }
                })
                .collect();
            qs.push_str(k);
            qs.push('=');
            qs.push_str(&encoded);
        }
        let full_path = if qs.is_empty() {
            path
        } else {
            format!("{}?{}", path, qs)
        };
        let req = self.auth_request(reqwest::Method::GET, &full_path);
        let resp = self.send_request(&full_path, req).await?;
        if resp.status() == reqwest::StatusCode::ACCEPTED {
            #[derive(serde::Deserialize)]
            struct IndexingBody {
                #[serde(default)]
                retry_after: Option<f64>,
            }
            let body: IndexingBody = resp
                .json()
                .await
                .unwrap_or(IndexingBody { retry_after: None });
            let retry = body.retry_after.unwrap_or(2.5);
            Ok(crate::chat::search::SearchApiResponse::Indexing { retry_after: retry })
        } else {
            let payload = resp.json::<crate::chat::search::SearchPayload>().await?;
            Ok(crate::chat::search::SearchApiResponse::Ready(payload))
        }
    }

    /// GET /api/v10/guilds/{guild_id}/messages/search
    pub async fn search_guild_messages(
        &self,
        guild_id: &str,
        query: &crate::chat::search::SearchQuery,
        offset: u32,
        sort_order: Option<&str>,
    ) -> Result<crate::chat::search::SearchApiResponse> {
        let path = format!("/guilds/{}/messages/search", guild_id);
        let mut params = query.to_api_query_params();
        if offset > 0 {
            params.push(("offset", offset.to_string()));
        }
        if let Some(order) = sort_order {
            params.push(("sort_by", "timestamp".to_string()));
            params.push(("sort_order", order.to_string()));
        }
        let mut qs = String::new();
        for (k, v) in &params {
            if !qs.is_empty() {
                qs.push('&');
            }
            let encoded: String = v
                .chars()
                .flat_map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                        vec![c]
                    } else {
                        format!("%{:02X}", c as u8).chars().collect()
                    }
                })
                .collect();
            qs.push_str(k);
            qs.push('=');
            qs.push_str(&encoded);
        }
        let full_path = if qs.is_empty() {
            path
        } else {
            format!("{}?{}", path, qs)
        };
        let req = self.auth_request(reqwest::Method::GET, &full_path);
        let resp = self.send_request(&full_path, req).await?;
        if resp.status() == reqwest::StatusCode::ACCEPTED {
            #[derive(serde::Deserialize)]
            struct IndexingBody {
                #[serde(default)]
                retry_after: Option<f64>,
            }
            let body: IndexingBody = resp
                .json()
                .await
                .unwrap_or(IndexingBody { retry_after: None });
            let retry = body.retry_after.unwrap_or(2.5);
            Ok(crate::chat::search::SearchApiResponse::Indexing { retry_after: retry })
        } else {
            let payload = resp.json::<crate::chat::search::SearchPayload>().await?;
            Ok(crate::chat::search::SearchApiResponse::Ready(payload))
        }
    }

    /// Edits an existing message (PATCH /channels/{channel_id}/messages/{message_id})
    pub async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        content: &str,
    ) -> Result<Message> {
        let path = format!("/channels/{}/messages/{}", channel_id, message_id);
        let body = json!({ "content": content });
        let req = self.auth_request(reqwest::Method::PATCH, &path).json(&body);
        let resp = self.send_request(&path, req).await?;
        let val: serde_json::Value = resp.json().await?;

        let msg = parse_message_from_json(&val)
            .context("Failed to deserialize edited message response from Discord API")?;

        self.cache_engine
            .apply_gateway_message_update(channel_id, message_id, content)
            .await;

        Ok(msg)
    }

    /// Pins a message in a channel (PUT /channels/{channel_id}/pins/{message_id})
    pub async fn pin_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let path = format!("/channels/{}/pins/{}", channel_id, message_id);
        let req = self.auth_request(reqwest::Method::PUT, &path);
        self.send_request(&path, req).await?;
        Ok(())
    }

    /// Unpins a message in a channel (DELETE /channels/{channel_id}/pins/{message_id})
    pub async fn unpin_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let path = format!("/channels/{}/pins/{}", channel_id, message_id);
        let req = self.auth_request(reqwest::Method::DELETE, &path);
        self.send_request(&path, req).await?;
        Ok(())
    }

    /// Requests guild members for message authors via Gateway Opcode 8 (chunks of <= 100)
    pub async fn request_guild_members(&self, guild_id: &str, user_ids: Vec<String>) -> Result<()> {
        for chunk in user_ids.chunks(100) {
            let payload = json!({
                "op": 8,
                "d": {
                    "guild_id": guild_id,
                    "user_ids": chunk
                }
            });
            self.send_gateway_payload(&payload).await?;
        }
        Ok(())
    }

    /// Updates active viewport member list ranges via Gateway Opcode 14
    pub async fn update_guild_member_viewport(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> Result<()> {
        if self.token.token_type == TokenType::User {
            let coordinator = crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared();
            let client = self.clone();
            match coordinator
                .try_update_viewport_ranges_and_send(guild_id, channel_id, ranges, move |payload| {
                    let client = client.clone();
                    async move { client.send_gateway_payload(&payload).await }
                })
                .await
            {
                Some(result) => result,
                None => Ok(()),
            }
        } else {
            Ok(())
        }
    }
}

fn handle_guild_members_chunk_dispatch(
    data: &serde_json::Value,
    event_tx: &broadcast::Sender<TransportEvent>,
) {
    let guild_id = match data.get("guild_id").and_then(|v| v.as_str()) {
        Some(gid) => gid,
        None => return,
    };
    let not_found: Vec<String> = data
        .get("not_found")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    for uid in &not_found {
        crate::protocol::cache::lazy_guilds::mark_op8_requested(guild_id, uid);
    }

    let members_arr = match data.get("members").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            let _ = event_tx.send(TransportEvent::GuildMembersChunk {
                guild_id: guild_id.to_string(),
                members: Vec::new(),
                not_found,
            });
            return;
        }
    };

    let guild_roles = crate::protocol::cache::lazy_guilds::get_cached_guild_roles(guild_id);
    let mut parsed_members = Vec::new();

    for m in members_arr {
        let user = match m.get("user") {
            Some(u) => u,
            None => continue,
        };
        let user_id = match user.get("id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => continue,
        };
        let username = user
            .get("username")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let global_name = user
            .get("global_name")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let avatar = user
            .get("avatar")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let is_bot = user.get("bot").and_then(|v| v.as_bool()).unwrap_or(false);
        let avatar_decoration_data = user
            .get("avatar_decoration_data")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let roles: Vec<String> = m
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| r.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();

        crate::protocol::cache::lazy_guilds::cache_author_roles(guild_id, user_id, roles.clone());
        crate::protocol::cache::lazy_guilds::mark_op8_requested(guild_id, user_id);

        let mut role_color = 0;
        if !guild_roles.is_empty() {
            role_color = crate::protocol::cache::lazy_guilds::resolve_member_role_color(
                &roles,
                &guild_roles,
            );
            if role_color != 0 {
                crate::protocol::cache::lazy_guilds::cache_author_role_color(
                    guild_id, user_id, role_color,
                );
            }
        }

        parsed_members.push(crate::protocol::cache::lazy_guilds::GuildMemberChunkItem {
            id: user_id.to_string(),
            username,
            global_name,
            avatar,
            avatar_decoration_data,
            roles,
            role_color,
            is_bot,
        });
    }

    let _ = event_tx.send(TransportEvent::GuildMembersChunk {
        guild_id: guild_id.to_string(),
        members: parsed_members,
        not_found,
    });
}
