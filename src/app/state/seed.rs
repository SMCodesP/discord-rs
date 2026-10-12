use super::*;
use std::collections::{HashMap, HashSet};
use chrono::{Duration, Utc};
use crate::domain::channel::ChannelType;
use crate::domain::message::Attachment;
use crate::domain::message::Author;
use crate::domain::message::Message;
use crate::domain::message::Reaction;
use crate::domain::settings::AccessibilitySettings;
use crate::domain::settings::AccountConnection;
use crate::domain::settings::AuthorizedApp;
use crate::domain::settings::FamilyCenterLink;
use crate::domain::settings::MessageDisplayMode;
use crate::domain::settings::NitroTier;
use crate::domain::settings::ServerBoost;
use crate::domain::settings::Subscription;
use crate::domain::settings::ThemeChoice;
use crate::protocol::ConnectionMode;
use crate::protocol::TokenStorage;

pub(super) fn generate_seed_messages() -> HashMap<String, Vec<Message>> {
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
        avatar_decoration_data: None,
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
pub(super) fn organize_channels_into_categories(
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
            user_limit: c.user_limit,
            avatar_decoration_data: None,
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

impl AppState {
    /// Somente testes/fixtures; nunca chamar em produção.
    #[doc(hidden)]
    pub fn seeded_for_tests() -> Self {
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                            user_limit: None,
                            avatar_decoration_data: None,
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
                        user_limit: None,
                        avatar_decoration_data: None,
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
                avatar_decoration_data: None,
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
                avatar_decoration_data: None,
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
                avatar_decoration_data: None,
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
                avatar_decoration_data: None,
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
                avatar_decoration_data: None,
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
                avatar_decoration_data: None,
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
            "Não conectado".to_string()
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
                avatar_decoration_data: None,
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
            presences_by_user: HashMap::new(),
            pinned_dms: Vec::new(),
            pinned_dms_path: None,
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
                        avatar_decoration_data: None,
                    },
                ]
            },
            friends_tab: FriendsTab::Online,
            add_friend_input: String::new(),
            add_friend_feedback: None,
            is_sending_friend_request: false,
            pending_relationship_actions: HashSet::new(),
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
                    name: "usuario_a".into(),
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
            account_theme: None,
            account_client_theme_settings: None,
            message_display_mode: MessageDisplayMode::Cozy,
            accessibility_settings: AccessibilitySettings::default(),
            media_cache: crate::protocol::media_cache::MediaCacheManager::default_path(),
            profile_popout: None,
            profile_anchor: None,
            profile_modal_open: false,
            profile_modal_tab: ProfileModalTab::AboutMe,
            profile_effects_catalog: crate::components::ProfileEffectCatalog::new(),
            voice_states_by_channel: HashMap::new(),
        };
        state.sync_guild_media();
        state
    }
}
