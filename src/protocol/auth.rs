use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Official Discord OAuth2 authorization endpoint.
pub const DISCORD_OAUTH_AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
/// Official Discord OAuth2 token endpoint.
pub const DISCORD_OAUTH_TOKEN_URL: &str = "https://discord.com/api/oauth2/token";
/// Default lifetime of a pending authorization request (state/nonce/PKCE).
pub const OAUTH_REQUEST_TTL: Duration = Duration::from_secs(600);
/// Scopes requested by the native client login flow.
pub const DEFAULT_OAUTH_SCOPES: &[&str] = &["identify", "guilds"];

/// Discord token authorization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenType {
    /// Discord Bot Token (Recommended, 100% compliant with Discord Developer Terms of Service).
    Bot,
    /// Discord User Token.
    ///
    /// # Warning & ToS Notice
    /// Automating a personal Discord user account (commonly referred to as a "self-bot"
    /// or unauthorized client automation) violates Discord Terms of Service.
    /// Discord actively detects and suspends user accounts automating Gateway/REST traffic.
    /// Use for educational or approved client experimentation at your own risk.
    User,
    /// OAuth2 access token obtained through Authorization Code + PKCE.
    ///
    /// Sent as `Authorization: Bearer <token>` and only persisted after the
    /// token endpoint (and the Gateway handshake) confirm the credential.
    OAuth,
}

/// Managed Discord authentication token with secret masking.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthToken {
    token: String,
    pub token_type: TokenType,
    /// OAuth2 refresh token, when the credential came from Authorization Code + PKCE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    /// Unix timestamp (seconds) at which the OAuth2 access token expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<u64>,
}

impl AuthToken {
    /// Creates a bot token.
    pub fn bot(token: impl Into<String>) -> Self {
        Self {
            token: token.into().trim().to_string(),
            token_type: TokenType::Bot,
            refresh_token: None,
            expires_at: None,
        }
    }

    /// Creates a user token.
    pub fn user(token: impl Into<String>) -> Self {
        Self {
            token: token.into().trim().to_string(),
            token_type: TokenType::User,
            refresh_token: None,
            expires_at: None,
        }
    }

    /// Creates an OAuth2 bearer token from a successful token exchange.
    pub fn oauth(access_token: impl Into<String>) -> Self {
        Self {
            token: access_token.into().trim().to_string(),
            token_type: TokenType::OAuth,
            refresh_token: None,
            expires_at: None,
        }
    }

    /// Creates an OAuth2 bearer token carrying refresh metadata.
    pub fn oauth_with_refresh(
        access_token: impl Into<String>,
        refresh_token: Option<String>,
        expires_at: Option<u64>,
    ) -> Self {
        Self {
            token: access_token.into().trim().to_string(),
            token_type: TokenType::OAuth,
            refresh_token: refresh_token.map(|value| value.trim().to_string()),
            expires_at,
        }
    }

    /// Raw secret token string.
    pub fn secret(&self) -> &str {
        &self.token
    }

    /// OAuth2 refresh token, if any.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Unix timestamp (seconds) at which this OAuth2 token expires.
    pub fn expires_at(&self) -> Option<u64> {
        self.expires_at
    }

    /// Whether this OAuth2 token is expired at `now` (Unix seconds).
    /// Non-OAuth tokens never expire locally.
    pub fn is_expired_at(&self, now: u64) -> bool {
        self.token_type == TokenType::OAuth
            && self.expires_at.is_some_and(|expires_at| expires_at <= now)
    }

    /// Constructs the Authorization HTTP header value.
    pub fn authorization_header(&self) -> String {
        match self.token_type {
            TokenType::Bot => format!("Bot {}", self.token),
            TokenType::User => self.token.clone(),
            TokenType::OAuth => format!("Bearer {}", self.token),
        }
    }

    /// Validates token format (must be non-empty and have reasonable length).
    pub fn is_valid_format(&self) -> bool {
        let trimmed = self.token.trim();
        // Discord tokens are long opaque strings. Reject whitespace/control
        // characters and values too short to be a real credential.
        !trimmed.is_empty()
            && trimmed.len() >= 24
            && trimmed.chars().all(|ch| !ch.is_whitespace() && !ch.is_control())
    }
}

impl fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked = mask_secret(&self.token);

        f.debug_struct("AuthToken")
            .field("token_type", &self.token_type)
            .field("token", &masked)
            .field("refresh_token", &self.refresh_token.as_deref().map(mask_secret))
            .finish()
    }
}

fn mask_secret(secret: &str) -> String {
    if secret.len() > 8 {
        format!("{}...***", &secret[..6])
    } else {
        "***".to_string()
    }
}

/// Token storage and persistence manager.
pub struct TokenStorage {
    file_path: PathBuf,
}

impl Default for TokenStorage {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            let path = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from(".config"))
                .join("discord-rs")
                .join("auth.json");
            Self::with_path(path)
        })
    }
}

impl TokenStorage {
    /// Creates storage pointing to standard user config directory (~/.config/discord-rs/auth.json).
    pub fn new() -> Result<Self> {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("discord-rs")
            .join("auth.json");
        Ok(Self { file_path: path })
    }

    /// Creates storage pointing to a custom path.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: path.into(),
        }
    }

    /// Storage file path.
    pub fn path(&self) -> &Path {
        &self.file_path
    }

    /// Reads authentication token from DISCORD_TOKEN environment variable if available.
    pub fn load_from_env() -> Option<AuthToken> {
        if let Ok(val) = std::env::var("DISCORD_TOKEN") {
            let val = val.trim();
            if !val.is_empty() {
                if let Some(bot_token) = val.strip_prefix("Bot ") {
                    return Some(AuthToken::bot(bot_token));
                }
                // Check if user explicitly set token type
                if std::env::var("DISCORD_TOKEN_IS_USER").map(|v| v == "1" || v == "true").unwrap_or(false) {
                    return Some(AuthToken::user(val));
                } else {
                    return Some(AuthToken::bot(val));
                }
            }
        }
        None
    }

    /// Loads persisted token from disk.
    pub fn load(&self) -> Result<Option<AuthToken>> {
        if !self.file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.file_path)
            .with_context(|| format!("Failed to read auth file: {:?}", self.file_path))?;

        let token: AuthToken = serde_json::from_str(&content)
            .with_context(|| "Failed to parse stored authentication token JSON")?;

        Ok(Some(token))
    }

    /// Persists token securely to disk with 0600 permissions on Unix.
    ///
    /// The write is atomic (temporary file + rename) and rejects malformed
    /// credentials *before* touching the previous file. This guarantees an
    /// invalid token can never overwrite a valid saved credential.
    pub fn save(&self, token: &AuthToken) -> Result<()> {
        if !token.is_valid_format() {
            bail!("Refusing to persist an invalid Discord token format");
        }

        let parent = self
            .file_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        fs::create_dir_all(&parent)
            .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;

        let serialized = serde_json::to_string_pretty(token)
            .context("Failed to serialize auth token")?;

        static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".auth-{}-{}.tmp",
            std::process::id(),
            unique
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600) // Secure permissions: only readable/writable by current user
                .open(&temp_path)
                .with_context(|| format!("Failed to open auth temp file securely: {:?}", temp_path))?;
            file.write_all(serialized.as_bytes())?;
            file.sync_all()?;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))?;
        }

        #[cfg(not(unix))]
        {
            let mut file = fs::File::create(&temp_path)
                .with_context(|| format!("Failed to create auth temp file: {:?}", temp_path))?;
            file.write_all(serialized.as_bytes())?;
            file.sync_all()?;
        }

        if let Err(error) = fs::rename(&temp_path, &self.file_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error)
                .with_context(|| format!("Failed to replace auth file: {:?}", self.file_path));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // `rename` keeps the temp file permissions, but enforce them again
            // so an existing 0644 file can never survive an overwrite.
            fs::set_permissions(&self.file_path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }

    /// Clears and removes stored credentials.
    pub fn clear(&self) -> Result<()> {
        if self.file_path.exists() {
            fs::remove_file(&self.file_path)
                .with_context(|| format!("Failed to remove auth file: {:?}", self.file_path))?;
        }
        Ok(())
    }
}

// ============================================================================
// OAuth2 Authorization Code + PKCE
// ============================================================================

/// Errors produced by the OAuth2 Authorization Code + PKCE flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthError {
    /// The OAuth2 client id is missing.
    EmptyClientId,
    /// The OAuth2 redirect URI is missing.
    EmptyRedirectUri,
    /// The pending authorization request expired before the callback.
    Expired,
    /// The callback `state` does not match the request (CSRF protection).
    StateMismatch,
    /// The callback did not carry an authorization code.
    MissingCode,
    /// The user cancelled the authorization (or Discord denied access).
    Cancelled,
    /// Discord returned an OAuth2 error.
    ProviderError { code: String, description: String },
    /// The token endpoint returned a malformed or unsafe response.
    InvalidResponse(&'static str),
    /// The HTTP request could not be completed.
    Http(String),
    /// The PKCE verifier does not follow RFC 7636.
    InvalidPkceVerifier,
    /// A secret value (authorization code) was empty.
    EmptyAuthorizationCode,
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyClientId => write!(f, "DISCORD_CLIENT_ID ausente para o login OAuth2"),
            Self::EmptyRedirectUri => write!(f, "redirect URI ausente para o login OAuth2"),
            Self::Expired => write!(f, "A solicitação de autorização expirou"),
            Self::StateMismatch => write!(f, "O parâmetro state não corresponde à solicitação"),
            Self::MissingCode => write!(f, "A autorização não retornou um código"),
            Self::Cancelled => write!(f, "Autorização cancelada pelo usuário"),
            Self::ProviderError { code, description } => {
                if description.is_empty() {
                    write!(f, "OAuth2 recusado pelo Discord: {code}")
                } else {
                    write!(f, "OAuth2 recusado pelo Discord: {code} ({description})")
                }
            }
            Self::InvalidResponse(reason) => write!(f, "Resposta OAuth2 inválida: {reason}"),
            Self::Http(reason) => write!(f, "Falha de rede no login OAuth2: {reason}"),
            Self::InvalidPkceVerifier => {
                write!(f, "code_verifier PKCE inválido (RFC 7636: 43-128 caracteres unreserved)")
            }
            Self::EmptyAuthorizationCode => write!(f, "Código de autorização vazio"),
        }
    }
}

impl std::error::Error for OAuthError {}

/// OAuth2 client configuration for the official Authorization Code + PKCE flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthConfig {
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
}

impl OAuthConfig {
    /// Creates a config with the official Discord endpoints and default scopes.
    pub fn new(
        client_id: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Result<Self, OAuthError> {
        let client_id = client_id.into().trim().to_string();
        let redirect_uri = redirect_uri.into().trim().to_string();
        if client_id.is_empty() {
            return Err(OAuthError::EmptyClientId);
        }
        if redirect_uri.is_empty() {
            return Err(OAuthError::EmptyRedirectUri);
        }
        Ok(Self {
            client_id,
            redirect_uri,
            scopes: DEFAULT_OAUTH_SCOPES.iter().map(|s| s.to_string()).collect(),
            authorization_endpoint: DISCORD_OAUTH_AUTHORIZE_URL.to_string(),
            token_endpoint: DISCORD_OAUTH_TOKEN_URL.to_string(),
        })
    }

    /// Reads the client id and redirect URI from the environment.
    ///
    /// `DISCORD_OAUTH_CLIENT_ID` (or `DISCORD_CLIENT_ID`) and
    /// `DISCORD_OAUTH_REDIRECT_URI` must be provided explicitly. There is no
    /// hardcoded/fake client id.
    pub fn from_env() -> Result<Self, OAuthError> {
        let client_id = std::env::var("DISCORD_OAUTH_CLIENT_ID")
            .or_else(|_| std::env::var("DISCORD_CLIENT_ID"))
            .map_err(|_| OAuthError::EmptyClientId)?;
        let redirect_uri = std::env::var("DISCORD_OAUTH_REDIRECT_URI")
            .map_err(|_| OAuthError::EmptyRedirectUri)?;
        Self::new(client_id, redirect_uri)
    }

    /// Overrides the requested scopes.
    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }
}

/// RFC 7636 PKCE verifier/challenge pair.
#[derive(Clone, PartialEq, Eq)]
pub struct PkceChallenge {
    verifier: String,
    challenge: String,
}

impl PkceChallenge {
    /// Generates a cryptographically random verifier and its S256 challenge.
    pub fn generate() -> Result<Self, OAuthError> {
        let verifier = random_url_safe(64).map_err(OAuthError::Http)?;
        Self::from_verifier(verifier)
    }

    /// Derives the S256 challenge from an existing verifier.
    pub fn from_verifier(verifier: impl Into<String>) -> Result<Self, OAuthError> {
        let verifier = verifier.into();
        if !is_valid_pkce_verifier(&verifier) {
            return Err(OAuthError::InvalidPkceVerifier);
        }
        let challenge = s256_challenge(&verifier);
        Ok(Self {
            verifier,
            challenge,
        })
    }

    /// The secret `code_verifier` (never include in the authorization URL).
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    /// The public `code_challenge` (base64url SHA-256, no padding).
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

impl fmt::Debug for PkceChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PkceChallenge")
            .field("challenge", &self.challenge)
            .field("verifier", &"***")
            .finish()
    }
}

/// Authorization callback query parameters returned by the browser.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OAuthCallback {
    pub state: Option<String>,
    pub code: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

impl OAuthCallback {
    /// Parses a raw query string (without the leading `?`).
    pub fn from_query(query: &str) -> Self {
        let mut callback = Self::default();
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = percent_decode(raw_key);
            let value = percent_decode(raw_value);
            match key.as_str() {
                "state" if callback.state.is_none() => callback.state = Some(value),
                "code" if callback.code.is_none() => callback.code = Some(value),
                "error" if callback.error.is_none() => callback.error = Some(value),
                "error_description" if callback.error_description.is_none() => {
                    callback.error_description = Some(value)
                }
                _ => {}
            }
        }
        callback
    }
}

/// A validated authorization code. Debug output never reveals the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorizationCode(String);

impl AuthorizationCode {
    /// Raw authorization code, only for the token exchange request.
    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AuthorizationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthorizationCode(***)")
    }
}

/// OAuth2 token endpoint response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    #[serde(default = "default_bearer_token_type")]
    pub token_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

fn default_bearer_token_type() -> String {
    "Bearer".to_string()
}

impl OAuthTokens {
    /// Validates the minimum contract of a token endpoint response.
    pub fn validate(&self) -> Result<(), OAuthError> {
        if self.access_token.trim().is_empty()
            || self
                .access_token
                .chars()
                .any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return Err(OAuthError::InvalidResponse("access_token vazio ou inválido"));
        }
        if !self.token_type.eq_ignore_ascii_case("bearer") {
            return Err(OAuthError::InvalidResponse("token_type diferente de Bearer"));
        }
        if self
            .refresh_token
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(OAuthError::InvalidResponse("refresh_token vazio"));
        }
        Ok(())
    }

    /// Expiry as a Unix timestamp, computed from `expires_in`.
    pub fn expires_at(&self, issued_at_unix: u64) -> Option<u64> {
        self.expires_in.map(|seconds| issued_at_unix.saturating_add(seconds))
    }

    /// Converts the successful exchange into the gateway credential representation.
    pub fn to_auth_token(&self) -> AuthToken {
        self.to_auth_token_at(now_unix())
    }

    /// Converts the successful exchange into an `AuthToken` using an explicit clock.
    pub fn to_auth_token_at(&self, issued_at_unix: u64) -> AuthToken {
        AuthToken::oauth_with_refresh(
            self.access_token.clone(),
            self.refresh_token.clone(),
            self.expires_at(issued_at_unix),
        )
    }
}

/// A pending OAuth2 Authorization Code + PKCE request.
#[derive(Clone)]
pub struct OAuthPkceFlow {
    config: OAuthConfig,
    state: String,
    nonce: String,
    pkce: PkceChallenge,
    created_at: Instant,
    ttl: Duration,
    cancelled: bool,
}

impl fmt::Debug for OAuthPkceFlow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthPkceFlow")
            .field("client_id", &self.config.client_id)
            .field("state", &self.state)
            .field("nonce", &self.nonce)
            .field("pkce", &self.pkce)
            .field("ttl", &self.ttl)
            .field("cancelled", &self.cancelled)
            .finish()
    }
}

impl OAuthPkceFlow {
    /// Starts a new authorization request with a random state, nonce and PKCE pair.
    pub fn new(config: OAuthConfig) -> Result<Self, OAuthError> {
        Self::with_lifetime(config, OAUTH_REQUEST_TTL)
    }

    /// Starts a new request with a custom lifetime (useful for tests).
    pub fn with_lifetime(config: OAuthConfig, ttl: Duration) -> Result<Self, OAuthError> {
        let state = random_url_safe(32).map_err(OAuthError::Http)?;
        let nonce = random_url_safe(32).map_err(OAuthError::Http)?;
        let pkce = PkceChallenge::generate()?;
        Ok(Self {
            config,
            state,
            nonce,
            pkce,
            created_at: Instant::now(),
            ttl,
            cancelled: false,
        })
    }

    /// OAuth2 configuration used by this request.
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// CSRF `state` value.
    pub fn state(&self) -> &str {
        &self.state
    }

    /// OIDC-style nonce bound to this request.
    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    /// Public PKCE challenge.
    pub fn code_challenge(&self) -> &str {
        self.pkce.challenge()
    }

    /// Secret PKCE verifier (only sent to the token endpoint).
    pub fn code_verifier(&self) -> &str {
        self.pkce.verifier()
    }

    /// Marks the request as cancelled by the user.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Whether the request was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Whether the request has exceeded its lifetime.
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(Instant::now())
    }

    /// Whether the request has exceeded its lifetime at `now`.
    pub fn is_expired_at(&self, now: Instant) -> bool {
        now.duration_since(self.created_at) >= self.ttl
    }

    /// Builds the official authorization URL with state, nonce and PKCE S256.
    pub fn authorization_url(&self) -> String {
        let scope = self.config.scopes.join(" ");
        let params = [
            ("response_type", "code"),
            ("client_id", self.config.client_id.as_str()),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("scope", scope.as_str()),
            ("state", self.state.as_str()),
            ("nonce", self.nonce.as_str()),
            ("code_challenge", self.pkce.challenge()),
            ("code_challenge_method", "S256"),
        ];
        let query = params
            .iter()
            .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{}", self.config.authorization_endpoint, query)
    }

    /// Validates a callback against state, expiry and cancellation.
    pub fn validate_callback(
        &self,
        callback: &OAuthCallback,
    ) -> Result<AuthorizationCode, OAuthError> {
        self.validate_callback_at(callback, Instant::now())
    }

    /// Validates a callback with an explicit clock (useful for tests).
    pub fn validate_callback_at(
        &self,
        callback: &OAuthCallback,
        now: Instant,
    ) -> Result<AuthorizationCode, OAuthError> {
        if self.cancelled {
            return Err(OAuthError::Cancelled);
        }
        if self.is_expired_at(now) {
            return Err(OAuthError::Expired);
        }
        let callback_state = callback.state.as_deref().unwrap_or("");
        if !constant_time_eq(callback_state.as_bytes(), self.state.as_bytes()) {
            return Err(OAuthError::StateMismatch);
        }
        if let Some(error) = callback.error.as_deref() {
            if error.eq_ignore_ascii_case("access_denied") {
                return Err(OAuthError::Cancelled);
            }
            return Err(OAuthError::ProviderError {
                code: error.to_string(),
                description: callback
                    .error_description
                    .clone()
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect(),
            });
        }
        let code = callback.code.as_deref().unwrap_or("").trim();
        if code.is_empty() {
            return Err(OAuthError::MissingCode);
        }
        Ok(AuthorizationCode(code.to_string()))
    }

    /// Exchanges a validated authorization code for tokens using a new HTTP client.
    pub async fn exchange_code(
        &self,
        code: &AuthorizationCode,
    ) -> Result<OAuthTokens, OAuthError> {
        let client = reqwest::Client::new();
        self.exchange_code_with_client(&client, code).await
    }

    /// Exchanges a validated authorization code for tokens.
    pub async fn exchange_code_with_client(
        &self,
        client: &reqwest::Client,
        code: &AuthorizationCode,
    ) -> Result<OAuthTokens, OAuthError> {
        if code.secret().trim().is_empty() {
            return Err(OAuthError::EmptyAuthorizationCode);
        }
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code.secret()),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("client_id", self.config.client_id.as_str()),
            ("code_verifier", self.pkce.verifier()),
        ];
        self.post_token_request(client, &params).await
    }

    /// Refreshes an OAuth2 access token using a new HTTP client.
    pub async fn refresh(&self, refresh_token: &str) -> Result<OAuthTokens, OAuthError> {
        let client = reqwest::Client::new();
        self.refresh_with_client(&client, refresh_token).await
    }

    /// Refreshes an OAuth2 access token using a caller-provided HTTP client.
    pub async fn refresh_with_client(
        &self,
        client: &reqwest::Client,
        refresh_token: &str,
    ) -> Result<OAuthTokens, OAuthError> {
        if refresh_token.trim().is_empty() {
            return Err(OAuthError::InvalidResponse("refresh_token vazio"));
        }
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", self.config.client_id.as_str()),
        ];
        self.post_token_request(client, &params).await
    }

    /// Waits for the OAuth2 loopback redirect, validates the callback and
    /// exchanges the authorization code.
    ///
    /// The redirect URI must point at a loopback address with an explicit port
    /// (for example `http://127.0.0.1:38291/oauth/callback`).
    pub async fn wait_for_loopback_callback(&self) -> Result<OAuthTokens, OAuthError> {
        let redirect = parse_loopback_redirect(&self.config.redirect_uri)?;
        let listener = tokio::net::TcpListener::bind((redirect.host.as_str(), redirect.port))
            .await
            .map_err(|error| OAuthError::Http(format!("Falha ao escutar callback OAuth2: {error}")))?;

        let remaining = self.ttl.saturating_sub(self.created_at.elapsed());
        let deadline = tokio::time::Instant::now() + remaining;

        loop {
            let accepted = tokio::time::timeout_at(deadline, listener.accept()).await;
            let (mut stream, _peer) = match accepted {
                Ok(Ok(connection)) => connection,
                Ok(Err(error)) => {
                    return Err(OAuthError::Http(format!(
                        "Falha ao aceitar callback OAuth2: {error}"
                    )))
                }
                Err(_) => return Err(OAuthError::Expired),
            };

            let mut buffer = vec![0u8; 8192];
            let read = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::io::AsyncReadExt::read(&mut stream, &mut buffer),
            )
            .await;
            let read = match read {
                Ok(Ok(read)) => read,
                Ok(Err(error)) => {
                    let _ = write_http_response(&mut stream, 400, "Requisição inválida").await;
                    log::warn!("Callback OAuth2 ilegível: {error}");
                    continue;
                }
                Err(_) => continue,
            };

            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or("");
            let mut parts = first_line.split_whitespace();
            let method = parts.next().unwrap_or("");
            let target = parts.next().unwrap_or("");

            if method != "GET" {
                let _ = write_http_response(&mut stream, 405, "Método não permitido").await;
                continue;
            }

            let (path, query) = target.split_once('?').unwrap_or((target, ""));
            if path != redirect.path {
                let _ = write_http_response(&mut stream, 404, "Rota não encontrada").await;
                continue;
            }

            let callback = OAuthCallback::from_query(query);
            let code = match self.validate_callback(&callback) {
                Ok(code) => code,
                Err(error) => {
                    let _ = write_http_response(&mut stream, 400, "Autorização recusada").await;
                    return Err(error);
                }
            };

            match self.exchange_code(&code).await {
                Ok(tokens) => {
                    let _ = write_http_response(
                        &mut stream,
                        200,
                        "Login concluído. Você pode fechar esta janela e voltar ao Discord RS.",
                    )
                    .await;
                    return Ok(tokens);
                }
                Err(error) => {
                    let _ = write_http_response(
                        &mut stream,
                        502,
                        "Não foi possível concluir o login. Tente novamente.",
                    )
                    .await;
                    return Err(error);
                }
            }
        }
    }

    async fn post_token_request(
        &self,
        client: &reqwest::Client,
        params: &[(&str, &str)],
    ) -> Result<OAuthTokens, OAuthError> {
        let encoded = params
            .iter()
            .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        let response = client
            .post(&self.config.token_endpoint)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(encoded)
            .send()
            .await
            .map_err(|error| OAuthError::Http(error.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| OAuthError::Http(error.to_string()))?;

        if !status.is_success() {
            let (code, description) = parse_provider_error(&body);
            return Err(OAuthError::ProviderError { code, description });
        }

        let tokens: OAuthTokens = serde_json::from_str(&body)
            .map_err(|_| OAuthError::InvalidResponse("JSON do token endpoint inválido"))?;
        tokens.validate()?;
        Ok(tokens)
    }
}

struct LoopbackRedirect {
    host: String,
    port: u16,
    path: String,
}

fn parse_loopback_redirect(uri: &str) -> Result<LoopbackRedirect, OAuthError> {
    let rest = uri
        .strip_prefix("http://")
        .ok_or(OAuthError::EmptyRedirectUri)?;
    let (authority, path) = rest
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((rest, "/".to_string()));
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or(OAuthError::EmptyRedirectUri)?;
    let host = host.trim_matches(['[', ']']).to_string();
    if !matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1") {
        return Err(OAuthError::InvalidResponse(
            "redirect URI OAuth2 deve usar loopback (127.0.0.1/localhost)",
        ));
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| OAuthError::EmptyRedirectUri)?;
    if port == 0 {
        return Err(OAuthError::EmptyRedirectUri);
    }
    Ok(LoopbackRedirect { host, port, path })
}

async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    message: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        502 => "Bad Gateway",
        _ => "Error",
    };
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Discord RS</title><p>{message}</p>"
    );
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    tokio::io::AsyncWriteExt::write_all(stream, response.as_bytes()).await?;
    tokio::io::AsyncWriteExt::flush(stream).await
}

fn parse_provider_error(body: &str) -> (String, String) {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let code = value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("token_request_failed")
        .chars()
        .take(80)
        .collect::<String>();
    let description = value
        .get("error_description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    (code, description)
}

fn is_valid_pkce_verifier(verifier: &str) -> bool {
    let len = verifier.len();
    (43..=128).contains(&len)
        && verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

fn s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64_url_no_pad(&digest)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn base64_url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let combined = (b0 << 16) | (b1 << 8) | b2;
        output.push(ALPHABET[((combined >> 18) & 0x3f) as usize] as char);
        output.push(ALPHABET[((combined >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[((combined >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(combined & 0x3f) as usize] as char);
        }
    }
    output
}

fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(byte as char)
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = (bytes[index + 1] as char).to_digit(16);
                let low = (bytes[index + 2] as char).to_digit(16);
                if let (Some(high), Some(low)) = (high, low) {
                    output.push((high * 16 + low) as u8);
                    index += 3;
                    continue;
                }
                output.push(bytes[index]);
                index += 1;
            }
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn random_url_safe(bytes: usize) -> std::result::Result<String, String> {
    let mut buffer = vec![0u8; bytes];
    fill_secure_random(&mut buffer)?;
    Ok(base64_url_no_pad(&buffer))
}

#[cfg(unix)]
fn fill_secure_random(buffer: &mut [u8]) -> std::result::Result<(), String> {
    use std::io::Read;
    let mut file = fs::File::open("/dev/urandom")
        .map_err(|error| format!("Falha ao abrir /dev/urandom: {error}"))?;
    file.read_exact(buffer)
        .map_err(|error| format!("Falha ao ler /dev/urandom: {error}"))
}

#[cfg(not(unix))]
fn fill_secure_random(buffer: &mut [u8]) -> std::result::Result<(), String> {
    fill_fallback_random(buffer);
    Ok(())
}

#[cfg(not(unix))]
fn fill_fallback_random(buffer: &mut [u8]) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut offset = 0;
    while offset < buffer.len() {
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let seed = format!("{nanos}:{}:{counter}", std::process::id());
        let digest = Sha256::digest(seed.as_bytes());
        let take = (buffer.len() - offset).min(digest.len());
        buffer[offset..offset + take].copy_from_slice(&digest[..take]);
        offset += take;
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
