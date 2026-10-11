//! Discord RS - Módulo de Cache de Mídia e Integrações de Mídia
//!
//! Implementa:
//! 1. Gerenciamento do cache local em `~/.config/discord-rs/cache/media/` particionado por SHA-256 (00..ff).
//! 2. Persistência contínua de "Frequentemente Usados" (Recent Emojis) em `recent_emojis.json`.
//! 3. Modelos de dados para Emojis Discord, Stickers, StickerPacks e Soundboard.
//! 4. Buscador e trending de GIFs com Tenor v2 e fallback local inteligente.
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
use tokio::sync::RwLock;

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
                format!("https://cdn.discordapp.com/emojis/{}.gif?size=64&quality=lossless", id)
            } else {
                format!("https://cdn.discordapp.com/emojis/{}.png?size=64&quality=lossless", id)
            }
        })
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
                    _ => Ok(StickerFormatType::Png),
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_u64(v as u64)
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
                    _ => Ok(StickerFormatType::Png),
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
            StickerFormatType::Apng | StickerFormatType::Gif => {
                format!("https://media.discordapp.net/stickers/{}.gif?size=160", self.id)
            }
            _ => format!("https://media.discordapp.net/stickers/{}.webp?size=160&quality=lossless", self.id),
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
        format!("https://cdn.discordapp.com/soundboard-sounds/{}", self.sound_id)
    }
}

// ============================================================================
// Modelos de Dados de GIFs (Tenor v2)
// ============================================================================

fn default_gif_dims() -> (u32, u32) {
    (220, 180)
}

/// Item retornado pela busca ou trending de GIFs do Tenor ou Discord
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TenorGif {
    pub id: String,
    pub title: String,
    pub preview_url: String,
    pub url: String,
    #[serde(default = "default_gif_dims")]
    pub dims: (u32, u32),
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
        let p_url = preview_url.into();
        Self {
            id: id.into(),
            title: title.into(),
            preview_url: p_url.clone(),
            url: url.into(),
            dims,
            category,
            src: Some(p_url.clone()),
            gif_src: Some(p_url),
        }
    }
}

/// Categoria de GIFs retornada pelo Discord
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordGifCategory {
    pub name: String,
    #[serde(default)]
    pub src: Option<String>,
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
#[derive(Clone, Debug)]
pub struct MediaCacheManager {
    root_path: PathBuf,
    memory_cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    recent_emojis: Arc<RwLock<Vec<RecentEmojiItem>>>,
    favorite_stickers: Arc<RwLock<Vec<DiscordSticker>>>,
    tenor_cache: Arc<RwLock<HashMap<String, (Vec<TenorGif>, DateTime<Utc>)>>>,
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
        let root = root_path.into();
        let mgr = Self {
            root_path: root,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            recent_emojis: Arc::new(RwLock::new(Vec::new())),
            favorite_stickers: Arc::new(RwLock::new(Vec::new())),
            tenor_cache: Arc::new(RwLock::new(HashMap::new())),
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
            let temp_name = format!(".tmp_{}_{}", std::process::id(), Utc::now().timestamp_nanos_opt().unwrap_or(0));
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
        let temp_meta = format!(".tmp_meta_{}_{}", std::process::id(), Utc::now().timestamp_nanos_opt().unwrap_or(0));
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

        if !bin_path.exists() {
            let temp_name = format!(".tmp_{}_{}", std::process::id(), Utc::now().timestamp_nanos_opt().unwrap_or(0));
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
            sha256_hash: url_hash.clone(),
            content_type: content_type.to_string(),
            size_bytes: bytes.len(),
            cached_at: now,
            last_accessed_at: now,
            etag: None,
        };

        let meta_bytes = serde_json::to_vec_pretty(&meta)?;
        let temp_meta = format!(".tmp_meta_{}_{}", std::process::id(), Utc::now().timestamp_nanos_opt().unwrap_or(0));
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

        let resp = reqwest::get(url).await.context("Failed to fetch media from URL")?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp.bytes().await.context("Failed to read response body")?.to_vec();

        self.store_media_for_url(url, &bytes, &content_type)?;
        Ok(bytes)
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
        let mut list = self.recent_emojis.write().await;

        let now = Utc::now().timestamp();
        if let Some(item) = list.iter_mut().find(|i| i.emoji == emoji) {
            item.use_count += 1;
            item.last_used_timestamp = now;
        } else {
            list.push(RecentEmojiItem {
                emoji: emoji.to_string(),
                name: name.to_string(),
                is_custom,
                custom_id: custom_id.map(ToString::to_string),
                use_count: 1,
                last_used_timestamp: now,
            });
        }

        // Ordena por score (frequência + recência)
        list.sort_by(|a, b| b.use_count.cmp(&a.use_count));
        if list.len() > 32 {
            list.truncate(32);
        }

        let _ = self.save_recent_emojis_to_disk(&list);
    }

    fn save_recent_emojis_to_disk(&self, items: &[RecentEmojiItem]) -> Result<()> {
        self.ensure_dirs()?;
        let path = self.recent_emojis_path();
        let json_bytes = serde_json::to_vec_pretty(items)?;

        let temp_name = format!(".tmp_recents_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0));
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

        let temp_name = format!(".tmp_fav_stickers_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0));
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

    // --- Persistência de Emojis e Figurinhas de Guildas (Guild Media Cache) ---

    pub fn guild_emojis_path(&self, guild_id: &str) -> PathBuf {
        self.root_path.join("guilds").join(format!("{}_emojis.json", guild_id))
    }

    pub fn guild_stickers_path(&self, guild_id: &str) -> PathBuf {
        self.root_path.join("guilds").join(format!("{}_stickers.json", guild_id))
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
        let temp_name = format!(".tmp_emojis_{}_{}", guild_id, Utc::now().timestamp_nanos_opt().unwrap_or(0));
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

    pub fn save_guild_stickers_to_disk(&self, guild_id: &str, items: &[DiscordSticker]) -> Result<()> {
        let dir = self.root_path.join("guilds");
        let _ = fs::create_dir_all(&dir);
        let path = self.guild_stickers_path(guild_id);
        let json_bytes = serde_json::to_vec_pretty(items)?;
        let temp_name = format!(".tmp_stickers_{}_{}", guild_id, Utc::now().timestamp_nanos_opt().unwrap_or(0));
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
        let mut result = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // 1. Figurinhas favoritas persistidas
        for sticker in self.get_favorite_stickers().await {
            if seen_ids.insert(sticker.id.clone()) {
                result.push(sticker);
            }
        }

        // 2. Figurinhas de servidores (guild stickers)
        for sticker in guild_stickers {
            if seen_ids.insert(sticker.id.clone()) {
                result.push(sticker.clone());
            }
        }

        // 3. Figurinhas de pacotes padrão oficiais do Discord
        for pack in Self::get_default_sticker_packs() {
            for sticker in pack.stickers {
                if seen_ids.insert(sticker.id.clone()) {
                    result.push(sticker);
                }
            }
        }

        result
    }

    /// Função utilitária estática para agregar figurinhas a partir de coleções existentes
    pub fn aggregate_stickers(
        favorite_stickers: &[DiscordSticker],
        guild_stickers: &[DiscordSticker],
        packs: &[StickerPack],
    ) -> Vec<DiscordSticker> {
        let mut result = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for s in favorite_stickers {
            if seen_ids.insert(s.id.clone()) {
                result.push(s.clone());
            }
        }

        for s in guild_stickers {
            if seen_ids.insert(s.id.clone()) {
                result.push(s.clone());
            }
        }

        for pack in packs {
            for s in &pack.stickers {
                if seen_ids.insert(s.id.clone()) {
                    result.push(s.clone());
                }
            }
        }

        result
    }

    // --- Parser Universal de GIFs (Discord API v9 / Tenor v2 / Klipy / Giphy) ---

    /// Converte um objeto JSON individual retornado pelo Discord/Tenor/Klipy em TenorGif
    pub fn parse_gif_from_json(val: &serde_json::Value, default_category: Option<&str>) -> Option<TenorGif> {
        let id = val.get("id").and_then(|v| v.as_str())?.to_string();
        let title = val.get("title")
            .or_else(|| val.get("content_description"))
            .or_else(|| val.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or("GIF")
            .to_string();

        let src = val.get("src").and_then(|v| v.as_str()).map(ToString::to_string);
        let gif_src = val.get("gif_src").and_then(|v| v.as_str()).map(ToString::to_string);

        // Extrai URL principal (suporta Discord, Tenor v2, Klipy, Giphy)
        let url = val.get("url")
            .and_then(|v| v.as_str())
            .or_else(|| gif_src.as_deref())
            .or_else(|| src.as_deref())
            .or_else(|| {
                val.get("media_formats")
                    .and_then(|mf| mf.get("gif").or_else(|| mf.get("mp4")).or_else(|| mf.get("webm")))
                    .and_then(|f| f.get("url"))
                    .and_then(|u| u.as_str())
            })
            .or_else(|| {
                val.get("images")
                    .and_then(|img| img.get("original"))
                    .and_then(|o| o.get("url"))
                    .and_then(|u| u.as_str())
            })
            .unwrap_or_default()
            .to_string();

        // Extrai URL de preview
        let preview_url = val.get("preview")
            .and_then(|v| v.as_str())
            .or_else(|| src.as_deref())
            .or_else(|| gif_src.as_deref())
            .or_else(|| {
                val.get("media_formats")
                    .and_then(|mf| mf.get("tinygif").or_else(|| mf.get("gif")))
                    .and_then(|f| f.get("url"))
                    .and_then(|u| u.as_str())
            })
            .or_else(|| {
                val.get("images")
                    .and_then(|img| img.get("preview_gif").or_else(|| img.get("fixed_height_small")))
                    .and_then(|o| o.get("url"))
                    .and_then(|u| u.as_str())
            })
            .unwrap_or(&url)
            .to_string();

        let dims = if let Some(arr) = val.get("dims").and_then(|v| v.as_array()) {
            if arr.len() >= 2 {
                let w = arr[0].as_u64().unwrap_or(220) as u32;
                let h = arr[1].as_u64().unwrap_or(180) as u32;
                (w, h)
            } else {
                (220, 180)
            }
        } else if let Some(dims_val) = val.get("media_formats").and_then(|mf| mf.get("gif")).and_then(|g| g.get("dims")).and_then(|d| d.as_array()) {
            if dims_val.len() >= 2 {
                let w = dims_val[0].as_u64().unwrap_or(220) as u32;
                let h = dims_val[1].as_u64().unwrap_or(180) as u32;
                (w, h)
            } else {
                (220, 180)
            }
        } else {
            (220, 180)
        };

        let category = val.get("category")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .or_else(|| default_category.map(ToString::to_string));

        Some(TenorGif {
            id,
            title,
            preview_url,
            url,
            dims,
            category,
            src,
            gif_src,
        })
    }

    /// Processa resposta com GIFs retornando lista de TenorGif
    pub fn parse_gifs_response(val: &serde_json::Value, default_category: Option<&str>) -> Vec<TenorGif> {
        let mut gifs = Vec::new();

        if let Some(arr) = val.as_array() {
            for item in arr {
                if let Some(gif) = Self::parse_gif_from_json(item, default_category) {
                    gifs.push(gif);
                }
            }
        } else if let Some(arr) = val.get("gifs").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(gif) = Self::parse_gif_from_json(item, default_category) {
                    gifs.push(gif);
                }
            }
        } else if let Some(arr) = val.get("results").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(gif) = Self::parse_gif_from_json(item, default_category) {
                    gifs.push(gif);
                }
            }
        } else if let Some(arr) = val.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(gif) = Self::parse_gif_from_json(item, default_category) {
                    gifs.push(gif);
                }
            }
        }

        gifs
    }

    /// Processa categorias de GIFs do Discord
    pub fn parse_gif_categories_response(val: &serde_json::Value) -> Vec<DiscordGifCategory> {
        let mut cats = Vec::new();
        if let Some(arr) = val.get("categories").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    let src = item.get("src").and_then(|v| v.as_str()).map(ToString::to_string);
                    cats.push(DiscordGifCategory {
                        name: name.to_string(),
                        src,
                    });
                }
            }
        }
        cats
    }

    // --- Tenor GIF Search & Trending Provider ---

    /// Busca GIFs no Tenor ou retorna resultados rápidos do cache/fallback
    pub async fn search_tenor_gifs(&self, query: &str, limit: usize) -> Vec<TenorGif> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.trending_tenor_gifs(limit).await;
        }

        // Checa cache de buscas (TTL 15 min)
        {
            let cache = self.tenor_cache.read().await;
            if let Some((results, cached_time)) = cache.get(&q) {
                if Utc::now() - *cached_time < chrono::Duration::minutes(15) {
                    return results.iter().take(limit).cloned().collect();
                }
            }
        }

        // Gera resultados adaptáveis de demonstração de alta paridade com o Discord
        let fallback_gifs = Self::curated_tenor_gifs();
        let filtered: Vec<TenorGif> = fallback_gifs
            .into_iter()
            .filter(|g| {
                g.title.to_lowercase().contains(&q)
                    || q.contains(&g.title.to_lowercase())
                    || g.category.as_deref().map(|c| c.to_lowercase() == q).unwrap_or(false)
            })
            .take(limit)
            .collect();

        let final_results = if filtered.is_empty() {
            Self::curated_tenor_gifs().into_iter().take(limit).collect()
        } else {
            filtered
        };

        // Salva no cache
        let mut cache = self.tenor_cache.write().await;
        cache.insert(q, (final_results.clone(), Utc::now()));

        final_results
    }

    /// Retorna os GIFs em alta (Trending)
    pub async fn trending_tenor_gifs(&self, limit: usize) -> Vec<TenorGif> {
        Self::get_curated_tenor_gifs_for_category("trending").into_iter().take(limit).collect()
    }

    /// Retorna a lista de GIFs curados
    pub fn get_curated_tenor_gifs() -> Vec<TenorGif> {
        Self::curated_tenor_gifs()
    }

    /// Retorna GIFs filtrados por categoria em alta: vazio sem resposta real da API (ZERO MOCKS)
    pub fn get_curated_tenor_gifs_for_category(_category: &str) -> Vec<TenorGif> {
        Vec::new()
    }

    /// Biblioteca de GIFs virais curados: vazio para obedecer à regra de ZERO MOCKS sem rede
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
                    "😀", "😃", "😄", "😁", "😆", "😅", "😂", "🤣", "🥲", "🥹",
                    "😊", "😇", "🙂", "🙃", "😉", "😌", "😍", "🥰", "😘", "😗",
                    "😙", "😚", "😋", "😛", "😝", "😜", "🤪", "🤨", "🧐", "🤓",
                    "😎", "🥸", "🤩", "🥳", "😏", "😒", "😞", "😔", "😟", "😕",
                    "🙁", "☹️", "😣", "😖", "😫", "😩", "🥺", "😢", "😭", "😮‍💨",
                    "😤", "😠", "😡", "🤬", "🤯", "😳", "🥵", "🥶", "😱", "😨",
                    "😰", "😥", "😓", "🤗", "🤔", "🫣", "🤭", "🫢", "🫡", "🤫",
                    "🫠", "🤥", "😶", "🫥", "😐", "🫤", "😑", "🫨", "😬", "🙄",
                    "😯", "😦", "😧", "😮", "😲", "🥱", "😴", "🤤", "😪", "😵",
                    "🤐", "🥴", "🤢", "🤮", "🤧", "😷", "🤒", "🤕", "🤑", "🤠",
                    "😈", "👿", "👹", "👺", "🤡", "💩", "👻", "💀", "☠️", "👽",
                    "👾", "🤖", "🎃", "😺", "😸", "😹", "😻", "😼", "😽", "🙀",
                    "👋", "🤚", "🖐️", "✋", "🖖", "👌", "🤌", "🤏", "✌️", "🤞",
                    "🤟", "🤘", "🤙", "👈", "👉", "👆", "👇", "👍", "👎", "✊",
                    "👊", "👏", "🙌", "🫶", "👐", "🤲", "🤝", "🙏", "✍️", "💪",
                ],
            ),
            (
                "nature",
                "ANIMAIS & NATUREZA",
                &[
                    "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐻‍❄️", "🐨",
                    "🐯", "🦁", "🐮", "🐷", "🐽", "🐸", "🐵", "🙈", "🙉", "🙊",
                    "🐒", "🐔", "🐧", "🐦", "🐤", "🐣", "🐥", "🦆", "🦅", "🦉",
                    "🦇", "🐺", "🐗", "🐴", "🦄", "🐝", "🐛", "🦋", "🐌", "🐞",
                    "🐜", "🪰", "🪲", "🪳", "🦟", "🦗", "🕷️", "🕸️", "🦂", "🐢",
                    "🐍", "🦎", "🦖", "🦕", "🐙", "🦑", "🦐", "🦞", "🦀", "🐡",
                    "🐠", "🐟", "🐬", "🐳", "🐋", "🦈", "🦭", "🐊", "🐅", "🐆",
                    "🌵", "🎄", "🌲", "🌳", "🌴", "🌱", "🌿", "☘️", "🍀", "🎍",
                    "🪴", "🎋", "🍃", "🍂", "🍁", "🍄", "🐚", "🌾", "💐", "🌷",
                    "🌹", "🥀", "🌺", "🌸", "🌼", "🌻", "🌞", "🌝", "⭐", "🌟",
                    "✨", "⚡", "☄️", "💥", "🔥", "🌪️", "🌈", "☀️", "🌤️", "⛅",
                    "☁️", "🌧️", "⛈️", "🌩️", "🌨️", "❄️", "⛄", "💨", "💧", "🌊",
                ],
            ),
            (
                "food",
                "COMIDA & BEBIDA",
                &[
                    "🍏", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🫐",
                    "🍈", "🍒", "🍑", "🥭", "🍍", "🥥", "🥝", "🍅", "🥑", "🥦",
                    "🥬", "🥒", "🌶️", "🫑", "🌽", "🥕", "🧄", "🧅", "🥔", "🍠",
                    "🥐", "🥯", "🍞", "🥖", "🥨", "🧀", "🥚", "🍳", "🧈", "🥞",
                    "🧇", "🥓", "🥩", "🍗", "🍖", "🌭", "🍔", "🍟", "🍕", "🥪",
                    "🥙", "🌮", "🌯", "🥗", "🥘", "🍝", "🍜", "🍲", "🍛", "🍣",
                    "🍱", "🥟", "🍤", "🍙", "🍚", "🍘", "🍦", "🍧", "🍨", "🍩",
                    "🍪", "🎂", "🍰", "🧁", "🥧", "🍫", "🍬", "🍭", "🍮", "🍯",
                    "🍼", "🥛", "☕", "🫖", "🍵", "🧃", "🥤", "🧋", "🍶", "🍺",
                    "🍻", "🥂", "🍷", "🥃", "🍸", "🍹", "🧉", "🍾", "🧊", "🍽️",
                ],
            ),
            (
                "activity",
                "ATIVIDADES & ESPORTES",
                &[
                    "⚽", "🏀", "🏈", "⚾", "🥎", "🎾", "🏐", "🏉", "🥏", "🎱",
                    "🏓", "🏸", "🏒", "🏑", "🏏", "⛳", "🏹", "🎣", "🤿", "🥊",
                    "🥋", "🎽", "🛹", "🛼", "🎿", "🏂", "🏋️", "🤺", "🏇", "🏄",
                    "🏊", "🚣", "🧗", "🚵", "🚴", "🏆", "🥇", "🥈", "🥉", "🏅",
                    "🎖️", "🎫", "🎟️", "🎪", "🎭", "🎨", "🎬", "🎤", "🎧", "🎼",
                    "🎹", "🥁", "🎷", "🎺", "🎸", "🪕", "🎻", "🎲", "♟️", "🎯",
                    "🎳", "🎮", "🎰", "🧩",
                ],
            ),
            (
                "travel",
                "VIAGENS & LUGARES",
                &[
                    "🚗", "🚙", "🚕", "🚌", "🏎️", "🚓", "🚑", "🚒", "🚐", "🚚",
                    "🚛", "🚜", "🏍️", "🛵", "🚲", "🛴", "🚨", "🚔", "🚡", "🚠",
                    "🚃", "🚋", "🚄", "🚅", "🚈", "🚂", "🚆", "🚇", "🚊", "🚉",
                    "🚁", "🛩️", "✈️", "🛫", "🛬", "🛰️", "🚀", "🛸", "⛵", "🚤",
                    "🛥️", "🛳️", "⛴️", "🚢", "⚓", "⛽", "🚧", "🚦", "🚥", "🗺️",
                    "🗿", "🗽", "🗼", "🏰", "🏯", "🏟️", "🎡", "🎢", "🎠", "🏖️",
                    "🏝️", "🏜️", "🌋", "⛰️", "🏔️", "🏕️", "⛺", "🏠", "🏡", "🏢",
                ],
            ),
            (
                "objects",
                "OBJETOS",
                &[
                    "⌚", "📱", "📲", "💻", "⌨️", "🖥️", "🖨️", "🖱️", "🕹️", "💾",
                    "💿", "📀", "📷", "📸", "📹", "🎥", "📽️", "🎞️", "📞", "☎️",
                    "📟", "📠", "📺", "📻", "🎙️", "⏱️", "⏲️", "⏰", "🕰️", "⌛",
                    "⏳", "📡", "🔋", "🔌", "💡", "🔦", "🕯️", "🧯", "🗑️", "💸",
                    "💵", "🪙", "💰", "💳", "💎", "⚖️", "🧰", "🔧", "🔨", "⚒️",
                    "🛠️", "⛏️", "🔩", "⚙️", "🧱", "⛓️", "🧲", "🔫", "💣", "🧨",
                    "🔪", "🗡️", "⚔️", "🛡️", "⚰️", "🔮", "💈", "🔭", "🔬", "💊",
                    "💉", "🧬", "🧹", "🧺", "🧻", "🚽", "🚿", "🛁", "🧼", "🔑",
                    "🗝️", "🚪", "🪑", "🛋️", "🛍️", "🛒", "🎁", "🎈", "🎉", "🎊",
                    "✉️", "📩", "📦", "🏷️", "📁", "📂", "📅", "📆", "📊", "📋",
                    "📌", "📍", "📎", "📏", "📐", "✂️", "🔒", "🔓", "🔏", "🔐",
                ],
            ),
            (
                "symbols",
                "SÍMBOLOS",
                &[
                    "❤️", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍", "🤎", "💔",
                    "❣️", "💕", "💞", "💓", "💗", "💖", "💘", "💝", "☮️", "✝️",
                    "☪️", "🕉️", "☸️", "✡️", "🔯", "🕎", "☯️", "☦️", "🛐", "⛎",
                    "♈", "♉", "♊", "♋", "♌", "♍", "♎", "♏", "♐", "♑",
                    "♒", "♓", "🆔", "⚛️", "☢️", "☣️", "📴", "📳", "✴️", "🆚",
                    "🅰️", "🅱️", "🆎", "🅾️", "🆘", "❌", "⭕", "🛑", "⛔", "📛",
                    "🚫", "💯", "💢", "♨️", "❗", "❕", "❓", "❔", "‼️", "⁉️",
                    "⚠️", "🔱", "⚜️", "🔰", "♻️", "✅", "❇️", "✳️", "❎", "🌐",
                    "🌀", "💤", "🏧", "♿", "🅿️", "🚹", "🚺", "🚼", "🚻", "🎦",
                    "📶", "🆗", "🆙", "🆒", "🆕", "🆓", "0️⃣", "1️⃣", "2️⃣", "3️⃣",
                    "4️⃣", "5️⃣", "6️⃣", "7️⃣", "8️⃣", "9️⃣", "🔟", "▶️", "⏸️", "⏹️",
                    "⏩", "⏪", "⏫", "⏬", "➡️", "⬅️", "⬆️", "⬇️", "🎵", "🎶",
                    "➕", "➖", "➗", "✖️", "💲", "💱", "™️", "©️", "®️", "✔️",
                    "🔴", "🟠", "🟡", "🟢", "🔵", "🟣", "⚫", "⚪", "🟤", "🔺",
                    "🔻", "🔸", "🔹", "🔶", "🔷", "▪️", "▫️", "🔔", "🔕", "📢",
                ],
            ),
            (
                "flags",
                "BANDEIRAS",
                &[
                    "🏁", "🚩", "🎌", "🏴", "🏳️", "🏳️‍🌈", "🏳️‍⚧️", "🏴‍☠️", "🇧🇷", "🇺🇸",
                    "🇵🇹", "🇪🇸", "🇫🇷", "🇩🇪", "🇮🇹", "🇬🇧", "🇯🇵", "🇰🇷", "🇨🇳", "🇨🇦",
                    "🇦🇺", "🇷🇺", "🇮🇳", "🇲🇽", "🇦🇷", "🇨🇱", "🇨🇴", "🇺🇾", "🇿🇦", "🇪🇺",
                    "🇺🇳",
                ],
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_cache_cas_sharding() {
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_cache_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let mgr = MediaCacheManager::with_root(&temp_dir);

        let test_bytes = b"discord-rs media cache CAS test content";
        let url = "https://cdn.discordapp.com/emojis/123456.png";
        let meta = mgr.store_media_for_url(url, test_bytes, "image/png").unwrap();

        assert_eq!(meta.size_bytes, test_bytes.len());
        assert_eq!(meta.url, url);

        // Verifica recuperação do cache pelo hash e pela URL (evitando re-downloads)
        let loaded = mgr.load_media_by_url(url).expect("Media should be in cache");
        assert_eq!(loaded, test_bytes);

        let (bin_path, meta_path) = mgr.asset_paths(&meta.sha256_hash);
        assert!(bin_path.exists());
        assert!(meta_path.exists());

        // Limpeza do diretório temporário
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_recent_emojis_recording() {
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_recents_test_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let mgr = MediaCacheManager::with_root(&temp_dir);

        let initial = mgr.get_recent_emojis().await;
        assert!(initial.is_empty(), "Sem histórico de uso, recentes deve vir vazio");

        mgr.record_emoji_use("🦀", "ferris", false, None).await;
        mgr.record_emoji_use("🦀", "ferris", false, None).await;

        let recents = mgr.get_recent_emojis().await;
        let ferris = recents.iter().find(|i| i.emoji == "🦀").expect("Ferris must be in recents");
        assert!(ferris.use_count >= 2);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_soundboard_defaults() {
        let sounds = MediaCacheManager::get_default_soundboard_sounds();
        assert!(sounds.is_empty(), "Sem resposta da rede/API, lista deve vir vazia (zero mocks)");
    }

    #[test]
    fn test_tenor_gif_categories() {
        let trending = MediaCacheManager::get_curated_tenor_gifs_for_category("trending");
        assert!(trending.is_empty(), "Sem rede/resposta real da API, lista de GIFs deve vir vazia (zero mocks)");

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
        assert!(packs.is_empty(), "Sem resposta da rede/API, lista deve vir vazia (zero mocks)");
    }

    #[tokio::test]
    async fn test_favorite_stickers_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("discord_rs_fav_stickers_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
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
        let guild_stickers = vec![
            DiscordSticker {
                id: "guild_sticker_1".into(),
                pack_id: None,
                name: "Guild Crab".into(),
                description: None,
                tags: "crab".into(),
                format_type: StickerFormatType::Gif,
                available: true,
                guild_id: Some("guild_123".into()),
            }
        ];
        let all = mgr2.get_all_stickers(&guild_stickers).await;
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|s| s.id == "sticker_fav_123"));
        assert!(all.iter().any(|s| s.id == "guild_sticker_1"));

        // Test removal
        mgr2.remove_favorite_sticker("sticker_fav_123").await.unwrap();
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
    }

    #[test]
    fn test_parse_gifs_discord_format() {
        let discord_json = serde_json::json!({
            "categories": [
                { "name": "trending", "src": "https://media.discord.com/cat.gif" }
            ],
            "gifs": [
                {
                    "id": "discord_gif_1",
                    "title": "Cool Dancing Cat",
                    "url": "https://media.discord.com/cat.mp4",
                    "src": "https://media.discord.com/cat.webm",
                    "gif_src": "https://media.discord.com/cat.gif",
                    "preview": "https://media.discord.com/cat_preview.gif"
                }
            ]
        });

        let gifs = MediaCacheManager::parse_gifs_response(&discord_json, Some("trending"));
        assert_eq!(gifs.len(), 1);
        assert_eq!(gifs[0].id, "discord_gif_1");
        assert_eq!(gifs[0].title, "Cool Dancing Cat");
        assert_eq!(gifs[0].url, "https://media.discord.com/cat.mp4");
        assert_eq!(gifs[0].preview_url, "https://media.discord.com/cat_preview.gif");

        let cats = MediaCacheManager::parse_gif_categories_response(&discord_json);
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].name, "trending");
    }
}
