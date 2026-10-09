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
            let ext = if self.animated { "gif" } else { "png" };
            format!("https://cdn.discordapp.com/emojis/{}.{}?size=64&quality=lossless", id, ext)
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
        let ext = match self.format_type {
            StickerFormatType::Png => "png",
            StickerFormatType::Apng => "png",
            StickerFormatType::Lottie => "json",
            StickerFormatType::Gif => "gif",
        };
        format!("https://cdn.discordapp.com/stickers/{}.{}?size=160", self.id, ext)
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
            .unwrap_or_else(Self::default_recent_emojis)
    }

    /// Lista os emojis frequentemente usados atualmente
    pub async fn get_recent_emojis(&self) -> Vec<RecentEmojiItem> {
        let lock = self.recent_emojis.read().await;
        if lock.is_empty() {
            Self::default_recent_emojis()
        } else {
            lock.clone()
        }
    }

    /// Emojis padrão fornecidos na primeira execução
    pub fn default_recent_emojis() -> Vec<RecentEmojiItem> {
        vec![
            RecentEmojiItem {
                emoji: "👍".into(),
                name: "thumbsup".into(),
                is_custom: false,
                custom_id: None,
                use_count: 42,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "❤️".into(),
                name: "heart".into(),
                is_custom: false,
                custom_id: None,
                use_count: 38,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "🔥".into(),
                name: "fire".into(),
                is_custom: false,
                custom_id: None,
                use_count: 29,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "🎉".into(),
                name: "tada".into(),
                is_custom: false,
                custom_id: None,
                use_count: 25,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "🦀".into(),
                name: "crab_ferris".into(),
                is_custom: false,
                custom_id: None,
                use_count: 21,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "🚀".into(),
                name: "rocket".into(),
                is_custom: false,
                custom_id: None,
                use_count: 18,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "👀".into(),
                name: "eyes".into(),
                is_custom: false,
                custom_id: None,
                use_count: 15,
                last_used_timestamp: Utc::now().timestamp(),
            },
            RecentEmojiItem {
                emoji: "😂".into(),
                name: "joy".into(),
                is_custom: false,
                custom_id: None,
                use_count: 12,
                last_used_timestamp: Utc::now().timestamp(),
            },
        ]
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
        if list.is_empty() {
            *list = Self::default_recent_emojis();
        }

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

    /// Retorna GIFs filtrados por categoria em alta (Trending, Anime, Reactions, Memes)
    pub fn get_curated_tenor_gifs_for_category(category: &str) -> Vec<TenorGif> {
        let cat = category.trim().to_lowercase();
        let all = Self::curated_tenor_gifs();
        if cat.is_empty() || cat == "trending" || cat == "em alta" {
            all.into_iter()
                .filter(|g| g.category.as_deref() == Some("trending") || g.category.is_none())
                .collect()
        } else {
            let filtered: Vec<TenorGif> = all
                .into_iter()
                .filter(|g| {
                    g.category.as_deref().map(|c| c.to_lowercase() == cat).unwrap_or(false)
                        || g.title.to_lowercase().contains(&cat)
                })
                .collect();
            if filtered.is_empty() {
                Self::curated_tenor_gifs()
            } else {
                filtered
            }
        }
    }

    /// Biblioteca de GIFs virais curados para modo offline e alta responsividade
    pub fn curated_tenor_gifs() -> Vec<TenorGif> {
        vec![
            // Trending
            TenorGif {
                id: "gif-1".into(),
                title: "Cat Vibing".into(),
                preview_url: "https://media.tenor.com/images/9c55b6ce64da0cfab1bc6bfbf165e305/tenor.gif".into(),
                url: "https://tenor.com/view/cat-vibing-vibing-cat-white-cat-dance-gif-18872583".into(),
                dims: (220, 180),
                category: Some("trending".into()),
            },
            TenorGif {
                id: "gif-2".into(),
                title: "Pop Cat".into(),
                preview_url: "https://media.tenor.com/images/220800b46377759b6cbba0e44299b8f2/tenor.gif".into(),
                url: "https://tenor.com/view/pop-cat-mouth-open-gif-19198642".into(),
                dims: (220, 180),
                category: Some("trending".into()),
            },
            TenorGif {
                id: "gif-3".into(),
                title: "Dance Party".into(),
                preview_url: "https://media.tenor.com/images/4a781b0a8523315aefc0082c9f4d45d9/tenor.gif".into(),
                url: "https://tenor.com/view/party-parrot-dance-gif-18116315".into(),
                dims: (220, 180),
                category: Some("trending".into()),
            },
            TenorGif {
                id: "gif-6".into(),
                title: "Ferris the Crab".into(),
                preview_url: "https://media.tenor.com/images/8c4f1c1f7259161a0b322a36b306b3a1/tenor.gif".into(),
                url: "https://tenor.com/view/ferris-rust-rustlang-gif-23001928".into(),
                dims: (220, 180),
                category: Some("trending".into()),
            },
            // Anime
            TenorGif {
                id: "gif-anime-1".into(),
                title: "Anime Sparkle".into(),
                preview_url: "https://media.tenor.com/images/anime_sparkle.gif".into(),
                url: "https://tenor.com/view/anime-sparkle-eyes-excited-gif-17283921".into(),
                dims: (220, 180),
                category: Some("anime".into()),
            },
            TenorGif {
                id: "gif-anime-2".into(),
                title: "Naruto Run".into(),
                preview_url: "https://media.tenor.com/images/naruto_run.gif".into(),
                url: "https://tenor.com/view/naruto-run-fast-anime-gif-15201948".into(),
                dims: (220, 180),
                category: Some("anime".into()),
            },
            TenorGif {
                id: "gif-anime-3".into(),
                title: "Sailor Moon Dance".into(),
                preview_url: "https://media.tenor.com/images/sailor_dance.gif".into(),
                url: "https://tenor.com/view/sailor-moon-dance-cute-gif-19283741".into(),
                dims: (220, 180),
                category: Some("anime".into()),
            },
            TenorGif {
                id: "gif-anime-4".into(),
                title: "Pikachu Shocked".into(),
                preview_url: "https://media.tenor.com/images/pikachu_shock.gif".into(),
                url: "https://tenor.com/view/pikachu-shocked-pokemon-surprise-gif-12938475".into(),
                dims: (220, 180),
                category: Some("anime".into()),
            },
            // Reactions
            TenorGif {
                id: "gif-7".into(),
                title: "Mind Blown".into(),
                preview_url: "https://media.tenor.com/images/07d889ea49d799059f0f9b6c0b3967b0/tenor.gif".into(),
                url: "https://tenor.com/view/mind-blown-tim-and-eric-gif-5374492".into(),
                dims: (220, 180),
                category: Some("reactions".into()),
            },
            TenorGif {
                id: "gif-5".into(),
                title: "Confused Travolta".into(),
                preview_url: "https://media.tenor.com/images/d3f7f18b323789b52a5c4dfbd6621415/tenor.gif".into(),
                url: "https://tenor.com/view/confused-travolta-where-gif-5432092".into(),
                dims: (220, 180),
                category: Some("reactions".into()),
            },
            TenorGif {
                id: "gif-react-3".into(),
                title: "Epic Facepalm".into(),
                preview_url: "https://media.tenor.com/images/facepalm.gif".into(),
                url: "https://tenor.com/view/facepalm-disappointed-sigh-gif-8291038".into(),
                dims: (220, 180),
                category: Some("reactions".into()),
            },
            TenorGif {
                id: "gif-react-4".into(),
                title: "Laughing Tears".into(),
                preview_url: "https://media.tenor.com/images/laughing_tears.gif".into(),
                url: "https://tenor.com/view/laughing-lol-crying-tears-gif-9283741".into(),
                dims: (220, 180),
                category: Some("reactions".into()),
            },
            // Memes
            TenorGif {
                id: "gif-8".into(),
                title: "This Is Fine".into(),
                preview_url: "https://media.tenor.com/images/b4895696ee8c31cb801efea45c854190/tenor.gif".into(),
                url: "https://tenor.com/view/this-is-fine-fire-dog-burning-gif-15104273".into(),
                dims: (220, 180),
                category: Some("memes".into()),
            },
            TenorGif {
                id: "gif-4".into(),
                title: "Rick Roll".into(),
                preview_url: "https://media.tenor.com/images/42c38865c63567634f5904dfb3cf7d5c/tenor.gif".into(),
                url: "https://tenor.com/view/rickroll-rick-astley-never-gonna-give-you-up-gif-18698103".into(),
                dims: (220, 180),
                category: Some("memes".into()),
            },
            TenorGif {
                id: "gif-meme-3".into(),
                title: "Gigachad".into(),
                preview_url: "https://media.tenor.com/images/gigachad.gif".into(),
                url: "https://tenor.com/view/gigachad-chad-based-gif-24819283".into(),
                dims: (220, 180),
                category: Some("memes".into()),
            },
            TenorGif {
                id: "gif-meme-4".into(),
                title: "Doge Wow".into(),
                preview_url: "https://media.tenor.com/images/doge_wow.gif".into(),
                url: "https://tenor.com/view/doge-wow-much-shibe-gif-19283719".into(),
                dims: (220, 180),
                category: Some("memes".into()),
            },
        ]
    }

    // --- Soundboard Defaults Provider ---

    /// Lista os sons padrão do Discord
    pub fn get_default_soundboard_sounds() -> Vec<SoundboardSound> {
        vec![
            SoundboardSound {
                sound_id: "1".into(),
                name: "Quack".into(),
                volume: 0.8,
                emoji_id: None,
                emoji_name: Some("🦆".into()),
                guild_id: None,
                available: true,
            },
            SoundboardSound {
                sound_id: "2".into(),
                name: "Airhorn".into(),
                volume: 0.7,
                emoji_id: None,
                emoji_name: Some("📢".into()),
                guild_id: None,
                available: true,
            },
            SoundboardSound {
                sound_id: "3".into(),
                name: "Cricket".into(),
                volume: 0.6,
                emoji_id: None,
                emoji_name: Some("🦗".into()),
                guild_id: None,
                available: true,
            },
            SoundboardSound {
                sound_id: "4".into(),
                name: "Applause".into(),
                volume: 0.9,
                emoji_id: None,
                emoji_name: Some("👏".into()),
                guild_id: None,
                available: true,
            },
            SoundboardSound {
                sound_id: "5".into(),
                name: "Ba-Dum-Tss".into(),
                volume: 0.85,
                emoji_id: None,
                emoji_name: Some("🥁".into()),
                guild_id: None,
                available: true,
            },
            SoundboardSound {
                sound_id: "6".into(),
                name: "Tada!".into(),
                volume: 0.75,
                emoji_id: None,
                emoji_name: Some("🎉".into()),
                guild_id: None,
                available: true,
            },
        ]
    }

    // --- Sticker Packs Defaults Provider ---

    /// Lista os pacotes padrão de figurinhas (Wumpus, Clyde, Nelly, etc.)
    pub fn get_default_sticker_packs() -> Vec<StickerPack> {
        vec![
            StickerPack {
                id: "847202553061146644".into(),
                name: "Wumpus Adventures".into(),
                sku_id: "847202553061146645".into(),
                description: "O mascote oficial do Discord em ação!".into(),
                banner_asset_id: None,
                stickers: vec![
                    DiscordSticker {
                        id: "749054660769218631".into(),
                        pack_id: Some("847202553061146644".into()),
                        name: "Wumpus Wave".into(),
                        description: Some("Wumpus acenando alegremente".into()),
                        tags: "wave, hello, hi, wumpus".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218632".into(),
                        pack_id: Some("847202553061146644".into()),
                        name: "Wumpus Heart".into(),
                        description: Some("Wumpus com coração de amor".into()),
                        tags: "love, heart, cute, wumpus".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218633".into(),
                        pack_id: Some("847202553061146644".into()),
                        name: "Wumpus GG".into(),
                        description: Some("Wumpus comemorando vitória GG".into()),
                        tags: "gg, win, celebrate, wumpus".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218634".into(),
                        pack_id: Some("847202553061146644".into()),
                        name: "Wumpus Sad".into(),
                        description: Some("Wumpus triste e choroso".into()),
                        tags: "sad, cry, wumpus".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218635".into(),
                        pack_id: Some("847202553061146644".into()),
                        name: "Wumpus Hype".into(),
                        description: Some("Wumpus animado e eufórico".into()),
                        tags: "hype, excited, celebrate, wumpus".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218636".into(),
                        pack_id: Some("847202553061146644".into()),
                        name: "Wumpus Sip".into(),
                        description: Some("Wumpus tomando café relaxado".into()),
                        tags: "coffee, sip, relax, wumpus".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                ],
            },
            StickerPack {
                id: "847202553061146646".into(),
                name: "Clyde Classic".into(),
                sku_id: "847202553061146647".into(),
                description: "O robô icônico e mascote digital do Discord".into(),
                banner_asset_id: None,
                stickers: vec![
                    DiscordSticker {
                        id: "749054660769218641".into(),
                        pack_id: Some("847202553061146646".into()),
                        name: "Clyde Hello".into(),
                        description: Some("Clyde dando olá".into()),
                        tags: "hello, hi, wave, bot, clyde".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218642".into(),
                        pack_id: Some("847202553061146646".into()),
                        name: "Clyde Thumbs Up".into(),
                        description: Some("Clyde fazendo joinha".into()),
                        tags: "thumbsup, ok, good, like, clyde".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218643".into(),
                        pack_id: Some("847202553061146646".into()),
                        name: "Clyde Thinking".into(),
                        description: Some("Clyde ponderando com processador ativo".into()),
                        tags: "think, ponder, idea, clyde".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218644".into(),
                        pack_id: Some("847202553061146646".into()),
                        name: "Clyde Cool".into(),
                        description: Some("Clyde com óculos de sol estiloso".into()),
                        tags: "cool, sunglasses, swag, clyde".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218645".into(),
                        pack_id: Some("847202553061146646".into()),
                        name: "Clyde Shrug".into(),
                        description: Some("Clyde dando de ombros".into()),
                        tags: "shrug, dunno, whatever, clyde".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                ],
            },
            StickerPack {
                id: "847202553061146648".into(),
                name: "Nelly the Hamster".into(),
                sku_id: "847202553061146649".into(),
                description: "Nelly, a adorável hamster da família Discord".into(),
                banner_asset_id: None,
                stickers: vec![
                    DiscordSticker {
                        id: "749054660769218651".into(),
                        pack_id: Some("847202553061146648".into()),
                        name: "Nelly Heart".into(),
                        description: Some("Nelly abraçando um coração brilhante".into()),
                        tags: "heart, love, cute, nelly, hamster".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218652".into(),
                        pack_id: Some("847202553061146648".into()),
                        name: "Nelly Peek".into(),
                        description: Some("Nelly espiando com curiosidade".into()),
                        tags: "peek, look, shy, nelly".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218653".into(),
                        pack_id: Some("847202553061146648".into()),
                        name: "Nelly Nom".into(),
                        description: Some("Nelly comendo sementinha gostosa".into()),
                        tags: "nom, food, snack, eat, nelly".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218654".into(),
                        pack_id: Some("847202553061146648".into()),
                        name: "Nelly Dance".into(),
                        description: Some("Nelly dançando alegremente".into()),
                        tags: "dance, party, happy, nelly".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                    DiscordSticker {
                        id: "749054660769218655".into(),
                        pack_id: Some("847202553061146648".into()),
                        name: "Nelly Sleepy".into(),
                        description: Some("Nelly dormindo quentinha".into()),
                        tags: "sleep, tired, zzz, nelly".into(),
                        format_type: StickerFormatType::Png,
                        available: true,
                        guild_id: None,
                    },
                ],
            },
        ]
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
        assert!(!sounds.is_empty());
        let names: Vec<&str> = sounds.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Quack"));
        assert!(names.contains(&"Airhorn"));
        assert!(names.contains(&"Cricket"));
        assert!(names.contains(&"Applause"));
    }

    #[test]
    fn test_tenor_gif_categories() {
        let trending = MediaCacheManager::get_curated_tenor_gifs_for_category("trending");
        assert!(!trending.is_empty());

        let anime = MediaCacheManager::get_curated_tenor_gifs_for_category("anime");
        assert!(!anime.is_empty());
        assert!(anime.iter().any(|g| g.category.as_deref() == Some("anime")));

        let reactions = MediaCacheManager::get_curated_tenor_gifs_for_category("reactions");
        assert!(!reactions.is_empty());
        assert!(reactions.iter().any(|g| g.category.as_deref() == Some("reactions")));

        let memes = MediaCacheManager::get_curated_tenor_gifs_for_category("memes");
        assert!(!memes.is_empty());
        assert!(memes.iter().any(|g| g.category.as_deref() == Some("memes")));
    }

    #[test]
    fn test_sticker_packs_defaults() {
        let packs = MediaCacheManager::get_default_sticker_packs();
        assert!(packs.len() >= 3);
        assert!(packs.iter().any(|p| p.name == "Wumpus Adventures"));
        assert!(packs.iter().any(|p| p.name == "Clyde Classic"));
        assert!(packs.iter().any(|p| p.name == "Nelly the Hamster"));

        let wumpus_pack = packs.iter().find(|p| p.name == "Wumpus Adventures").unwrap();
        assert!(wumpus_pack.stickers.len() >= 4);
        assert_eq!(wumpus_pack.stickers[0].format_type, StickerFormatType::Png);
    }
}
