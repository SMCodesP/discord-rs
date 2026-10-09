use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use log::{debug, warn};
use tokio::sync::{Mutex, RwLock};

/// State of an individual rate limit bucket returned by Discord API headers
#[derive(Debug, Clone)]
pub struct BucketState {
    pub limit: u32,
    pub remaining: u32,
    pub reset_at: Instant,
    pub bucket_hash: Option<String>,
}

impl BucketState {
    pub fn new(limit: u32, remaining: u32, reset_after: Duration, hash: Option<String>) -> Self {
        Self {
            limit,
            remaining,
            reset_at: Instant::now() + reset_after,
            bucket_hash: hash,
        }
    }

    /// Checks if the bucket currently has capacity or needs to wait
    pub fn delay_needed(&self) -> Option<Duration> {
        let now = Instant::now();
        if self.remaining > 0 {
            return None;
        }
        if now < self.reset_at {
            Some(self.reset_at - now)
        } else {
            None
        }
    }
}

/// Thread-safe client-side Rate Limit Manager that enforces Discord API rate-limiting RFC
/// Prevents bombarding the Discord API and eliminates HTTP 429 errors.
#[derive(Clone)]
pub struct RateLimitManager {
    route_buckets: Arc<RwLock<HashMap<String, Arc<Mutex<BucketState>>>>>,
    global_lock: Arc<RwLock<Option<Instant>>>,
    typing_debounce: Arc<RwLock<HashMap<String, Instant>>>,
    ack_debounce: Arc<RwLock<HashMap<String, Instant>>>,
}

impl RateLimitManager {
    pub fn new() -> Self {
        Self {
            route_buckets: Arc::new(RwLock::new(HashMap::new())),
            global_lock: Arc::new(RwLock::new(None)),
            typing_debounce: Arc::new(RwLock::new(HashMap::new())),
            ack_debounce: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Normalizes a route path to its canonical rate limit bucket key
    /// e.g. `/channels/12345/messages` -> `channels:messages`
    pub fn route_key(path: &str) -> String {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["channels", _id, "messages"] => "channels:messages".to_string(),
            ["channels", _id, "messages", _mid, "ack"] => "channels:ack".to_string(),
            ["channels", _id, "typing"] => "channels:typing".to_string(),
            ["guilds", _id] => "guilds:detail".to_string(),
            ["users", "@me"] => "users:me".to_string(),
            ["users", "@me", "channels"] => "users:me:channels".to_string(),
            _ => path.to_string(),
        }
    }

    /// Waits for rate limit permit before executing a request
    pub async fn acquire_permit(&self, path: &str) {
        // 1. Check Global Rate Limit
        loop {
            let global_reset = *self.global_lock.read().await;
            if let Some(reset_at) = global_reset {
                let now = Instant::now();
                if now < reset_at {
                    let wait = reset_at - now;
                    warn!("[RateLimiter] Rate limit global ativo! Aguardando {:?}", wait);
                    tokio::time::sleep(wait).await;
                    continue;
                } else {
                    *self.global_lock.write().await = None;
                }
            }
            break;
        }

        // 2. Check Route-Specific Bucket
        let key = Self::route_key(path);
        let bucket_arc = {
            let mut map = self.route_buckets.write().await;
            map.entry(key.clone())
                .or_insert_with(|| {
                    Arc::new(Mutex::new(BucketState::new(
                        5,
                        5,
                        Duration::from_secs(5),
                        None,
                    )))
                })
                .clone()
        };

        let mut bucket = bucket_arc.lock().await;
        if let Some(delay) = bucket.delay_needed() {
            warn!("[RateLimiter] Bucket '{}' esgotado! Segurando requisição por {:?}", key, delay);
            tokio::time::sleep(delay).await;
            // Reset bucket after wait
            bucket.remaining = bucket.limit;
            bucket.reset_at = Instant::now() + Duration::from_secs(5);
        }

        bucket.remaining = bucket.remaining.saturating_sub(1);
    }

    /// Updates bucket state from HTTP response headers returned by Discord
    pub async fn update_from_headers(&self, path: &str, headers: &reqwest::header::HeaderMap) {
        let key = Self::route_key(path);

        // Check X-RateLimit-Global
        if let Some(is_global) = headers.get("X-RateLimit-Global").and_then(|v| v.to_str().ok()) {
            if is_global == "true" {
                if let Some(retry_after) = headers.get("Retry-After").and_then(|v| v.to_str().ok()) {
                    if let Ok(secs) = retry_after.parse::<f64>() {
                        warn!("[RateLimiter] GLOBAL Rate Limit acionado! Retry-After: {}s", secs);
                        *self.global_lock.write().await = Some(Instant::now() + Duration::from_secs_f64(secs));
                        return;
                    }
                }
            }
        }

        let limit = headers
            .get("X-RateLimit-Limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(5);

        let remaining = headers
            .get("X-RateLimit-Remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(limit);

        let reset_after_secs = headers
            .get("X-RateLimit-Reset-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(5.0);

        let bucket_hash = headers
            .get("X-RateLimit-Bucket")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let bucket_arc = {
            let mut map = self.route_buckets.write().await;
            map.entry(key.clone())
                .or_insert_with(|| {
                    Arc::new(Mutex::new(BucketState::new(
                        limit,
                        remaining,
                        Duration::from_secs_f64(reset_after_secs),
                        bucket_hash.clone(),
                    )))
                })
                .clone()
        };

        let mut bucket = bucket_arc.lock().await;
        bucket.limit = limit;
        bucket.remaining = remaining;
        bucket.reset_at = Instant::now() + Duration::from_secs_f64(reset_after_secs);
        bucket.bucket_hash = bucket_hash;

        debug!(
            "[RateLimiter] Bucket '{}' atualizado: limit={}, remaining={}, reset_in={:.2}s",
            key, limit, remaining, reset_after_secs
        );
    }

    /// Handles HTTP 429 Too Many Requests response dynamically
    pub async fn handle_429(&self, path: &str, headers: &reqwest::header::HeaderMap) -> Duration {
        let retry_after = headers
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0);

        let wait_duration = Duration::from_secs_f64(retry_after + 0.1); // Add 100ms safety buffer
        warn!(
            "[RateLimiter] HTTP 429 recebido em '{}'! Pausando requisições por {:?}",
            path, wait_duration
        );

        let key = Self::route_key(path);
        let bucket_arc = {
            let mut map = self.route_buckets.write().await;
            map.entry(key).or_insert_with(|| {
                Arc::new(Mutex::new(BucketState::new(1, 0, wait_duration, None)))
            }).clone()
        };

        let mut bucket = bucket_arc.lock().await;
        bucket.remaining = 0;
        bucket.reset_at = Instant::now() + wait_duration;

        wait_duration
    }

    /// Checks if a typing indicator should be sent (debounced: at most once every 9 seconds per channel)
    pub async fn should_send_typing(&self, channel_id: &str) -> bool {
        let mut map = self.typing_debounce.write().await;
        let now = Instant::now();
        if let Some(&last_sent) = map.get(channel_id) {
            if now.duration_since(last_sent) < Duration::from_secs(9) {
                return false;
            }
        }
        map.insert(channel_id.to_string(), now);
        true
    }

    /// Checks if a read ACK should be sent (debounced: at most once every 2 seconds per channel)
    pub async fn should_send_ack(&self, channel_id: &str) -> bool {
        let mut map = self.ack_debounce.write().await;
        let now = Instant::now();
        if let Some(&last_sent) = map.get(channel_id) {
            if now.duration_since(last_sent) < Duration::from_secs(2) {
                return false;
            }
        }
        map.insert(channel_id.to_string(), now);
        true
    }
}

impl Default for RateLimitManager {
    fn default() -> Self {
        Self::new()
    }
}
