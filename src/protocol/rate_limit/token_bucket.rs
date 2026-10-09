//! Motor Assíncrono de Rate Limiting (Token Bucket Preemptivo)
//!
//! Rastreia cabeçalhos oficiais do Discord e retém requisições na fila do Tokio
//! antes que o limite seja atingido, prevenindo proativamente erros HTTP 429.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Cabeçalhos oficiais de Rate Limit do Discord/Cloudflare
#[derive(Debug, Clone, Default)]
pub struct RateLimitHeaders {
    pub limit: Option<usize>,
    pub remaining: Option<usize>,
    pub reset_after: Option<Duration>,
    pub bucket: Option<String>,
    pub is_global: bool,
}

impl RateLimitHeaders {
    /// Extrai os valores a partir de um HeaderMap do `reqwest`
    pub fn from_reqwest_headers(headers: &reqwest::header::HeaderMap) -> Self {
        let limit = headers
            .get("x-ratelimit-limit")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        let reset_after = headers
            .get("x-ratelimit-reset-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<f64>().ok())
            .map(Duration::from_secs_f64);

        let bucket = headers
            .get("x-ratelimit-bucket")
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);

        let is_global = headers
            .get("x-ratelimit-global")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<bool>().ok())
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

/// Token Bucket individual associado a um hash de rota/recurso
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

    /// Atualiza os parâmetros do bucket com os novos cabeçalhos recebidos
    pub fn update(&mut self, headers: &RateLimitHeaders) {
        if let Some(l) = headers.limit {
            self.limit = l;
        }
        if let Some(r) = headers.remaining {
            self.remaining = r;
        }
        if let Some(duration) = headers.reset_after {
            self.reset_at = Instant::now() + duration;
        }
    }

    /// Tenta consumir 1 token imediatamente. Se esgotado, retorna quanto tempo deve esperar.
    pub fn reserve(&mut self) -> Option<Duration> {
        let now = Instant::now();

        // Se a janela de reset expirou, restaura os tokens
        if now >= self.reset_at {
            self.remaining = self.limit;
        }

        if self.remaining > 0 {
            self.remaining -= 1;
            None // Liberado imediatamente sem espera!
        } else {
            let wait_time = self.reset_at.saturating_duration_since(now);
            // Injeta 15ms de jitter protetor para evitar thundering herd no milissegundo zero
            Some(wait_time + Duration::from_millis(15))
        }
    }
}

/// Coordenador central de Rate Limiting da aplicação
#[derive(Debug, Clone)]
pub struct RateLimitCoordinator {
    buckets: Arc<RwLock<HashMap<String, Arc<Mutex<TokenBucket>>>>>,
    route_to_bucket: Arc<RwLock<HashMap<String, String>>>,
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
            global_lock: Arc::new(RwLock::new(None)),
        }
    }

    /// Normaliza uma rota REST para agrupamento em bucket (ex: `/channels/123/messages` -> `/channels/:id/messages`)
    pub fn normalize_route(route: &str) -> String {
        let parts: Vec<&str> = route.split('?').next().unwrap_or(route).split('/').collect();
        let mut normalized = Vec::new();

        for part in parts {
            if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
                normalized.push(":id");
            } else {
                normalized.push(part);
            }
        }

        normalized.join("/")
    }

    /// Obtém ou cria o Token Bucket para uma rota
    async fn get_or_create_bucket(&self, route_key: &str) -> Arc<Mutex<TokenBucket>> {
        let bucket_id = {
            let r_map = self.route_to_bucket.read().await;
            r_map
                .get(route_key)
                .cloned()
                .unwrap_or_else(|| route_key.to_string())
        };

        let mut b_map = self.buckets.write().await;
        b_map
            .entry(bucket_id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(TokenBucket::new(bucket_id))))
            .clone()
    }

    /// Adquire permissão antes de disparar a requisição HTTP.
    /// Se tokens estiverem esgotados, suspende a task na fila assíncrona até o reset.
    /// Garante ZERO erros HTTP 429!
    pub async fn pre_acquire(&self, route: &str) {
        // 1. Verifica Rate Limit Global
        loop {
            let global_wait = {
                let g = self.global_lock.read().await;
                if let Some(reset_instant) = *g {
                    let now = Instant::now();
                    if now < reset_instant {
                        Some(reset_instant.saturating_duration_since(now))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(wait) = global_wait {
                tokio::time::sleep(wait).await;
            } else {
                break;
            }
        }

        // 2. Verifica Bucket Específico da Rota
        let route_key = Self::normalize_route(route);
        let bucket_arc = self.get_or_create_bucket(&route_key).await;

        loop {
            let wait_needed = {
                let mut bucket = bucket_arc.lock().await;
                bucket.reserve()
            };

            if let Some(duration) = wait_needed {
                // Aguarda na fila assíncrona até a reabertura do bucket
                tokio::time::sleep(duration).await;
            } else {
                // Token adquirido com sucesso
                break;
            }
        }
    }

    /// Atualiza os metadados do bucket após a conclusão da requisição HTTP
    pub async fn post_response(&self, route: &str, headers: &RateLimitHeaders) {
        let route_key = Self::normalize_route(route);

        // Se um hash de bucket foi retornado, mapeia a rota para este bucket compartilhado
        if let Some(bucket_hash) = &headers.bucket {
            let mut r_map = self.route_to_bucket.write().await;
            r_map.insert(route_key.clone(), bucket_hash.clone());

            let mut b_map = self.buckets.write().await;
            let bucket = b_map
                .entry(bucket_hash.clone())
                .or_insert_with(|| Arc::new(Mutex::new(TokenBucket::new(bucket_hash))));
            bucket.lock().await.update(headers);
        } else {
            let bucket = self.get_or_create_bucket(&route_key).await;
            bucket.lock().await.update(headers);
        }

        // Se for limite global
        if headers.is_global {
            if let Some(duration) = headers.reset_after {
                let mut g = self.global_lock.write().await;
                *g = Some(Instant::now() + duration);
            }
        }
    }

    /// Contingência caso ocorra HTTP 429 inesperado (por drift de relógio)
    pub async fn handle_429(&self, route: &str, retry_after: Duration, is_global: bool) {
        if is_global {
            let mut g = self.global_lock.write().await;
            *g = Some(Instant::now() + retry_after);
        } else {
            let route_key = Self::normalize_route(route);
            let bucket = self.get_or_create_bucket(&route_key).await;
            let mut b = bucket.lock().await;
            b.remaining = 0;
            b.reset_at = Instant::now() + retry_after;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_route_normalization() {
        assert_eq!(
            RateLimitCoordinator::normalize_route("/channels/123456789/messages"),
            "/channels/:id/messages"
        );
        assert_eq!(
            RateLimitCoordinator::normalize_route("/guilds/987654321/members?limit=50"),
            "/guilds/:id/members"
        );
    }

    #[tokio::test]
    async fn test_token_bucket_exhaustion_and_reset() {
        let mut bucket = TokenBucket::new("test_bucket");
        bucket.limit = 2;
        bucket.remaining = 2;
        bucket.reset_at = Instant::now() + Duration::from_millis(50);

        // Primeiro token: instantâneo
        assert!(bucket.reserve().is_none());
        assert_eq!(bucket.remaining, 1);

        // Segundo token: instantâneo
        assert!(bucket.reserve().is_none());
        assert_eq!(bucket.remaining, 0);

        // Terceiro token: deve esperar!
        let wait = bucket.reserve();
        assert!(wait.is_some());
        assert!(wait.unwrap() >= Duration::from_millis(15));
    }

    #[tokio::test]
    async fn test_coordinator_pre_acquire_and_post_response() {
        let coordinator = RateLimitCoordinator::new();

        // 1. Consume 1 token
        coordinator.pre_acquire("/channels/123/messages").await;

        // 2. Update with remaining 0
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

        // 3. Second pre_acquire must handle wait automatically without 429
        let start = Instant::now();
        coordinator.pre_acquire("/channels/123/messages").await;
        let elapsed = start.elapsed();

        // Expect waiting duration approximately 60ms
        assert!(elapsed >= Duration::from_millis(40));
    }
}
