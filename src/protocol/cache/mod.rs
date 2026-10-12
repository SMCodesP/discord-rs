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
pub use lazy_guilds::{
    apply_member_list_update, cache_author_role_color, cache_author_roles, cache_guild_roles,
    convert_list_item_to_state, convert_member_to_item_state, get_cached_author_role_color,
    get_cached_author_roles, get_cached_guild_roles, parse_guild_member_list_update,
    resolve_member_role_color, resolve_member_role_group, GuildMemberListUpdate,
    GuildSubscription, LazyGuildCoordinator, MemberListItem, MemberListOp, MemberRange,
};
pub use revalidation::{CacheEntry, CacheRevalidator, HttpCacheValidation};
pub use startup::{HydratedInitialState, InstantStartupOrchestrator};
