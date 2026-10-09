//! Subsistema de Cache Local Idêntico ao Discord Oficial
//!
//! Gerencia o armazenamento persistente em disco sob `~/.config/discord-rs/cache/`,
//! endereçamento por conteúdo (CAS SHA-256) de assets, requisições condicionais
//! HTTP com ETags (HTTP 304), revalidação reativa orientada a eventos do Gateway,
//! Lazy Guilds (Opcode 14) e inicialização instantânea (0ms blank screen).

pub mod directory;
pub mod lazy_guilds;
pub mod revalidation;
pub mod startup;

pub use directory::{AssetCasStore, AssetMetadata, CacheDirectory, CacheManifest};
pub use lazy_guilds::{GuildSubscription, LazyGuildCoordinator, MemberRange};
pub use revalidation::{CacheEntry, CacheRevalidator, HttpCacheValidation};
pub use startup::{HydratedInitialState, InstantStartupOrchestrator};
