//! Discord RS - Módulo de Cache de Mídia e Integrações de Mídia
//!
//! Implementa:
//! 1. Gerenciamento do cache local em `~/.config/discord-rs/cache/media/` particionado por SHA-256 (00..ff).
//! 2. Persistência contínua de "Frequentemente Usados" (Recent Emojis) em `recent_emojis.json`.
//! 3. Modelos de dados para Emojis Discord, Stickers, StickerPacks e Soundboard.
//! 4. Parsers e cache de GIFs reais do Discord/Tenor, sem fallback fictício.
//! 5. Provider de categorias canônicas do Discord (8 categorias Unicode + Servidores).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};

use crate::protocol::image_decode::{
    DecodedImage, ImageDecodeCache, DEFAULT_CACHE_MAX_BYTES, DEFAULT_CACHE_MAX_ENTRIES,
    MAX_IMAGE_DIMENSION,
};

// ============================================================================
// Modelos de Dados de Emojis do Discord
// ============================================================================

/// Representa um Emoji oficial do Discord (Customizado de Guilda ou Unicode)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordEmoji {
    /// Snowflake ID do emoji (None para emojis unicode puros)
    pub id: Option<String>,
    /// Nome do emoji (ex: "ferris", "thumbsup")
    pub name: String,
    /// Cargos autorizados a usar este emoji (em guildas)
    #[serde(default)]
    pub roles: Vec<String>,
    /// Usuário que fez upload do emoji
    pub user: Option<serde_json::Value>,
    /// Se requer dois-pontos para digitação (`:emoji:`)
    #[serde(default = "default_true")]
    pub require_colons: bool,
    /// Se é gerenciado por uma integração externa (Twitch, YouTube)
    #[serde(default)]
    pub managed: bool,
    /// Se o emoji é animado (GIF)
    #[serde(default)]
    pub animated: bool,
    /// Se o emoji está disponível para uso
    #[serde(default = "default_true")]
    pub available: bool,
    /// ID da guilda de origem, se for um emoji customizado
    pub guild_id: Option<String>,
}

fn default_true() -> bool {
    true
}

impl DiscordEmoji {
    /// Cria um emoji customizado de servidor
    pub fn new_custom(
        id: impl Into<String>,
        name: impl Into<String>,
        animated: bool,
        guild_id: Option<String>,
    ) -> Self {
        Self {
            id: Some(id.into()),
            name: name.into(),
            roles: Vec::new(),
            user: None,
            require_colons: true,
            managed: false,
            animated,
            available: true,
            guild_id,
        }
    }

    /// Cria um emoji unicode padrão
    pub fn new_unicode(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            roles: Vec::new(),
            user: None,
            require_colons: false,
            managed: false,
            animated: false,
            available: true,
            guild_id: None,
        }
    }

    /// Retorna a tag canônica do Discord para inserção no chat
    pub fn to_tag(&self) -> String {
        match &self.id {
            Some(id) => {
                if self.animated {
                    format!("<a:{}:{}>", self.name, id)
                } else {
                    format!("<:{}:{}>", self.name, id)
                }
            }
            None => self.name.clone(),
        }
    }

    /// URL da imagem no CDN oficial do Discord
    pub fn cdn_url(&self) -> Option<String> {
        self.id.as_ref().map(|id| {
            if self.animated {
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

    /// Converte uma tag canônica `<:name:id>` ou `<a:name:id>` em um emoji customizado.
    pub fn from_tag(tag: &str) -> Option<Self> {
        let (animated, inner) = if let Some(inner) = tag.strip_prefix("<a:") {
            (true, inner)
        } else if let Some(inner) = tag.strip_prefix("<:") {
            (false, inner)
        } else {
            return None;
        };
        let inner = inner.strip_suffix('>')?;
        let (name, id) = inner.rsplit_once(':')?;
        if name.is_empty() || id.is_empty() {
            return None;
        }
        Some(Self::new_custom(id, name, animated, None))
    }

    /// Indica se é um emoji unicode nativo
    pub fn is_unicode(&self) -> bool {
        self.id.is_none()
    }
}

// ============================================================================
// Modelos de Dados de Stickers (Figurinhas)
// ============================================================================

/// Tipo de formato de figurinha do Discord
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StickerFormatType {
    Png = 1,
    Apng = 2,
    Lottie = 3,
    Gif = 4,
}

impl Default for StickerFormatType {
    fn default() -> Self {
        Self::Png
    }
}

impl Serialize for StickerFormatType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for StickerFormatType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = StickerFormatType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sticker format type as integer (1..4) or string")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    1 => Ok(StickerFormatType::Png),
                    2 => Ok(StickerFormatType::Apng),
                    3 => Ok(StickerFormatType::Lottie),
                    4 => Ok(StickerFormatType::Gif),
                    _ => Err(E::custom(format!("unknown sticker format type: {}", v))),
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v < 0 {
                    Err(E::custom(format!("unknown sticker format type: {}", v)))
                } else {
                    self.visit_u64(v as u64)
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v.to_lowercase().as_str() {
                    "png" | "1" => Ok(StickerFormatType::Png),
                    "apng" | "2" => Ok(StickerFormatType::Apng),
                    "lottie" | "3" => Ok(StickerFormatType::Lottie),
                    "gif" | "4" => Ok(StickerFormatType::Gif),
                    _ => Err(E::custom(format!("unknown sticker format type: {}", v))),
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

/// Representa uma Figurinha (Sticker) do Discord
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordSticker {
    pub id: String,
    pub pack_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub tags: String,
    pub format_type: StickerFormatType,
    #[serde(default = "default_true")]
    pub available: bool,
    pub guild_id: Option<String>,
}

impl DiscordSticker {
    /// URL no CDN oficial do Discord
    pub fn cdn_url(&self) -> String {
        match self.format_type {
            StickerFormatType::Png | StickerFormatType::Apng => {
                format!("https://cdn.discordapp.com/stickers/{}.png", self.id)
            }
            StickerFormatType::Lottie => {
                format!("https://cdn.discordapp.com/stickers/{}.json", self.id)
            }
            StickerFormatType::Gif => {
                format!(
                    "https://media.discordapp.net/stickers/{}.gif?size=160",
                    self.id
                )
            }
        }
    }
}

/// Pacote de Figurinhas padrão do Discord (Wumpus, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickerPack {
    pub id: String,
    pub name: String,
    pub sku_id: String,
    pub description: String,
    pub stickers: Vec<DiscordSticker>,
    pub banner_asset_id: Option<String>,
}

// ============================================================================
// Modelos de Dados de Soundboard
// ============================================================================

/// Som da Soundboard do Discord (Padrão ou de Servidor)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundboardSound {
    pub sound_id: String,
    pub name: String,
    pub volume: f64,
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
    pub guild_id: Option<String>,
    #[serde(default = "default_true")]
    pub available: bool,
}

impl SoundboardSound {
    /// URL de streaming de áudio
    pub fn sound_url(&self) -> String {
        format!(
            "https://cdn.discordapp.com/soundboard-sounds/{}",
            self.sound_id
        )
    }
}

// ============================================================================
// Modelos de Dados de GIFs (Discord / Tenor v2)
// ============================================================================

fn default_gif_dims() -> (u32, u32) {
    (220, 180)
}

/// Item retornado pela busca ou trending de GIFs do Tenor ou Discord.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenorGif {
    pub id: String,
    pub title: String,
    /// Poster derivado de `gif_src`; o vídeo bruto fica em `preview`.
    pub preview_url: String,
    /// Página do provedor no Discord; mídia direta no Tenor v2.
    pub url: String,
    #[serde(default = "default_gif_dims")]
    pub dims: (u32, u32),
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub preview: String,
    /// Contexto local de navegação; não é um campo do objeto Gif.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub src: Option<String>,
    #[serde(default)]
    pub gif_src: Option<String>,
}

impl TenorGif {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        preview_url: impl Into<String>,
        url: impl Into<String>,
        dims: (u32, u32),
        category: Option<String>,
    ) -> Self {
        let preview_url = preview_url.into();
        Self {
            id: id.into(),
            title: title.into(),
            preview_url: preview_url.clone(),
            url: url.into(),
            dims,
            width: dims.0,
            height: dims.1,
            preview: preview_url.clone(),
            category,
            src: Some(preview_url.clone()),
            gif_src: Some(preview_url),
        }
    }

    /// Mídia confirmada para envio. A página do provedor nunca é usada como fallback.
    pub fn send_url(&self) -> Option<&str> {
        self.gif_src
            .as_deref()
            .filter(|url| !url.is_empty() && Self::is_http_url(url))
    }

    fn is_http_url(url: &str) -> bool {
        reqwest::Url::parse(url)
            .map(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
            .unwrap_or(false)
    }

    fn is_image_url(url: &str) -> bool {
        let path = reqwest::Url::parse(url)
            .ok()
            .and_then(|parsed| {
                let path = parsed.path().to_ascii_lowercase();
                (!path.is_empty()).then_some(path)
            })
            .unwrap_or_else(|| {
                url.split(['?', '#'])
                    .next()
                    .unwrap_or(url)
                    .to_ascii_lowercase()
            });
        [".gif", ".png", ".jpg", ".jpeg", ".webp", ".avif"]
            .iter()
            .any(|extension| path.ends_with(extension))
    }
}

/// Categoria de GIFs retornada pelo Discord
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordGifCategory {
    pub name: String,
    pub src: String,
}

// ============================================================================
// Histórico de Emojis Frequentemente Usados (Recent Emojis)
// ============================================================================

/// Registro de uso de emoji persistido em disco
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentEmojiItem {
    pub emoji: String,
    pub name: String,
    pub is_custom: bool,
    pub custom_id: Option<String>,
    pub use_count: u32,
    pub last_used_timestamp: i64,
}

impl RecentEmojiItem {
    /// Reconstrói o emoji para o seletor, inclusive registros antigos que só guardaram a tag.
    pub fn to_discord_emoji(&self) -> Option<DiscordEmoji> {
        if let Some(emoji) = DiscordEmoji::from_tag(&self.emoji) {
            Some(emoji)
        } else if let Some(id) = self.custom_id.as_deref().filter(|id| !id.is_empty()) {
            Some(DiscordEmoji::new_custom(id, self.name.clone(), false, None))
        } else if self.is_custom {
            None
        } else if self.emoji.is_empty() {
            None
        } else {
            Some(DiscordEmoji::new_unicode(self.emoji.clone()))
        }
    }
}

// ============================================================================
// Metadados de Cache de Mídia (Content-Addressed Storage)
// ============================================================================

/// Metadados de um arquivo de mídia em cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCacheMetadata {
    pub url: String,
    pub sha256_hash: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub cached_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub etag: Option<String>,
}

// ============================================================================
// MediaCacheManager
// ============================================================================

/// Gerenciador unificado de Cache de Mídia e Provedor de Conteúdo de Emojis/Stickers/GIFs
#[derive(Clone)]
pub struct MediaCacheManager {
    root_path: PathBuf,
    memory_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    recent_emojis: Arc<RwLock<Vec<RecentEmojiItem>>>,
    favorite_stickers: Arc<RwLock<Vec<DiscordSticker>>>,
    tenor_cache: Arc<RwLock<HashMap<String, (Vec<TenorGif>, DateTime<Utc>)>>>,
    gif_categories: Arc<RwLock<Option<(Vec<DiscordGifCategory>, DateTime<Utc>)>>>,
    decoded_image_cache: Arc<ImageDecodeCache>,
    decode_semaphore: Arc<Semaphore>,
}

impl std::fmt::Debug for MediaCacheManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaCacheManager")
            .field("root_path", &self.root_path)
            .finish_non_exhaustive()
    }
}

impl MediaCacheManager {
    /// Inicializa o gerenciador no diretório padrão: `~/.config/discord-rs/cache/media/`
    pub fn default_path() -> Self {
        let root = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("discord-rs")
            .join("cache")
            .join("media");
        Self::with_root(root)
    }

    /// Inicializa com diretório raiz customizado (ideal para testes isolados)
    pub fn with_root(root_path: impl Into<PathBuf>) -> Self {
        Self::with_root_and_decode_limits(
            root_path,
            DEFAULT_CACHE_MAX_ENTRIES,
            DEFAULT_CACHE_MAX_BYTES,
            2,
        )
    }

    /// Inicializa com limites explícitos para cache de imagens decodificadas e concorrência de decode.
    pub fn with_root_and_decode_limits(
        root_path: impl Into<PathBuf>,
        decoded_cache_max_entries: usize,
        decoded_cache_max_bytes: usize,
        decode_concurrency: usize,
    ) -> Self {
        let root = root_path.into();
        let mgr = Self {
            root_path: root,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            recent_emojis: Arc::new(RwLock::new(Vec::new())),
            favorite_stickers: Arc::new(RwLock::new(Vec::new())),
            tenor_cache: Arc::new(RwLock::new(HashMap::new())),
            gif_categories: Arc::new(RwLock::new(None)),
            decoded_image_cache: Arc::new(ImageDecodeCache::new(
                decoded_cache_max_entries,
                decoded_cache_max_bytes,
            )),
            decode_semaphore: Arc::new(Semaphore::new(decode_concurrency.max(1))),
        };
        // Inicializa carregar recentes do disco
        let loaded = mgr.load_recent_emojis_from_disk();
        if let Ok(mut lock) = mgr.recent_emojis.try_write() {
            *lock = loaded;
        }
        // Inicializa carregar figurinhas favoritas do disco
        let loaded_favs = mgr.load_favorite_stickers_from_disk();
        if let Ok(mut lock) = mgr.favorite_stickers.try_write() {
            *lock = loaded_favs;
        }
        mgr
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Garante que o diretório raiz e as 256 partições SHA-256 existem
    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.root_path).context("Failed to create media cache root dir")?;
        Ok(())
    }

    /// Calcula o hash SHA-256
    pub fn compute_hash(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    /// Resolve o caminho particionado do asset no disco baseado nos 2 primeiros caracteres hexadecimais
    pub fn asset_paths(&self, sha256_hash: &str) -> (PathBuf, PathBuf) {
        let prefix = if sha256_hash.len() >= 2 {
            &sha256_hash[..2]
        } else {
            "00"
        };
        let partition_dir = self.root_path.join(prefix);
        let bin_path = partition_dir.join(format!("{}.bin", sha256_hash));
        let meta_path = partition_dir.join(format!("{}.meta.json", sha256_hash));
        (bin_path, meta_path)
    }

    /// Armazena um binário de mídia no cache CAS
    pub fn store_media(
        &self,
        url: &str,
        bytes: &[u8],
        content_type: &str,
        etag: Option<String>,
    ) -> Result<MediaCacheMetadata> {
        self.ensure_dirs()?;
        let sha256_hash = Self::compute_hash(bytes);
        let (bin_path, meta_path) = self.asset_paths(&sha256_hash);

        if let Some(parent) = bin_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Grava atomicamente se não existir
        if !bin_path.exists() {
            let temp_name = format!(
                ".tmp_{}_{}",
                std::process::id(),
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            );
            let temp_path = bin_path.parent().unwrap().join(temp_name);
            {
                let mut f = File::create(&temp_path)?;
                f.write_all(bytes)?;
                f.sync_data()?;
            }
            fs::rename(&temp_path, &bin_path)?;
        }

        let now = Utc::now();
        let meta = MediaCacheMetadata {
            url: url.to_string(),
            sha256_hash: sha256_hash.clone(),
            content_type: content_type.to_string(),
            size_bytes: bytes.len(),
            cached_at: now,
            last_accessed_at: now,
            etag,
        };

        let meta_bytes = serde_json::to_vec_pretty(&meta)?;
        let temp_meta = format!(
            ".tmp_meta_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_meta_path = meta_path.parent().unwrap().join(temp_meta);
        {
            let mut f = File::create(&temp_meta_path)?;
            f.write_all(&meta_bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_meta_path, &meta_path)?;

        // Guarda em memória (L1 Cache)
        if let Ok(mut mem) = self.memory_cache.try_write() {
            mem.insert(sha256_hash, bytes.to_vec());
        }

        Ok(meta)
    }

    /// Carrega os bytes da mídia do cache (primeiro checa L1 RAM, depois L2 disco)
    pub fn load_media_bytes(&self, sha256_hash: &str) -> Option<Vec<u8>> {
        if let Ok(mem) = self.memory_cache.try_read() {
            if let Some(data) = mem.get(sha256_hash) {
                return Some(data.clone());
            }
        }

        let (bin_path, _) = self.asset_paths(sha256_hash);
        if bin_path.exists() {
            if let Ok(bytes) = fs::read(&bin_path) {
                if let Ok(mut mem) = self.memory_cache.try_write() {
                    mem.insert(sha256_hash.to_string(), bytes.clone());
                }
                return Some(bytes);
            }
        }
        None
    }

    /// Calcula hash SHA-256 da URL canônica da mídia
    pub fn compute_url_hash(url: &str) -> String {
        Self::compute_hash(url.as_bytes())
    }

    /// Carrega os bytes da mídia a partir de sua URL canônica
    pub fn load_media_by_url(&self, url: &str) -> Option<Vec<u8>> {
        let hash = Self::compute_url_hash(url);
        self.load_media_bytes(&hash)
    }

    /// Armazena mídia associada à sua URL canônica no cache CAS particionado em disco
    pub fn store_media_for_url(
        &self,
        url: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<MediaCacheMetadata> {
        let url_hash = Self::compute_url_hash(url);
        self.ensure_dirs()?;
        let (bin_path, meta_path) = self.asset_paths(&url_hash);

        if let Some(parent) = bin_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // A mesma URL pode ser atualizada; sempre substitui o binário atomicamente.
        let temp_name = format!(
            ".tmp_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_path = bin_path.parent().unwrap().join(temp_name);
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_path, &bin_path)?;

        let now = Utc::now();
        let meta = MediaCacheMetadata {
            url: url.to_string(),
            sha256_hash: url_hash.clone(),
            content_type: content_type.to_string(),
            size_bytes: bytes.len(),
            cached_at: now,
            last_accessed_at: now,
            etag: None,
        };

        let meta_bytes = serde_json::to_vec_pretty(&meta)?;
        let temp_meta = format!(
            ".tmp_meta_{}_{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_meta_path = meta_path.parent().unwrap().join(temp_meta);
        {
            let mut f = File::create(&temp_meta_path)?;
            f.write_all(&meta_bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_meta_path, &meta_path)?;

        if let Ok(mut mem) = self.memory_cache.try_write() {
            mem.insert(url_hash, bytes.to_vec());
        }

        Ok(meta)
    }

    /// Baixa ou obtém do cache local uma URL de imagem/mídia (CAS SHA-256 evitando re-downloads)
    pub async fn fetch_or_load_media(&self, url: &str) -> Result<Vec<u8>> {
        if let Some(cached) = self.load_media_by_url(url) {
            return Ok(cached);
        }

        let resp = reqwest::get(url)
            .await
            .context("Failed to fetch media from URL")?
            .error_for_status()
            .context("Media URL returned an error status")?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .context("Failed to read response body")?
            .to_vec();
        if bytes.is_empty() {
            anyhow::bail!("Media URL returned an empty body");
        }

        self.store_media_for_url(url, &bytes, &content_type)?;
        Ok(bytes)
    }

    /// Cache de imagens decodificadas (RGBA) com LRU por entradas/bytes.
    pub fn decoded_image_cache(&self) -> &Arc<ImageDecodeCache> {
        &self.decoded_image_cache
    }

    /// Decodifica bytes de imagem fora da UI thread com concorrência limitada.
    /// O resultado é armazenado no cache decodificado quando `cache_key` é fornecido.
    pub async fn decode_image_bytes(
        &self,
        cache_key: impl Into<String>,
        bytes: Vec<u8>,
        target: Option<(u32, u32)>,
    ) -> Result<Arc<DecodedImage>> {
        let key = cache_key.into();
        if let Some(cached) = self.decoded_image_cache.get(&key).await {
            return Ok(cached);
        }

        let _permit = self
            .decode_semaphore
            .acquire()
            .await
            .context("image decode semaphore closed")?;

        if let Some(cached) = self.decoded_image_cache.get(&key).await {
            return Ok(cached);
        }

        let decode_key = key.clone();
        let max_dimension = MAX_IMAGE_DIMENSION;
        let decoded = tokio::task::spawn_blocking(move || {
            crate::protocol::image_decode::decode_and_resize(&bytes, target, max_dimension)
                .map(|img| (decode_key, img))
        })
        .await
        .context("image decode worker panicked or was cancelled")??;

        let (decoded_key, decoded_img) = decoded;
        let image = Arc::new(decoded_img);
        self.decoded_image_cache
            .put(decoded_key, Arc::clone(&image))
            .await;
        Ok(image)
    }

    /// Busca mídia (cache CAS em disco/RAM) e decodifica fora da UI thread com
    /// concorrência limitada, armazenando o frame RGBA no cache decodificado.
    pub async fn fetch_and_decode_image(
        &self,
        url: &str,
        target: Option<(u32, u32)>,
    ) -> Result<Arc<DecodedImage>> {
        let key = Self::decoded_image_cache_key(url, target);
        if let Some(cached) = self.decoded_image_cache.get(&key).await {
            return Ok(cached);
        }

        let bytes = self.fetch_or_load_media(url).await?;
        self.decode_image_bytes(key, bytes, target).await
    }

    /// Chave canônica do cache decodificado para uma URL e tamanho alvo.
    pub fn decoded_image_cache_key(url: &str, target: Option<(u32, u32)>) -> String {
        let hash = Self::compute_url_hash(url);
        match target {
            Some((w, h)) => format!("img:{hash}:{}x{}", w.max(1), h.max(1)),
            None => format!("img:{hash}:orig"),
        }
    }

    // --- Persistência de Frequentemente Usados (Recent Emojis) ---

    pub fn recent_emojis_path(&self) -> PathBuf {
        self.root_path.join("recent_emojis.json")
    }

    fn load_recent_emojis_from_disk(&self) -> Vec<RecentEmojiItem> {
        let path = self.recent_emojis_path();
        if !path.exists() {
            return Self::default_recent_emojis();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }

    /// Lista os emojis frequentemente usados atualmente
    pub async fn get_recent_emojis(&self) -> Vec<RecentEmojiItem> {
        let lock = self.recent_emojis.read().await;
        lock.clone()
    }

    /// Retorna os emojis frequentemente usados sincronamente
    pub fn get_recent_emojis_sync(&self) -> Vec<RecentEmojiItem> {
        if let Ok(lock) = self.recent_emojis.try_read() {
            lock.clone()
        } else {
            self.load_recent_emojis_from_disk()
        }
    }

    /// Emojis padrão: vazio para obedecer à regra de ZERO MOCKS sem rede
    pub fn default_recent_emojis() -> Vec<RecentEmojiItem> {
        Vec::new()
    }

    /// Registra o uso de um emoji e atualiza o arquivo `recent_emojis.json` atomicamente
    pub async fn record_emoji_use(
        &self,
        emoji: &str,
        name: &str,
        is_custom: bool,
        custom_id: Option<&str>,
    ) {
        let parsed = DiscordEmoji::from_tag(emoji);
        let valid_custom_id = custom_id.filter(|id| !id.is_empty());
        if parsed.is_none() && is_custom && valid_custom_id.is_none() {
            return;
        }
        let mut list = self.recent_emojis.write().await;
        let canonical_emoji = parsed
            .as_ref()
            .map(DiscordEmoji::to_tag)
            .or_else(|| {
                valid_custom_id.map(|id| DiscordEmoji::new_custom(id, name, false, None).to_tag())
            })
            .unwrap_or_else(|| emoji.to_string());
        let canonical_name = parsed
            .as_ref()
            .map(|emoji| emoji.name.clone())
            .unwrap_or_else(|| name.to_string());
        let canonical_custom = parsed.is_some() || valid_custom_id.is_some();
        let canonical_id = parsed
            .as_ref()
            .and_then(|emoji| emoji.id.clone())
            .or_else(|| valid_custom_id.map(ToString::to_string));

        let now = Utc::now().timestamp();
        if let Some(item) = list.iter_mut().find(|i| i.emoji == canonical_emoji) {
            item.use_count += 1;
            item.last_used_timestamp = now;
            item.name = canonical_name;
            item.is_custom = canonical_custom;
            item.custom_id = canonical_id;
        } else {
            list.push(RecentEmojiItem {
                emoji: canonical_emoji,
                name: canonical_name,
                is_custom: canonical_custom,
                custom_id: canonical_id,
                use_count: 1,
                last_used_timestamp: now,
            });
        }

        // Ordena por frequência e, em empate, pelo uso mais recente.
        list.sort_by(|a, b| {
            b.use_count
                .cmp(&a.use_count)
                .then_with(|| b.last_used_timestamp.cmp(&a.last_used_timestamp))
        });
        if list.len() > 32 {
            list.truncate(32);
        }

        let _ = self.save_recent_emojis_to_disk(&list);
    }

    fn save_recent_emojis_to_disk(&self, items: &[RecentEmojiItem]) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.recent_emojis_path();
        let json_bytes = serde_json::to_vec_pretty(items)?;

        let temp_name = format!(
            ".tmp_recents_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_path = self.root_path.join(temp_name);
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(&json_bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    // --- Persistência de Figurinhas Favoritas (Favorite Stickers) ---

    pub fn favorite_stickers_path(&self) -> PathBuf {
        self.root_path.join("favorite_stickers.json")
    }

    fn load_favorite_stickers_from_disk(&self) -> Vec<DiscordSticker> {
        let path = self.favorite_stickers_path();
        if !path.exists() {
            return Vec::new();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    }

    fn save_favorite_stickers_to_disk(&self, items: &[DiscordSticker]) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.favorite_stickers_path();
        let json_bytes = serde_json::to_vec_pretty(items)?;

        let temp_name = format!(
            ".tmp_fav_stickers_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_path = self.root_path.join(temp_name);
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(&json_bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    /// Retorna a lista de figurinhas favoritas persistidas
    pub async fn get_favorite_stickers(&self) -> Vec<DiscordSticker> {
        let lock = self.favorite_stickers.read().await;
        lock.clone()
    }

    /// Retorna as figurinhas favoritas sincronamente (para rápido acesso no render)
    pub fn get_favorite_stickers_sync(&self) -> Vec<DiscordSticker> {
        if let Ok(lock) = self.favorite_stickers.try_read() {
            lock.clone()
        } else {
            self.load_favorite_stickers_from_disk()
        }
    }

    /// Adiciona uma figurinha aos favoritos e persiste em disco
    pub async fn add_favorite_sticker(&self, sticker: DiscordSticker) -> Result<()> {
        let mut list = self.favorite_stickers.write().await;
        if !list.iter().any(|s| s.id == sticker.id) {
            list.push(sticker);
            self.save_favorite_stickers_to_disk(&list)?;
        }
        Ok(())
    }

    /// Remove uma figurinha dos favoritos pelo ID e persiste em disco
    pub async fn remove_favorite_sticker(&self, sticker_id: &str) -> Result<()> {
        let mut list = self.favorite_stickers.write().await;
        let initial_len = list.len();
        list.retain(|s| s.id != sticker_id);
        if list.len() != initial_len {
            self.save_favorite_stickers_to_disk(&list)?;
        }
        Ok(())
    }

    // --- Cache de pacotes oficiais de figurinhas ---

    pub fn sticker_packs_path(&self) -> PathBuf {
        self.root_path.join("sticker_packs.json")
    }

    pub fn load_sticker_packs_from_disk(&self) -> Vec<StickerPack> {
        let path = self.sticker_packs_path();
        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save_sticker_packs_to_disk(&self, packs: &[StickerPack]) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.sticker_packs_path();
        let bytes = serde_json::to_vec_pretty(packs)?;
        let temp_path = self.root_path.join(format!(
            ".tmp_sticker_packs_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        {
            let mut file = File::create(&temp_path)?;
            file.write_all(&bytes)?;
            file.sync_data()?;
        }
        fs::rename(temp_path, path)?;
        Ok(())
    }

    pub fn get_sticker_packs_cached(&self) -> Vec<StickerPack> {
        self.load_sticker_packs_from_disk()
    }

    // --- Persistência de Emojis e Figurinhas de Guildas (Guild Media Cache) ---

    pub fn guild_emojis_path(&self, guild_id: &str) -> PathBuf {
        self.root_path
            .join("guilds")
            .join(format!("{}_emojis.json", guild_id))
    }

    pub fn guild_stickers_path(&self, guild_id: &str) -> PathBuf {
        self.root_path
            .join("guilds")
            .join(format!("{}_stickers.json", guild_id))
    }

    pub fn load_guild_emojis_from_disk(&self, guild_id: &str) -> Option<Vec<DiscordEmoji>> {
        let path = self.guild_emojis_path(guild_id);
        if !path.exists() {
            return None;
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
    }

    pub fn save_guild_emojis_to_disk(&self, guild_id: &str, items: &[DiscordEmoji]) -> Result<()> {
        let dir = self.root_path.join("guilds");
        let _ = fs::create_dir_all(&dir);
        let path = self.guild_emojis_path(guild_id);
        let json_bytes = serde_json::to_vec_pretty(items)?;
        let temp_name = format!(
            ".tmp_emojis_{}_{}",
            guild_id,
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_path = dir.join(temp_name);
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(&json_bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    pub fn load_guild_stickers_from_disk(&self, guild_id: &str) -> Option<Vec<DiscordSticker>> {
        let path = self.guild_stickers_path(guild_id);
        if !path.exists() {
            return None;
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
    }

    pub fn save_guild_stickers_to_disk(
        &self,
        guild_id: &str,
        items: &[DiscordSticker],
    ) -> Result<()> {
        let dir = self.root_path.join("guilds");
        let _ = fs::create_dir_all(&dir);
        let path = self.guild_stickers_path(guild_id);
        let json_bytes = serde_json::to_vec_pretty(items)?;
        let temp_name = format!(
            ".tmp_stickers_{}_{}",
            guild_id,
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let temp_path = dir.join(temp_name);
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(&json_bytes)?;
            f.sync_data()?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    }

    /// Agrega figurinhas favoritas, figurinhas de servidores (guild stickers) e pacotes padrão oficiais do Discord
    pub async fn get_all_stickers(&self, guild_stickers: &[DiscordSticker]) -> Vec<DiscordSticker> {
        let favorites = self.get_favorite_stickers().await;
        let packs = self.get_sticker_packs_cached();
        Self::aggregate_stickers(&favorites, guild_stickers, &packs)
    }

    /// Função utilitária estática para agregar figurinhas a partir de coleções existentes
    pub fn aggregate_stickers(
        favorite_stickers: &[DiscordSticker],
        guild_stickers: &[DiscordSticker],
        packs: &[StickerPack],
    ) -> Vec<DiscordSticker> {
        let mut result = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // Fontes frescas têm prioridade sobre o snapshot de favoritos. Um item
        // frescamente indisponível também precisa bloquear o favorito antigo.
        for s in guild_stickers
            .iter()
            .chain(packs.iter().flat_map(|pack| pack.stickers.iter()))
        {
            if seen_ids.insert(s.id.clone()) && s.available {
                result.push(s.clone());
            }
        }
        for s in favorite_stickers {
            if seen_ids.insert(s.id.clone()) && s.available {
                result.push(s.clone());
            }
        }

        result
    }

    // --- Parsers de mídia real (Discord API / Tenor / Klipy / Giphy) ---

    /// Converte um objeto JSON individual do Discord ou Tenor v2.
    /// A página do provedor nunca é confundida com mídia e entradas inválidas são descartadas.
    pub fn parse_gif_from_json(
        val: &serde_json::Value,
        default_category: Option<&str>,
    ) -> Option<TenorGif> {
        let string = |key: &str| {
            val.get(key)
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
        };
        let http_string = |key: &str| string(key).filter(|url| TenorGif::is_http_url(url));
        let id = string("id")?.to_string();
        let title = string("title")
            .or_else(|| string("content_description"))
            .or_else(|| string("description"))
            .unwrap_or_default()
            .to_string();
        let media_formats = val
            .get("media_formats")
            .filter(|formats| formats.is_object());
        let media_url = |keys: &[&str]| {
            keys.iter().find_map(|key| {
                media_formats
                    .and_then(|formats| formats.get(*key))
                    .and_then(|format| format.get("url"))
                    .and_then(|url| url.as_str())
                    .filter(|url| TenorGif::is_http_url(url))
            })
        };

        let (url, src, gif_src, preview) = if media_formats.is_some() {
            let media = media_url(&["gif"])?;
            (
                media.to_string(),
                http_string("src")
                    .or_else(|| media_url(&["mp4", "webm"]))
                    .unwrap_or(media)
                    .to_string(),
                http_string("gif_src").unwrap_or(media).to_string(),
                http_string("preview")
                    .or_else(|| media_url(&["preview"]))
                    .or_else(|| media_url(&["tinygif", "nanogif"]))
                    .unwrap_or(media)
                    .to_string(),
            )
        } else {
            (
                http_string("url")?.to_string(),
                http_string("src")?.to_string(),
                http_string("gif_src")?.to_string(),
                http_string("preview").unwrap_or_default().to_string(),
            )
        };

        let top_width = val
            .get("width")
            .and_then(|value| value.as_u64())
            .filter(|value| *value > 0);
        let top_height = val
            .get("height")
            .and_then(|value| value.as_u64())
            .filter(|value| *value > 0);
        let valid_dims = |dims: &serde_json::Value| {
            dims.as_array()
                .filter(|dims| {
                    dims.len() >= 2
                        && dims[0].as_u64().is_some_and(|value| value > 0)
                        && dims[1].as_u64().is_some_and(|value| value > 0)
                })
                .map(|dims| {
                    (
                        dims[0].as_u64().unwrap_or_default() as u32,
                        dims[1].as_u64().unwrap_or_default() as u32,
                    )
                })
        };
        let legacy_dims = val.get("dims").and_then(valid_dims).or_else(|| {
            media_formats
                .and_then(|formats| formats.get("gif"))
                .and_then(|format| format.get("dims"))
                .and_then(valid_dims)
        });
        let dims = match (top_width, top_height) {
            (Some(width), Some(height)) => (width as u32, height as u32),
            _ => legacy_dims.unwrap_or_else(default_gif_dims),
        };
        let width = top_width.map(|value| value as u32).unwrap_or(dims.0);
        let height = top_height.map(|value| value as u32).unwrap_or(dims.1);

        let preview_url = if TenorGif::is_image_url(&gif_src) {
            gif_src.clone()
        } else if media_formats.is_some() {
            media_url(&["tinygif", "nanogif"])
                .filter(|url| TenorGif::is_image_url(url))
                .unwrap_or_default()
                .to_string()
        } else if TenorGif::is_image_url(&preview) {
            preview.clone()
        } else {
            String::new()
        };
        let category = string("category").map(ToString::to_string).or_else(|| {
            default_category
                .filter(|category| !category.is_empty())
                .map(ToString::to_string)
        });

        Some(TenorGif {
            id,
            title,
            preview_url,
            url,
            dims,
            width,
            height,
            preview,
            category,
            src: Some(src),
            gif_src: Some(gif_src),
        })
    }

    /// Processa respostas de busca/trending sem duplicar IDs.
    pub fn parse_gifs_response(
        val: &serde_json::Value,
        default_category: Option<&str>,
    ) -> Vec<TenorGif> {
        let items = val
            .as_array()
            .or_else(|| val.get("gifs").and_then(|value| value.as_array()))
            .or_else(|| val.get("results").and_then(|value| value.as_array()))
            .or_else(|| val.get("data").and_then(|value| value.as_array()));
        let mut gifs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(items) = items {
            for item in items {
                if let Some(gif) = Self::parse_gif_from_json(item, default_category) {
                    if seen.insert(gif.id.clone()) {
                        gifs.push(gif);
                    }
                }
            }
        }
        gifs
    }

    /// Parser estrito de `GET /gifs/trending`: categories e gifs são obrigatórios.
    pub fn parse_trending_gifs_response(
        val: &serde_json::Value,
    ) -> Result<(Vec<DiscordGifCategory>, Vec<TenorGif>)> {
        let raw_categories = val
            .get("categories")
            .and_then(|value| value.as_array())
            .ok_or_else(|| anyhow::anyhow!("trending response is missing categories array"))?;
        let raw_gifs = val
            .get("gifs")
            .and_then(|value| value.as_array())
            .ok_or_else(|| anyhow::anyhow!("trending response is missing gifs array"))?;
        let mut categories = Vec::new();
        let mut seen_categories = std::collections::HashSet::new();
        for item in raw_categories {
            let category = Self::parse_gif_category_from_json(item)
                .ok_or_else(|| anyhow::anyhow!("invalid GIF category in trending response"))?;
            if seen_categories.insert(category.name.clone()) {
                categories.push(category);
            }
        }
        let mut gifs = Vec::new();
        let mut seen_gifs = std::collections::HashSet::new();
        for item in raw_gifs {
            if item.get("media_formats").is_none() && !Self::has_complete_discord_gif_contract(item)
            {
                anyhow::bail!("incomplete Discord GIF in trending response");
            }
            let gif = Self::parse_gif_from_json(item, None)
                .ok_or_else(|| anyhow::anyhow!("invalid GIF in trending response"))?;
            if seen_gifs.insert(gif.id.clone()) {
                gifs.push(gif);
            }
        }
        Ok((categories, gifs))
    }

    /// Parser estrito de `GET /gifs/search`: somente array na raiz é aceito.
    pub fn parse_search_gifs_response(val: &serde_json::Value) -> Result<Vec<TenorGif>> {
        let items = val
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("GIF search response must be a root array"))?;
        let mut gifs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for item in items {
            if item.get("media_formats").is_none() && !Self::has_complete_discord_gif_contract(item)
            {
                anyhow::bail!("incomplete Discord GIF in search response");
            }
            let gif = Self::parse_gif_from_json(item, None)
                .ok_or_else(|| anyhow::anyhow!("invalid GIF in search response"))?;
            if seen.insert(gif.id.clone()) {
                gifs.push(gif);
            }
        }
        Ok(gifs)
    }

    fn has_complete_discord_gif_contract(val: &serde_json::Value) -> bool {
        ["url", "src", "gif_src", "preview"].iter().all(|key| {
            val.get(*key)
                .and_then(|value| value.as_str())
                .is_some_and(|url| TenorGif::is_http_url(url))
        }) && val
            .get("width")
            .and_then(|value| value.as_u64())
            .is_some_and(|value| value > 0)
            && val
                .get("height")
                .and_then(|value| value.as_u64())
                .is_some_and(|value| value > 0)
    }

    fn parse_gif_category_from_json(val: &serde_json::Value) -> Option<DiscordGifCategory> {
        let name = val
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|name| !name.is_empty())?;
        let src = val
            .get("src")
            .and_then(|value| value.as_str())
            .filter(|src| TenorGif::is_http_url(src))?;
        Some(DiscordGifCategory {
            name: name.to_string(),
            src: src.to_string(),
        })
    }

    /// Processa categorias retornadas junto ao trending ou por um array dedicado.
    pub fn parse_gif_categories_response(val: &serde_json::Value) -> Vec<DiscordGifCategory> {
        let items = val
            .as_array()
            .or_else(|| val.get("categories").and_then(|value| value.as_array()));
        let mut categories = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(items) = items {
            for item in items {
                if let Some(category) = Self::parse_gif_category_from_json(item) {
                    if seen.insert(category.name.clone()) {
                        categories.push(category);
                    }
                }
            }
        }
        categories
    }

    /// Converte um sticker da API. Campos ausentes continuam vazios, sem nomes ou IDs fictícios.
    pub fn parse_sticker_from_json(
        val: &serde_json::Value,
        guild_id: Option<&str>,
    ) -> Option<DiscordSticker> {
        let id = val
            .get("id")
            .and_then(|value| value.as_str())
            .filter(|id| !id.is_empty())?;
        let name = val
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|name| !name.is_empty())?;
        let tags = val.get("tags").and_then(|value| value.as_str())?;
        if !matches!(
            val.get("type").and_then(|value| value.as_u64()),
            Some(1 | 2)
        ) {
            return None;
        }
        let format_type = serde_json::from_value(val.get("format_type")?.clone()).ok()?;
        Some(DiscordSticker {
            id: id.to_string(),
            pack_id: val
                .get("pack_id")
                .and_then(|value| value.as_str())
                .filter(|id| !id.is_empty())
                .map(ToString::to_string),
            name: name.to_string(),
            description: val
                .get("description")
                .and_then(|value| value.as_str())
                .map(ToString::to_string),
            tags: tags.to_string(),
            format_type,
            available: val
                .get("available")
                .and_then(|value| value.as_bool())
                .unwrap_or(true),
            guild_id: val
                .get("guild_id")
                .and_then(|value| value.as_str())
                .filter(|id| !id.is_empty())
                .map(ToString::to_string)
                .or_else(|| guild_id.map(ToString::to_string)),
        })
    }

    /// Converte um pacote de stickers da API.
    pub fn parse_sticker_pack_from_json(val: &serde_json::Value) -> Option<StickerPack> {
        let id = val
            .get("id")
            .and_then(|value| value.as_str())
            .filter(|id| !id.is_empty())?;
        let name = val
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|name| !name.is_empty())?;
        let sku_id = val
            .get("sku_id")
            .and_then(|value| value.as_str())
            .filter(|sku| !sku.is_empty())?;
        let description = val.get("description").and_then(|value| value.as_str())?;
        let stickers = val
            .get("stickers")
            .and_then(|value| value.as_array())?
            .iter()
            .map(|sticker| Self::parse_sticker_from_json(sticker, None))
            .collect::<Option<Vec<_>>>()?;
        Some(StickerPack {
            id: id.to_string(),
            name: name.to_string(),
            sku_id: sku_id.to_string(),
            description: description.to_string(),
            stickers,
            banner_asset_id: val
                .get("banner_asset_id")
                .and_then(|value| value.as_str())
                .filter(|id| !id.is_empty())
                .map(ToString::to_string),
        })
    }

    /// Processa `GET /sticker-packs`.
    pub fn parse_sticker_packs_response(val: &serde_json::Value) -> Vec<StickerPack> {
        let items = val
            .as_array()
            .or_else(|| val.get("sticker_packs").and_then(|value| value.as_array()));
        let mut packs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(items) = items {
            for item in items {
                if let Some(pack) = Self::parse_sticker_pack_from_json(item) {
                    if seen.insert(pack.id.clone()) {
                        packs.push(pack);
                    }
                }
            }
        }
        packs
    }

    // --- Cache de respostas reais de GIF ---

    /// Chave canônica: locale e media_format fazem parte da identidade da resposta.
    pub fn gif_cache_key(locale: &str, media_format: &str, query: Option<&str>) -> String {
        let locale = locale.trim().to_lowercase();
        let media_format = media_format.trim().to_lowercase();
        match query.map(str::trim).filter(|query| !query.is_empty()) {
            Some(query) => format!(
                "gifs|{}|{}|search|{}",
                locale,
                media_format,
                query.to_lowercase()
            ),
            None => format!("gifs|{}|{}|trending", locale, media_format),
        }
    }

    /// Registra uma resposta real da API, inclusive um hit vazio.
    pub async fn cache_gifs(
        &self,
        locale: &str,
        media_format: &str,
        query: Option<&str>,
        gifs: &[TenorGif],
    ) {
        let key = Self::gif_cache_key(locale, media_format, query);
        self.tenor_cache
            .write()
            .await
            .insert(key, (gifs.to_vec(), Utc::now()));
    }

    pub async fn get_cached_gifs(
        &self,
        locale: &str,
        media_format: &str,
        query: Option<&str>,
    ) -> Vec<TenorGif> {
        let key = Self::gif_cache_key(locale, media_format, query);
        let cache = self.tenor_cache.read().await;
        cache
            .get(&key)
            .filter(|(_, cached_at)| Utc::now() - *cached_at < chrono::Duration::minutes(15))
            .map(|(gifs, _)| gifs.clone())
            .unwrap_or_default()
    }

    /// Distingue um hit vazio de um miss para a UI não refazer a requisição.
    pub async fn has_cached_gifs(
        &self,
        locale: &str,
        media_format: &str,
        query: Option<&str>,
    ) -> bool {
        let key = Self::gif_cache_key(locale, media_format, query);
        self.tenor_cache
            .read()
            .await
            .get(&key)
            .is_some_and(|(_, cached_at)| Utc::now() - *cached_at < chrono::Duration::minutes(15))
    }

    pub fn get_cached_gifs_sync(
        &self,
        locale: &str,
        media_format: &str,
        query: Option<&str>,
    ) -> Vec<TenorGif> {
        let key = Self::gif_cache_key(locale, media_format, query);
        self.tenor_cache
            .try_read()
            .ok()
            .and_then(|cache| {
                cache.get(&key).and_then(|(gifs, cached_at)| {
                    (Utc::now() - *cached_at < chrono::Duration::minutes(15)).then(|| gifs.clone())
                })
            })
            .unwrap_or_default()
    }

    pub async fn cache_gif_categories(&self, categories: &[DiscordGifCategory]) {
        *self.gif_categories.write().await = Some((categories.to_vec(), Utc::now()));
    }

    pub fn get_gif_categories_sync(&self) -> Vec<DiscordGifCategory> {
        self.gif_categories
            .try_read()
            .ok()
            .and_then(|cache| {
                cache.as_ref().and_then(|(categories, cached_at)| {
                    (Utc::now() - *cached_at < chrono::Duration::minutes(15))
                        .then(|| categories.clone())
                })
            })
            .unwrap_or_default()
    }

    /// Compatibilidade: retorna somente resultados reais previamente registrados no cache.
    pub async fn search_tenor_gifs(&self, query: &str, limit: usize) -> Vec<TenorGif> {
        self.get_cached_gifs("pt-BR", "mp4", Some(query))
            .await
            .into_iter()
            .take(limit)
            .collect()
    }

    /// Compatibilidade: retorna somente o trending real previamente registrado no cache.
    pub async fn trending_tenor_gifs(&self, limit: usize) -> Vec<TenorGif> {
        self.get_cached_gifs("pt-BR", "mp4", None)
            .await
            .into_iter()
            .take(limit)
            .collect()
    }

    /// Mantido para compatibilidade; não existe biblioteca curada em produção.
    pub fn get_curated_tenor_gifs() -> Vec<TenorGif> {
        Vec::new()
    }

    /// Mantido para compatibilidade; não existe biblioteca curada em produção.
    pub fn get_curated_tenor_gifs_for_category(_category: &str) -> Vec<TenorGif> {
        Vec::new()
    }

    /// Mantido para compatibilidade; não existe biblioteca curada em produção.
    pub fn curated_tenor_gifs() -> Vec<TenorGif> {
        Vec::new()
    }

    // --- Soundboard Defaults Provider ---

    /// Lista os sons padrão do Discord
    pub fn get_default_soundboard_sounds() -> Vec<SoundboardSound> {
        Vec::new()
    }

    // --- Sticker Packs Defaults Provider ---

    /// Lista os pacotes padrão de figurinhas oficiais do Discord
    pub fn get_default_sticker_packs() -> Vec<StickerPack> {
        Vec::new()
    }

    // --- Categorias Canônicas de Emojis Unicode do Discord ---

    /// Retorna as 8 categorias canônicas do Discord com seus emojis organizados
    pub fn get_standard_categories() -> Vec<(&'static str, &'static str, &'static [&'static str])> {
        vec![
            (
                "people",
                "PESSOAS & EXPRESSÕES",
                &[
                    "😀",
                    "😃",
                    "😄",
                    "😁",
                    "😆",
                    "😅",
                    "😂",
                    "🤣",
                    "🥲",
                    "🥹",
                    "😊",
                    "😇",
                    "🙂",
                    "🙃",
                    "😉",
                    "😌",
                    "😍",
                    "🥰",
                    "😘",
                    "😗",
                    "😙",
                    "😚",
                    "😋",
                    "😛",
                    "😝",
                    "😜",
                    "🤪",
                    "🤨",
                    "🧐",
                    "🤓",
                    "😎",
                    "🥸",
                    "🤩",
                    "🥳",
                    "😏",
                    "😒",
                    "😞",
                    "😔",
                    "😟",
                    "😕",
                    "🙁",
                    "☹️",
                    "😣",
                    "😖",
                    "😫",
                    "😩",
                    "🥺",
                    "😢",
                    "😭",
                    "😮‍💨",
                    "😤",
                    "😠",
                    "😡",
                    "🤬",
                    "🤯",
                    "😳",
                    "🥵",
                    "🥶",
                    "😱",
                    "😨",
                    "😰",
                    "😥",
                    "😓",
                    "🤗",
                    "🤔",
                    "🫣",
                    "🤭",
                    "🫢",
                    "🫡",
                    "🤫",
                    "🫠",
                    "🤥",
                    "😶",
                    "🫥",
                    "😐",
                    "🫤",
                    "😑",
                    "🫨",
                    "😬",
                    "🙄",
                    "😯",
                    "😦",
                    "😧",
                    "😮",
                    "😲",
                    "🥱",
                    "😴",
                    "🤤",
                    "😪",
                    "😵",
                    "🤐",
                    "🥴",
                    "🤢",
                    "🤮",
                    "🤧",
                    "😷",
                    "🤒",
                    "🤕",
                    "🤑",
                    "🤠",
                    "😈",
                    "👿",
                    "👹",
                    "👺",
                    "🤡",
                    "💩",
                    "👻",
                    "💀",
                    "☠️",
                    "👽",
                    "👾",
                    "🤖",
                    "🎃",
                    "😺",
                    "😸",
                    "😹",
                    "😻",
                    "😼",
                    "😽",
                    "🙀",
                    "👋",
                    "🤚",
                    "🖐️",
                    "✋",
                    "🖖",
                    "👌",
                    "🤌",
                    "🤏",
                    "✌️",
                    "🤞",
                    "🤟",
                    "🤘",
                    "🤙",
                    "👈",
                    "👉",
                    "👆",
                    "👇",
                    "👍",
                    "👎",
                    "✊",
                    "👊",
                    "👏",
                    "🙌",
                    "🫶",
                    "👐",
                    "🤲",
                    "🤝",
                    "🙏",
                    "✍️",
                    "💪",
                ],
            ),
            (
                "nature",
                "ANIMAIS & NATUREZA",
                &[
                    "🐶",
                    "🐱",
                    "🐭",
                    "🐹",
                    "🐰",
                    "🦊",
                    "🐻",
                    "🐼",
                    "🐻‍❄️",
                    "🐨",
                    "🐯",
                    "🦁",
                    "🐮",
                    "🐷",
                    "🐽",
                    "🐸",
                    "🐵",
                    "🙈",
                    "🙉",
                    "🙊",
                    "🐒",
                    "🐔",
                    "🐧",
                    "🐦",
                    "🐤",
                    "🐣",
                    "🐥",
                    "🦆",
                    "🦅",
                    "🦉",
                    "🦇",
                    "🐺",
                    "🐗",
                    "🐴",
                    "🦄",
                    "🐝",
                    "🐛",
                    "🦋",
                    "🐌",
                    "🐞",
                    "🐜",
                    "🪰",
                    "🪲",
                    "🪳",
                    "🦟",
                    "🦗",
                    "🕷️",
                    "🕸️",
                    "🦂",
                    "🐢",
                    "🐍",
                    "🦎",
                    "🦖",
                    "🦕",
                    "🐙",
                    "🦑",
                    "🦐",
                    "🦞",
                    "🦀",
                    "🐡",
                    "🐠",
                    "🐟",
                    "🐬",
                    "🐳",
                    "🐋",
                    "🦈",
                    "🦭",
                    "🐊",
                    "🐅",
                    "🐆",
                    "🌵",
                    "🎄",
                    "🌲",
                    "🌳",
                    "🌴",
                    "🌱",
                    "🌿",
                    "☘️",
                    "🍀",
                    "🎍",
                    "🪴",
                    "🎋",
                    "🍃",
                    "🍂",
                    "🍁",
                    "🍄",
                    "🐚",
                    "🌾",
                    "💐",
                    "🌷",
                    "🌹",
                    "🥀",
                    "🌺",
                    "🌸",
                    "🌼",
                    "🌻",
                    "🌞",
                    "🌝",
                    "⭐",
                    "🌟",
                    "✨",
                    "⚡",
                    "☄️",
                    "💥",
                    "🔥",
                    "🌪️",
                    "🌈",
                    "☀️",
                    "🌤️",
                    "⛅",
                    "☁️",
                    "🌧️",
                    "⛈️",
                    "🌩️",
                    "🌨️",
                    "❄️",
                    "⛄",
                    "💨",
                    "💧",
                    "🌊",
                ],
            ),
            (
                "food",
                "COMIDA & BEBIDA",
                &[
                    "🍏", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🫐", "🍈", "🍒", "🍑",
                    "🥭", "🍍", "🥥", "🥝", "🍅", "🥑", "🥦", "🥬", "🥒", "🌶️", "🫑", "🌽", "🥕",
                    "🧄", "🧅", "🥔", "🍠", "🥐", "🥯", "🍞", "🥖", "🥨", "🧀", "🥚", "🍳", "🧈",
                    "🥞", "🧇", "🥓", "🥩", "🍗", "🍖", "🌭", "🍔", "🍟", "🍕", "🥪", "🥙", "🌮",
                    "🌯", "🥗", "🥘", "🍝", "🍜", "🍲", "🍛", "🍣", "🍱", "🥟", "🍤", "🍙", "🍚",
                    "🍘", "🍦", "🍧", "🍨", "🍩", "🍪", "🎂", "🍰", "🧁", "🥧", "🍫", "🍬", "🍭",
                    "🍮", "🍯", "🍼", "🥛", "☕", "🫖", "🍵", "🧃", "🥤", "🧋", "🍶", "🍺", "🍻",
                    "🥂", "🍷", "🥃", "🍸", "🍹", "🧉", "🍾", "🧊", "🍽️",
                ],
            ),
            (
                "activity",
                "ATIVIDADES & ESPORTES",
                &[
                    "⚽", "🏀", "🏈", "⚾", "🥎", "🎾", "🏐", "🏉", "🥏", "🎱", "🏓", "🏸", "🏒",
                    "🏑", "🏏", "⛳", "🏹", "🎣", "🤿", "🥊", "🥋", "🎽", "🛹", "🛼", "🎿", "🏂",
                    "🏋️", "🤺", "🏇", "🏄", "🏊", "🚣", "🧗", "🚵", "🚴", "🏆", "🥇", "🥈", "🥉",
                    "🏅", "🎖️", "🎫", "🎟️", "🎪", "🎭", "🎨", "🎬", "🎤", "🎧", "🎼", "🎹", "🥁",
                    "🎷", "🎺", "🎸", "🪕", "🎻", "🎲", "♟️", "🎯", "🎳", "🎮", "🎰", "🧩",
                ],
            ),
            (
                "travel",
                "VIAGENS & LUGARES",
                &[
                    "🚗", "🚙", "🚕", "🚌", "🏎️", "🚓", "🚑", "🚒", "🚐", "🚚", "🚛", "🚜", "🏍️",
                    "🛵", "🚲", "🛴", "🚨", "🚔", "🚡", "🚠", "🚃", "🚋", "🚄", "🚅", "🚈", "🚂",
                    "🚆", "🚇", "🚊", "🚉", "🚁", "🛩️", "✈️", "🛫", "🛬", "🛰️", "🚀", "🛸", "⛵",
                    "🚤", "🛥️", "🛳️", "⛴️", "🚢", "⚓", "⛽", "🚧", "🚦", "🚥", "🗺️", "🗿", "🗽",
                    "🗼", "🏰", "🏯", "🏟️", "🎡", "🎢", "🎠", "🏖️", "🏝️", "🏜️", "🌋", "⛰️", "🏔️",
                    "🏕️", "⛺", "🏠", "🏡", "🏢",
                ],
            ),
            (
                "objects",
                "OBJETOS",
                &[
                    "⌚", "📱", "📲", "💻", "⌨️", "🖥️", "🖨️", "🖱️", "🕹️", "💾", "💿", "📀", "📷",
                    "📸", "📹", "🎥", "📽️", "🎞️", "📞", "☎️", "📟", "📠", "📺", "📻", "🎙️", "⏱️",
                    "⏲️", "⏰", "🕰️", "⌛", "⏳", "📡", "🔋", "🔌", "💡", "🔦", "🕯️", "🧯", "🗑️",
                    "💸", "💵", "🪙", "💰", "💳", "💎", "⚖️", "🧰", "🔧", "🔨", "⚒️", "🛠️", "⛏️",
                    "🔩", "⚙️", "🧱", "⛓️", "🧲", "🔫", "💣", "🧨", "🔪", "🗡️", "⚔️", "🛡️", "⚰️",
                    "🔮", "💈", "🔭", "🔬", "💊", "💉", "🧬", "🧹", "🧺", "🧻", "🚽", "🚿", "🛁",
                    "🧼", "🔑", "🗝️", "🚪", "🪑", "🛋️", "🛍️", "🛒", "🎁", "🎈", "🎉", "🎊", "✉️",
                    "📩", "📦", "🏷️", "📁", "📂", "📅", "📆", "📊", "📋", "📌", "📍", "📎", "📏",
                    "📐", "✂️", "🔒", "🔓", "🔏", "🔐",
                ],
            ),
            (
                "symbols",
                "SÍMBOLOS",
                &[
                    "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔", "❣️", "💕", "💞",
                    "💓", "💗", "💖", "💘", "💝", "☮️", "✝️", "☪️", "🕉️", "☸️", "✡️", "🔯", "🕎",
                    "☯️", "☦️", "🛐", "⛎", "♈", "♉", "♊", "♋", "♌", "♍", "♎", "♏", "♐",
                    "♑", "♒", "♓", "🆔", "⚛️", "☢️", "☣️", "📴", "📳", "✴️", "🆚", "🅰️", "🅱️",
                    "🆎", "🅾️", "🆘", "❌", "⭕", "🛑", "⛔", "📛", "🚫", "💯", "💢", "♨️", "❗",
                    "❕", "❓", "❔", "‼️", "⁉️", "⚠️", "🔱", "⚜️", "🔰", "♻️", "✅", "❇️", "✳️",
                    "❎", "🌐", "🌀", "💤", "🏧", "♿", "🅿️", "🚹", "🚺", "🚼", "🚻", "🎦", "📶",
                    "🆗", "🆙", "🆒", "🆕", "🆓", "0️⃣", "1️⃣", "2️⃣", "3️⃣", "4️⃣", "5️⃣", "6️⃣", "7️⃣",
                    "8️⃣", "9️⃣", "🔟", "▶️", "⏸️", "⏹️", "⏩", "⏪", "⏫", "⏬", "➡️", "⬅️", "⬆️",
                    "⬇️", "🎵", "🎶", "➕", "➖", "➗", "✖️", "💲", "💱", "™️", "©️", "®️", "✔️",
                    "🔴", "🟠", "🟡", "🟢", "🔵", "🟣", "⚫", "⚪", "🟤", "🔺", "🔻", "🔸", "🔹",
                    "🔶", "🔷", "▪️", "▫️", "🔔", "🔕", "📢",
                ],
            ),
            (
                "flags",
                "BANDEIRAS",
                &[
                    "🏁",
                    "🚩",
                    "🎌",
                    "🏴",
                    "🏳️",
                    "🏳️‍🌈",
                    "🏳️‍⚧️",
                    "🏴‍☠️",
                    "🇧🇷",
                    "🇺🇸",
                    "🇵🇹",
                    "🇪🇸",
                    "🇫🇷",
                    "🇩🇪",
                    "🇮🇹",
                    "🇬🇧",
                    "🇯🇵",
                    "🇰🇷",
                    "🇨🇳",
                    "🇨🇦",
                    "🇦🇺",
                    "🇷🇺",
                    "🇮🇳",
                    "🇲🇽",
                    "🇦🇷",
                    "🇨🇱",
                    "🇨🇴",
                    "🇺🇾",
                    "🇿🇦",
                    "🇪🇺",
                    "🇺🇳",
                ],
            ),
        ]
    }

    /// Nome oficial Unicode (UTS #51, Emoji 18.0) usado pela busca do seletor.
    pub fn unicode_emoji_name(emoji: &str) -> Option<&'static str> {
        match emoji {
            "0️⃣" => Some("keycap: 0"),
            "1️⃣" => Some("keycap: 1"),
            "2️⃣" => Some("keycap: 2"),
            "3️⃣" => Some("keycap: 3"),
            "4️⃣" => Some("keycap: 4"),
            "5️⃣" => Some("keycap: 5"),
            "6️⃣" => Some("keycap: 6"),
            "7️⃣" => Some("keycap: 7"),
            "8️⃣" => Some("keycap: 8"),
            "9️⃣" => Some("keycap: 9"),
            "©️" => Some("copyright"),
            "®️" => Some("registered"),
            "‼️" => Some("double exclamation mark"),
            "⁉️" => Some("exclamation question mark"),
            "™️" => Some("trade mark"),
            "⌚" => Some("watch"),
            "⌛" => Some("hourglass done"),
            "⌨️" => Some("keyboard"),
            "⏩" => Some("fast-forward button"),
            "⏪" => Some("fast reverse button"),
            "⏫" => Some("fast up button"),
            "⏬" => Some("fast down button"),
            "⏰" => Some("alarm clock"),
            "⏱️" => Some("stopwatch"),
            "⏲️" => Some("timer clock"),
            "⏳" => Some("hourglass not done"),
            "⏸️" => Some("pause button"),
            "⏹️" => Some("stop button"),
            "▪️" => Some("black small square"),
            "▫️" => Some("white small square"),
            "▶️" => Some("play button"),
            "☀️" => Some("sun"),
            "☁️" => Some("cloud"),
            "☄️" => Some("comet"),
            "☎️" => Some("telephone"),
            "☕" => Some("hot beverage"),
            "☘️" => Some("shamrock"),
            "☠️" => Some("skull and crossbones"),
            "☢️" => Some("radioactive"),
            "☣️" => Some("biohazard"),
            "☦️" => Some("orthodox cross"),
            "☪️" => Some("star and crescent"),
            "☮️" => Some("peace symbol"),
            "☯️" => Some("yin yang"),
            "☸️" => Some("wheel of dharma"),
            "☹️" => Some("frowning face"),
            "♈" => Some("Aries"),
            "♉" => Some("Taurus"),
            "♊" => Some("Gemini"),
            "♋" => Some("Cancer"),
            "♌" => Some("Leo"),
            "♍" => Some("Virgo"),
            "♎" => Some("Libra"),
            "♏" => Some("Scorpio"),
            "♐" => Some("Sagittarius"),
            "♑" => Some("Capricorn"),
            "♒" => Some("Aquarius"),
            "♓" => Some("Pisces"),
            "♟️" => Some("chess pawn"),
            "♨️" => Some("hot springs"),
            "♻️" => Some("recycling symbol"),
            "♿" => Some("wheelchair symbol"),
            "⚒️" => Some("hammer and pick"),
            "⚓" => Some("anchor"),
            "⚔️" => Some("crossed swords"),
            "⚖️" => Some("balance scale"),
            "⚙️" => Some("gear"),
            "⚛️" => Some("atom symbol"),
            "⚜️" => Some("fleur-de-lis"),
            "⚠️" => Some("warning"),
            "⚡" => Some("high voltage"),
            "⚪" => Some("white circle"),
            "⚫" => Some("black circle"),
            "⚰️" => Some("coffin"),
            "⚽" => Some("soccer ball"),
            "⚾" => Some("baseball"),
            "⛄" => Some("snowman without snow"),
            "⛅" => Some("sun behind cloud"),
            "⛈️" => Some("cloud with lightning and rain"),
            "⛎" => Some("Ophiuchus"),
            "⛏️" => Some("pick"),
            "⛓️" => Some("chains"),
            "⛔" => Some("no entry"),
            "⛰️" => Some("mountain"),
            "⛳" => Some("flag in hole"),
            "⛴️" => Some("ferry"),
            "⛵" => Some("sailboat"),
            "⛺" => Some("tent"),
            "⛽" => Some("fuel pump"),
            "✂️" => Some("scissors"),
            "✅" => Some("check mark button"),
            "✈️" => Some("airplane"),
            "✉️" => Some("envelope"),
            "✊" => Some("raised fist"),
            "✋" => Some("raised hand"),
            "✌️" => Some("victory hand"),
            "✍️" => Some("writing hand"),
            "✔️" => Some("check mark"),
            "✖️" => Some("multiply"),
            "✝️" => Some("latin cross"),
            "✡️" => Some("star of David"),
            "✨" => Some("sparkles"),
            "✳️" => Some("eight-spoked asterisk"),
            "✴️" => Some("eight-pointed star"),
            "❄️" => Some("snowflake"),
            "❇️" => Some("sparkle"),
            "❌" => Some("cross mark"),
            "❎" => Some("cross mark button"),
            "❓" => Some("red question mark"),
            "❔" => Some("white question mark"),
            "❕" => Some("white exclamation mark"),
            "❗" => Some("red exclamation mark"),
            "❣️" => Some("heart exclamation"),
            "❤️" => Some("red heart"),
            "➕" => Some("plus"),
            "➖" => Some("minus"),
            "➗" => Some("divide"),
            "➡️" => Some("right arrow"),
            "⬅️" => Some("left arrow"),
            "⬆️" => Some("up arrow"),
            "⬇️" => Some("down arrow"),
            "⭐" => Some("star"),
            "⭕" => Some("hollow red circle"),
            "🅰️" => Some("A button (blood type)"),
            "🅱️" => Some("B button (blood type)"),
            "🅾️" => Some("O button (blood type)"),
            "🅿️" => Some("P button"),
            "🆎" => Some("AB button (blood type)"),
            "🆒" => Some("COOL button"),
            "🆓" => Some("FREE button"),
            "🆔" => Some("ID button"),
            "🆕" => Some("NEW button"),
            "🆗" => Some("OK button"),
            "🆘" => Some("SOS button"),
            "🆙" => Some("UP! button"),
            "🆚" => Some("VS button"),
            "🇦🇷" => Some("flag: Argentina"),
            "🇦🇺" => Some("flag: Australia"),
            "🇧🇷" => Some("flag: Brazil"),
            "🇨🇦" => Some("flag: Canada"),
            "🇨🇱" => Some("flag: Chile"),
            "🇨🇳" => Some("flag: China"),
            "🇨🇴" => Some("flag: Colombia"),
            "🇩🇪" => Some("flag: Germany"),
            "🇪🇸" => Some("flag: Spain"),
            "🇪🇺" => Some("flag: European Union"),
            "🇫🇷" => Some("flag: France"),
            "🇬🇧" => Some("flag: United Kingdom"),
            "🇮🇳" => Some("flag: India"),
            "🇮🇹" => Some("flag: Italy"),
            "🇯🇵" => Some("flag: Japan"),
            "🇰🇷" => Some("flag: South Korea"),
            "🇲🇽" => Some("flag: Mexico"),
            "🇵🇹" => Some("flag: Portugal"),
            "🇷🇺" => Some("flag: Russia"),
            "🇺🇳" => Some("flag: United Nations"),
            "🇺🇸" => Some("flag: United States"),
            "🇺🇾" => Some("flag: Uruguay"),
            "🇿🇦" => Some("flag: South Africa"),
            "🌀" => Some("cyclone"),
            "🌈" => Some("rainbow"),
            "🌊" => Some("water wave"),
            "🌋" => Some("volcano"),
            "🌐" => Some("globe with meridians"),
            "🌝" => Some("full moon face"),
            "🌞" => Some("sun with face"),
            "🌟" => Some("glowing star"),
            "🌤️" => Some("sun behind small cloud"),
            "🌧️" => Some("cloud with rain"),
            "🌨️" => Some("cloud with snow"),
            "🌩️" => Some("cloud with lightning"),
            "🌪️" => Some("tornado"),
            "🌭" => Some("hot dog"),
            "🌮" => Some("taco"),
            "🌯" => Some("burrito"),
            "🌱" => Some("seedling"),
            "🌲" => Some("evergreen tree"),
            "🌳" => Some("deciduous tree"),
            "🌴" => Some("palm tree"),
            "🌵" => Some("cactus"),
            "🌶️" => Some("hot pepper"),
            "🌷" => Some("tulip"),
            "🌸" => Some("cherry blossom"),
            "🌹" => Some("rose"),
            "🌺" => Some("hibiscus"),
            "🌻" => Some("sunflower"),
            "🌼" => Some("blossom"),
            "🌽" => Some("ear of corn"),
            "🌾" => Some("sheaf of rice"),
            "🌿" => Some("herb"),
            "🍀" => Some("four leaf clover"),
            "🍁" => Some("maple leaf"),
            "🍂" => Some("fallen leaf"),
            "🍃" => Some("leaf fluttering in wind"),
            "🍄" => Some("mushroom"),
            "🍅" => Some("tomato"),
            "🍇" => Some("grapes"),
            "🍈" => Some("melon"),
            "🍉" => Some("watermelon"),
            "🍊" => Some("tangerine"),
            "🍋" => Some("lemon"),
            "🍌" => Some("banana"),
            "🍍" => Some("pineapple"),
            "🍎" => Some("red apple"),
            "🍏" => Some("green apple"),
            "🍐" => Some("pear"),
            "🍑" => Some("peach"),
            "🍒" => Some("cherries"),
            "🍓" => Some("strawberry"),
            "🍔" => Some("hamburger"),
            "🍕" => Some("pizza"),
            "🍖" => Some("meat on bone"),
            "🍗" => Some("poultry leg"),
            "🍘" => Some("rice cracker"),
            "🍙" => Some("rice ball"),
            "🍚" => Some("cooked rice"),
            "🍛" => Some("curry rice"),
            "🍜" => Some("steaming bowl"),
            "🍝" => Some("spaghetti"),
            "🍞" => Some("bread"),
            "🍟" => Some("french fries"),
            "🍠" => Some("roasted sweet potato"),
            "🍣" => Some("sushi"),
            "🍤" => Some("fried shrimp"),
            "🍦" => Some("soft ice cream"),
            "🍧" => Some("shaved ice"),
            "🍨" => Some("ice cream"),
            "🍩" => Some("doughnut"),
            "🍪" => Some("cookie"),
            "🍫" => Some("chocolate bar"),
            "🍬" => Some("candy"),
            "🍭" => Some("lollipop"),
            "🍮" => Some("custard"),
            "🍯" => Some("honey pot"),
            "🍰" => Some("shortcake"),
            "🍱" => Some("bento box"),
            "🍲" => Some("pot of food"),
            "🍳" => Some("cooking"),
            "🍵" => Some("teacup without handle"),
            "🍶" => Some("sake"),
            "🍷" => Some("wine glass"),
            "🍸" => Some("cocktail glass"),
            "🍹" => Some("tropical drink"),
            "🍺" => Some("beer mug"),
            "🍻" => Some("clinking beer mugs"),
            "🍼" => Some("baby bottle"),
            "🍽️" => Some("fork and knife with plate"),
            "🍾" => Some("bottle with popping cork"),
            "🎁" => Some("wrapped gift"),
            "🎂" => Some("birthday cake"),
            "🎃" => Some("jack-o-lantern"),
            "🎄" => Some("Christmas tree"),
            "🎈" => Some("balloon"),
            "🎉" => Some("party popper"),
            "🎊" => Some("confetti ball"),
            "🎋" => Some("tanabata tree"),
            "🎌" => Some("crossed flags"),
            "🎍" => Some("pine decoration"),
            "🎖️" => Some("military medal"),
            "🎙️" => Some("studio microphone"),
            "🎞️" => Some("film frames"),
            "🎟️" => Some("admission tickets"),
            "🎠" => Some("carousel horse"),
            "🎡" => Some("ferris wheel"),
            "🎢" => Some("roller coaster"),
            "🎣" => Some("fishing pole"),
            "🎤" => Some("microphone"),
            "🎥" => Some("movie camera"),
            "🎦" => Some("cinema"),
            "🎧" => Some("headphone"),
            "🎨" => Some("artist palette"),
            "🎪" => Some("circus tent"),
            "🎫" => Some("ticket"),
            "🎬" => Some("clapper board"),
            "🎭" => Some("performing arts"),
            "🎮" => Some("video game"),
            "🎯" => Some("bullseye"),
            "🎰" => Some("slot machine"),
            "🎱" => Some("pool 8 ball"),
            "🎲" => Some("game die"),
            "🎳" => Some("bowling"),
            "🎵" => Some("musical note"),
            "🎶" => Some("musical notes"),
            "🎷" => Some("saxophone"),
            "🎸" => Some("guitar"),
            "🎹" => Some("musical keyboard"),
            "🎺" => Some("trumpet"),
            "🎻" => Some("violin"),
            "🎼" => Some("musical score"),
            "🎽" => Some("running shirt"),
            "🎾" => Some("tennis"),
            "🎿" => Some("skis"),
            "🏀" => Some("basketball"),
            "🏁" => Some("chequered flag"),
            "🏂" => Some("snowboarder"),
            "🏄" => Some("person surfing"),
            "🏅" => Some("sports medal"),
            "🏆" => Some("trophy"),
            "🏇" => Some("horse racing"),
            "🏈" => Some("american football"),
            "🏉" => Some("rugby football"),
            "🏊" => Some("person swimming"),
            "🏋️" => Some("person lifting weights"),
            "🏍️" => Some("motorcycle"),
            "🏎️" => Some("racing car"),
            "🏏" => Some("cricket game"),
            "🏐" => Some("volleyball"),
            "🏑" => Some("field hockey"),
            "🏒" => Some("ice hockey"),
            "🏓" => Some("ping pong"),
            "🏔️" => Some("snow-capped mountain"),
            "🏕️" => Some("camping"),
            "🏖️" => Some("beach with umbrella"),
            "🏜️" => Some("desert"),
            "🏝️" => Some("desert island"),
            "🏟️" => Some("stadium"),
            "🏠" => Some("house"),
            "🏡" => Some("house with garden"),
            "🏢" => Some("office building"),
            "🏧" => Some("ATM sign"),
            "🏯" => Some("Japanese castle"),
            "🏰" => Some("castle"),
            "🏳️" => Some("white flag"),
            "🏳️‍⚧️" => Some("transgender flag"),
            "🏳️‍🌈" => Some("rainbow flag"),
            "🏴" => Some("black flag"),
            "🏴‍☠️" => Some("pirate flag"),
            "🏷️" => Some("label"),
            "🏸" => Some("badminton"),
            "🏹" => Some("bow and arrow"),
            "🐅" => Some("tiger"),
            "🐆" => Some("leopard"),
            "🐊" => Some("crocodile"),
            "🐋" => Some("whale"),
            "🐌" => Some("snail"),
            "🐍" => Some("snake"),
            "🐒" => Some("monkey"),
            "🐔" => Some("chicken"),
            "🐗" => Some("boar"),
            "🐙" => Some("octopus"),
            "🐚" => Some("spiral shell"),
            "🐛" => Some("bug"),
            "🐜" => Some("ant"),
            "🐝" => Some("honeybee"),
            "🐞" => Some("lady beetle"),
            "🐟" => Some("fish"),
            "🐠" => Some("tropical fish"),
            "🐡" => Some("blowfish"),
            "🐢" => Some("turtle"),
            "🐣" => Some("hatching chick"),
            "🐤" => Some("baby chick"),
            "🐥" => Some("front-facing baby chick"),
            "🐦" => Some("bird"),
            "🐧" => Some("penguin"),
            "🐨" => Some("koala"),
            "🐬" => Some("dolphin"),
            "🐭" => Some("mouse face"),
            "🐮" => Some("cow face"),
            "🐯" => Some("tiger face"),
            "🐰" => Some("rabbit face"),
            "🐱" => Some("cat face"),
            "🐳" => Some("spouting whale"),
            "🐴" => Some("horse face"),
            "🐵" => Some("monkey face"),
            "🐶" => Some("dog face"),
            "🐷" => Some("pig face"),
            "🐸" => Some("frog"),
            "🐹" => Some("hamster"),
            "🐺" => Some("wolf"),
            "🐻" => Some("bear"),
            "🐻‍❄️" => Some("polar bear"),
            "🐼" => Some("panda"),
            "🐽" => Some("pig nose"),
            "👆" => Some("backhand index pointing up"),
            "👇" => Some("backhand index pointing down"),
            "👈" => Some("backhand index pointing left"),
            "👉" => Some("backhand index pointing right"),
            "👊" => Some("oncoming fist"),
            "👋" => Some("waving hand"),
            "👌" => Some("OK hand"),
            "👍" => Some("thumbs up"),
            "👎" => Some("thumbs down"),
            "👏" => Some("clapping hands"),
            "👐" => Some("open hands"),
            "👹" => Some("ogre"),
            "👺" => Some("goblin"),
            "👻" => Some("ghost"),
            "👽" => Some("alien"),
            "👾" => Some("alien monster"),
            "👿" => Some("angry face with horns"),
            "💀" => Some("skull"),
            "💈" => Some("barber pole"),
            "💉" => Some("syringe"),
            "💊" => Some("pill"),
            "💎" => Some("gem stone"),
            "💐" => Some("bouquet"),
            "💓" => Some("beating heart"),
            "💔" => Some("broken heart"),
            "💕" => Some("two hearts"),
            "💖" => Some("sparkling heart"),
            "💗" => Some("growing heart"),
            "💘" => Some("heart with arrow"),
            "💙" => Some("blue heart"),
            "💚" => Some("green heart"),
            "💛" => Some("yellow heart"),
            "💜" => Some("purple heart"),
            "💝" => Some("heart with ribbon"),
            "💞" => Some("revolving hearts"),
            "💡" => Some("light bulb"),
            "💢" => Some("anger symbol"),
            "💣" => Some("bomb"),
            "💤" => Some("ZZZ"),
            "💥" => Some("collision"),
            "💧" => Some("droplet"),
            "💨" => Some("dashing away"),
            "💩" => Some("pile of poo"),
            "💪" => Some("flexed biceps"),
            "💯" => Some("hundred points"),
            "💰" => Some("money bag"),
            "💱" => Some("currency exchange"),
            "💲" => Some("heavy dollar sign"),
            "💳" => Some("credit card"),
            "💵" => Some("dollar banknote"),
            "💸" => Some("money with wings"),
            "💻" => Some("laptop"),
            "💾" => Some("floppy disk"),
            "💿" => Some("optical disk"),
            "📀" => Some("dvd"),
            "📁" => Some("file folder"),
            "📂" => Some("open file folder"),
            "📅" => Some("calendar"),
            "📆" => Some("tear-off calendar"),
            "📊" => Some("bar chart"),
            "📋" => Some("clipboard"),
            "📌" => Some("pushpin"),
            "📍" => Some("round pushpin"),
            "📎" => Some("paperclip"),
            "📏" => Some("straight ruler"),
            "📐" => Some("triangular ruler"),
            "📛" => Some("name badge"),
            "📞" => Some("telephone receiver"),
            "📟" => Some("pager"),
            "📠" => Some("fax machine"),
            "📡" => Some("satellite antenna"),
            "📢" => Some("loudspeaker"),
            "📦" => Some("package"),
            "📩" => Some("envelope with arrow"),
            "📱" => Some("mobile phone"),
            "📲" => Some("mobile phone with arrow"),
            "📳" => Some("vibration mode"),
            "📴" => Some("mobile phone off"),
            "📶" => Some("antenna bars"),
            "📷" => Some("camera"),
            "📸" => Some("camera with flash"),
            "📹" => Some("video camera"),
            "📺" => Some("television"),
            "📻" => Some("radio"),
            "📽️" => Some("film projector"),
            "🔋" => Some("battery"),
            "🔌" => Some("electric plug"),
            "🔏" => Some("locked with pen"),
            "🔐" => Some("locked with key"),
            "🔑" => Some("key"),
            "🔒" => Some("locked"),
            "🔓" => Some("unlocked"),
            "🔔" => Some("bell"),
            "🔕" => Some("bell with slash"),
            "🔟" => Some("keycap: 10"),
            "🔥" => Some("fire"),
            "🔦" => Some("flashlight"),
            "🔧" => Some("wrench"),
            "🔨" => Some("hammer"),
            "🔩" => Some("nut and bolt"),
            "🔪" => Some("kitchen knife"),
            "🔫" => Some("water pistol"),
            "🔬" => Some("microscope"),
            "🔭" => Some("telescope"),
            "🔮" => Some("crystal ball"),
            "🔯" => Some("dotted six-pointed star"),
            "🔰" => Some("Japanese symbol for beginner"),
            "🔱" => Some("trident emblem"),
            "🔴" => Some("red circle"),
            "🔵" => Some("blue circle"),
            "🔶" => Some("large orange diamond"),
            "🔷" => Some("large blue diamond"),
            "🔸" => Some("small orange diamond"),
            "🔹" => Some("small blue diamond"),
            "🔺" => Some("red triangle pointed up"),
            "🔻" => Some("red triangle pointed down"),
            "🕉️" => Some("om"),
            "🕎" => Some("menorah"),
            "🕯️" => Some("candle"),
            "🕰️" => Some("mantelpiece clock"),
            "🕷️" => Some("spider"),
            "🕸️" => Some("spider web"),
            "🕹️" => Some("joystick"),
            "🖐️" => Some("hand with fingers splayed"),
            "🖖" => Some("vulcan salute"),
            "🖤" => Some("black heart"),
            "🖥️" => Some("desktop computer"),
            "🖨️" => Some("printer"),
            "🖱️" => Some("computer mouse"),
            "🗑️" => Some("wastebasket"),
            "🗝️" => Some("old key"),
            "🗡️" => Some("dagger"),
            "🗺️" => Some("world map"),
            "🗼" => Some("Tokyo tower"),
            "🗽" => Some("Statue of Liberty"),
            "🗿" => Some("moai"),
            "😀" => Some("grinning face"),
            "😁" => Some("beaming face with smiling eyes"),
            "😂" => Some("face with tears of joy"),
            "😃" => Some("grinning face with big eyes"),
            "😄" => Some("grinning face with smiling eyes"),
            "😅" => Some("grinning face with sweat"),
            "😆" => Some("grinning squinting face"),
            "😇" => Some("smiling face with halo"),
            "😈" => Some("smiling face with horns"),
            "😉" => Some("winking face"),
            "😊" => Some("smiling face with smiling eyes"),
            "😋" => Some("face savoring food"),
            "😌" => Some("relieved face"),
            "😍" => Some("smiling face with heart-eyes"),
            "😎" => Some("smiling face with sunglasses"),
            "😏" => Some("smirking face"),
            "😐" => Some("neutral face"),
            "😑" => Some("expressionless face"),
            "😒" => Some("unamused face"),
            "😓" => Some("downcast face with sweat"),
            "😔" => Some("pensive face"),
            "😕" => Some("confused face"),
            "😖" => Some("confounded face"),
            "😗" => Some("kissing face"),
            "😘" => Some("face blowing a kiss"),
            "😙" => Some("kissing face with smiling eyes"),
            "😚" => Some("kissing face with closed eyes"),
            "😛" => Some("face with tongue"),
            "😜" => Some("winking face with tongue"),
            "😝" => Some("squinting face with tongue"),
            "😞" => Some("disappointed face"),
            "😟" => Some("worried face"),
            "😠" => Some("angry face"),
            "😡" => Some("enraged face"),
            "😢" => Some("crying face"),
            "😣" => Some("persevering face"),
            "😤" => Some("face with steam from nose"),
            "😥" => Some("sad but relieved face"),
            "😦" => Some("frowning face with open mouth"),
            "😧" => Some("anguished face"),
            "😨" => Some("fearful face"),
            "😩" => Some("weary face"),
            "😪" => Some("sleepy face"),
            "😫" => Some("tired face"),
            "😬" => Some("grimacing face"),
            "😭" => Some("loudly crying face"),
            "😮" => Some("face with open mouth"),
            "😮‍💨" => Some("face exhaling"),
            "😯" => Some("hushed face"),
            "😰" => Some("anxious face with sweat"),
            "😱" => Some("face screaming in fear"),
            "😲" => Some("astonished face"),
            "😳" => Some("flushed face"),
            "😴" => Some("sleeping face"),
            "😵" => Some("face with crossed-out eyes"),
            "😶" => Some("face without mouth"),
            "😷" => Some("face with medical mask"),
            "😸" => Some("grinning cat with smiling eyes"),
            "😹" => Some("cat with tears of joy"),
            "😺" => Some("grinning cat"),
            "😻" => Some("smiling cat with heart-eyes"),
            "😼" => Some("cat with wry smile"),
            "😽" => Some("kissing cat"),
            "🙀" => Some("weary cat"),
            "🙁" => Some("slightly frowning face"),
            "🙂" => Some("slightly smiling face"),
            "🙃" => Some("upside-down face"),
            "🙄" => Some("face with rolling eyes"),
            "🙈" => Some("see-no-evil monkey"),
            "🙉" => Some("hear-no-evil monkey"),
            "🙊" => Some("speak-no-evil monkey"),
            "🙌" => Some("raising hands"),
            "🙏" => Some("folded hands"),
            "🚀" => Some("rocket"),
            "🚁" => Some("helicopter"),
            "🚂" => Some("locomotive"),
            "🚃" => Some("railway car"),
            "🚄" => Some("high-speed train"),
            "🚅" => Some("bullet train"),
            "🚆" => Some("train"),
            "🚇" => Some("metro"),
            "🚈" => Some("light rail"),
            "🚉" => Some("station"),
            "🚊" => Some("tram"),
            "🚋" => Some("tram car"),
            "🚌" => Some("bus"),
            "🚐" => Some("minibus"),
            "🚑" => Some("ambulance"),
            "🚒" => Some("fire engine"),
            "🚓" => Some("police car"),
            "🚔" => Some("oncoming police car"),
            "🚕" => Some("taxi"),
            "🚗" => Some("automobile"),
            "🚙" => Some("sport utility vehicle"),
            "🚚" => Some("delivery truck"),
            "🚛" => Some("articulated lorry"),
            "🚜" => Some("tractor"),
            "🚠" => Some("mountain cableway"),
            "🚡" => Some("aerial tramway"),
            "🚢" => Some("ship"),
            "🚣" => Some("person rowing boat"),
            "🚤" => Some("speedboat"),
            "🚥" => Some("horizontal traffic light"),
            "🚦" => Some("vertical traffic light"),
            "🚧" => Some("construction"),
            "🚨" => Some("police car light"),
            "🚩" => Some("triangular flag"),
            "🚪" => Some("door"),
            "🚫" => Some("prohibited"),
            "🚲" => Some("bicycle"),
            "🚴" => Some("person biking"),
            "🚵" => Some("person mountain biking"),
            "🚹" => Some("men’s room"),
            "🚺" => Some("women’s room"),
            "🚻" => Some("restroom"),
            "🚼" => Some("baby symbol"),
            "🚽" => Some("toilet"),
            "🚿" => Some("shower"),
            "🛁" => Some("bathtub"),
            "🛋️" => Some("couch and lamp"),
            "🛍️" => Some("shopping bags"),
            "🛐" => Some("place of worship"),
            "🛑" => Some("stop sign"),
            "🛒" => Some("shopping cart"),
            "🛠️" => Some("hammer and wrench"),
            "🛡️" => Some("shield"),
            "🛥️" => Some("motor boat"),
            "🛩️" => Some("small airplane"),
            "🛫" => Some("airplane departure"),
            "🛬" => Some("airplane arrival"),
            "🛰️" => Some("satellite"),
            "🛳️" => Some("passenger ship"),
            "🛴" => Some("kick scooter"),
            "🛵" => Some("motor scooter"),
            "🛸" => Some("flying saucer"),
            "🛹" => Some("skateboard"),
            "🛼" => Some("roller skate"),
            "🟠" => Some("orange circle"),
            "🟡" => Some("yellow circle"),
            "🟢" => Some("green circle"),
            "🟣" => Some("purple circle"),
            "🟤" => Some("brown circle"),
            "🤌" => Some("pinched fingers"),
            "🤍" => Some("white heart"),
            "🤎" => Some("brown heart"),
            "🤏" => Some("pinching hand"),
            "🤐" => Some("zipper-mouth face"),
            "🤑" => Some("money-mouth face"),
            "🤒" => Some("face with thermometer"),
            "🤓" => Some("nerd face"),
            "🤔" => Some("thinking face"),
            "🤕" => Some("face with head-bandage"),
            "🤖" => Some("robot"),
            "🤗" => Some("smiling face with open hands"),
            "🤘" => Some("sign of the horns"),
            "🤙" => Some("call me hand"),
            "🤚" => Some("raised back of hand"),
            "🤝" => Some("handshake"),
            "🤞" => Some("crossed fingers"),
            "🤟" => Some("love-you gesture"),
            "🤠" => Some("cowboy hat face"),
            "🤡" => Some("clown face"),
            "🤢" => Some("nauseated face"),
            "🤣" => Some("rolling on the floor laughing"),
            "🤤" => Some("drooling face"),
            "🤥" => Some("lying face"),
            "🤧" => Some("sneezing face"),
            "🤨" => Some("face with raised eyebrow"),
            "🤩" => Some("star-struck"),
            "🤪" => Some("zany face"),
            "🤫" => Some("shushing face"),
            "🤬" => Some("face with symbols on mouth"),
            "🤭" => Some("face with hand over mouth"),
            "🤮" => Some("face vomiting"),
            "🤯" => Some("exploding head"),
            "🤲" => Some("palms up together"),
            "🤺" => Some("person fencing"),
            "🤿" => Some("diving mask"),
            "🥀" => Some("wilted flower"),
            "🥁" => Some("drum"),
            "🥂" => Some("clinking glasses"),
            "🥃" => Some("tumbler glass"),
            "🥇" => Some("1st place medal"),
            "🥈" => Some("2nd place medal"),
            "🥉" => Some("3rd place medal"),
            "🥊" => Some("boxing glove"),
            "🥋" => Some("martial arts uniform"),
            "🥎" => Some("softball"),
            "🥏" => Some("flying disc"),
            "🥐" => Some("croissant"),
            "🥑" => Some("avocado"),
            "🥒" => Some("cucumber"),
            "🥓" => Some("bacon"),
            "🥔" => Some("potato"),
            "🥕" => Some("carrot"),
            "🥖" => Some("baguette bread"),
            "🥗" => Some("green salad"),
            "🥘" => Some("shallow pan of food"),
            "🥙" => Some("stuffed flatbread"),
            "🥚" => Some("egg"),
            "🥛" => Some("glass of milk"),
            "🥝" => Some("kiwi fruit"),
            "🥞" => Some("pancakes"),
            "🥟" => Some("dumpling"),
            "🥤" => Some("cup with straw"),
            "🥥" => Some("coconut"),
            "🥦" => Some("broccoli"),
            "🥧" => Some("pie"),
            "🥨" => Some("pretzel"),
            "🥩" => Some("cut of meat"),
            "🥪" => Some("sandwich"),
            "🥬" => Some("leafy green"),
            "🥭" => Some("mango"),
            "🥯" => Some("bagel"),
            "🥰" => Some("smiling face with hearts"),
            "🥱" => Some("yawning face"),
            "🥲" => Some("smiling face with tear"),
            "🥳" => Some("partying face"),
            "🥴" => Some("woozy face"),
            "🥵" => Some("hot face"),
            "🥶" => Some("cold face"),
            "🥸" => Some("disguised face"),
            "🥹" => Some("face holding back tears"),
            "🥺" => Some("pleading face"),
            "🦀" => Some("crab"),
            "🦁" => Some("lion"),
            "🦂" => Some("scorpion"),
            "🦄" => Some("unicorn"),
            "🦅" => Some("eagle"),
            "🦆" => Some("duck"),
            "🦇" => Some("bat"),
            "🦈" => Some("shark"),
            "🦉" => Some("owl"),
            "🦊" => Some("fox"),
            "🦋" => Some("butterfly"),
            "🦎" => Some("lizard"),
            "🦐" => Some("shrimp"),
            "🦑" => Some("squid"),
            "🦕" => Some("sauropod"),
            "🦖" => Some("T-Rex"),
            "🦗" => Some("cricket"),
            "🦞" => Some("lobster"),
            "🦟" => Some("mosquito"),
            "🦭" => Some("seal"),
            "🧀" => Some("cheese wedge"),
            "🧁" => Some("cupcake"),
            "🧃" => Some("beverage box"),
            "🧄" => Some("garlic"),
            "🧅" => Some("onion"),
            "🧇" => Some("waffle"),
            "🧈" => Some("butter"),
            "🧉" => Some("mate"),
            "🧊" => Some("ice"),
            "🧋" => Some("bubble tea"),
            "🧐" => Some("face with monocle"),
            "🧗" => Some("person climbing"),
            "🧡" => Some("orange heart"),
            "🧨" => Some("firecracker"),
            "🧩" => Some("puzzle piece"),
            "🧬" => Some("dna"),
            "🧯" => Some("fire extinguisher"),
            "🧰" => Some("toolbox"),
            "🧱" => Some("brick"),
            "🧲" => Some("magnet"),
            "🧹" => Some("broom"),
            "🧺" => Some("basket"),
            "🧻" => Some("roll of paper"),
            "🧼" => Some("soap"),
            "🪑" => Some("chair"),
            "🪕" => Some("banjo"),
            "🪙" => Some("coin"),
            "🪰" => Some("fly"),
            "🪲" => Some("beetle"),
            "🪳" => Some("cockroach"),
            "🪴" => Some("potted plant"),
            "🫐" => Some("blueberries"),
            "🫑" => Some("bell pepper"),
            "🫖" => Some("teapot"),
            "🫠" => Some("melting face"),
            "🫡" => Some("saluting face"),
            "🫢" => Some("face with open eyes and hand over mouth"),
            "🫣" => Some("face with peeking eye"),
            "🫤" => Some("face with diagonal mouth"),
            "🫥" => Some("dotted line face"),
            "🫨" => Some("shaking face"),
            "🫶" => Some("heart hands"),
            _ => None,
        }
    }

    /// Indica se o emoji aceita os modificadores oficiais de tom de pele.
    pub fn supports_skin_tone(emoji: &str) -> bool {
        matches!(
            emoji,
            "✊" | "✋"
                | "✌️"
                | "✍️"
                | "🏂"
                | "🏄"
                | "🏇"
                | "🏊"
                | "🏋️"
                | "👆"
                | "👇"
                | "👈"
                | "👉"
                | "👊"
                | "👋"
                | "👌"
                | "👍"
                | "👎"
                | "👏"
                | "👐"
                | "💪"
                | "🖐️"
                | "🖖"
                | "🙌"
                | "🙏"
                | "🚣"
                | "🚴"
                | "🚵"
                | "🤌"
                | "🤏"
                | "🤘"
                | "🤙"
                | "🤚"
                | "🤝"
                | "🤞"
                | "🤟"
                | "🤲"
                | "🧗"
                | "🫶"
        )
    }

    /// Aplica o modificador selecionado sem duplicar tons já presentes.
    pub fn apply_skin_tone(emoji: &str, modifier: &str) -> String {
        if modifier.is_empty() || !Self::supports_skin_tone(emoji) {
            return emoji.to_string();
        }
        let base: String = emoji
            .chars()
            .filter(|character| !matches!(character, '🏻' | '🏼' | '🏽' | '🏾' | '🏿'))
            .collect();
        format!("{}{}", base, modifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_cache_cas_sharding() {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_cache_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let mgr = MediaCacheManager::with_root(&temp_dir);

        let test_bytes = b"discord-rs media cache CAS test content";
        let url = "https://cdn.discordapp.com/emojis/123456.png";
        let meta = mgr
            .store_media_for_url(url, test_bytes, "image/png")
            .unwrap();

        assert_eq!(meta.size_bytes, test_bytes.len());
        assert_eq!(meta.url, url);

        // Verifica recuperação do cache pelo hash e pela URL (evitando re-downloads)
        let loaded = mgr
            .load_media_by_url(url)
            .expect("Media should be in cache");
        assert_eq!(loaded, test_bytes);

        let (bin_path, meta_path) = mgr.asset_paths(&meta.sha256_hash);
        assert!(bin_path.exists());
        assert!(meta_path.exists());

        // Limpeza do diretório temporário
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_recent_emojis_recording() {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_recents_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let mgr = MediaCacheManager::with_root(&temp_dir);

        let initial = mgr.get_recent_emojis().await;
        assert!(
            initial.is_empty(),
            "Sem histórico de uso, recentes deve vir vazio"
        );

        mgr.record_emoji_use("🦀", "ferris", false, None).await;
        mgr.record_emoji_use("🦀", "ferris", false, None).await;

        let recents = mgr.get_recent_emojis().await;
        let ferris = recents
            .iter()
            .find(|i| i.emoji == "🦀")
            .expect("Ferris must be in recents");
        assert!(ferris.use_count >= 2);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_soundboard_defaults() {
        let sounds = MediaCacheManager::get_default_soundboard_sounds();
        assert!(
            sounds.is_empty(),
            "Sem resposta da rede/API, lista deve vir vazia (zero mocks)"
        );
    }

    #[test]
    fn test_tenor_gif_categories() {
        let trending = MediaCacheManager::get_curated_tenor_gifs_for_category("trending");
        assert!(
            trending.is_empty(),
            "Sem rede/resposta real da API, lista de GIFs deve vir vazia (zero mocks)"
        );

        let anime = MediaCacheManager::get_curated_tenor_gifs_for_category("anime");
        assert!(anime.is_empty());

        let reactions = MediaCacheManager::get_curated_tenor_gifs_for_category("reactions");
        assert!(reactions.is_empty());

        let memes = MediaCacheManager::get_curated_tenor_gifs_for_category("memes");
        assert!(memes.is_empty());
    }

    #[test]
    fn test_sticker_packs_defaults() {
        let packs = MediaCacheManager::get_default_sticker_packs();
        assert!(
            packs.is_empty(),
            "Sem resposta da rede/API, lista deve vir vazia (zero mocks)"
        );
    }

    #[tokio::test]
    async fn test_favorite_stickers_persistence() {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_fav_stickers_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let mgr = MediaCacheManager::with_root(&temp_dir);

        let sticker = DiscordSticker {
            id: "sticker_fav_123".into(),
            pack_id: None,
            name: "Super Wumpus".into(),
            description: Some("Favorite Wumpus".into()),
            tags: "wumpus, favorite".into(),
            format_type: StickerFormatType::Png,
            available: true,
            guild_id: None,
        };

        mgr.add_favorite_sticker(sticker.clone()).await.unwrap();

        let favs = mgr.get_favorite_stickers().await;
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].id, "sticker_fav_123");

        // Test reload from disk
        let mgr2 = MediaCacheManager::with_root(&temp_dir);
        let favs2 = mgr2.get_favorite_stickers().await;
        assert_eq!(favs2.len(), 1);
        assert_eq!(favs2[0].name, "Super Wumpus");

        // Test aggregation in get_all_stickers
        let guild_stickers = vec![DiscordSticker {
            id: "guild_sticker_1".into(),
            pack_id: None,
            name: "Guild Crab".into(),
            description: None,
            tags: "crab".into(),
            format_type: StickerFormatType::Gif,
            available: true,
            guild_id: Some("guild_123".into()),
        }];
        let all = mgr2.get_all_stickers(&guild_stickers).await;
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|s| s.id == "sticker_fav_123"));
        assert!(all.iter().any(|s| s.id == "guild_sticker_1"));

        // Test removal
        mgr2.remove_favorite_sticker("sticker_fav_123")
            .await
            .unwrap();
        let favs3 = mgr2.get_favorite_stickers().await;
        assert!(favs3.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sticker_format_type_serde() {
        let json_int = "1";
        let fmt: StickerFormatType = serde_json::from_str(json_int).unwrap();
        assert_eq!(fmt, StickerFormatType::Png);

        let json_str = "\"apng\"";
        let fmt2: StickerFormatType = serde_json::from_str(json_str).unwrap();
        assert_eq!(fmt2, StickerFormatType::Apng);

        let serialized = serde_json::to_string(&fmt2).unwrap();
        assert_eq!(serialized, "2");

        for invalid in ["0", "5", "-1", "\"webp\""] {
            assert!(serde_json::from_str::<StickerFormatType>(invalid).is_err());
        }
    }

    #[test]
    fn test_sticker_urls_match_official_format_types() {
        let mut sticker = DiscordSticker {
            id: "9001".into(),
            pack_id: None,
            name: "Sticker".into(),
            description: None,
            tags: String::new(),
            format_type: StickerFormatType::Png,
            available: true,
            guild_id: None,
        };
        for (format, expected) in [
            (
                StickerFormatType::Png,
                "https://cdn.discordapp.com/stickers/9001.png",
            ),
            (
                StickerFormatType::Apng,
                "https://cdn.discordapp.com/stickers/9001.png",
            ),
            (
                StickerFormatType::Lottie,
                "https://cdn.discordapp.com/stickers/9001.json",
            ),
            (
                StickerFormatType::Gif,
                "https://media.discordapp.net/stickers/9001.gif?size=160",
            ),
        ] {
            sticker.format_type = format;
            assert_eq!(sticker.cdn_url(), expected);
        }
    }

    #[test]
    fn test_store_media_for_url_replaces_stale_bytes() {
        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_cache_replace_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let manager = MediaCacheManager::with_root(&temp_dir);
        let url = "https://cdn.discordapp.com/stickers/9001.png";
        manager
            .store_media_for_url(url, b"old-bytes", "image/png")
            .unwrap();
        manager
            .store_media_for_url(url, b"new-bytes", "image/png")
            .unwrap();

        let reloaded = MediaCacheManager::with_root(&temp_dir);
        assert_eq!(reloaded.load_media_by_url(url), Some(b"new-bytes".to_vec()));
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_parse_gifs_discord_format() {
        let discord_json = serde_json::json!({
            "categories": [
                { "name": "trending", "src": "https://media.discord.com/category.webp" }
            ],
            "gifs": [
                {
                    "id": "discord_gif_1",
                    "title": "Cool Dancing Cat",
                    "url": "https://tenor.example/view/discord_gif_1",
                    "src": "https://media.discord.com/cat.mp4",
                    "gif_src": "https://media.discord.com/cat.webp",
                    "preview": "https://media.discord.com/cat_preview.mp4",
                    "width": 640,
                    "height": 360
                }
            ]
        });

        let (categories, gifs) =
            MediaCacheManager::parse_trending_gifs_response(&discord_json).unwrap();
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].src, "https://media.discord.com/category.webp");
        assert_eq!(gifs.len(), 1);
        assert_eq!(gifs[0].url, "https://tenor.example/view/discord_gif_1");
        assert_eq!(
            gifs[0].send_url(),
            Some("https://media.discord.com/cat.webp")
        );
        assert_eq!(gifs[0].preview, "https://media.discord.com/cat_preview.mp4");
        assert_eq!(gifs[0].preview_url, "https://media.discord.com/cat.webp");
        assert_eq!((gifs[0].width, gifs[0].height), (640, 360));
        assert_eq!(gifs[0].dims, (640, 360));
    }

    #[test]
    fn test_tenor_gif_constructor_preserves_share_and_preview_sources() {
        let gif = TenorGif::new(
            "constructor-1",
            "Constructor",
            "https://media.example/constructor.webp",
            "https://example.com/view/constructor",
            (320, 240),
            Some("api".into()),
        );
        assert_eq!(gif.src.as_deref(), Some(gif.preview_url.as_str()));
        assert_eq!(gif.gif_src.as_deref(), Some(gif.preview_url.as_str()));
        assert_eq!(gif.preview, gif.preview_url);
        assert_eq!((gif.width, gif.height), (320, 240));
        assert_eq!(gif.send_url(), Some(gif.preview_url.as_str()));
    }

    #[test]
    fn test_parse_tenor_v2_search_results() {
        let payload = serde_json::json!([{
            "id": "tenor-1",
            "title": "Dancing cat",
            "url": "https://tenor.example/view/tenor-1",
            "content_description": "Dancing cat",
            "media_formats": {
                "gif": { "url": "https://media.tenor.com/tenor-1.gif", "dims": [320, 240] },
                "mp4": { "url": "https://media.tenor.com/tenor-1.mp4", "dims": [320, 240] },
                "preview": { "url": "https://media.tenor.com/tenor-1-preview.mp4", "dims": [320, 240] },
                "tinygif": { "url": "https://media.tenor.com/tenor-1-tiny.gif", "dims": [160, 120] }
            }
        }]);
        let gifs = MediaCacheManager::parse_search_gifs_response(&payload).unwrap();
        assert_eq!(gifs[0].url, "https://media.tenor.com/tenor-1.gif");
        assert_eq!(
            gifs[0].send_url(),
            Some("https://media.tenor.com/tenor-1.gif")
        );
        assert_eq!(
            gifs[0].src.as_deref(),
            Some("https://media.tenor.com/tenor-1.mp4")
        );
        assert_eq!(
            gifs[0].preview,
            "https://media.tenor.com/tenor-1-preview.mp4"
        );
        assert_eq!(gifs[0].preview_url, "https://media.tenor.com/tenor-1.gif");
        assert_eq!(gifs[0].dims, (320, 240));
    }

    #[test]
    fn test_gif_dimensions_prefer_top_level_then_valid_legacy_dims() {
        let payload = serde_json::json!({
            "id": "tenor-dims",
            "width": 640,
            "height": 360,
            "dims": [1, 1],
            "media_formats": {
                "gif": { "url": "https://media.example/dims.gif", "dims": [320, 240] }
            }
        });
        let gif = MediaCacheManager::parse_gif_from_json(&payload, None).unwrap();
        assert_eq!((gif.width, gif.height), (640, 360));
        assert_eq!(gif.dims, (640, 360));

        let malformed_top = serde_json::json!({
            "id": "tenor-dims",
            "dims": [0, "invalid"],
            "media_formats": {
                "gif": { "url": "https://media.example/dims.gif", "dims": [320, 240] }
            }
        });
        let gif = MediaCacheManager::parse_gif_from_json(&malformed_top, None).unwrap();
        assert_eq!((gif.width, gif.height), (320, 240));
        assert_eq!(gif.dims, (320, 240));
    }

    #[test]
    fn test_gif_categories_reject_invalid_sources() {
        for payload in [
            serde_json::json!([{ "name": "missing" }]),
            serde_json::json!([{ "name": "empty", "src": "" }]),
            serde_json::json!([{ "name": "file", "src": "file:///tmp/category.gif" }]),
        ] {
            assert!(MediaCacheManager::parse_gif_categories_response(&payload).is_empty());
        }
    }

    #[test]
    fn test_gif_parser_rejects_invalid_media_urls() {
        for payload in [
            serde_json::json!({ "id": "", "url": "https://example.com", "src": "https://example.com/a.mp4", "gif_src": "https://example.com/a.webp" }),
            serde_json::json!({ "id": "1", "src": "https://example.com/a.mp4", "gif_src": "https://example.com/a.webp" }),
            serde_json::json!({ "id": "1", "url": "file:///tmp/a", "src": "https://example.com/a.mp4", "gif_src": "https://example.com/a.webp" }),
            serde_json::json!({ "id": "1", "url": "https://example.com", "src": "https://example.com/a.mp4" }),
            serde_json::json!({ "id": "1", "media_formats": {} }),
            serde_json::json!({ "id": "1", "media_formats": { "gif": { "url": "" } } }),
            serde_json::json!({ "id": "1", "media_formats": { "gif": { "url": "file:///tmp/a.gif" } } }),
        ] {
            assert!(MediaCacheManager::parse_gif_from_json(&payload, None).is_none());
        }
    }

    #[test]
    fn test_gif_dimensions_fall_back_when_payload_dimensions_are_malformed() {
        let payload = serde_json::json!({
            "id": "legacy",
            "url": "https://example.com/view/legacy",
            "src": "https://example.com/legacy.mp4",
            "gif_src": "https://example.com/legacy.webp",
            "dims": [0, "invalid"]
        });
        let gif = MediaCacheManager::parse_gif_from_json(&payload, None).unwrap();
        assert_eq!(gif.dims, (220, 180));
    }

    #[tokio::test]
    async fn test_gif_cache_is_scoped_and_keeps_empty_hits() {
        assert_eq!(
            MediaCacheManager::gif_cache_key("en-US", "mp4", None),
            "gifs|en-us|mp4|trending"
        );
        assert_eq!(
            MediaCacheManager::gif_cache_key("en-US", "webm", Some(" Cat ")),
            "gifs|en-us|webm|search|cat"
        );
        let manager = MediaCacheManager::with_root(std::env::temp_dir().join(format!(
            "discord_rs_gif_cache_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        )));
        manager.cache_gifs("en-US", "mp4", None, &[]).await;
        assert!(manager.has_cached_gifs("en-US", "mp4", None).await);
        assert!(
            manager
                .get_cached_gifs("en-US", "mp4", None)
                .await
                .is_empty()
        );
        assert!(!manager.has_cached_gifs("pt-BR", "mp4", None).await);
        assert!(!manager.has_cached_gifs("en-US", "webm", None).await);
    }

    #[test]
    fn test_recent_custom_metadata_and_skin_tone() {
        let recent = RecentEmojiItem {
            emoji: "<a:party:9002>".into(),
            name: "ignored".into(),
            is_custom: false,
            custom_id: None,
            use_count: 1,
            last_used_timestamp: 1,
        };
        let emoji = recent.to_discord_emoji().unwrap();
        assert!(emoji.animated);
        assert_eq!(emoji.to_tag(), "<a:party:9002>");
        assert_eq!(
            emoji.cdn_url().as_deref(),
            Some("https://cdn.discordapp.com/emojis/9002.gif?size=64&quality=lossless")
        );

        let invalid = RecentEmojiItem {
            custom_id: None,
            is_custom: true,
            ..recent.clone()
        };
        assert!(invalid.to_discord_emoji().is_none());

        let empty = RecentEmojiItem {
            emoji: String::new(),
            name: String::new(),
            is_custom: false,
            custom_id: None,
            use_count: 1,
            last_used_timestamp: 1,
        };
        assert!(empty.to_discord_emoji().is_none());

        assert_eq!(
            MediaCacheManager::unicode_emoji_name("❤️"),
            Some("red heart")
        );
        assert_eq!(MediaCacheManager::apply_skin_tone("👍", "🏽"), "👍🏽");
        assert_eq!(MediaCacheManager::apply_skin_tone("❤️", "🏽"), "❤️");
    }

    #[test]
    fn test_sticker_pack_parser_and_disk_cache() {
        let payload = serde_json::json!({
            "sticker_packs": [{
                "id": "pack-1",
                "name": "Pack",
                "sku_id": "sku-1",
                "description": "Description",
                "stickers": [{
                    "id": "sticker-1",
                    "pack_id": "pack-1",
                    "name": "Sticker",
                    "description": "Description",
                    "tags": "tag",
                    "type": 1,
                    "format_type": 4,
                    "available": false,
                    "sort_value": 7
                }]
            }]
        });
        let packs = MediaCacheManager::parse_sticker_packs_response(&payload);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].stickers[0].format_type, StickerFormatType::Gif);
        assert!(!packs[0].stickers[0].available);

        let temp_dir = std::env::temp_dir().join(format!(
            "discord_rs_pack_cache_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let manager = MediaCacheManager::with_root(&temp_dir);
        manager.save_sticker_packs_to_disk(&packs).unwrap();
        assert_eq!(
            MediaCacheManager::with_root(&temp_dir).get_sticker_packs_cached(),
            packs
        );
        let _ = fs::remove_dir_all(temp_dir);

        let mut unknown = payload.clone();
        unknown["sticker_packs"][0]["stickers"][0]["format_type"] = serde_json::json!(99);
        assert!(MediaCacheManager::parse_sticker_packs_response(&unknown).is_empty());
    }

    #[test]
    fn test_aggregate_stickers_prefers_fresh_and_filters_unavailable() {
        let stale = DiscordSticker {
            id: "same".into(),
            pack_id: None,
            name: "Stale".into(),
            description: None,
            tags: String::new(),
            format_type: StickerFormatType::Png,
            available: true,
            guild_id: None,
        };
        let fresh = DiscordSticker {
            available: false,
            ..stale.clone()
        };
        let result = MediaCacheManager::aggregate_stickers(&[stale], &[fresh], &[]);
        assert!(result.is_empty());
    }
}
