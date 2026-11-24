use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;

pub const DEFAULT_GIFS_BASE_URL: &str = "https://discord.com/api/v9";
pub const DEFAULT_GIFS_LOCALE: &str = "pt-BR";
pub const DEFAULT_TRENDING_MEDIA_FORMAT: &str = "webm";
pub const DEFAULT_SEARCH_MEDIA_FORMAT: &str = "mp4";
pub const DEFAULT_CACHE_TTL_SECS: i64 = 300;
pub const DEFAULT_DEBOUNCE_MS: u64 = 350;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GifItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub preview_url: String,
    pub width: u32,
    pub height: u32,
    pub tenor_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GifQuery {
    pub query: String,
    pub locale: String,
    pub media_format: String,
    pub limit: Option<usize>,
}

impl GifQuery {
    pub fn trending() -> Self {
        Self {
            query: String::new(),
            locale: DEFAULT_GIFS_LOCALE.to_string(),
            media_format: DEFAULT_TRENDING_MEDIA_FORMAT.to_string(),
            limit: None,
        }
    }

    pub fn search(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            locale: DEFAULT_GIFS_LOCALE.to_string(),
            media_format: DEFAULT_SEARCH_MEDIA_FORMAT.to_string(),
            limit: None,
        }
    }

    fn cache_key(&self, endpoint: &str) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            endpoint,
            self.query.trim().to_lowercase(),
            self.locale.trim().to_lowercase(),
            self.media_format.trim().to_lowercase(),
            self.limit.unwrap_or(0)
        )
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    items: Vec<GifItem>,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GifsClientConfig {
    pub base_url: String,
    pub cache_ttl: ChronoDuration,
    pub debounce: Duration,
}

impl Default for GifsClientConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_GIFS_BASE_URL.to_string(),
            cache_ttl: ChronoDuration::seconds(DEFAULT_CACHE_TTL_SECS),
            debounce: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
        }
    }
}

#[derive(Debug)]
pub struct GifsClient {
    http: reqwest::Client,
    base_url: String,
    cache_ttl: ChronoDuration,
    cache: RwLock<HashMap<String, CacheEntry>>,
    debounce: Duration,
    pending_search: Mutex<Option<JoinHandle<()>>>,
}

impl GifsClient {
    pub fn new(http: reqwest::Client) -> Self {
        Self::with_config(http, GifsClientConfig::default())
    }

    pub fn with_config(http: reqwest::Client, config: GifsClientConfig) -> Self {
        Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            cache_ttl: config.cache_ttl,
            cache: RwLock::new(HashMap::new()),
            debounce: config.debounce,
            pending_search: Mutex::new(None),
        }
    }

    pub fn debounce_duration(&self) -> Duration {
        self.debounce
    }

    pub async fn trending(&self, mut query: GifQuery) -> Result<Vec<GifItem>> {
        if query.locale.trim().is_empty() {
            query.locale = DEFAULT_GIFS_LOCALE.to_string();
        }
        if query.media_format.trim().is_empty() {
            query.media_format = DEFAULT_TRENDING_MEDIA_FORMAT.to_string();
        }
        self.fetch_cached("/gifs/trending", query, false).await
    }

    pub async fn search(&self, mut query: GifQuery) -> Result<Vec<GifItem>> {
        if query.locale.trim().is_empty() {
            query.locale = DEFAULT_GIFS_LOCALE.to_string();
        }
        if query.media_format.trim().is_empty() {
            query.media_format = DEFAULT_SEARCH_MEDIA_FORMAT.to_string();
        }
        let q = query.query.trim().to_string();
        query.query = q.clone();
        if q.is_empty() {
            let mut trending = GifQuery::trending();
            trending.locale = query.locale;
            trending.media_format = if query.media_format == DEFAULT_SEARCH_MEDIA_FORMAT {
                DEFAULT_TRENDING_MEDIA_FORMAT.to_string()
            } else {
                query.media_format
            };
            trending.limit = query.limit;
            return self.trending(trending).await;
        }
        self.fetch_cached("/gifs/search", query, true).await
    }

    pub async fn search_debounced<F, Fut>(
        self: &Arc<Self>,
        query: GifQuery,
        on_result: F,
    ) -> Result<()>
    where
        F: FnOnce(Result<Vec<GifItem>>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut pending = self.pending_search.lock().await;
        if let Some(handle) = pending.take() {
            handle.abort();
        }
        let client = Arc::clone(self);
        let delay = self.debounce;
        let handle = tokio::spawn(async move {
            sleep(delay).await;
            let result = client.search(query).await;
            on_result(result).await;
        });
        *pending = Some(handle);
        Ok(())
    }

    pub async fn clear_cache(&self) {
        self.cache.write().await.clear();
    }

    pub async fn cache_len(&self) -> usize {
        self.cache.read().await.len()
    }

    async fn fetch_cached(
        &self,
        endpoint: &str,
        query: GifQuery,
        include_q: bool,
    ) -> Result<Vec<GifItem>> {
        let cache_key = query.cache_key(endpoint);
        if let Some(items) = self.get_cached(&cache_key).await {
            return Ok(items);
        }

        let url = self.build_url(endpoint, &query, include_q)?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request {}", endpoint))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("failed to read {} body", endpoint))?;
        if !status.is_success() {
            anyhow::bail!("{} returned HTTP {}: {}", endpoint, status, body);
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .with_context(|| format!("failed to parse {} JSON", endpoint))?;
        let mut items = parse_gifs_response(&value);
        if let Some(limit) = query.limit {
            items.truncate(limit);
        }

        self.store_cache(cache_key, items.clone()).await;
        Ok(items)
    }

    fn build_url(&self, endpoint: &str, query: &GifQuery, include_q: bool) -> Result<String> {
        let mut url = reqwest::Url::parse(&format!("{}{}", self.base_url, endpoint))
            .with_context(|| format!("invalid gifs base url for {}", endpoint))?;
        {
            let mut pairs = url.query_pairs_mut();
            if include_q {
                pairs.append_pair("q", query.query.trim());
            }
            pairs.append_pair("locale", query.locale.trim());
            pairs.append_pair("media_format", query.media_format.trim());
            if let Some(limit) = query.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
        }
        Ok(url.to_string())
    }

    async fn get_cached(&self, key: &str) -> Option<Vec<GifItem>> {
        let cache = self.cache.read().await;
        let entry = cache.get(key)?;
        if entry.expires_at > Utc::now() {
            return Some(entry.items.clone());
        }
        drop(cache);
        let mut cache = self.cache.write().await;
        if let Some(entry) = cache.get(key) {
            if entry.expires_at <= Utc::now() {
                cache.remove(key);
            }
        }
        None
    }

    async fn store_cache(&self, key: String, items: Vec<GifItem>) {
        let mut cache = self.cache.write().await;
        cache.insert(
            key,
            CacheEntry {
                items,
                expires_at: Utc::now() + self.cache_ttl,
            },
        );
    }
}

pub fn parse_gifs_response(value: &serde_json::Value) -> Vec<GifItem> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items.iter().filter_map(parse_gif_item).collect()
}

fn parse_gif_item(value: &serde_json::Value) -> Option<GifItem> {
    let id = value.get("id")?.as_str()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let url = value
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let preview_url = value
        .get("src")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("gif_src").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim()
        .to_string();
    let (width, height) = parse_dims(value.get("width"), value.get("height"))?;
    if url.is_empty() || preview_url.is_empty() {
        return None;
    }

    let tenor_url = extract_tenor_url(value);

    Some(GifItem {
        id,
        title,
        url,
        preview_url,
        width,
        height,
        tenor_url,
    })
}

fn parse_dims(width: Option<&serde_json::Value>, height: Option<&serde_json::Value>) -> Option<(u32, u32)> {
    let w = parse_u32(width?)?;
    let h = parse_u32(height?)?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

fn parse_u32(value: &serde_json::Value) -> Option<u32> {
    match value {
        serde_json::Value::Number(n) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .or_else(|| n.as_i64().and_then(|v| u32::try_from(v).ok())),
        serde_json::Value::String(s) => s.trim().parse::<u32>().ok(),
        _ => None,
    }
}

fn extract_tenor_url(value: &serde_json::Value) -> Option<String> {
    let direct = value
        .get("item_url")
        .or_else(|| value.get("content_url"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    if direct.is_some() {
        return direct;
    }
    let url = value.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if url.contains("tenor.com") {
        return Some(url.to_string());
    }
    None
}
