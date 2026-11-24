//! Estrutura de Diretórios de Cache Idêntica ao Discord Oficial
//!
//! Organiza o diretório `~/.config/discord-rs/cache/` com partições para:
//! - `guilds/`: Dados de servidores e manifestos de canais
//! - `channels/`: Metadados e permissões de canais
//! - `messages/`: Histórico particionado por canal em chunks contíguos
//! - `users/`: Perfis de usuários e `@me.json`
//! - `assets/`: Armazenamento com endereçamento por conteúdo (CAS SHA-256) particionado em 256 shards

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::channel::Channel;
use crate::domain::guild::Guild;
use crate::domain::message::Message;
use crate::domain::user::User;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Converte um identificador em um único segmento de caminho seguro.
/// IDs do Discord já são seguros; a codificação existe para impedir travessia
/// de diretório caso um payload inesperado chegue ao cache.
fn cache_file_id(id: &str) -> String {
    let mut encoded = String::with_capacity(id.len());
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'@') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    if encoded.is_empty() {
        encoded.push('_');
    }
    encoded
}

fn is_valid_sha256_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Manifesto global do cache local
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheManifest {
    pub version: u32,
    pub last_selected_guild_id: Option<String>,
    pub last_selected_channel_id: Option<String>,
    pub active_user_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl Default for CacheManifest {
    fn default() -> Self {
        Self {
            version: 1,
            last_selected_guild_id: None,
            last_selected_channel_id: None,
            active_user_id: None,
            updated_at: Utc::now(),
        }
    }
}

/// Metadados de um asset armazenado via Content-Addressing (CAS)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetMetadata {
    pub source_url: String,
    pub sha256_hash: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub etag: Option<String>,
    pub cached_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Gerenciador da estrutura de diretórios do cache em disco
#[derive(Debug, Clone)]
pub struct CacheDirectory {
    root_path: PathBuf,
}

impl CacheDirectory {
    /// Inicializa o gerenciador no caminho padrão: `~/.config/discord-rs/cache/`
    pub fn default_path() -> Self {
        let root = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("discord-rs")
            .join("cache");
        Self::with_root(root)
    }

    /// Inicializa o gerenciador com um diretório raiz customizado (ideal para testes)
    pub fn with_root(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    /// Retorna o caminho raiz do cache
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Subpasta de servidores: `<root>/guilds/`
    pub fn guilds_dir(&self) -> PathBuf {
        self.root_path.join("guilds")
    }

    /// Subpasta de canais: `<root>/channels/`
    pub fn channels_dir(&self) -> PathBuf {
        self.root_path.join("channels")
    }

    /// Subpasta de mensagens: `<root>/messages/`
    pub fn messages_dir(&self) -> PathBuf {
        self.root_path.join("messages")
    }

    /// Subpasta de mensagens de um canal específico: `<root>/messages/<channel_id>/`
    pub fn channel_messages_dir(&self, channel_id: &str) -> PathBuf {
        self.messages_dir().join(cache_file_id(channel_id))
    }

    /// Subpasta de usuários: `<root>/users/`
    pub fn users_dir(&self) -> PathBuf {
        self.root_path.join("users")
    }

    /// Subpasta de assets: `<root>/assets/`
    pub fn assets_dir(&self) -> PathBuf {
        self.root_path.join("assets")
    }

    /// Garante que toda a árvore de diretórios do cache existe
    pub fn ensure_dirs(&self) -> Result<()> {
        let dirs = [
            self.root_path.clone(),
            self.guilds_dir(),
            self.channels_dir(),
            self.messages_dir(),
            self.users_dir(),
            self.assets_dir(),
        ];
        for dir in &dirs {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create cache directory {:?}", dir))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
        }
        Ok(())
    }

    // --- Gravação Atômica (Crash-Resilient via Temp File + Rename) ---

    /// Grava dados em arquivo temporário com flush e renomeia atomicamente para o destino final
    pub fn atomic_write_bytes(&self, target_path: &Path, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_filename = format!(
            ".tmp_{}_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temp_path = target_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(temp_filename);

        let write_result = (|| -> Result<()> {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp_path)
                .with_context(|| format!("Failed to create temporary file {:?}", temp_path))?;
            file.write_all(bytes)?;
            file.sync_data()?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }

        if let Err(error) = fs::rename(&temp_path, target_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error).with_context(|| {
                format!(
                    "Failed to atomically rename {:?} to {:?}",
                    temp_path, target_path
                )
            });
        }

        // No Unix, sincroniza também o diretório para que o rename sobreviva
        // a uma queda imediata após a confirmação da escrita.
        #[cfg(unix)]
        if let Some(parent) = target_path.parent() {
            let directory = fs::File::open(parent)
                .with_context(|| format!("Failed to open cache directory {:?}", parent))?;
            directory
                .sync_all()
                .with_context(|| format!("Failed to fsync cache directory {:?}", parent))?;
        }

        Ok(())
    }

    /// Grava string JSON formatada de forma atômica
    pub fn atomic_write_json<T: Serialize>(&self, target_path: &Path, value: &T) -> Result<()> {
        let json_data = serde_json::to_vec_pretty(value)?;
        self.atomic_write_bytes(target_path, &json_data)
    }

    // --- Manifesto do Cache ---

    pub fn manifest_path(&self) -> PathBuf {
        self.root_path.join("manifest.json")
    }

    pub fn save_manifest(&self, manifest: &CacheManifest) -> Result<()> {
        self.ensure_dirs()?;
        self.atomic_write_json(&self.manifest_path(), manifest)
    }

    pub fn load_manifest(&self) -> Option<CacheManifest> {
        let path = self.manifest_path();
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    // --- Guilds Cache ---

    pub fn guild_path(&self, guild_id: &str) -> PathBuf {
        self.guilds_dir()
            .join(format!("{}.json", cache_file_id(guild_id)))
    }

    pub fn save_guild(&self, guild: &Guild) -> Result<()> {
        self.ensure_dirs()?;
        self.atomic_write_json(&self.guild_path(&guild.id), guild)
    }

    pub fn load_guild(&self, guild_id: &str) -> Option<Guild> {
        let path = self.guild_path(guild_id);
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    // --- Channels Cache ---

    pub fn channel_path(&self, channel_id: &str) -> PathBuf {
        self.channels_dir()
            .join(format!("{}.json", cache_file_id(channel_id)))
    }

    pub fn save_channel(&self, channel: &Channel) -> Result<()> {
        self.ensure_dirs()?;
        self.atomic_write_json(&self.channel_path(&channel.id), channel)
    }

    pub fn load_channel(&self, channel_id: &str) -> Option<Channel> {
        let path = self.channel_path(channel_id);
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    // --- Messages Cache ---

    pub fn save_channel_messages(&self, channel_id: &str, messages: &[Message]) -> Result<()> {
        self.ensure_dirs()?;
        let dir = self.channel_messages_dir(channel_id);
        fs::create_dir_all(&dir)?;

        let target_file = dir.join("messages_latest.json");
        self.atomic_write_json(&target_file, &messages)
    }

    pub fn load_channel_messages(&self, channel_id: &str) -> Option<Vec<Message>> {
        let target_file = self
            .channel_messages_dir(channel_id)
            .join("messages_latest.json");
        if !target_file.exists() {
            return None;
        }
        let data = fs::read_to_string(target_file).ok()?;
        serde_json::from_str(&data).ok()
    }

    // --- Users Cache ---

    pub fn user_path(&self, user_id: &str) -> PathBuf {
        self.users_dir()
            .join(format!("{}.json", cache_file_id(user_id)))
    }

    pub fn save_user(&self, user: &User) -> Result<()> {
        self.ensure_dirs()?;
        self.atomic_write_json(&self.user_path(&user.id), user)
    }

    pub fn load_user(&self, user_id: &str) -> Option<User> {
        let path = self.user_path(user_id);
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save_current_user(&self, user: &User) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.users_dir().join("@me.json");
        self.atomic_write_json(&path, user)
    }

    pub fn load_current_user(&self) -> Option<User> {
        let path = self.users_dir().join("@me.json");
        if !path.exists() {
            return None;
        }
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }
}

/// Armazenamento com Endereçamento por Conteúdo (CAS) com SHA-256
#[derive(Debug, Clone)]
pub struct AssetCasStore {
    cache_dir: CacheDirectory,
}

impl AssetCasStore {
    pub fn new(cache_dir: CacheDirectory) -> Self {
        Self { cache_dir }
    }

    /// Calcula o hash SHA-256 de um conjunto de bytes
    pub fn compute_hash(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    /// Resolve o caminho particionado do asset no disco baseado nos 2 primeiros caracteres hexadecimais
    pub fn asset_paths(&self, sha256_hash: &str) -> (PathBuf, PathBuf) {
        let safe_hash = cache_file_id(sha256_hash);
        let prefix = if safe_hash.len() >= 2 {
            &safe_hash[..2]
        } else {
            "00"
        };
        let partition_dir = self.cache_dir.assets_dir().join(prefix);
        let bin_path = partition_dir.join(format!("{}.bin", safe_hash));
        let meta_path = partition_dir.join(format!("{}.meta.json", safe_hash));
        (bin_path, meta_path)
    }

    /// Armazena um asset na estrutura CAS, evitando duplicatas se o hash já existir
    pub fn store_asset(
        &self,
        source_url: &str,
        bytes: &[u8],
        content_type: &str,
        etag: Option<String>,
        ttl_seconds: Option<i64>,
    ) -> Result<AssetMetadata> {
        self.cache_dir.ensure_dirs()?;
        let sha256_hash = Self::compute_hash(bytes);
        let (bin_path, meta_path) = self.asset_paths(&sha256_hash);

        if let Some(parent) = bin_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Reaproveita o binário apenas se existir e passar na verificação de
        // integridade; um arquivo corrompido é reparado atomicamente.
        let binary_is_valid = fs::read(&bin_path)
            .map(|existing| Self::compute_hash(&existing) == sha256_hash)
            .unwrap_or(false);
        if !binary_is_valid {
            self.cache_dir.atomic_write_bytes(&bin_path, bytes)?;
        }

        let now = Utc::now();
        let expires_at = ttl_seconds.map(|secs| now + chrono::Duration::seconds(secs));

        let meta = AssetMetadata {
            source_url: source_url.to_string(),
            sha256_hash: sha256_hash.clone(),
            content_type: content_type.to_string(),
            size_bytes: bytes.len(),
            etag,
            cached_at: now,
            last_accessed_at: now,
            expires_at,
        };

        self.cache_dir.atomic_write_json(&meta_path, &meta)?;
        Ok(meta)
    }

    /// Recupera os bytes e metadados de um asset pelo hash SHA-256
    pub fn load_asset(&self, sha256_hash: &str) -> Option<(Vec<u8>, AssetMetadata)> {
        if !is_valid_sha256_hash(sha256_hash) {
            return None;
        }
        let (bin_path, meta_path) = self.asset_paths(sha256_hash);
        if !bin_path.exists() || !meta_path.exists() {
            return None;
        }

        let bytes = fs::read(&bin_path).ok()?;
        if Self::compute_hash(&bytes) != sha256_hash {
            return None;
        }
        let meta_str = fs::read_to_string(&meta_path).ok()?;
        let mut meta: AssetMetadata = serde_json::from_str(&meta_str).ok()?;
        if meta.sha256_hash != sha256_hash {
            return None;
        }
        if meta
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
        {
            return None;
        }

        // Atualiza last_accessed_at
        meta.last_accessed_at = Utc::now();
        let _ = self.cache_dir.atomic_write_json(&meta_path, &meta);

        Some((bytes, meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache_dir(test_name: &str) -> CacheDirectory {
        let dir = std::env::temp_dir().join(format!(
            "discord_rs_cache_test_{}_{}",
            test_name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        CacheDirectory::with_root(dir)
    }

    #[test]
    fn test_cache_directory_creation_and_atomic_writes() -> Result<()> {
        let cache = temp_cache_dir("atomic");
        cache.ensure_dirs()?;

        assert!(cache.guilds_dir().exists());
        assert!(cache.channels_dir().exists());
        assert!(cache.messages_dir().exists());
        assert!(cache.users_dir().exists());
        assert!(cache.assets_dir().exists());

        let mut manifest = CacheManifest::default();
        manifest.last_selected_guild_id = Some("123".to_string());
        cache.save_manifest(&manifest)?;

        let loaded = cache.load_manifest().expect("Manifest should exist");
        assert_eq!(loaded.last_selected_guild_id.as_deref(), Some("123"));

        Ok(())
    }

    #[test]
    fn test_cache_ids_cannot_escape_their_directory() -> Result<()> {
        let cache = temp_cache_dir("safe_ids");
        let path = cache.guild_path("../../outside");
        assert!(path.starts_with(cache.guilds_dir()));
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "%2E%2E%2F%2E%2E%2Foutside.json"
        );
        assert!(
            AssetCasStore::new(cache.clone())
                .asset_paths("../../outside")
                .0
                .starts_with(cache.assets_dir())
        );
        Ok(())
    }

    #[test]
    fn test_cas_rejects_expired_and_corrupted_assets() -> Result<()> {
        let cache = temp_cache_dir("cas_integrity");
        let cas = AssetCasStore::new(cache.clone());
        let meta = cas.store_asset(
            "https://cdn.discordapp.com/icons/123/icon.png",
            b"synthetic-icon",
            "image/png",
            None,
            Some(-1),
        )?;
        assert!(cas.load_asset(&meta.sha256_hash).is_none());

        let meta = cas.store_asset(
            "https://cdn.discordapp.com/icons/123/icon.png",
            b"synthetic-icon",
            "image/png",
            None,
            Some(3600),
        )?;
        let (bin_path, _) = cas.asset_paths(&meta.sha256_hash);
        fs::write(bin_path, b"corrupted")?;
        assert!(cas.load_asset(&meta.sha256_hash).is_none());
        assert!(cas.load_asset("../../outside").is_none());

        let repaired = cas.store_asset(
            "https://cdn.discordapp.com/icons/123/icon.png",
            b"synthetic-icon",
            "image/png",
            None,
            Some(3600),
        )?;
        assert_eq!(
            cas.load_asset(&repaired.sha256_hash).unwrap().0,
            b"synthetic-icon"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_atomic_cache_files_are_owner_only() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let cache = temp_cache_dir("permissions");
        let target = cache.root_path().join("private.json");
        cache.atomic_write_bytes(&target, b"{}")?;
        let mode = fs::metadata(target)?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        Ok(())
    }

    #[test]
    fn test_cas_store_and_deduplication() -> Result<()> {
        let cache = temp_cache_dir("cas");
        let cas = AssetCasStore::new(cache.clone());

        let fake_icon = b"discord-rs-sample-icon-data-png";
        let meta = cas.store_asset(
            "https://cdn.discordapp.com/icons/123/icon.png",
            fake_icon,
            "image/png",
            Some("\"etag123\"".to_string()),
            Some(3600),
        )?;

        assert_eq!(meta.size_bytes, fake_icon.len());
        assert_eq!(meta.content_type, "image/png");

        let (bytes, loaded_meta) = cas
            .load_asset(&meta.sha256_hash)
            .expect("Asset must be loaded");
        assert_eq!(bytes, fake_icon);
        assert_eq!(
            loaded_meta.source_url,
            "https://cdn.discordapp.com/icons/123/icon.png"
        );

        // Deduplication test: storing the same content from another URL reuses the hash
        let meta_dup = cas.store_asset(
            "https://cdn.discordapp.com/avatars/456/avatar.png",
            fake_icon,
            "image/png",
            None,
            None,
        )?;
        assert_eq!(meta_dup.sha256_hash, meta.sha256_hash);

        Ok(())
    }
}
