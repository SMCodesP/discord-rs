//! Motor Assíncrono de Rate Limiting (Token Bucket Preemptivo)
//!
//! Rastreia cabeçalhos oficiais do Discord e retém requisições na fila do Tokio
//! antes que o limite seja atingido. Respostas 429 inesperadas atualizam o bucket
//! global ou específico usando `Retry-After`.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Cabeçalhos oficiais de Rate Limit do Discord/Cloudflare.
#[derive(Debug, Clone, Default)]
pub struct RateLimitHeaders {
    pub limit: Option<usize>,
    pub remaining: Option<usize>,
    pub reset_after: Option<Duration>,
    pub bucket: Option<String>,
    pub is_global: bool,
}

impl RateLimitHeaders {
    /// Extrai os valores a partir de um HeaderMap do `reqwest`.
    ///
    /// `X-RateLimit-Reset-After` é preferido, com fallback para o timestamp
    /// absoluto de `X-RateLimit-Reset`. Valores inválidos são ignorados.
    pub fn from_reqwest_headers(headers: &reqwest::header::HeaderMap) -> Self {
        let limit = header_string(headers, "x-ratelimit-limit")
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0);

        let remaining = header_string(headers, "x-ratelimit-remaining")
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| limit.map_or(value, |limit| value.min(limit)));

        let reset_after = header_string(headers, "x-ratelimit-reset-after")
            .and_then(|value| parse_duration_seconds(&value))
            .or_else(|| {
                let reset_at = header_string(headers, "x-ratelimit-reset")?
                    .parse::<f64>()
                    .ok()?;
                let now = Utc::now().timestamp() as f64;
                parse_duration_seconds(&(reset_at - now).to_string())
            });

        let bucket = header_string(headers, "x-ratelimit-bucket").filter(|value| !value.is_empty());

        let is_global = header_string(headers, "x-ratelimit-global")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        Self {
            limit,
            remaining,
            reset_after,
            bucket,
            is_global,
        }
    }
}

fn header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_duration_seconds(value: &str) -> Option<Duration> {
    let seconds = value.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(seconds).ok()
}

/// Interpreta `Retry-After` como segundos ou data HTTP (RFC 2822).
///
/// A data é usada apenas quando o cabeçalho não contém um número. Retorna
/// `None` para valores ausentes ou inválidos; o chamador aplica o fallback.
pub fn retry_after_from_headers(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = header_string(headers, "retry-after")?;
    parse_duration_seconds(&value).or_else(|| {
        let retry_at = DateTime::parse_from_rfc2822(&value)
            .ok()?
            .with_timezone(&Utc);
        let now = Utc::now();
        let delay = retry_at.signed_duration_since(now).to_std().ok()?;
        Some(delay)
    })
}

/// Token Bucket individual associado a um hash de rota/recurso.
#[derive(Debug)]
pub struct TokenBucket {
    pub bucket_id: String,
    pub limit: usize,
    pub remaining: usize,
    pub reset_at: Instant,
}

impl TokenBucket {
    pub fn new(bucket_id: impl Into<String>) -> Self {
        Self {
            bucket_id: bucket_id.into(),
            limit: 5,
            remaining: 5,
            reset_at: Instant::now(),
        }
    }

    /// Atualiza os parâmetros do bucket com os novos cabeçalhos recebidos.
    pub fn update(&mut self, headers: &RateLimitHeaders) {
        if let Some(limit) = headers.limit.filter(|limit| *limit > 0) {
            self.limit = limit;
        }
        if let Some(remaining) = headers.remaining {
            self.remaining = remaining.min(self.limit);
        }
        if let Some(duration) = headers.reset_after {
            self.reset_at = Instant::now() + duration;
        }
    }

    /// Tenta consumir 1 token imediatamente. Se esgotado, retorna quanto tempo esperar.
    pub fn reserve(&mut self) -> Option<Duration> {
        let now = Instant::now();

        if now >= self.reset_at {
            self.remaining = self.limit;
        }

        if self.remaining > 0 {
            self.remaining -= 1;
            return None;
        }

        let wait_time = self.reset_at.saturating_duration_since(now);
        // Jitter determinístico por bucket entre 15 e 50 ms para evitar
        // thundering herd sem depender de aleatoriedade ou estado global.
        let hash = self.bucket_id.bytes().fold(0u32, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(u32::from(byte))
        });
        let jitter_ms = 15 + (hash % 36);
        Some(wait_time + Duration::from_millis(u64::from(jitter_ms)))
    }
}

/// Coordenador central de Rate Limiting da aplicação.
#[derive(Debug, Clone)]
pub struct RateLimitCoordinator {
    buckets: Arc<RwLock<HashMap<String, Arc<Mutex<TokenBucket>>>>>,
    route_to_bucket: Arc<RwLock<HashMap<String, String>>>,
    resource_to_bucket: Arc<RwLock<HashMap<String, String>>>,
    global_lock: Arc<RwLock<Option<Instant>>>,
}

impl Default for RateLimitCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitCoordinator {
    pub fn new() -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            route_to_bucket: Arc::new(RwLock::new(HashMap::new())),
            resource_to_bucket: Arc::new(RwLock::new(HashMap::new())),
            global_lock: Arc::new(RwLock::new(None)),
        }
    }

    /// Normaliza uma rota REST para agrupamento em bucket.
    /// `/channels/123/messages?limit=50` -> `/channels/:id/messages`.
    pub fn normalize_route(route: &str) -> String {
        let path = route.split('?').next().unwrap_or(route);
        let parts: Vec<&str> = path.split('/').collect();
        let mut normalized = Vec::with_capacity(parts.len());

        for part in parts {
            if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
                normalized.push(":id");
            } else {
                normalized.push(part);
            }
        }

        normalized.join("/")
    }

    /// Chave usada antes de conhecer o hash do bucket. Preserva o recurso de
    /// nível superior (`channel_id`, `guild_id` ou `webhook_id`), pois o Discord
    /// calcula limites por recurso; os demais IDs são normalizados.
    fn bucket_route_key(route: &str) -> String {
        let path = route.split('?').next().unwrap_or(route);
        let parts: Vec<&str> = path.split('/').collect();
        let mut normalized = Vec::with_capacity(parts.len());
        let mut preserve_resource_id = false;

        for part in parts {
            if preserve_resource_id {
                normalized.push(if part.is_empty() { ":id" } else { part });
                preserve_resource_id = false;
            } else if matches!(part, "channels" | "guilds" | "webhooks") {
                normalized.push(part);
                preserve_resource_id = true;
            } else if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
                normalized.push(":id");
            } else {
                normalized.push(part);
            }
        }

        normalized.join("/")
    }

    fn bucket_resource_key(route: &str) -> Option<String> {
        let path = route.split('?').next().unwrap_or(route);
        let parts: Vec<&str> = path.split('/').collect();
        for (index, part) in parts.iter().enumerate() {
            if matches!(*part, "channels" | "guilds" | "webhooks") {
                let resource = parts.get(index + 1).filter(|id| !id.is_empty())?;
                return Some(format!("{}/{}", part, resource));
            }
        }
        None
    }

    /// Obtém ou cria o Token Bucket para uma rota.
    async fn get_or_create_bucket(&self, route_key: &str) -> Arc<Mutex<TokenBucket>> {
        let route_bucket = self.route_to_bucket.read().await.get(route_key).cloned();
        let bucket_id = if let Some(bucket_id) = route_bucket {
            bucket_id
        } else if let Some(resource) = Self::bucket_resource_key(route_key) {
            self.resource_to_bucket
                .read()
                .await
                .get(&resource)
                .cloned()
                .unwrap_or_else(|| route_key.to_string())
        } else {
            route_key.to_string()
        };

        let mut buckets = self.buckets.write().await;
        buckets
            .entry(bucket_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(TokenBucket::new(bucket_id))))
            .clone()
    }

    /// Adquire permissão antes de disparar a requisição HTTP.
    /// Se não houver tokens, suspende a task até a reabertura do bucket.
    pub async fn pre_acquire(&self, route: &str) {
        loop {
            let global_wait = {
                let mut global = self.global_lock.write().await;
                if let Some(reset_at) = *global {
                    let now = Instant::now();
                    if now < reset_at {
                        Some(reset_at.saturating_duration_since(now))
                    } else {
                        *global = None;
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(wait) = global_wait.filter(|wait| !wait.is_zero()) {
                tokio::time::sleep(wait).await;
            } else {
                break;
            }
        }

        let route_key = Self::bucket_route_key(route);
        let bucket = self.get_or_create_bucket(&route_key).await;

        loop {
            let wait_needed = {
                let mut bucket = bucket.lock().await;
                bucket.reserve()
            };

            if let Some(wait) = wait_needed {
                tokio::time::sleep(wait).await;
            } else {
                break;
            }
        }
    }

    /// Atualiza os metadados do bucket após a conclusão da requisição HTTP.
    pub async fn post_response(&self, route: &str, headers: &RateLimitHeaders) {
        let route_key = Self::bucket_route_key(route);

        if let Some(bucket_hash) = &headers.bucket {
            // O hash não inclui o recurso de nível superior. Combiná-lo evita
            // que limites independentes de canais/guildas sejam confundidos.
            let bucket_id = format!("{bucket_hash}:{route_key}");
            self.route_to_bucket
                .write()
                .await
                .insert(route_key.clone(), bucket_id.clone());
            if let Some(resource) = Self::bucket_resource_key(&route_key) {
                self.resource_to_bucket
                    .write()
                    .await
                    .insert(resource, bucket_id.clone());
            }

            let bucket = {
                let mut buckets = self.buckets.write().await;
                buckets
                    .entry(bucket_id.clone())
                    .or_insert_with(|| Arc::new(Mutex::new(TokenBucket::new(bucket_id))))
                    .clone()
            };
            bucket.lock().await.update(headers);
        } else {
            let bucket = self.get_or_create_bucket(&route_key).await;
            bucket.lock().await.update(headers);
        }

        if headers.is_global {
            if let Some(duration) = headers.reset_after {
                *self.global_lock.write().await = Some(Instant::now() + duration);
            }
        }
    }

    /// Bloqueia o bucket global ou específico após um HTTP 429 inesperado.
    /// Um pequeno buffer de segurança é somado ao `Retry-After` do servidor.
    pub async fn handle_429(
        &self,
        route: &str,
        retry_after: Duration,
        is_global: bool,
    ) -> Duration {
        let wait = retry_after.saturating_add(Duration::from_millis(100));

        if is_global {
            *self.global_lock.write().await = Some(Instant::now() + wait);
        } else {
            let route_key = Self::bucket_route_key(route);
            let bucket = self.get_or_create_bucket(&route_key).await;
            let mut bucket = bucket.lock().await;
            bucket.remaining = 0;
            bucket.reset_at = Instant::now() + wait;
        }

        wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_normalization_removes_query_and_ids() {
        assert_eq!(
            RateLimitCoordinator::normalize_route("/channels/123456789/messages"),
            "/channels/:id/messages"
        );
        assert_eq!(
            RateLimitCoordinator::normalize_route("/guilds/987654321/members?limit=50"),
            "/guilds/:id/members"
        );
        assert_eq!(
            RateLimitCoordinator::normalize_route("/gifs/search?q=conteudo_pessoal"),
            "/gifs/search"
        );
    }

    #[test]
    fn test_headers_parse_bucket_global_and_absolute_reset() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-limit", "10".parse().unwrap());
        headers.insert("x-ratelimit-remaining", "3".parse().unwrap());
        headers.insert("x-ratelimit-bucket", "bucket-synth".parse().unwrap());
        headers.insert("x-ratelimit-global", "TRUE".parse().unwrap());
        headers.insert(
            "x-ratelimit-reset",
            ((Utc::now().timestamp() + 2) as usize)
                .to_string()
                .parse()
                .unwrap(),
        );

        let parsed = RateLimitHeaders::from_reqwest_headers(&headers);
        assert_eq!(parsed.limit, Some(10));
        assert_eq!(parsed.remaining, Some(3));
        assert_eq!(parsed.bucket.as_deref(), Some("bucket-synth"));
        assert!(parsed.is_global);
        assert!(
            parsed
                .reset_after
                .is_some_and(|delay| delay > Duration::ZERO)
        );
    }

    #[test]
    fn test_retry_after_accepts_seconds_and_rejects_invalid_values() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("retry-after", "1.25".parse().unwrap());
        assert_eq!(
            retry_after_from_headers(&headers),
            Some(Duration::from_millis(1250))
        );

        headers.insert("retry-after", "NaN".parse().unwrap());
        assert_eq!(retry_after_from_headers(&headers), None);
        headers.insert("retry-after", "-1".parse().unwrap());
        assert_eq!(retry_after_from_headers(&headers), None);
    }

    #[test]
    fn test_token_bucket_exhaustion_and_reset() {
        let mut bucket = TokenBucket::new("test_bucket");
        bucket.limit = 2;
        bucket.remaining = 2;
        bucket.reset_at = Instant::now() + Duration::from_secs(60);

        assert!(bucket.reserve().is_none());
        assert_eq!(bucket.remaining, 1);
        assert!(bucket.reserve().is_none());
        assert_eq!(bucket.remaining, 0);

        let wait = bucket.reserve().expect("exhausted bucket must wait");
        assert!(wait >= Duration::from_millis(15));
        assert!(wait <= Duration::from_millis(50) + Duration::from_secs(60));

        bucket.reset_at = Instant::now() - Duration::from_millis(10);
        assert!(bucket.reserve().is_none());
        assert_eq!(bucket.remaining, 1);
    }

    #[tokio::test]
    async fn test_coordinator_pre_acquire_and_post_response() {
        let coordinator = RateLimitCoordinator::new();
        coordinator.pre_acquire("/channels/123/messages").await;

        let headers = RateLimitHeaders {
            limit: Some(5),
            remaining: Some(0),
            reset_after: Some(Duration::from_millis(60)),
            bucket: Some("hash_messages_bucket".to_string()),
            is_global: false,
        };
        coordinator
            .post_response("/channels/123/messages", &headers)
            .await;

        let start = Instant::now();
        coordinator.pre_acquire("/channels/123/messages").await;
        assert!(start.elapsed() >= Duration::from_millis(40));
    }

    #[tokio::test]
    async fn test_bucket_hash_is_shared_only_within_the_same_resource() {
        let coordinator = RateLimitCoordinator::new();
        let headers = RateLimitHeaders {
            limit: Some(1),
            remaining: Some(0),
            reset_after: Some(Duration::from_millis(80)),
            bucket: Some("shared-bucket".to_string()),
            is_global: false,
        };
        coordinator
            .post_response("/channels/111/messages", &headers)
            .await;

        let start = Instant::now();
        coordinator.pre_acquire("/channels/111/messages/222").await;
        assert!(start.elapsed() >= Duration::from_millis(60));

        // Outro canal possui cota independente, embora o hash seja o mesmo.
        let start = Instant::now();
        coordinator.pre_acquire("/channels/222/messages").await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_429_blocks_only_its_scope_and_honors_retry_after() {
        let coordinator = RateLimitCoordinator::new();
        let wait = coordinator
            .handle_429("/channels/123/messages", Duration::from_millis(50), false)
            .await;
        assert!(wait >= Duration::from_millis(149));

        let start = Instant::now();
        coordinator.pre_acquire("/channels/123/messages").await;
        assert!(start.elapsed() >= Duration::from_millis(120));

        // Outra rota não deve herdar o bloqueio específico.
        let start = Instant::now();
        coordinator.pre_acquire("/users/@me").await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }
}
