//! Subsistema de Rate Limiting Assíncrono e Prevenção Anti-Spam
//!
//! Implementa:
//! 1. Motor Token Bucket preemptivo que rastreia os cabeçalhos oficiais do Discord
//!    (`X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset-After`, `X-RateLimit-Bucket`).
//! 2. Fila de espera assíncrona para garantia de ZERO erros HTTP 429.
//! 3. Coalescing e debouncing de ações do usuário (Typing no máx 10s, Read Receipts /ack 2s).

pub mod coalescing;
pub mod token_bucket;

pub use coalescing::UserActionDebouncer;
pub use token_bucket::{RateLimitCoordinator, RateLimitHeaders, TokenBucket};
