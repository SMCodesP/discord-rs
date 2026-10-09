use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
}

/// Managed Discord authentication token with secret masking.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthToken {
    token: String,
    pub token_type: TokenType,
}

impl AuthToken {
    /// Creates a bot token.
    pub fn bot(token: impl Into<String>) -> Self {
        Self {
            token: token.into().trim().to_string(),
            token_type: TokenType::Bot,
        }
    }

    /// Creates a user token.
    pub fn user(token: impl Into<String>) -> Self {
        Self {
            token: token.into().trim().to_string(),
            token_type: TokenType::User,
        }
    }

    /// Raw secret token string.
    pub fn secret(&self) -> &str {
        &self.token
    }

    /// Constructs the Authorization HTTP header value.
    pub fn authorization_header(&self) -> String {
        match self.token_type {
            TokenType::Bot => format!("Bot {}", self.token),
            TokenType::User => self.token.clone(),
        }
    }

    /// Validates token format (must be non-empty and have reasonable length).
    pub fn is_valid_format(&self) -> bool {
        let trimmed = self.token.trim();
        // Discord tokens typically contain dots separating ID, timestamp, and HMAC
        !trimmed.is_empty() && trimmed.len() >= 24
    }
}

impl fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked = if self.token.len() > 8 {
            format!("{}...***", &self.token[..6])
        } else {
            "***".to_string()
        };

        f.debug_struct("AuthToken")
            .field("token_type", &self.token_type)
            .field("token", &masked)
            .finish()
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
    pub fn save(&self, token: &AuthToken) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
        }

        let serialized = serde_json::to_string_pretty(token)
            .context("Failed to serialize auth token")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600) // Secure permissions: only readable/writable by current user
                .open(&self.file_path)
                .with_context(|| format!("Failed to open auth file securely: {:?}", self.file_path))?;
            file.write_all(serialized.as_bytes())?;
        }

        #[cfg(not(unix))]
        {
            let mut file = fs::File::create(&self.file_path)
                .with_context(|| format!("Failed to create auth file: {:?}", self.file_path))?;
            file.write_all(serialized.as_bytes())?;
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
