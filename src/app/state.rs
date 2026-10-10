use std::collections::{HashMap, HashSet};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use crate::domain::channel::ChannelType;
use crate::domain::message::{Attachment, Author, Message, Reaction};
use crate::domain::settings::{
    AccountConnection, AccessibilitySettings, AuthorizedApp, FamilyCenterLink,
    MessageDisplayMode, NitroTier, ServerBoost, Subscription, ThemeChoice,
};
use crate::protocol::{AuthToken, ClientManager, ConnectionMode, TokenStorage, TransportEvent};

/// Status presence for users
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PresenceStatus {
    #[default]
    Online,
    Idle,
    DoNotDisturb,
    Offline,
}

impl PresenceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Idle => "Ausente",
            Self::DoNotDisturb => "Não Perturbar",
            Self::Offline => "Invisível",
        }
    }

    pub fn color_rgb(&self) -> u32 {
        match self {
            Self::Online => 0x23a55a,
            Self::Idle => 0xf0b232,
            Self::DoNotDisturb => 0xf23f43,
            Self::Offline => 0x80848e,
        }
    }
}

/// Active modal dialogs in the application
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    UserSettings,
    AddServer,
    ExploreServers,
    Login,
    LoginDiscord,
    AddFriendToGroup(String),
    CreateGroupDm,
}

/// User settings navigation tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    MyAccount,
    Profiles,
    Privacy,
    AuthorizedApps,
    Connections,
    Nitro,
    ServerBoost,
    FamilyCenter,
    Appearance,
    Accessibility,
    VoiceVideo,
    Notifications,
    Keybinds,
    Language,
}

impl SettingsTab {
    pub fn title(&self) -> &'static str {
        match self {
            Self::MyAccount => "Minha Conta",
            Self::Profiles => "Perfis",
            Self::Privacy => "Privacidade & Segurança",
            Self::AuthorizedApps => "Aplicativos Autorizados",
            Self::Connections => "Conexões",
            Self::Nitro => "Nitro",
            Self::ServerBoost => "Impulso de Servidor",
            Self::FamilyCenter => "Centro da Família",
            Self::Appearance => "Aparência",
            Self::Accessibility => "Acessibilidade",
            Self::VoiceVideo => "Voz & Vídeo",
            Self::Notifications => "Notificações",
            Self::Keybinds => "Atalhos de Teclado",
            Self::Language => "Idioma",
        }
    }
}

/// Channel types in sidebar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    Text,
    Voice,
    Announcement,
}

impl ChannelKind {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Text => "#",
            Self::Voice => "🔊",
            Self::Announcement => "📢",
        }
    }
}

/// Channel representation for sidebar display
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelItemState {
    pub id: String,
    pub name: String,
    pub kind: ChannelKind,
    pub topic: Option<String>,
    pub unread: bool,
    pub mention_count: u32,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub is_group_dm: bool,
    #[serde(default)]
    pub recipient_ids: Vec<String>,
    #[serde(default)]
    pub recipient_names: Vec<String>,
}

/// Category grouping in channel sidebar
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryItemState {
    pub id: String,
    pub name: String,
    pub channels: Vec<ChannelItemState>,
}

/// Guild item for guild rail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildItemState {
    pub id: String,
    pub name: String,
    pub acronym: String,
    pub icon_url: Option<String>,
    pub icon_hash: Option<String>,
    pub unread: bool,
    pub mention_count: u32,
    pub categories: Vec<CategoryItemState>,
    #[serde(default)]
    pub emojis: Vec<crate::protocol::media_cache::DiscordEmoji>,
    #[serde(default)]
    pub stickers: Vec<crate::protocol::media_cache::DiscordSticker>,
}

impl GuildItemState {
    /// Retorna a URL do ícone da guilda (resolvendo icon_url direta ou via CDN usando icon_hash)
    pub fn icon_url(&self) -> Option<String> {
        if let Some(ref url) = self.icon_url {
            if !url.is_empty() {
                return Some(url.clone());
            }
        }
        if let Some(ref hash) = self.icon_hash {
            if !hash.is_empty() {
                return Some(format!("https://cdn.discordapp.com/icons/{}/{}.png", self.id, hash));
            }
        }
        None
    }
}

/// Friend / Relationship representation in direct messages and friends list
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendItemState {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub custom_status: Option<String>,
    pub relationship_type: u32,
}

/// Navigation tabs in Friends Hub
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FriendsTab {
    #[default]
    Online,
    All,
    Pending,
    Blocked,
    AddFriend,
    MessageRequests,
}

/// Subtabs within Message Requests view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MessageRequestsSubTab {
    #[default]
    YouMayKnow,
    Spam,
}

/// Direct message request entry from a non-friend user
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRequestItem {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub mutual_guilds: usize,
    pub mutual_guild_name: Option<String>,
    pub preview_message: String,
    pub timestamp: String,
    pub is_spam: bool,
}

/// Activity category for Rich Presence & Active Now panel
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveNowKind {
    Game,
    RichPresence,
    Spotify,
    Quest,
}

/// Active Now Rich Presence card representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveNowItem {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub kind: ActiveNowKind,
    pub app_name: String,
    pub header_subtitle: String,
    pub app_icon_symbol: String,
    pub details_title: String,
    pub details_state: Option<String>,
    pub elapsed_time_text: Option<String>,
    pub spotify_track: Option<String>,
    pub spotify_artist: Option<String>,
    pub spotify_album: Option<String>,
    pub spotify_progress_text: Option<(String, String)>,
    pub spotify_progress_pct: Option<f32>,
    pub button_text: Option<String>,
}

/// Disk cache structure for persisting guilds and folder structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildsDiskCache {
    pub guilds: Vec<GuildItemState>,
    pub guild_folders: Vec<crate::protocol::GuildFolder>,
}

/// Path to the guilds cache file on disk (~/.config/discord-rs/guilds_cache.json)
pub fn guilds_cache_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".config"))
        .join("discord-rs")
        .join("guilds_cache.json")
}

/// Persists the list of guilds and guild folders to disk in JSON format
pub fn save_guilds_to_disk(guilds: &[GuildItemState], guild_folders: &[crate::protocol::GuildFolder]) {
    let path = guilds_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = GuildsDiskCache {
        guilds: guilds.to_vec(),
        guild_folders: guild_folders.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(&path, json);
    }
}

/// Loads cached guilds and guild folders from disk if available and non-empty
pub fn load_guilds_from_disk() -> Option<GuildsDiskCache> {
    let path = guilds_cache_path();
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    if let Ok(cache) = serde_json::from_str::<GuildsDiskCache>(&data) {
        if !cache.guilds.is_empty() {
            return Some(cache);
        }
    }
    if let Ok(guilds) = serde_json::from_str::<Vec<GuildItemState>>(&data) {
        if !guilds.is_empty() {
            return Some(GuildsDiskCache {
                guilds,
                guild_folders: Vec::new(),
            });
        }
    }
    None
}

/// Voice participant in an active call
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceParticipantState {
    pub id: String,
    pub name: String,
    pub avatar_text: String,
    pub is_speaking: bool,
    pub is_muted: bool,
    pub is_deafened: bool,
}

/// Active voice call session
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveVoiceCall {
    pub channel_id: String,
    pub channel_name: String,
    pub guild_name: String,
    pub ping_ms: u32,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_screen_sharing: bool,
    pub participants: Vec<VoiceParticipantState>,
}

/// Current logged in user profile
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUserProfile {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub discriminator: Option<String>,
    pub avatar_text: String,
    pub avatar_url: Option<String>,
    pub custom_status: Option<String>,
    pub presence: PresenceStatus,
    pub is_muted: bool,
    pub is_deafened: bool,
}

/// Member displayed in the member sidebar
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberItemState {
    pub id: String,
    pub name: String,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub role_group: String,
    pub role_color: u32,
    pub is_bot: bool,
    pub activity: Option<String>,
}

/// Main Application State
#[derive(Debug, Clone)]
pub struct AppState {
    /// Selected guild Snowflake ID (`None` = Home / DMs)
    pub selected_guild_id: Option<String>,
    /// Selected channel Snowflake ID
    pub selected_channel_id: Option<String>,
    /// Connected voice call session
    pub active_voice_call: Option<ActiveVoiceCall>,
    /// Whether the member sidebar (right column) is visible
    pub show_members: bool,
    /// Currently opened modal dialog
    pub active_modal: Option<ActiveModal>,
    /// Active tab in User Settings modal
    pub user_settings_tab: SettingsTab,
    /// Set of collapsed category IDs
    pub collapsed_categories: HashSet<String>,
    /// Logged in user profile
    pub current_user: CurrentUserProfile,
    /// List of user's guilds
    pub guilds: Vec<GuildItemState>,
    /// List of members in currently selected guild
    pub members: Vec<MemberItemState>,
    /// Global or channel search query
    pub search_query: String,
    /// Whether the search flyout sidebar is open
    pub is_search_open: bool,
    /// Grouped search results
    pub search_results: Vec<crate::chat::search::SearchResultGroup>,
    /// Search executing flag
    pub is_searching: bool,
    /// Target message Snowflake ID to focus/scroll to in chat
    pub target_message_id: Option<String>,
    /// Real-time coalesced typing tracker: channel_id -> user_id -> (username, timestamp)
    pub typing_tracker: HashMap<String, HashMap<String, (String, std::time::Instant)>>,

    // Real-Time Discord Gateway & Chat State
    pub is_connected_to_gateway: bool,
    pub auth_token: Option<String>,
    pub connection_status: String,
    pub connection_mode: ConnectionMode,
    pub client_manager: Option<ClientManager>,
    pub messages_by_channel: HashMap<String, Vec<Message>>,
    pub active_draft: String,
    pub replying_to_message: Option<Message>,
    pub typing_users: Vec<String>,
    pub token_input: String,
    pub is_user_token: bool,
    pub save_token_to_disk: bool,
    pub auth_error: Option<String>,
    pub is_connecting: bool,
    /// Direct Message (DM) channels
    pub dm_channels: Vec<ChannelItemState>,
    /// User's guild folders grouping
    pub guild_folders: Vec<crate::protocol::GuildFolder>,
    /// Set of expanded folder IDs in guild rail
    pub expanded_folders: std::collections::HashSet<String>,
    /// Friends / user relationships list
    pub friends: Vec<FriendItemState>,
    /// Active tab in Friends Hub
    pub friends_tab: FriendsTab,
    /// Active subtab in Message Requests
    pub message_requests_subtab: MessageRequestsSubTab,
    /// Direct message requests (Pending & Spam)
    pub message_requests: Vec<MessageRequestItem>,
    /// Rich Presence activities in Active Now panel
    pub active_now_items: Vec<ActiveNowItem>,
    /// Telemetry & Parity monitoring handle
    pub telemetry: Option<crate::telemetry::TelemetryHandle>,

    // Advanced User Settings & Account Subsystems
    pub nitro_tier: NitroTier,
    pub active_subscription: Option<Subscription>,
    pub server_boosts: Vec<ServerBoost>,
    pub connections: Vec<AccountConnection>,
    pub authorized_apps: Vec<AuthorizedApp>,
    pub family_center_links: Vec<FamilyCenterLink>,
    pub theme_choice: ThemeChoice,
    pub message_display_mode: MessageDisplayMode,
    pub accessibility_settings: AccessibilitySettings,
    pub media_cache: crate::protocol::media_cache::MediaCacheManager,
}

fn generate_seed_messages() -> HashMap<String, Vec<Message>> {
    let mut messages = HashMap::new();
    let now = Utc::now();

    let ferran = Author::new("user-ferran", "ferran_rs");
    let ana = Author::new("user-ana", "ana_code");
    let carlos = Author::new("user-carlos", "carlos_systems");
    let alice = Author::new("user-alice", "alice_tokio");
    let mod_bot = Author {
        id: "user-bot-mod".into(),
        username: "rust_bot".into(),
        global_name: Some("Rust Bot".into()),
        avatar: None,
        bot: true,
    };

    // chan-general
    let m1 = Message::new(
        "msg-gen-1",
        "chan-general",
        ferran.clone(),
        "Olá pessoal! Sejam muito bem-vindos ao **Discord RS**, nosso cliente nativo construído 100% em **Rust** com renderização por GPU no **GPUI**! 🦀🚀",
    )
    .with_timestamp(now - Duration::minutes(45))
    .with_reactions(vec![
        Reaction::new("🦀", 8, true),
        Reaction::new("❤️", 5, false),
        Reaction::new("🚀", 7, false),
    ]);

    let m2 = Message::new(
        "msg-gen-2",
        "chan-general",
        ana.clone(),
        "A velocidade de renderização do GPUI usando aceleração por GPU Vulkan é impressionante! Frames cravados em 120fps no Wayland com latência sub-milissegundo.",
    )
    .with_timestamp(now - Duration::minutes(32))
    .with_reactions(vec![Reaction::new("🔥", 6, false)]);

    let m3 = Message::new(
        "msg-gen-3",
        "chan-general",
        carlos.clone(),
        "Como está estruturada a camada de sincronização entre o Tokio async runtime e os elementos visuais do GPUI?",
    )
    .with_timestamp(now - Duration::minutes(15));

    let m4 = Message::new(
        "msg-gen-4",
        "chan-general",
        alice.clone(),
        "Usamos um bridge com `tokio::sync::broadcast` canalizando eventos do Gateway diretamente para models observáveis no GPUI. Zero bloqueio no thread de interface!\n```rust\nlet client = DiscordApiClient::new(token);\nclient.connect().await?;\n```",
    )
    .with_timestamp(now - Duration::minutes(8))
    .with_reactions(vec![Reaction::new("💡", 4, true), Reaction::new("👏", 3, false)]);

    messages.insert("chan-general".into(), vec![m1, m2, m3, m4]);

    // chan-rules
    let r1 = Message::new(
        "msg-rules-1",
        "chan-rules",
        mod_bot.clone(),
        "📜 **Diretrizes da Comunidade Rust Brasil**\n\n1. Respeite todos os membros da comunidade.\n2. Mantenha discussões técnicas produtivas nos canais apropriados.\n3. Sempre use blocos de código com syntax highlighting para compartilhar código.\n4. Divirta-se construindo software veloz e seguro em Rust!",
    )
    .with_timestamp(now - Duration::hours(48));
    messages.insert("chan-rules".into(), vec![r1]);

    // chan-help
    let h1 = Message::new(
        "msg-help-1",
        "chan-help",
        Author::new("user-gabriel", "gabriel_dev"),
        "Alguém tem um exemplo prático de como implementar streaming de áudio Opus com PipeWire no Linux?",
    )
    .with_timestamp(now - Duration::minutes(25));

    let h2 = Message::new(
        "msg-help-2",
        "chan-help",
        ferran.clone(),
        "Com certeza! O PipeWire opera nativamente a 48.000 Hz, que é a taxa exata do codec Opus do Discord. Você pode usar a crate `cpal` ou ler diretamente os streams via D-Bus portal.",
    )
    .with_timestamp(now - Duration::minutes(20));

    messages.insert("chan-help".into(), vec![h1, h2]);

    // chan-showcase
    let s1 = Message::new(
        "msg-show-1",
        "chan-showcase",
        Author::new("user-marcos", "marcos_rs"),
        "Acabei de publicar a primeira versão do cliente Discord em Rust! Toda a UI nativa desenhada pelo GPUI.",
    )
    .with_timestamp(now - Duration::minutes(10))
    .with_attachments(vec![Attachment {
        id: "att-1".into(),
        filename: "discord_rs_gpui_preview.png".into(),
        size: 245100,
        url: "https://raw.githubusercontent.com/rust-lang/rust-artwork/master/logo/rust-logo-512x512.png".into(),
        proxy_url: None,
        content_type: Some("image/png".into()),
        width: Some(512),
        height: Some(512),
    }]);
    messages.insert("chan-showcase".into(), vec![s1]);
    messages
}

/// Groups channels into their real Discord categories based on category headers (ChannelType::Category),
/// parent category IDs, and position ordering.
fn organize_channels_into_categories(
    guild_id: &str,
    channels: Vec<crate::domain::Channel>,
) -> Vec<CategoryItemState> {
    // 1. Separar cabeçalhos de categoria (ChannelType::Category) e canais normais
    let mut category_headers = Vec::new();
    let mut normal_channels = Vec::new();

    for c in channels {
        if c.channel_type == ChannelType::Category {
            category_headers.push(c);
        } else {
            normal_channels.push(c);
        }
    }

    // Ordenar cabeçalhos de categoria por position oficial do Discord
    category_headers.sort_by_key(|c| c.position);

    // Canais sem category_id (órfãos / topo) e canais agrupados por category_id
    let mut top_channels = Vec::new();
    let mut channels_by_category: HashMap<String, Vec<crate::domain::Channel>> = HashMap::new();

    for c in normal_channels {
        if let Some(ref cat_id) = c.category_id {
            channels_by_category
                .entry(cat_id.clone())
                .or_default()
                .push(c);
        } else {
            top_channels.push(c);
        }
    }

    // Ordenar canais dentro de cada categoria por position oficial
    top_channels.sort_by_key(|c| c.position);
    for list in channels_by_category.values_mut() {
        list.sort_by_key(|c| c.position);
    }

    let to_item_state = |c: crate::domain::Channel| -> ChannelItemState {
        ChannelItemState {
            id: c.id,
            name: c.name,
            kind: match c.channel_type {
                ChannelType::Voice => ChannelKind::Voice,
                ChannelType::Announcement => ChannelKind::Announcement,
                _ => ChannelKind::Text,
            },
            topic: c.topic,
            unread: false,
            mention_count: 0,
            icon_url: c.icon_url,
            is_pinned: c.is_pinned,
            last_message_id: c.last_message_id,
            is_group_dm: c.channel_type == ChannelType::GroupDm,
            recipient_ids: Vec::new(),
            recipient_names: Vec::new(),
        }
    };

    let mut result_categories = Vec::new();

    // Canais com category_id.is_none() ficam em uma categoria raiz 'GERAL' no topo
    if !top_channels.is_empty() {
        result_categories.push(CategoryItemState {
            id: format!("cat-{}-root", guild_id),
            name: "GERAL".to_string(),
            channels: top_channels.into_iter().map(to_item_state).collect(),
        });
    }

    // Adicionar categorias reais ordenadas
    for cat in category_headers {
        let cat_id = cat.id.clone();
        let cat_name = cat.name.to_uppercase();
        let cat_channels = channels_by_category.remove(&cat_id).unwrap_or_default();
        result_categories.push(CategoryItemState {
            id: cat_id,
            name: cat_name,
            channels: cat_channels.into_iter().map(to_item_state).collect(),
        });
    }

    // Fallback: caso haja canais com category_id que não corresponda a nenhum cabeçalho existente
    for (cat_id, remaining) in channels_by_category {
        if !remaining.is_empty() {
            result_categories.push(CategoryItemState {
                id: cat_id,
                name: "OUTROS".to_string(),
                channels: remaining.into_iter().map(to_item_state).collect(),
            });
        }
    }

    result_categories
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let rust_brasil = GuildItemState {
            id: "guild-rust-br".into(),
            name: "Rust Brasil".into(),
            acronym: "RB".into(),
            icon_url: None,
            icon_hash: None,
            unread: false,
            mention_count: 3,
            categories: vec![
                CategoryItemState {
                    id: "cat-br-welcome".into(),
                    name: "BOAS-VINDAS".into(),
                    channels: vec![
                        ChannelItemState {
                            id: "chan-rules".into(),
                            name: "regras".into(),
                            kind: ChannelKind::Announcement,
                            topic: Some("Diretrizes da comunidade Rust Brasil".into()),
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                        ChannelItemState {
                            id: "chan-announcements".into(),
                            name: "anúncios".into(),
                            kind: ChannelKind::Announcement,
                            topic: Some("Novidades oficiais e eventos".into()),
                            unread: true,
                            mention_count: 1,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                    ],
                },
                CategoryItemState {
                    id: "cat-br-text".into(),
                    name: "CANAIS DE TEXTO".into(),
                    channels: vec![
                        ChannelItemState {
                            id: "chan-general".into(),
                            name: "geral".into(),
                            kind: ChannelKind::Text,
                            topic: Some("Canal principal para discussões de Rust".into()),
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                        ChannelItemState {
                            id: "chan-help".into(),
                            name: "dúvidas".into(),
                            kind: ChannelKind::Text,
                            topic: Some("Tire suas dúvidas de código, lifetimes e borrow checker".into()),
                            unread: false,
                            mention_count: 2,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                        ChannelItemState {
                            id: "chan-showcase".into(),
                            name: "projetos".into(),
                            kind: ChannelKind::Text,
                            topic: Some("Mostre seus projetos criados em Rust!".into()),
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                    ],
                },
                CategoryItemState {
                    id: "cat-br-voice".into(),
                    name: "CANAIS DE VOZ".into(),
                    channels: vec![
                        ChannelItemState {
                            id: "chan-voice-lounge".into(),
                            name: "Bate-papo Geral".into(),
                            kind: ChannelKind::Voice,
                            topic: None,
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                        ChannelItemState {
                            id: "chan-voice-pair".into(),
                            name: "Pair Programming 1".into(),
                            kind: ChannelKind::Voice,
                            topic: None,
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                    ],
                },
            ],
            emojis: vec![
                crate::protocol::media_cache::DiscordEmoji::new_custom("1001", "ferris", false, Some("guild-rust-br".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("1002", "rustacean", false, Some("guild-rust-br".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("1003", "crab_party", true, Some("guild-rust-br".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("1004", "cargo_ship", false, Some("guild-rust-br".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("1005", "unsafe_code", false, Some("guild-rust-br".into())),
            ],
            stickers: vec![
                crate::protocol::media_cache::DiscordSticker {
                    id: "781291131828699156".into(),
                    name: "Ferris Wave".into(),
                    description: Some("Ferris dando tchauzinho".into()),
                    tags: "ferris,wave,hi".into(),
                    format_type: crate::protocol::media_cache::StickerFormatType::Png,
                    available: true,
                    guild_id: Some("guild-rust-br".into()),
                    pack_id: None,
                },
                crate::protocol::media_cache::DiscordSticker {
                    id: "781291131828699157".into(),
                    name: "Ferris Hype".into(),
                    description: Some("Ferris comemorando".into()),
                    tags: "ferris,hype,party".into(),
                    format_type: crate::protocol::media_cache::StickerFormatType::Png,
                    available: true,
                    guild_id: Some("guild-rust-br".into()),
                    pack_id: None,
                },
            ],
        };

        let gpui_hub = GuildItemState {
            id: "guild-gpui-hub".into(),
            name: "GPUI Hub".into(),
            acronym: "GH".into(),
            icon_url: None,
            icon_hash: None,
            unread: true,
            mention_count: 0,
            categories: vec![
                CategoryItemState {
                    id: "cat-gh-text".into(),
                    name: "GPUI DEVELOPMENT".into(),
                    channels: vec![
                        ChannelItemState {
                            id: "chan-gpui-chat".into(),
                            name: "general-gpui".into(),
                            kind: ChannelKind::Text,
                            topic: Some("Building native Rust GUI applications with GPUI".into()),
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                        ChannelItemState {
                            id: "chan-gpui-showcase".into(),
                            name: "apps-showcase".into(),
                            kind: ChannelKind::Text,
                            topic: Some("Share apps built with GPUI".into()),
                            unread: true,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                        },
                    ],
                },
                CategoryItemState {
                    id: "cat-gh-voice".into(),
                    name: "VOICE ROOMS".into(),
                    channels: vec![ChannelItemState {
                        id: "chan-gpui-stage".into(),
                        name: "GPUI Community Stage".into(),
                        kind: ChannelKind::Voice,
                        topic: None,
                        unread: false,
                        mention_count: 0,
                        icon_url: None,
                        is_pinned: false,
                        last_message_id: None,
                        is_group_dm: false,
                        recipient_ids: Vec::new(),
                        recipient_names: Vec::new(),
                    }],
                },
            ],
            emojis: vec![
                crate::protocol::media_cache::DiscordEmoji::new_custom("2001", "zed_editor", false, Some("guild-gpui-hub".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("2002", "gpu_accel", false, Some("guild-gpui-hub".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("2003", "pixel_perfect", false, Some("guild-gpui-hub".into())),
                crate::protocol::media_cache::DiscordEmoji::new_custom("2004", "fps_144", true, Some("guild-gpui-hub".into())),
            ],
            stickers: vec![
                crate::protocol::media_cache::DiscordSticker {
                    id: "781291131828699158".into(),
                    name: "GPUI Rocket".into(),
                    description: Some("Foguete GPUI voando a 144fps".into()),
                    tags: "gpui,fast,rocket".into(),
                    format_type: crate::protocol::media_cache::StickerFormatType::Png,
                    available: true,
                    guild_id: Some("guild-gpui-hub".into()),
                    pack_id: None,
                },
            ],
        };

        let members = vec![
            MemberItemState {
                id: "user-rustdev".into(),
                name: "RustDev".into(),
                avatar_text: "R".into(),
                presence: PresenceStatus::Online,
                role_group: "ADMINISTRADOR".into(),
                role_color: 0x5865f2,
                is_bot: false,
                activity: Some("Codando discord-rs".into()),
            },
            MemberItemState {
                id: "user-rustacean".into(),
                name: "Rustacean".into(),
                avatar_text: "R".into(),
                presence: PresenceStatus::Online,
                role_group: "MODERADOR".into(),
                role_color: 0x23a55a,
                is_bot: false,
                activity: Some("Jogando cargo run".into()),
            },
            MemberItemState {
                id: "user-gpui-bot".into(),
                name: "GPUI Bot".into(),
                avatar_text: "⚡".into(),
                presence: PresenceStatus::Online,
                role_group: "BOTS".into(),
                role_color: 0x5865f2,
                is_bot: true,
                activity: Some("/help para comandos".into()),
            },
            MemberItemState {
                id: "user-dev-idle".into(),
                name: "AsyncDev".into(),
                avatar_text: "A".into(),
                presence: PresenceStatus::Idle,
                role_group: "MEMBROS".into(),
                role_color: 0xdbdee1,
                is_bot: false,
                activity: Some("Compilando crates".into()),
            },
            MemberItemState {
                id: "user-borrow-checker".into(),
                name: "BorrowChecker".into(),
                avatar_text: "B".into(),
                presence: PresenceStatus::DoNotDisturb,
                role_group: "MEMBROS".into(),
                role_color: 0xdbdee1,
                is_bot: false,
                activity: Some("Rejeitando referências".into()),
            },
            MemberItemState {
                id: "user-tokio-fan".into(),
                name: "TokioFan".into(),
                avatar_text: "T".into(),
                presence: PresenceStatus::Offline,
                role_group: "MEMBROS".into(),
                role_color: 0x80848e,
                is_bot: false,
                activity: None,
            },
        ];

        let token_storage = TokenStorage::default();
        let (stored_token, is_user) = match token_storage.load() {
            Ok(Some(t)) => (t.secret().to_string(), t.token_type == crate::protocol::TokenType::User),
            _ => (String::new(), false),
        };
        let has_token = !stored_token.is_empty();

        let cached_cache = load_guilds_from_disk();
        let (mut guilds, guild_folders) = if let Some(cache) = cached_cache {
            (cache.guilds, cache.guild_folders)
        } else {
            (vec![rust_brasil, gpui_hub], Vec::new())
        };

        if !guild_folders.is_empty() {
            let mut order_map = HashMap::new();
            let mut idx = 0;
            for folder in &guild_folders {
                for gid in &folder.guild_ids {
                    order_map.entry(gid.clone()).or_insert(idx);
                    idx += 1;
                }
            }
            guilds.sort_by_key(|g| {
                order_map.get(&g.id).copied().unwrap_or(usize::MAX)
            });
        }

        let selected_guild_id = guilds.first().map(|g| g.id.clone());
        let selected_channel_id = guilds.first().and_then(|g| {
            for cat in &g.categories {
                if let Some(ch) = cat.channels.iter().find(|c| c.kind == ChannelKind::Text) {
                    return Some(ch.id.clone());
                }
            }
            g.categories.first().and_then(|c| c.channels.first().map(|ch| ch.id.clone()))
        });

        let is_connecting = has_token;
        let connection_status = if has_token {
            "Conectando ao Discord...".to_string()
        } else {
            "Modo Demonstração (Offline)".to_string()
        };

        let messages_by_channel = generate_seed_messages();

        let mut state = Self {
            selected_guild_id,
            selected_channel_id,
            active_voice_call: None,
            show_members: true,
            active_modal: None,
            user_settings_tab: SettingsTab::MyAccount,
            collapsed_categories: HashSet::new(),
            current_user: CurrentUserProfile {
                id: "user-me".into(),
                username: "vanguard_dev".into(),
                display_name: "Vanguard".into(),
                discriminator: Some("0001".into()),
                avatar_text: "V".into(),
                avatar_url: None,
                custom_status: Some("Navegando no Discord RS 🦀".into()),
                presence: PresenceStatus::Online,
                is_muted: false,
                is_deafened: false,
            },
            guilds,
            members: if has_token { Vec::new() } else { members },
            search_query: String::new(),
            is_search_open: false,
            search_results: Vec::new(),
            is_searching: false,
            target_message_id: None,
            typing_tracker: HashMap::new(),
            is_connected_to_gateway: false,
            auth_token: if has_token { Some(stored_token.clone()) } else { None },
            connection_status,
            connection_mode: ConnectionMode::Mock,
            client_manager: None,
            messages_by_channel,
            active_draft: String::new(),
            replying_to_message: None,
            typing_users: Vec::new(),
            token_input: stored_token,
            is_user_token: is_user,
            save_token_to_disk: true,
            auth_error: None,
            is_connecting,
            dm_channels: Vec::new(),
            guild_folders,
            expanded_folders: std::collections::HashSet::new(),
            friends: if has_token {
                Vec::new()
            } else {
                vec![
                    FriendItemState {
                        id: "user-ana".into(),
                        username: "ana".into(),
                        display_name: "Ana Dev".into(),
                        avatar_url: None,
                        avatar_text: "A".into(),
                        presence: PresenceStatus::Online,
                        custom_status: Some("Testando GPUI".into()),
                        relationship_type: 1,
                    },
                ]
            },
            friends_tab: FriendsTab::Online,
            message_requests_subtab: MessageRequestsSubTab::YouMayKnow,
            message_requests: if has_token {
                Vec::new()
            } else {
                vec![
                    MessageRequestItem {
                        id: "req-1".into(),
                        user_id: "user-guilherme".into(),
                        username: "guilherme_dev".into(),
                        display_name: "Guilherme Santos".into(),
                        avatar_url: None,
                        avatar_text: "G".into(),
                        presence: PresenceStatus::Online,
                        mutual_guilds: 1,
                        mutual_guild_name: Some("Rust Brasil".into()),
                        preview_message: "Opa! Vi seus commits no discord-rs com GPUI, muito top! Queria tirar uma dúvida sobre async tokio.".into(),
                        timestamp: "Hoje às 18:24".into(),
                        is_spam: false,
                    },
                    MessageRequestItem {
                        id: "req-2".into(),
                        user_id: "user-roberto".into(),
                        username: "roberto_sys".into(),
                        display_name: "Roberto Almeida".into(),
                        avatar_text: "R".into(),
                        avatar_url: None,
                        presence: PresenceStatus::Idle,
                        mutual_guilds: 2,
                        mutual_guild_name: Some("Rust Brasil".into()),
                        preview_message: "E aí, tudo bem? Você usa Wayland ou X11 no teste de rendering do GPUI?".into(),
                        timestamp: "Ontem às 21:10".into(),
                        is_spam: false,
                    },
                    MessageRequestItem {
                        id: "req-spam-1".into(),
                        user_id: "user-scam-bot".into(),
                        username: "free_nitro_bot_99".into(),
                        display_name: "Free Nitro 2026".into(),
                        avatar_text: "🎁".into(),
                        avatar_url: None,
                        presence: PresenceStatus::Offline,
                        mutual_guilds: 0,
                        mutual_guild_name: None,
                        preview_message: "Parabéns! Você ganhou 3 meses de Discord Nitro grátis! Clique para resgatar: http://discrod-free-nitro.xyz/gift".into(),
                        timestamp: "Há 3 dias".into(),
                        is_spam: true,
                    },
                ]
            },
            active_now_items: Vec::new(),
            telemetry: Some(crate::telemetry::global().clone()),

            nitro_tier: NitroTier::Full,
            active_subscription: Some(Subscription {
                id: "sub_nitro_monthly_premium".into(),
                tier: NitroTier::Full,
                status: "active".into(),
                current_period_end: Utc::now() + Duration::days(28),
                plan_name: "Discord Nitro Mensal".into(),
                payment_gateway: "Stripe".into(),
            }),
            server_boosts: vec![
                ServerBoost {
                    id: "boost_01".into(),
                    guild_id: "guild-rust-br".into(),
                    guild_name: "Rust Brasil".into(),
                    guild_icon: None,
                    cooldown_ends: Utc::now() + Duration::days(4),
                },
                ServerBoost {
                    id: "boost_02".into(),
                    guild_id: "guild-rust-br".into(),
                    guild_name: "Rust Brasil".into(),
                    guild_icon: None,
                    cooldown_ends: Utc::now() + Duration::days(4),
                },
            ],
            connections: vec![
                AccountConnection {
                    platform: "github".into(),
                    id: "conn-gh-01".into(),
                    name: "smcodes".into(),
                    verified: true,
                    show_on_profile: true,
                    show_activity: false,
                },
                AccountConnection {
                    platform: "spotify".into(),
                    id: "conn-sp-01".into(),
                    name: "Ferran Vanguard".into(),
                    verified: true,
                    show_on_profile: true,
                    show_activity: true,
                },
                AccountConnection {
                    platform: "steam".into(),
                    id: "conn-st-01".into(),
                    name: "vanguard_rust".into(),
                    verified: true,
                    show_on_profile: true,
                    show_activity: true,
                },
            ],
            authorized_apps: vec![
                AuthorizedApp {
                    id: "app-midjourney".into(),
                    name: "Midjourney".into(),
                    icon_url: None,
                    scopes: vec!["identify".into(), "guilds".into(), "bot".into()],
                    authorized_at: Utc::now() - Duration::days(32),
                },
                AuthorizedApp {
                    id: "app-github".into(),
                    name: "GitHub Integration".into(),
                    icon_url: None,
                    scopes: vec!["identify".into(), "connections".into(), "email".into()],
                    authorized_at: Utc::now() - Duration::days(14),
                },
                AuthorizedApp {
                    id: "app-spotify-bot".into(),
                    name: "Spotify Listen Along".into(),
                    icon_url: None,
                    scopes: vec!["identify".into(), "rpc".into(), "connections".into()],
                    authorized_at: Utc::now() - Duration::days(9),
                },
            ],
            family_center_links: vec![
                FamilyCenterLink {
                    id: "fc-01".into(),
                    user_id: "user-teen-01".into(),
                    username: "Lucas Vanguard".into(),
                    is_guardian: false,
                    linked_at: Utc::now() - Duration::days(60),
                },
            ],
            theme_choice: ThemeChoice::Dark,
            message_display_mode: MessageDisplayMode::Cozy,
            accessibility_settings: AccessibilitySettings::default(),
            media_cache: crate::protocol::media_cache::MediaCacheManager::default_path(),
        };
        state.sync_guild_media();
        state
    }

    /// Selects active tab in Friends Hub (Online, All, Pending, Blocked, AddFriend, MessageRequests)
    pub fn select_friends_tab(&mut self, tab: FriendsTab) {
        self.friends_tab = tab;
    }

    /// Selects subtab in Message Requests view (YouMayKnow or Spam)
    pub fn select_message_requests_subtab(&mut self, subtab: MessageRequestsSubTab) {
        self.message_requests_subtab = subtab;
    }

    /// Accepts an incoming message request, creating a DM channel and navigating to it
    pub fn accept_message_request(&mut self, request_id: &str) {
        if let Some(pos) = self.message_requests.iter().position(|r| r.id == request_id) {
            let req = self.message_requests.remove(pos);
            self.open_dm_with_user(&req.user_id, &req.display_name);
        }
    }

    /// Ignores/dismisses an incoming message request
    pub fn ignore_message_request(&mut self, request_id: &str) {
        self.message_requests.retain(|r| r.id != request_id);
    }

    /// Clears all flagged spam message requests
    pub fn clear_spam_requests(&mut self) {
        self.message_requests.retain(|r| !r.is_spam);
    }

    /// Selects a guild (or None for DMs / Home)
    pub fn select_guild(&mut self, guild_id: Option<String>) {
        if let Some(ref tel) = self.telemetry {
            let guild_name = guild_id
                .as_ref()
                .and_then(|gid| self.guilds.iter().find(|g| &g.id == gid).map(|g| g.name.clone()))
                .unwrap_or_else(|| "Home / DMs".into());
            let mut meta = std::collections::HashMap::new();
            meta.insert("guild_id".into(), guild_id.clone().unwrap_or_else(|| "home".into()));
            meta.insert("guild_name".into(), guild_name.clone());

            let screen = if let Some(ref gid) = guild_id {
                crate::telemetry::ScreenKind::Guild {
                    id: gid.clone(),
                    name: guild_name.clone(),
                }
            } else {
                crate::telemetry::ScreenKind::FriendsHub
            };
            let guard = tel.start_screen_transition(screen, meta);
            guard.complete(None);
            tel.track_guild_select(guild_id.clone().unwrap_or_else(|| "home".into()), guild_name, 0);
        }

        self.selected_guild_id = guild_id;
        if let Some(ref gid) = self.selected_guild_id {
            let gid_clone = gid.clone();
            let mut categories_empty = false;
            let mut first_text_channel_id = None;

            if let Some(guild) = self.guilds.iter().find(|g| &g.id == &gid_clone) {
                if guild.categories.is_empty() {
                    categories_empty = true;
                } else {
                    // Troca instantânea de servidor! Não faz requisição de rede se categories não estiver vazio
                    for cat in &guild.categories {
                        if let Some(ch) = cat.channels.iter().find(|c| c.kind == ChannelKind::Text) {
                            first_text_channel_id = Some(ch.id.clone());
                            break;
                        }
                    }
                    if first_text_channel_id.is_none() {
                        if let Some(ch) = guild.categories.first().and_then(|c| c.channels.first()) {
                            first_text_channel_id = Some(ch.id.clone());
                        }
                    }
                }
            }

            if let Some(cid) = first_text_channel_id {
                self.select_channel(cid);
            } else if categories_empty {
                self.selected_channel_id = None;
                // Carregamento sob demanda (lazy loading) dos canais da guild
                if let Some(ref cm) = self.client_manager {
                    let cm = cm.clone();
                    let gid_fetch = gid_clone;
                    tokio::spawn(async move {
                        match cm.list_channels_for_guild(&gid_fetch).await {
                            Ok(channels) => {
                                cm.emit_event(crate::protocol::TransportEvent::GuildChannelsUpdated {
                                    guild_id: gid_fetch,
                                    channels,
                                });
                            }
                            Err(e) => {
                                log::warn!("Falha ao carregar canais sob demanda para guild {}: {:?}", gid_fetch, e);
                            }
                        }
                    });
                }
            }
        } else {
            // Modo Direct Messages / Home
            self.sort_dm_channels();
            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                tokio::spawn(async move {
                    match cm.list_dm_channels().await {
                        Ok(channels) => {
                            cm.emit_event(crate::protocol::TransportEvent::DmChannelsUpdated(channels));
                        }
                        Err(e) => {
                            log::warn!("Falha ao carregar canais de DM sob demanda: {:?}", e);
                        }
                    }
                    match cm.list_relationships().await {
                        Ok(relationships) => {
                            cm.emit_event(crate::protocol::TransportEvent::RelationshipsUpdated(relationships));
                        }
                        Err(e) => {
                            log::warn!("Falha ao carregar relacionamentos/amigos sob demanda: {:?}", e);
                        }
                    }
                });
            }

            if let Some(first_dm) = self.dm_channels.first() {
                self.select_channel(first_dm.id.clone());
            } else {
                self.selected_channel_id = None;
            }
        }
    }

    /// Sorts DMs according to Discord rules:
    /// 1. Pinned DMs (is_pinned: true) at the top.
    /// 2. Channels with the most recent messages closest to the top (last_message_id descending Snowflake).
    pub fn sort_dm_channels(&mut self) {
        self.dm_channels.sort_by(|a, b| {
            // 1. DMs fixadas no topo (is_pinned: true vem antes de false)
            match (a.is_pinned, b.is_pinned) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }

            // 2. Mensagem mais recente mais perto do topo (decrescente por Snowflake)
            match (&a.last_message_id, &b.last_message_id) {
                (Some(id_a), Some(id_b)) => {
                    if let (Ok(na), Ok(nb)) = (id_a.parse::<u64>(), id_b.parse::<u64>()) {
                        nb.cmp(&na)
                    } else {
                        id_b.cmp(id_a)
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
    }

    /// Selects a channel
    pub fn select_channel(&mut self, channel_id: String) {
        if let Some(ref tel) = self.telemetry {
            let (ch_name, ch_kind) = self
                .find_channel(&channel_id)
                .map(|(_, ch)| (ch.name.clone(), ch.kind))
                .unwrap_or_else(|| (channel_id.clone(), ChannelKind::Text));
            let kind_str = match ch_kind {
                ChannelKind::Voice => "voice",
                ChannelKind::Announcement => "announcement",
                ChannelKind::Text => "text",
            };
            let mut meta = std::collections::HashMap::new();
            meta.insert("channel_id".into(), channel_id.clone());
            meta.insert("channel_name".into(), ch_name.clone());
            meta.insert("kind".into(), kind_str.into());

            let screen = match ch_kind {
                ChannelKind::Voice => crate::telemetry::ScreenKind::VoiceChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
                ChannelKind::Announcement => crate::telemetry::ScreenKind::AnnouncementChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
                ChannelKind::Text => crate::telemetry::ScreenKind::TextChannel {
                    id: channel_id.clone(),
                    name: ch_name.clone(),
                    guild_id: self.selected_guild_id.clone(),
                },
            };
            let guard = tel.start_screen_transition(screen, meta);
            guard.complete(None);
            tel.track_channel_select(channel_id.clone(), ch_name, kind_str, 0);
        }

        let guild_name = self
            .get_selected_guild()
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Servidor".into());

        let voice_to_connect = if let Some((_, channel)) = self.find_channel(&channel_id) {
            if channel.kind == ChannelKind::Voice {
                Some((channel.id.clone(), channel.name.clone(), guild_name))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((cid, cname, gname)) = voice_to_connect {
            self.connect_voice(&cid, &cname, &gname);
        }
        self.selected_channel_id = Some(channel_id.clone());
        self.typing_users = self.get_active_typing_users(&channel_id);

        // Se houver ClientManager (seja Mock ou Discord API), resolver canal de DM temporário sob demanda
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let cid = channel_id.clone();
            if cid.starts_with("dm-") && !cid.starts_with("dm-group-") {
                let uid = cid.trim_start_matches("dm-").to_string();
                let temp_id = cid.clone();
                tokio::spawn(async move {
                    if let Ok(ch) = cm.create_or_get_dm_channel(&uid).await {
                        cm.emit_event(TransportEvent::DmChannelResolved {
                            temp_id,
                            channel: ch,
                        });
                    }
                });
            } else if self.is_connected_to_gateway {
                let has_messages = self.messages_by_channel.get(&cid).map_or(false, |msgs| !msgs.is_empty());
                if !has_messages {
                    tokio::spawn(async move {
                        if let Ok(mut msgs) = cm.get_messages(&cid, 50).await {
                            // Assegurar ordenação cronológica ascendente (mais antiga para mais nova)
                            msgs.sort_by_key(|m| m.timestamp);
                            cm.emit_event(TransportEvent::ChannelMessagesLoaded {
                                channel_id: cid,
                                messages: msgs,
                            });
                        }
                    });
                }
            }
        }
    }

    /// Toggles collapse on a channel category
    pub fn toggle_category(&mut self, category_id: &str) {
        if self.collapsed_categories.contains(category_id) {
            self.collapsed_categories.remove(category_id);
        } else {
            self.collapsed_categories.insert(category_id.to_string());
        }
    }

    /// Checks if a category is collapsed
    pub fn is_category_collapsed(&self, category_id: &str) -> bool {
        self.collapsed_categories.contains(category_id)
    }

    /// Toggles expanded state of a guild folder in the guild rail
    pub fn toggle_folder(&mut self, folder_id: &str) {
        if self.expanded_folders.contains(folder_id) {
            self.expanded_folders.remove(folder_id);
        } else {
            self.expanded_folders.insert(folder_id.to_string());
        }
    }

    /// Checks if a guild folder is expanded
    pub fn is_folder_expanded(&self, folder_id: &str) -> bool {
        self.expanded_folders.contains(folder_id)
    }

    /// Toggles member sidebar visibility
    pub fn toggle_members(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_members", "Toggle Member Sidebar", 0);
        }
        self.show_members = !self.show_members;
    }

    /// Opens a modal dialog
    pub fn open_modal(&mut self, modal: ActiveModal) {
        if let Some(ref tel) = self.telemetry {
            let screen = match modal {
                ActiveModal::UserSettings => crate::telemetry::ScreenKind::UserSettingsModal {
                    tab: self.user_settings_tab.title().into(),
                },
                ActiveModal::AddServer => crate::telemetry::ScreenKind::AddServerModal,
                ActiveModal::ExploreServers => crate::telemetry::ScreenKind::ExploreServersModal,
                ActiveModal::Login | ActiveModal::LoginDiscord => crate::telemetry::ScreenKind::LoginModal,
                ActiveModal::AddFriendToGroup(_) | ActiveModal::CreateGroupDm => {
                    crate::telemetry::ScreenKind::FriendsHub
                }
            };
            let mut meta = std::collections::HashMap::new();
            meta.insert("modal".into(), format!("{:?}", modal));
            let guard = tel.start_screen_transition(screen, meta);
            guard.complete(None);
            tel.track_button_click("open_modal", format!("{:?}", modal), 0);
        }
        self.active_modal = Some(modal);
    }

    /// Closes the active modal
    pub fn close_modal(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("close_modal", "Fechar Modal", 0);
        }
        self.active_modal = None;
    }

    /// Sets the active settings tab
    pub fn set_settings_tab(&mut self, tab: SettingsTab) {
        if let Some(ref tel) = self.telemetry {
            tel.track_tab_change(tab.title(), 0);
            let mut meta = std::collections::HashMap::new();
            meta.insert("tab".into(), tab.title().into());
            let guard = tel.start_screen_transition(
                crate::telemetry::ScreenKind::UserSettingsModal {
                    tab: tab.title().into(),
                },
                meta,
            );
            guard.complete(None);
        }
        self.user_settings_tab = tab;
    }

    /// Sets the Nitro subscription tier
    pub fn set_nitro_tier(&mut self, tier: NitroTier) {
        self.nitro_tier = tier;
        if tier == NitroTier::None {
            self.active_subscription = None;
        } else {
            self.active_subscription = Some(Subscription {
                id: format!("sub-nitro-{}", Utc::now().timestamp()),
                tier,
                status: "active".into(),
                current_period_end: Utc::now() + Duration::days(30),
                plan_name: match tier {
                    NitroTier::Basic => "Nitro Basic Mensal (R$ 8,99)".into(),
                    NitroTier::Full => "Discord Nitro Mensal (R$ 24,99)".into(),
                    NitroTier::None => "Sem Plano".into(),
                },
                payment_gateway: "Stripe".into(),
            });
        }
    }

    /// Cancels active Nitro subscription
    pub fn cancel_nitro_subscription(&mut self) {
        self.nitro_tier = NitroTier::None;
        if let Some(ref mut sub) = self.active_subscription {
            sub.status = "canceled".into();
        }
    }

    /// Toggles showing a connected platform on public profile
    pub fn toggle_connection_profile(&mut self, platform: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.platform == platform) {
            conn.show_on_profile = !conn.show_on_profile;
        }
    }

    /// Toggles broadcasting activity (e.g. Spotify listening along, Steam gaming)
    pub fn toggle_connection_activity(&mut self, platform: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.platform == platform) {
            conn.show_activity = !conn.show_activity;
        }
    }

    /// Connects an account platform or updates existing
    pub fn connect_platform(&mut self, platform: &str, username: &str) {
        if let Some(existing) = self.connections.iter_mut().find(|c| c.platform == platform) {
            existing.name = username.to_string();
        } else {
            self.connections.push(AccountConnection {
                platform: platform.to_string(),
                id: format!("conn-{}-{}", platform, Utc::now().timestamp()),
                name: username.to_string(),
                verified: true,
                show_on_profile: true,
                show_activity: platform == "spotify" || platform == "steam",
            });
        }
    }

    /// Disconnects a linked platform
    pub fn disconnect_platform(&mut self, platform: &str) {
        self.connections.retain(|c| c.platform != platform);
    }

    /// Revokes authorization for an OAuth2 application
    pub fn revoke_authorized_app(&mut self, app_id: &str) {
        self.authorized_apps.retain(|a| a.id != app_id);
    }

    /// Sets the Discord visual theme choice
    pub fn set_theme_choice(&mut self, theme: ThemeChoice) {
        self.theme_choice = theme;
    }

    /// Sets chat message display mode (Cozy vs Compact)
    pub fn set_message_display_mode(&mut self, mode: MessageDisplayMode) {
        self.message_display_mode = mode;
    }

    /// Sets chat font size (12px to 24px)
    pub fn set_chat_font_size(&mut self, size: f32) {
        self.accessibility_settings.chat_font_size = size;
    }

    /// Sets motion reduction preference
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.accessibility_settings.reduce_motion = reduce;
    }

    /// Sets color saturation (0.0 to 1.0)
    pub fn set_color_saturation(&mut self, saturation: f32) {
        self.accessibility_settings.saturation = saturation;
    }

    /// Sets enhanced contrast preference
    pub fn set_enhanced_contrast(&mut self, contrast: bool) {
        self.accessibility_settings.high_contrast = contrast;
    }

    /// Connects to a voice call
    pub fn connect_voice(&mut self, channel_id: &str, channel_name: &str, guild_name: &str) {
        self.active_voice_call = Some(ActiveVoiceCall {
            channel_id: channel_id.to_string(),
            channel_name: channel_name.to_string(),
            guild_name: guild_name.to_string(),
            ping_ms: 18,
            is_muted: self.current_user.is_muted,
            is_deafened: self.current_user.is_deafened,
            is_screen_sharing: false,
            participants: vec![
                VoiceParticipantState {
                    id: self.current_user.id.clone(),
                    name: self.current_user.display_name.clone(),
                    avatar_text: self.current_user.avatar_text.clone(),
                    is_speaking: false,
                    is_muted: self.current_user.is_muted,
                    is_deafened: self.current_user.is_deafened,
                },
            ],
        });
    }

    /// Disconnects from current voice call
    pub fn disconnect_voice(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("disconnect_voice", "Desconectar Voz", 0);
        }
        self.active_voice_call = None;
    }

    /// Toggles self mute
    pub fn toggle_mute(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_mute", "Toggle Mute", 0);
        }
        self.current_user.is_muted = !self.current_user.is_muted;
        if let Some(ref mut call) = self.active_voice_call {
            call.is_muted = self.current_user.is_muted;
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == self.current_user.id) {
                p.is_muted = self.current_user.is_muted;
            }
        }
    }

    /// Toggles self deafen
    pub fn toggle_deafen(&mut self) {
        if let Some(ref tel) = self.telemetry {
            tel.track_button_click("toggle_deafen", "Toggle Deafen", 0);
        }
        self.current_user.is_deafened = !self.current_user.is_deafened;
        if self.current_user.is_deafened {
            self.current_user.is_muted = true;
        }
        if let Some(ref mut call) = self.active_voice_call {
            call.is_deafened = self.current_user.is_deafened;
            call.is_muted = self.current_user.is_muted;
            if let Some(p) = call.participants.iter_mut().find(|p| p.id == self.current_user.id) {
                p.is_deafened = self.current_user.is_deafened;
                p.is_muted = self.current_user.is_muted;
            }
        }
    }

    /// Gets the currently selected guild
    pub fn get_selected_guild(&self) -> Option<&GuildItemState> {
        let gid = self.selected_guild_id.as_ref()?;
        self.guilds.iter().find(|g| &g.id == gid)
    }

    /// Finds a channel by ID across guilds
    pub fn find_channel(&self, channel_id: &str) -> Option<(&GuildItemState, &ChannelItemState)> {
        for guild in &self.guilds {
            for cat in &guild.categories {
                for ch in &cat.channels {
                    if ch.id == channel_id {
                        return Some((guild, ch));
                    }
                }
            }
        }
        None
    }

    /// Gets the currently selected channel
    pub fn get_selected_channel(&self) -> Option<(&GuildItemState, &ChannelItemState)> {
        let cid = self.selected_channel_id.as_ref()?;
        self.find_channel(cid)
    }

    /// Gets the currently selected channel item (from either guilds or DM channels)
    pub fn get_active_channel_item(&self) -> Option<ChannelItemState> {
        let cid = self.selected_channel_id.as_ref()?;
        if let Some((_, ch)) = self.find_channel(cid) {
            Some(ch.clone())
        } else {
            self.dm_channels.iter().find(|d| &d.id == cid).cloned()
        }
    }

    /// Gets message history for the currently selected channel without copying
    pub fn get_active_messages(&self) -> &[Message] {
        if let Some(ref cid) = self.selected_channel_id {
            self.messages_by_channel.get(cid).map(|v| v.as_slice()).unwrap_or(&[])
        } else {
            &[]
        }
    }

    /// Injects or appends a message to a channel's history
    pub fn inject_message(&mut self, channel_id: &str, message: Message) {
        crate::chat::markdown::prewarm_markdown_cache(&message.content);
        self.messages_by_channel
            .entry(channel_id.to_string())
            .or_default()
            .push(message);
    }

    /// Adds an emoji reaction to a message in state
    pub fn add_reaction_to_message(&mut self, channel_id: &str, message_id: &str, emoji: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                if let Some(rx) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                    rx.count += 1;
                    rx.me = true;
                } else {
                    msg.reactions.push(Reaction {
                        emoji: emoji.to_string(),
                        count: 1,
                        me: true,
                    });
                }
            }
        }
    }

    /// Deletes a message by ID from the specified channel
    pub fn delete_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            messages.retain(|m| m.id != message_id);
        }
    }

    /// Toggles the pinned status of a message by ID
    pub fn pin_message(&mut self, channel_id: &str, message_id: &str) {
        if let Some(messages) = self.messages_by_channel.get_mut(channel_id) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.pinned = !msg.pinned;
            }
        }
    }

    /// Sets / synchronizes real messages received from the API/Gateway for a channel
    pub fn set_channel_messages(&mut self, channel_id: &str, mut messages: Vec<Message>) {
        for msg in &messages {
            crate::chat::markdown::prewarm_markdown_cache(&msg.content);
        }
        // Preserva quaisquer mensagens otimistas/pendentes ainda não confirmadas no histórico carregado
        if let Some(existing) = self.messages_by_channel.get(channel_id) {
            for m in existing {
                if (m.is_pending || m.id.starts_with("temp-"))
                    && !messages.iter().any(|new_m| {
                        new_m.id == m.id
                            || (new_m.content == m.content
                                && (new_m.author.id == m.author.id
                                    || new_m.author.username == m.author.username
                                    || m.author.id == "user-me"
                                    || m.author.id == self.current_user.id))
                    })
                {
                    messages.push(m.clone());
                }
            }
        }
        self.messages_by_channel.insert(channel_id.to_string(), messages);
    }

    /// Authenticates with a Discord token and updates connection status
    pub fn authenticate(&mut self, token: String, is_user: bool) {
        let trimmed = token.trim().to_string();
        if trimmed.is_empty() {
            self.auth_error = Some("O token não pode estar vazio.".to_string());
            self.is_connecting = false;
            return;
        }

        self.auth_token = Some(trimmed.clone());
        self.token_input = trimmed.clone();
        self.is_user_token = is_user;
        self.auth_error = None;

        // Quick development / offline testing bypass
        if trimmed.starts_with("mock_") || trimmed == "test" || trimmed == "demo" {
            self.is_connected_to_gateway = true;
            self.is_connecting = false;
            self.connection_mode = ConnectionMode::LiveDiscord("rustacean".to_string());
            self.connection_status = "Conectado (Ambiente de Testes)".to_string();
            self.current_user.username = "rustacean".into();
            self.current_user.display_name = "Rustacean".into();
            self.close_modal();
            return;
        }

        self.is_connecting = true;
        self.is_connected_to_gateway = false;
        self.connection_status = "Conectando ao Gateway v10...".to_string();

        if self.save_token_to_disk {
            let auth = if is_user {
                AuthToken::user(trimmed.clone())
            } else {
                AuthToken::bot(trimmed.clone())
            };
            let storage = TokenStorage::default();
            let _ = storage.save(&auth);
        }

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let auth = if is_user {
                AuthToken::user(trimmed)
            } else {
                AuthToken::bot(trimmed)
            };
            tokio::spawn(async move {
                if let Err(e) = cm.connect_with_token(auth).await {
                    log::error!("Falha ao conectar via ClientManager: {:?}", e);
                }
            });
        } else {
            self.is_connected_to_gateway = true;
            self.is_connecting = false;
            self.connection_mode = ConnectionMode::Live;
            self.connection_status = if is_user {
                "Conectado via Conta de Usuário (Gateway v10)".to_string()
            } else {
                "Conectado via Bot Discord (Gateway v10)".to_string()
            };
            self.close_modal();
            self.current_user.username = if is_user { "usuario_discord".into() } else { "bot_discord".into() };
            self.current_user.display_name = if is_user { "Usuário Conectado".into() } else { "Bot Discord".into() };
            self.current_user.custom_status = Some("Conectado ao Discord Oficial".into());
        }
    }

    /// Disconnects from live gateway
    pub fn disconnect_gateway(&mut self) {
        self.is_connected_to_gateway = false;
        self.auth_token = None;
        self.connection_mode = ConnectionMode::Mock;
        self.connection_status = "Modo Demonstração (Offline)".to_string();
        self.messages_by_channel = generate_seed_messages();
        self.dm_channels = Vec::new();

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            tokio::spawn(async move {
                let _ = cm.connect_mock().await;
            });
        }
    }

    /// Dispatches a real-time event received from the Gateway / ClientManager
    pub fn handle_transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::Ready { user, guilds, guild_folders } => {
                let display_name = user.display_name().to_string();
                let avatar_text = user.username.chars().next().unwrap_or('U').to_uppercase().to_string();
                let avatar_url = user.avatar_url();
                let username = user.username;
                self.current_user.id = user.id;
                self.current_user.username = username.clone();
                self.current_user.display_name = display_name;
                self.current_user.avatar_text = avatar_text;
                self.current_user.avatar_url = avatar_url;
                self.is_connected_to_gateway = true;
                self.is_connecting = false;
                self.connection_mode = ConnectionMode::LiveDiscord(username);
                self.connection_status = format!("Conectado como @{}", self.current_user.username);
                self.auth_error = None;
                if !guild_folders.is_empty() {
                    self.guild_folders = guild_folders;
                }
                self.close_modal();

                // Limpar mensagens fictícias para exibir apenas dados reais carregados da API
                self.messages_by_channel.clear();

                if !guilds.is_empty() {
                    let mut new_guilds = Vec::new();
                    for g in guilds {
                        let icon_url = g.icon_url();
                        let icon_hash = g.icon.clone();
                        let categories = organize_channels_into_categories(&g.id, g.channels);

                        let emojis = if !g.emojis.is_empty() {
                            let _ = self.media_cache.save_guild_emojis_to_disk(&g.id, &g.emojis);
                            g.emojis
                        } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.emojis.clone()) {
                            if !existing.is_empty() {
                                existing
                            } else {
                                self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                            }
                        } else {
                            self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                        };

                        let stickers = if !g.stickers.is_empty() {
                            let _ = self.media_cache.save_guild_stickers_to_disk(&g.id, &g.stickers);
                            g.stickers
                        } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.stickers.clone()) {
                            if !existing.is_empty() {
                                existing
                            } else {
                                self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
                            }
                        } else {
                            self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
                        };

                        new_guilds.push(GuildItemState {
                            id: g.id.clone(),
                            name: g.name.clone(),
                            acronym: g.name.chars().take(3).collect(),
                            icon_url,
                            icon_hash,
                            unread: false,
                            mention_count: 0,
                            categories,
                            emojis,
                            stickers,
                        });
                    }

                    // Ordenar com base na ordem das pastas de servidores
                    if !self.guild_folders.is_empty() {
                        let mut order_map = HashMap::new();
                        let mut idx = 0;
                        for folder in &self.guild_folders {
                            for gid in &folder.guild_ids {
                                order_map.entry(gid.clone()).or_insert(idx);
                                idx += 1;
                            }
                        }
                        new_guilds.sort_by_key(|g| {
                            order_map.get(&g.id).copied().unwrap_or(usize::MAX)
                        });
                    }

                    self.guilds = new_guilds;
                    self.sync_guild_media();
                    save_guilds_to_disk(&self.guilds, &self.guild_folders);
                    if let Some(first) = self.guilds.first() {
                        self.select_guild(Some(first.id.clone()));
                    }
                }
            }
            TransportEvent::MessageCreate(msg) => {
                self.clear_typing(&msg.channel_id, &msg.author.id);
                crate::chat::markdown::prewarm_markdown_cache(&msg.content);
                let channel_id = msg.channel_id.clone();

                // 1. Identificar com precisão se a mensagem pertence a uma GUILDA/SERVIDOR.
                // Uma mensagem é estritamente de guilda se:
                // - msg.guild_id.is_some(), OU
                // - o canal pertencer a qualquer guilda conhecida em self.guilds.
                let is_guild_message = msg.guild_id.is_some()
                    || self.guilds.iter().any(|g| {
                        g.categories.iter().any(|cat| cat.channels.iter().any(|c| c.id == channel_id))
                    });

                // Se NÃO for mensagem de guilda, verificar se havia mensagem pendente em canal temporário dm-*
                if !is_guild_message {
                    for (temp_cid, temp_msgs) in self.messages_by_channel.iter_mut() {
                        if temp_cid.starts_with("dm-") && temp_cid != &channel_id {
                            if let Some(pos) = temp_msgs.iter().position(|m| {
                                (m.id.starts_with("temp-") || m.is_pending)
                                    && (m.author.id == msg.author.id
                                        || m.author.username == msg.author.username
                                        || m.author.id == "user-me"
                                        || m.author.id == self.current_user.id)
                                    && m.content == msg.content
                            }) {
                                temp_msgs.remove(pos);
                                break;
                            }
                        }
                    }
                }

                let channel_msgs = self.messages_by_channel.entry(channel_id.clone()).or_default();
                
                // Substituição otimista: se existir uma mensagem temporária pendente com mesmo conteúdo e autor
                if let Some(pos) = channel_msgs.iter().position(|m| {
                    (m.id.starts_with("temp-") || m.is_pending)
                        && (m.author.id == msg.author.id
                            || m.author.username == msg.author.username
                            || m.author.id == "user-me"
                            || m.author.id == self.current_user.id)
                        && m.content == msg.content
                }) {
                    channel_msgs[pos] = msg.clone();
                } else if !channel_msgs.iter().any(|m| m.id == msg.id) {
                    channel_msgs.push(msg.clone());
                }

                if is_guild_message {
                    // Atualiza o last_message_id do canal da guilda, se existir
                    for guild in &mut self.guilds {
                        for cat in &mut guild.categories {
                            for ch in &mut cat.channels {
                                if ch.id == channel_id {
                                    ch.last_message_id = Some(msg.id.clone());
                                }
                            }
                        }
                    }
                    // MENSAGEM DE GUILDA NUNCA MODIFICA dm_channels NEM selected_channel_id DE DM!
                } else {
                    // Mensagem comprovadamente de DM privada (msg.guild_id.is_none() e canal não pertence a nenhuma guilda)
                    let mut dm_updated = false;

                    // 1. Tentar correspondência EXATA com o ID do canal de DM existente
                    if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == channel_id) {
                        dm.last_message_id = Some(msg.id.clone());
                        dm_updated = true;
                    } else if let Some(dm) = self.dm_channels.iter_mut().find(|c| {
                        // 2. Apenas se for canal temporário dm-* ainda não resolvido E o destinatário bater com o autor
                        c.id.starts_with("dm-") && c.recipient_ids.contains(&msg.author.id)
                    }) {
                        let temp_id = dm.id.clone();
                        dm.id = channel_id.clone();
                        dm.last_message_id = Some(msg.id.clone());
                        dm_updated = true;

                        // Se a seleção atual estiver no temporário correspondente, migra para o canal real da DM
                        if self.selected_channel_id.as_deref() == Some(&temp_id) {
                            self.selected_channel_id = Some(channel_id.clone());
                        }

                        // Migra mensagens pendentes do canal temporário para o canal real
                        if temp_id != channel_id {
                            if let Some(mut pending_msgs) = self.messages_by_channel.remove(&temp_id) {
                                for m in &mut pending_msgs {
                                    m.channel_id = channel_id.clone();
                                }
                                let target_msgs = self.messages_by_channel.entry(channel_id.clone()).or_default();
                                for p in pending_msgs {
                                    if !target_msgs.iter().any(|m| m.id == p.id) {
                                        target_msgs.push(p);
                                    }
                                }
                            }
                        }
                    }

                    if dm_updated {
                        self.sort_dm_channels();
                    }

                    // Se a seleção atual estiver em dm-{user_id} e esta DM real corresponde a esse mesmo user_id, migra a seleção
                    if let Some(ref sel) = self.selected_channel_id.clone() {
                        if sel.starts_with("dm-") && sel != &channel_id {
                            let uid = sel.trim_start_matches("dm-");
                            if uid == msg.author.id
                                && self.dm_channels.iter().any(|d| d.id == channel_id && d.recipient_ids.contains(&uid.to_string()))
                            {
                                self.selected_channel_id = Some(channel_id.clone());
                            }
                        }
                    }
                }
            }
            TransportEvent::ChannelMessagesLoaded { channel_id, messages } => {
                self.set_channel_messages(&channel_id, messages);
            }
            TransportEvent::MessageUpdate(msg) => {
                crate::chat::markdown::prewarm_markdown_cache(&msg.content);
                if let Some(msgs) = self.messages_by_channel.get_mut(&msg.channel_id) {
                    if let Some(existing) = msgs.iter_mut().find(|m| m.id == msg.id) {
                        *existing = msg;
                    }
                }
            }
            TransportEvent::MessageDelete { channel_id, message_id } => {
                if let Some(msgs) = self.messages_by_channel.get_mut(&channel_id) {
                    msgs.retain(|m| m.id != message_id);
                }
            }
            TransportEvent::ReactionAdd { channel_id, message_id, emoji, .. } => {
                self.add_reaction_to_message(&channel_id, &message_id, &emoji);
            }
            TransportEvent::TypingStart { channel_id, user_id, .. } => {
                self.record_typing(&channel_id, &user_id, None);
            }
            TransportEvent::Connected => {
                self.is_connected_to_gateway = true;
                self.is_connecting = false;
                self.connection_status = "Conectado".to_string();
                self.auth_error = None;
                self.close_modal();
            }
            TransportEvent::Disconnected(reason) => {
                self.is_connected_to_gateway = false;
                self.is_connecting = false;
                self.auth_error = Some(reason.clone());
                self.connection_status = format!("Desconectado: {}", reason);
            }
            TransportEvent::GuildChannelsUpdated { guild_id, channels } => {
                let categories = organize_channels_into_categories(&guild_id, channels);

                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.categories = categories;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);

                if self.selected_guild_id.as_deref() == Some(&guild_id) && self.selected_channel_id.is_none() {
                    if let Some(guild) = self.guilds.iter().find(|g| g.id == guild_id) {
                        for cat in &guild.categories {
                            if let Some(first_text) = cat.channels.iter().find(|c| c.kind == ChannelKind::Text) {
                                self.select_channel(first_text.id.clone());
                                break;
                            }
                        }
                    }
                }
            }
            TransportEvent::GuildCreate(g) => {
                let icon_url = g.icon_url();
                let icon_hash = g.icon.clone();
                let categories = organize_channels_into_categories(&g.id, g.channels);

                let emojis = if !g.emojis.is_empty() {
                    let _ = self.media_cache.save_guild_emojis_to_disk(&g.id, &g.emojis);
                    g.emojis
                } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.emojis.clone()) {
                    if !existing.is_empty() {
                        existing
                    } else {
                        self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                    }
                } else {
                    self.media_cache.load_guild_emojis_from_disk(&g.id).unwrap_or_default()
                };

                let stickers = if !g.stickers.is_empty() {
                    let _ = self.media_cache.save_guild_stickers_to_disk(&g.id, &g.stickers);
                    g.stickers
                } else if let Some(existing) = self.guilds.iter().find(|guild| guild.id == g.id).map(|guild| guild.stickers.clone()) {
                    if !existing.is_empty() {
                        existing
                    } else {
                        self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
                    }
                } else {
                    self.media_cache.load_guild_stickers_from_disk(&g.id).unwrap_or_default()
                };

                let item = GuildItemState {
                    id: g.id.clone(),
                    name: g.name.clone(),
                    acronym: g.name.chars().take(3).collect(),
                    icon_url,
                    icon_hash,
                    unread: false,
                    mention_count: 0,
                    categories,
                    emojis,
                    stickers,
                };
                if let Some(pos) = self.guilds.iter().position(|guild| guild.id == g.id) {
                    self.guilds[pos] = item;
                } else {
                    self.guilds.push(item);
                }
                if !self.guild_folders.is_empty() {
                    let mut order_map = HashMap::new();
                    let mut idx = 0;
                    for folder in &self.guild_folders {
                        for gid in &folder.guild_ids {
                            order_map.entry(gid.clone()).or_insert(idx);
                            idx += 1;
                        }
                    }
                    self.guilds.sort_by_key(|g| {
                        order_map.get(&g.id).copied().unwrap_or(usize::MAX)
                    });
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildEmojisUpdated { guild_id, emojis } => {
                let _ = self.media_cache.save_guild_emojis_to_disk(&guild_id, &emojis);
                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.emojis = emojis;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::GuildStickersUpdated { guild_id, stickers } => {
                let _ = self.media_cache.save_guild_stickers_to_disk(&guild_id, &stickers);
                if let Some(guild) = self.guilds.iter_mut().find(|g| g.id == guild_id) {
                    guild.stickers = stickers;
                }
                save_guilds_to_disk(&self.guilds, &self.guild_folders);
            }
            TransportEvent::DmChannelsUpdated(channels) => {
                let mut new_dms: Vec<ChannelItemState> = channels
                    .into_iter()
                    .map(|c| {
                        let is_group = c.channel_type == ChannelType::GroupDm;
                        ChannelItemState {
                            id: c.id,
                            name: c.name,
                            kind: ChannelKind::Text,
                            topic: c.topic,
                            unread: false,
                            mention_count: 0,
                            icon_url: c.icon_url,
                            is_pinned: c.is_pinned,
                            last_message_id: c.last_message_id,
                            is_group_dm: is_group,
                            recipient_ids: c.recipient_ids,
                            recipient_names: c.recipient_names,
                        }
                    })
                    .collect();

                // Preservar canais de DM temporários ainda não resolvidos (ex: dm-<user_id> abertos dinamicamente)
                for existing in &self.dm_channels {
                    // Ignorar canais mock / fixos de demonstração antigos
                    if existing.id == "dm-ferris"
                        || existing.id == "dm-group-gpui"
                        || existing.id == "dm-ana"
                        || existing.recipient_ids.iter().any(|r| r == "user-ferris")
                        || existing.name.eq_ignore_ascii_case("ferris")
                    {
                        continue;
                    }

                    if existing.id.starts_with("dm-") {
                        let already_resolved = new_dms.iter().any(|d| {
                            existing.recipient_ids.iter().any(|uid| d.recipient_ids.contains(uid))
                        });
                        if !already_resolved && !new_dms.iter().any(|d| d.id == existing.id) {
                            new_dms.push(existing.clone());
                        }
                    }
                }

                self.dm_channels = new_dms;
                self.sort_dm_channels();

                if self.selected_guild_id.is_none() && self.selected_channel_id.is_none() {
                    if let Some(first_dm) = self.dm_channels.first() {
                        self.select_channel(first_dm.id.clone());
                    }
                }
            }
            TransportEvent::DmChannelCreated(c) => {
                let is_group = c.channel_type == ChannelType::GroupDm;
                let real_id = c.id.clone();
                let mut temp_id_to_migrate: Option<String> = None;
                if let Some(existing) = self.dm_channels.iter_mut().find(|d| {
                    d.id == real_id
                        || (d.id.starts_with("dm-")
                            && !d.is_group_dm
                            && !c.recipient_ids.is_empty()
                            && c.recipient_ids.iter().any(|uid| d.recipient_ids.contains(uid)))
                }) {
                    if existing.id.starts_with("dm-") {
                        temp_id_to_migrate = Some(existing.id.clone());
                    }
                    existing.id = real_id.clone();
                    existing.name = c.name;
                    existing.icon_url = c.icon_url;
                    existing.recipient_ids = c.recipient_ids;
                    existing.recipient_names = c.recipient_names;
                    existing.is_group_dm = is_group;
                } else {
                    self.dm_channels.insert(
                        0,
                        ChannelItemState {
                            id: real_id.clone(),
                            name: c.name,
                            kind: ChannelKind::Text,
                            topic: c.topic,
                            unread: false,
                            mention_count: 0,
                            icon_url: c.icon_url,
                            is_pinned: c.is_pinned,
                            last_message_id: c.last_message_id,
                            is_group_dm: is_group,
                            recipient_ids: c.recipient_ids,
                            recipient_names: c.recipient_names,
                        },
                    );
                }
                self.sort_dm_channels();

                if let Some(temp_id) = temp_id_to_migrate {
                    if self.selected_channel_id.as_deref() == Some(&temp_id) {
                        self.selected_channel_id = Some(real_id.clone());
                    }
                    if temp_id != real_id {
                        if let Some(mut pending_msgs) = self.messages_by_channel.remove(&temp_id) {
                            for msg in &mut pending_msgs {
                                msg.channel_id = real_id.clone();
                            }
                            let target_msgs = self.messages_by_channel.entry(real_id.clone()).or_default();
                            for p_msg in pending_msgs {
                                if !target_msgs.iter().any(|m| m.id == p_msg.id) {
                                    target_msgs.push(p_msg);
                                }
                            }
                        }
                    }
                }
            }
            TransportEvent::DmChannelResolved { temp_id, channel } => {
                let real_id = channel.id.clone();
                let is_group = channel.channel_type == ChannelType::GroupDm;

                // 1. Atualizar ou inserir em self.dm_channels
                if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == temp_id || c.id == real_id) {
                    dm.id = real_id.clone();
                    dm.name = channel.name.clone();
                    dm.icon_url = channel.icon_url.clone();
                    dm.is_group_dm = is_group;
                    dm.recipient_ids = channel.recipient_ids.clone();
                    dm.recipient_names = channel.recipient_names.clone();
                } else {
                    self.dm_channels.insert(
                        0,
                        ChannelItemState {
                            id: real_id.clone(),
                            name: channel.name.clone(),
                            kind: ChannelKind::Text,
                            topic: channel.topic.clone(),
                            unread: false,
                            mention_count: 0,
                            icon_url: channel.icon_url.clone(),
                            is_pinned: channel.is_pinned,
                            last_message_id: channel.last_message_id.clone(),
                            is_group_dm: is_group,
                            recipient_ids: channel.recipient_ids.clone(),
                            recipient_names: channel.recipient_names.clone(),
                        },
                    );
                }
                self.sort_dm_channels();

                // 2. Se o canal selecionado era o temporário, atualiza para o ID real
                if self.selected_channel_id.as_deref() == Some(&temp_id) {
                    self.selected_channel_id = Some(real_id.clone());
                }

                // 3. Migra mensagens pendentes/armazenadas do ID temporário para o real
                if temp_id != real_id {
                    if let Some(mut pending_msgs) = self.messages_by_channel.remove(&temp_id) {
                        for msg in &mut pending_msgs {
                            msg.channel_id = real_id.clone();
                        }
                        let target_msgs = self.messages_by_channel.entry(real_id.clone()).or_default();
                        for p_msg in pending_msgs {
                            if !target_msgs.iter().any(|m| m.id == p_msg.id) {
                                target_msgs.push(p_msg);
                            }
                        }
                    }
                }

                // 4. Carrega histórico de mensagens reais da Discord API ou Mock se ainda não estiver presente
                if let Some(ref cm) = self.client_manager {
                    let has_messages = self.messages_by_channel.get(&real_id).map_or(false, |msgs| !msgs.is_empty());
                    if !has_messages || self.is_connected_to_gateway {
                        let cm = cm.clone();
                        let cid = real_id.clone();
                        tokio::spawn(async move {
                            if let Ok(mut msgs) = cm.get_messages(&cid, 50).await {
                                msgs.sort_by_key(|m| m.timestamp);
                                cm.emit_event(TransportEvent::ChannelMessagesLoaded {
                                    channel_id: cid,
                                    messages: msgs,
                                });
                            }
                        });
                    }
                }
            }
            TransportEvent::RelationshipsUpdated(relationships) => {
                self.friends = relationships
                    .into_iter()
                    .map(|r| {
                        let display_name = r.global_name.unwrap_or_else(|| r.username.clone());
                        let avatar_text = display_name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string();
                        FriendItemState {
                            id: r.id.clone(),
                            username: r.username,
                            display_name,
                            avatar_url: r.avatar.as_ref().map(|hash| {
                                if hash.starts_with("http://") || hash.starts_with("https://") {
                                    hash.clone()
                                } else {
                                    format!("https://cdn.discordapp.com/avatars/{}/{}.png?size=96", r.id, hash)
                                }
                            }),
                            avatar_text,
                            presence: PresenceStatus::Online,
                            custom_status: None,
                            relationship_type: r.rel_type,
                        }
                    })
                    .collect();
            }
            _ => {}
        }
    }

    /// Sends a chat message to the currently selected channel via ClientManager
    pub fn send_chat_message(&mut self, content: &str) {
        if let Some(cid) = self.selected_channel_id.clone() {
            self.send_message(&cid, content);
        }
    }

    /// Sends a message to a specific channel via ClientManager with optimistic UI update
    pub fn send_message(&mut self, channel_id: &str, content: &str) {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        let cid = channel_id.to_string();
        if let Some(ref tel) = self.telemetry {
            tel.track_message_send(&cid, trimmed.len(), true, 0);
            tel.track_button_click("send_message", "Enviar Mensagem", 0);
        }
        // Optimistically insert message into UI with temporary ID temp-<timestamp> and pending flag
        let new_id = format!("temp-{}", chrono::Utc::now().timestamp_millis());
        let new_msg = Message::new(
            new_id,
            cid.clone(),
            crate::domain::message::Author {
                id: self.current_user.id.clone(),
                username: self.current_user.username.clone(),
                global_name: Some(self.current_user.display_name.clone()),
                avatar: None,
                bot: false,
            },
            trimmed,
        )
        .with_pending(true);

        self.inject_message(&cid, new_msg);
        if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == cid) {
            dm.last_message_id = Some(format!("{}", chrono::Utc::now().timestamp_millis()));
            self.sort_dm_channels();
        }
        self.replying_to_message = None;
        self.active_draft = String::new();

        // Dispatch via ClientManager if present
        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let target_cid = cid;
            let text = trimmed.to_string();
            tokio::spawn(async move {
                if let Err(e) = cm.send_message(&target_cid, &text).await {
                    log::warn!("Falha ao enviar mensagem via ClientManager: {:?}", e);
                }
            });
        }
    }

    /// Sends a sticker message to a specific channel immediately with authentic sticker rendering
    pub fn send_sticker(&mut self, channel_id: &str, sticker: &crate::protocol::media_cache::DiscordSticker) {
        let cid = channel_id.to_string();
        let new_id = format!("temp-{}", chrono::Utc::now().timestamp_millis());

        let mut new_msg = Message::new(
            new_id,
            cid.clone(),
            crate::domain::message::Author {
                id: self.current_user.id.clone(),
                username: self.current_user.username.clone(),
                global_name: Some(self.current_user.display_name.clone()),
                avatar: None,
                bot: false,
            },
            "",
        )
        .with_pending(true);

        new_msg.sticker_items.push(crate::domain::message::MessageStickerItem::new(
            sticker.id.clone(),
            sticker.name.clone(),
            sticker.format_type as u8,
        ));

        self.inject_message(&cid, new_msg);
        if let Some(dm) = self.dm_channels.iter_mut().find(|c| c.id == cid) {
            dm.last_message_id = Some(format!("{}", chrono::Utc::now().timestamp_millis()));
            self.sort_dm_channels();
        }
        self.replying_to_message = None;

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let target_cid = cid;
            let sticker_id = sticker.id.clone();
            tokio::spawn(async move {
                if let Err(e) = cm.send_sticker(&target_cid, &sticker_id).await {
                    log::warn!("Falha ao enviar figurinha via ClientManager: {:?}", e);
                }
            });
        }
    }

    /// Records emoji usage in local media cache
    pub fn record_emoji_usage(&self, emoji: &str) {
        let mgr = self.media_cache.clone();
        let em = emoji.to_string();
        tokio::spawn(async move {
            mgr.record_emoji_use(&em, &em, false, None).await;
        });
    }

    /// Returns custom emojis for the currently selected guild, or across all guilds
    pub fn get_current_custom_emojis(&self) -> Vec<crate::protocol::media_cache::DiscordEmoji> {
        if let Some(ref gid) = self.selected_guild_id {
            if let Some(guild) = self.guilds.iter().find(|g| &g.id == gid) {
                if !guild.emojis.is_empty() {
                    return guild.emojis.clone();
                }
            }
        }
        let mut all = Vec::new();
        for g in &self.guilds {
            all.extend(g.emojis.clone());
        }
        all
    }

    /// Returns custom stickers and favorites for the currently selected guild
    pub fn get_current_stickers(&self) -> Vec<crate::protocol::media_cache::DiscordSticker> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1. Figurinhas favoritas persistidas
        for s in self.media_cache.get_favorite_stickers_sync() {
            if seen.insert(s.id.clone()) {
                result.push(s);
            }
        }

        // 2. Figurinhas do servidor selecionado (ou todos)
        if let Some(ref gid) = self.selected_guild_id {
            if let Some(guild) = self.guilds.iter().find(|g| &g.id == gid) {
                for s in &guild.stickers {
                    if seen.insert(s.id.clone()) {
                        result.push(s.clone());
                    }
                }
            }
        } else {
            for g in &self.guilds {
                for s in &g.stickers {
                    if seen.insert(s.id.clone()) {
                        result.push(s.clone());
                    }
                }
            }
        }
        result
    }

    /// Carrega emojis e figurinhas customizados de cada guilda a partir do cache em disco imediatamente
    /// e busca da API em segundo plano caso algum servidor ainda não os possua.
    pub fn sync_guild_media(&mut self) {
        let mut missing_emojis = Vec::new();
        let mut missing_stickers = Vec::new();

        // 1. Fast-path: carrega do cache particionado em disco
        for guild in &mut self.guilds {
            if guild.emojis.is_empty() {
                if let Some(cached) = self.media_cache.load_guild_emojis_from_disk(&guild.id) {
                    guild.emojis = cached;
                }
            }
            if guild.stickers.is_empty() {
                if let Some(cached) = self.media_cache.load_guild_stickers_from_disk(&guild.id) {
                    guild.stickers = cached;
                }
            }

            if guild.emojis.is_empty() {
                missing_emojis.push(guild.id.clone());
            }
            if guild.stickers.is_empty() {
                missing_stickers.push(guild.id.clone());
            }
        }

        // 2. Background-path: busca da API se houver ClientManager
        if let Some(ref cm) = self.client_manager {
            if !missing_emojis.is_empty() || !missing_stickers.is_empty() {
                let cm = cm.clone();
                tokio::spawn(async move {
                    for gid in missing_emojis {
                        if let Ok(emojis) = cm.get_guild_emojis(&gid).await {
                            if !emojis.is_empty() {
                                cm.emit_event(TransportEvent::GuildEmojisUpdated {
                                    guild_id: gid,
                                    emojis,
                                });
                            }
                        }
                    }
                    for gid in missing_stickers {
                        if let Ok(stickers) = cm.get_guild_stickers(&gid).await {
                            if !stickers.is_empty() {
                                cm.emit_event(TransportEvent::GuildStickersUpdated {
                                    guild_id: gid,
                                    stickers,
                                });
                            }
                        }
                    }
                });
            }
        }
    }

    /// Adds a reaction and dispatches it via ClientManager
    pub fn add_reaction(&mut self, message_id: &str, emoji: &str) {
        if let Some(ref tel) = self.telemetry {
            tel.track_reaction(emoji, message_id, true, 0);
        }
        let channel_id_opt = self.selected_channel_id.clone();
        if let Some(cid) = channel_id_opt {
            self.add_reaction_to_message(&cid, message_id, emoji);

            if let Some(ref cm) = self.client_manager {
                let cm = cm.clone();
                let cid = cid.clone();
                let mid = message_id.to_string();
                let em = emoji.to_string();
                tokio::spawn(async move {
                    if let Err(e) = cm.add_reaction(&cid, &mid, &em).await {
                        log::warn!("Falha ao adicionar reação via ClientManager: {:?}", e);
                    }
                });
            }
        }
    }

    // =========================================================================
    // Real-Time Coalesced Typing Tracker Subsystem
    // =========================================================================

    /// Records a typing event with 9-10 second coalescing window
    pub fn record_typing(&mut self, channel_id: &str, user_id: &str, username: Option<&str>) {
        let resolved_name = if let Some(un) = username {
            un.to_string()
        } else if let Some(m) = self.members.iter().find(|m| m.id == user_id) {
            m.name.clone()
        } else if let Some(f) = self.friends.iter().find(|f| f.id == user_id) {
            f.display_name.clone()
        } else if let Some(dm) = self.dm_channels.iter().find(|d| d.id == user_id || d.name.to_lowercase() == user_id.to_lowercase()) {
            dm.name.clone()
        } else {
            user_id.to_string()
        };

        let channel_map = self.typing_tracker.entry(channel_id.to_string()).or_default();
        channel_map.insert(user_id.to_string(), (resolved_name, std::time::Instant::now()));

        if self.selected_channel_id.as_deref() == Some(channel_id) {
            self.typing_users = self.get_active_typing_users(channel_id);
        }
    }

    /// Clears typing status for a user in a channel (e.g. upon MessageCreate)
    pub fn clear_typing(&mut self, channel_id: &str, user_id: &str) {
        if let Some(channel_map) = self.typing_tracker.get_mut(channel_id) {
            channel_map.remove(user_id);
        }
        if self.selected_channel_id.as_deref() == Some(channel_id) {
            self.typing_users = self.get_active_typing_users(channel_id);
        }
    }

    /// Retrieves active typers in a channel whose events arrived within the 9-second TTL window
    pub fn get_active_typing_users(&self, channel_id: &str) -> Vec<String> {
        let now = std::time::Instant::now();
        if let Some(channel_map) = self.typing_tracker.get(channel_id) {
            let mut users: Vec<String> = channel_map
                .values()
                .filter(|(_, ts)| now.duration_since(*ts).as_secs() <= 9)
                .map(|(name, _)| name.clone())
                .collect();
            users.sort();
            users.dedup();
            users
        } else {
            Vec::new()
        }
    }

    /// Formats active typing users into localized natural language string
    pub fn get_active_typing_text(&self, channel_id: &str) -> Option<String> {
        let users = self.get_active_typing_users(channel_id);
        match users.len() {
            0 => None,
            1 => Some(format!("{} está digitando...", users[0])),
            2 => Some(format!("{} e {} estão digitando...", users[0], users[1])),
            3 => Some(format!("{}, {} e {} estão digitando...", users[0], users[1], users[2])),
            _ => Some("Várias pessoas estão digitando...".to_string()),
        }
    }

    // =========================================================================
    // Search Engine & Search Flyout Subsystem
    // =========================================================================

    /// Opens the search flyout panel
    pub fn open_search(&mut self) {
        self.is_search_open = true;
        if !self.search_query.is_empty() {
            let q = self.search_query.clone();
            self.execute_search(&q);
        }
    }

    /// Closes the search flyout panel
    pub fn close_search(&mut self) {
        self.is_search_open = false;
    }

    /// Toggles the search flyout panel
    pub fn toggle_search(&mut self) {
        if self.is_search_open {
            self.close_search();
        } else {
            self.open_search();
        }
    }

    /// Executes search across active channel and all cached channels in memory
    pub fn execute_search(&mut self, query_str: &str) {
        self.search_query = query_str.to_string();
        self.is_searching = true;
        let query = crate::chat::search::SearchQuery::parse(query_str);

        if query.is_empty() {
            self.search_results.clear();
            self.is_searching = false;
            return;
        }

        let mut results = Vec::new();

        // 1. Search in current channel first if selected
        if let Some(ref current_cid) = self.selected_channel_id {
            if let Some(msgs) = self.messages_by_channel.get(current_cid) {
                let ch_name = self.find_channel(current_cid).map(|(_, c)| c.name.as_str())
                    .or_else(|| self.dm_channels.iter().find(|d| &d.id == current_cid).map(|d| d.name.as_str()));
                results.extend(crate::chat::search::LocalSearchEngine::search_channel_messages(
                    msgs,
                    &query,
                    current_cid,
                    ch_name,
                    1,
                ));
            }
        }

        // 2. Search across other cached channels
        for (cid, msgs) in &self.messages_by_channel {
            if self.selected_channel_id.as_deref() == Some(cid) {
                continue;
            }
            let ch_name = self.find_channel(cid).map(|(_, c)| c.name.as_str())
                .or_else(|| self.dm_channels.iter().find(|d| &d.id == cid).map(|d| d.name.as_str()));
            results.extend(crate::chat::search::LocalSearchEngine::search_channel_messages(
                msgs,
                &query,
                cid,
                ch_name,
                1,
            ));
        }

        self.search_results = results;
        self.is_searching = false;
    }

    /// Clears search query and result list
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.is_searching = false;
    }

    /// Jumps to and highlights a specific message in the chat
    pub fn jump_to_message(&mut self, channel_id: &str, message_id: &str) {
        if self.selected_channel_id.as_deref() != Some(channel_id) {
            self.select_channel(channel_id.to_string());
        }
        self.target_message_id = Some(message_id.to_string());
    }

    // =========================================================================
    // Direct Messages & Group DM Subsystem
    // =========================================================================

    /// Opens or selects a 1-on-1 DM with a user
    pub fn open_dm_with_user(&mut self, user_id: &str, username: &str) -> String {
        if let Some(existing) = self.dm_channels.iter().find(|d| {
            !d.is_group_dm && (d.recipient_ids.contains(&user_id.to_string()) || d.name == username)
        }) {
            let id = existing.id.clone();
            self.select_channel(id.clone());
            if id.starts_with("dm-") {
                if let Some(ref cm) = self.client_manager {
                    let cm = cm.clone();
                    let uid = user_id.to_string();
                    let temp_id = id.clone();
                    tokio::spawn(async move {
                        if let Ok(ch) = cm.create_or_get_dm_channel(&uid).await {
                            cm.emit_event(TransportEvent::DmChannelResolved {
                                temp_id,
                                channel: ch,
                            });
                        }
                    });
                }
            }
            return id;
        }

        let new_id = format!("dm-{}", user_id);
        let item = ChannelItemState {
            id: new_id.clone(),
            name: username.to_string(),
            kind: ChannelKind::Text,
            topic: Some(format!("Mensagens Diretas com {}", username)),
            unread: false,
            mention_count: 0,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: false,
            recipient_ids: vec![user_id.to_string()],
            recipient_names: vec![username.to_string()],
        };

        self.dm_channels.insert(0, item);
        self.select_channel(new_id.clone());

        if let Some(ref cm) = self.client_manager {
            let cm = cm.clone();
            let uid = user_id.to_string();
            let temp_id = new_id.clone();
            tokio::spawn(async move {
                if let Ok(ch) = cm.create_or_get_dm_channel(&uid).await {
                    cm.emit_event(TransportEvent::DmChannelResolved {
                        temp_id,
                        channel: ch,
                    });
                }
            });
        }

        new_id
    }

    /// Creates a new Group DM channel with multiple participants
    pub fn create_group_dm(
        &mut self,
        recipient_ids: Vec<String>,
        recipient_names: Vec<String>,
        name: Option<String>,
    ) -> String {
        let gid = format!("dm-group-{}", chrono::Utc::now().timestamp_millis());
        let display_name = name.unwrap_or_else(|| {
            if recipient_names.is_empty() {
                "Grupo Vazio".to_string()
            } else {
                recipient_names.join(", ")
            }
        });

        let item = ChannelItemState {
            id: gid.clone(),
            name: display_name,
            kind: ChannelKind::Text,
            topic: Some("Conversa em Grupo".into()),
            unread: false,
            mention_count: 0,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: true,
            recipient_ids,
            recipient_names,
        };

        self.dm_channels.insert(0, item);
        self.sort_dm_channels();
        self.select_channel(gid.clone());
        self.close_modal();
        gid
    }

    /// Adds a friend/recipient to an existing Group DM or escalates a 1-on-1 DM
    pub fn add_recipient_to_group(&mut self, channel_id: &str, user_id: &str, username: &str) {
        if let Some(ch) = self.dm_channels.iter_mut().find(|c| c.id == channel_id) {
            if !ch.recipient_ids.contains(&user_id.to_string()) {
                ch.recipient_ids.push(user_id.to_string());
                ch.recipient_names.push(username.to_string());
                ch.is_group_dm = true;
                if ch.topic.as_deref().map_or(true, |t| t.starts_with("Mensagens Diretas com")) {
                    ch.name = ch.recipient_names.join(", ");
                    ch.topic = Some("Conversa em Grupo".to_string());
                }
            }
        }
        self.close_modal();
    }
}
