//! End-to-End Automated Test Suite for Discord RS
//!
//! Validates the complete AppState and UI lifecycle:
//! 1. Initialization with token & guild disk cache
//! 2. Guild rail switching and server selection
//! 3. Category grouping, channel collapse and expansion
//! 4. Navigation to Direct Messages (DMs) and Friends list
//! 5. Optimistic message sending and Gateway confirmation
//! 6. Emoji reactions, remote reactions, and message replies
//! 7. Modal dialogs lifecycle (UserSettings, AddServer, Login, tab switching)
//! 8. Stress test with high volume (1,000 messages & 500 members)
//! 9. Pinned and recency sorting for Direct Messages (DMs)

use chrono::{Duration, Utc};
use std::time::Instant;

use discord_rs::app::{
    ActiveModal, AppState, CategoryItemState, ChannelItemState, ChannelKind,
    GuildItemState, GuildsDiskCache, MemberItemState, PresenceStatus,
    SettingsTab,
};
use discord_rs::domain::channel::Channel;
use discord_rs::domain::message::{Author, Message, Reaction};
use discord_rs::protocol::{
    AuthToken, GuildFolder, Relationship, TokenStorage, TransportEvent,
};

/// Helper to create a sample guild with categories and channels
fn create_test_guild(id: &str, name: &str, text_chan_id: &str) -> GuildItemState {
    GuildItemState {
        id: id.to_string(),
        name: name.to_string(),
        acronym: name.chars().take(3).collect(),
        icon_url: Some(format!("https://cdn.discordapp.com/icons/{}/icon.png", id)),
        icon_hash: Some("icon_hash_123".to_string()),
        unread: false,
        mention_count: 0,
        categories: vec![
            CategoryItemState {
                id: format!("cat-{}-welcome", id),
                name: "BOAS-VINDAS".to_string(),
                channels: vec![ChannelItemState {
                    id: format!("chan-{}-rules", id),
                    name: "regras".to_string(),
                    kind: ChannelKind::Announcement,
                    topic: Some("Regras do servidor".to_string()),
                    unread: false,
                    mention_count: 0,
                    icon_url: None,
                    is_pinned: false,
                    last_message_id: None,
                    is_group_dm: false,
                    recipient_ids: Vec::new(),
                    recipient_names: Vec::new(),
                    user_limit: None,
                }],
            },
            CategoryItemState {
                id: format!("cat-{}-text", id),
                name: "CANAIS DE TEXTO".to_string(),
                channels: vec![
                    ChannelItemState {
                        id: text_chan_id.to_string(),
                        name: "geral".to_string(),
                        kind: ChannelKind::Text,
                        topic: Some("Discussão geral".to_string()),
                        unread: false,
                        mention_count: 0,
                        icon_url: None,
                        is_pinned: false,
                        last_message_id: None,
                        is_group_dm: false,
                        recipient_ids: Vec::new(),
                        recipient_names: Vec::new(),
                        user_limit: None,
                    },
                    ChannelItemState {
                        id: format!("chan-{}-dev", id),
                        name: "desenvolvimento".to_string(),
                        kind: ChannelKind::Text,
                        topic: Some("Canal de código".to_string()),
                        unread: true,
                        mention_count: 2,
                        icon_url: None,
                        is_pinned: false,
                        last_message_id: None,
                        is_group_dm: false,
                        recipient_ids: Vec::new(),
                        recipient_names: Vec::new(),
                        user_limit: None,
                    },
                ],
            },
        ],
        emojis: Vec::new(),
        stickers: Vec::new(),
    }
}

// =========================================================================
// 1. Inicialização com cache de guilds e token
// =========================================================================

#[test]
fn test_e2e_initialization_with_guild_cache_and_token() {
    // 1.1 Test TokenStorage serialization & parsing
    let temp_token_path = std::env::temp_dir().join(format!("test_token_{}.json", Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let token_storage = TokenStorage::with_path(&temp_token_path);

    let sample_token = AuthToken::user("sample_user_token_abc123xyz456");
    token_storage.save(&sample_token).expect("Token save failed");
    assert!(temp_token_path.exists());

    let loaded_token = token_storage.load().expect("Token load failed").expect("Expected saved token");
    assert_eq!(loaded_token.secret(), "sample_user_token_abc123xyz456");
    assert_eq!(loaded_token.token_type, discord_rs::protocol::TokenType::User);

    token_storage.clear().expect("Token clear failed");
    assert_eq!(token_storage.load().expect("Expected clean load"), None);
    let _ = std::fs::remove_file(&temp_token_path);

    // 1.2 Test GuildsDiskCache serialization & sorting by guild folders
    let guild1 = create_test_guild("guild-1", "Servidor Alpha", "chan-alpha-general");
    let guild2 = create_test_guild("guild-2", "Servidor Beta", "chan-beta-general");
    let folder = GuildFolder {
        id: Some("folder-1".to_string()),
        name: Some("Comunidades".to_string()),
        color: Some(0x5865f2),
        guild_ids: vec!["guild-2".to_string(), "guild-1".to_string()],
    };

    let cache = GuildsDiskCache {
        guilds: vec![guild1, guild2],
        guild_folders: vec![folder.clone()],
    };
    let json = serde_json::to_string_pretty(&cache).expect("Serialization failed");
    let loaded_cache: GuildsDiskCache = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(loaded_cache.guilds.len(), 2);
    assert_eq!(loaded_cache.guild_folders.len(), 1);
    assert_eq!(loaded_cache.guild_folders[0].guild_ids, vec!["guild-2", "guild-1"]);

    // 1.3 AppState default initialization
    let state = AppState::new();
    assert_eq!(state.current_user.id, "user-me");
    assert_eq!(state.current_user.username, "vanguard_dev");
    assert!(!state.guilds.is_empty(), "Initial state must have guilds");
    assert!(state.selected_guild_id.is_some(), "Default guild must be selected");
    assert!(state.selected_channel_id.is_some(), "Default channel must be selected");
}

// =========================================================================
// 2. Alternância de servidores (Guild Rail) e seleção de servidor
// =========================================================================

#[test]
fn test_e2e_guild_rail_switching_and_server_selection() {
    let mut state = AppState::new();

    let guild_a = create_test_guild("guild-a", "Servidor Rust", "chan-rust-general");
    let guild_b = create_test_guild("guild-b", "Servidor GPUI", "chan-gpui-general");
    state.guilds = vec![guild_a, guild_b];

    // Select Guild A
    state.select_guild(Some("guild-a".to_string()));
    assert_eq!(state.selected_guild_id.as_deref(), Some("guild-a"));
    assert_eq!(state.selected_channel_id.as_deref(), Some("chan-rust-general"));

    // Verify helper accessors
    let selected_guild = state.get_selected_guild().expect("Guild A should be selected");
    assert_eq!(selected_guild.id, "guild-a");
    assert_eq!(selected_guild.name, "Servidor Rust");
    assert_eq!(selected_guild.acronym, "Ser");

    let (active_guild, active_chan) = state.get_selected_channel().expect("Active channel should exist");
    assert_eq!(active_guild.id, "guild-a");
    assert_eq!(active_chan.id, "chan-rust-general");
    assert_eq!(active_chan.name, "geral");

    // Instant switch to Guild B (categories already cached in memory, zero network request)
    state.select_guild(Some("guild-b".to_string()));
    assert_eq!(state.selected_guild_id.as_deref(), Some("guild-b"));
    assert_eq!(state.selected_channel_id.as_deref(), Some("chan-gpui-general"));

    let selected_guild_b = state.get_selected_guild().expect("Guild B should be selected");
    assert_eq!(selected_guild_b.name, "Servidor GPUI");
}

// =========================================================================
// 3. Categorias reais e expansão/colapso de canais
// =========================================================================

#[test]
fn test_e2e_categories_and_channel_collapse_expand() {
    let mut state = AppState::new();

    let cat_id = "cat-br-welcome";
    // Initially no category is collapsed
    assert!(!state.is_category_collapsed(cat_id));
    assert!(!state.collapsed_categories.contains(cat_id));

    // Collapse category
    state.toggle_category(cat_id);
    assert!(state.is_category_collapsed(cat_id));
    assert!(state.collapsed_categories.contains(cat_id));

    // Expand category
    state.toggle_category(cat_id);
    assert!(!state.is_category_collapsed(cat_id));
    assert!(!state.collapsed_categories.contains(cat_id));

    // Also test folder expansion in Guild Rail
    let folder_id = "folder-dev";
    assert!(!state.is_folder_expanded(folder_id));
    state.toggle_folder(folder_id);
    assert!(state.is_folder_expanded(folder_id));
    state.toggle_folder(folder_id);
    assert!(!state.is_folder_expanded(folder_id));
}

// =========================================================================
// 4. Navegação para DMs e Amigos (Home / Amigos)
// =========================================================================

#[test]
fn test_e2e_home_and_dm_navigation_with_friends() {
    let mut state = AppState::new();
    state.dm_channels.clear();

    // Switch to Home / Direct Messages by passing None as guild_id
    state.select_guild(None);
    assert!(state.selected_guild_id.is_none());

    // Simulate Gateway / REST dispatching DMs
    let dm_channel_1 = Channel::text("dm-ferris", "Ferris the Crab")
        .with_topic("Direct Message com Ferris");
    let mut dm_channel_2 = Channel::text("dm-ana", "Ana Dev")
        .with_topic("Direct Message com Ana");
    dm_channel_2.icon_url = Some("https://cdn.discordapp.com/avatars/user-ana/hash.png".to_string());

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![
        dm_channel_1,
        dm_channel_2,
    ]));

    assert_eq!(state.dm_channels.len(), 2);
    assert_eq!(state.dm_channels[0].id, "dm-ferris");
    assert_eq!(state.dm_channels[0].name, "Ferris the Crab");
    assert_eq!(state.dm_channels[1].id, "dm-ana");
    assert_eq!(state.dm_channels[1].icon_url, Some("https://cdn.discordapp.com/avatars/user-ana/hash.png".to_string()));

    // Automatic selection of first DM channel when in Home mode
    assert_eq!(state.selected_channel_id.as_deref(), Some("dm-ferris"));

    // Select second DM channel
    state.select_channel("dm-ana".to_string());
    assert_eq!(state.selected_channel_id.as_deref(), Some("dm-ana"));

    // Simulate Gateway / REST dispatching Friends / Relationships
    let relationships = vec![
        Relationship {
            id: "user-ferris".to_string(),
            rel_type: 1, // Friend
            username: "ferris_crab".to_string(),
            global_name: Some("Ferris".to_string()),
            avatar: Some("https://cdn.discordapp.com/avatars/user-ferris/avatar.png".to_string()),
        },
        Relationship {
            id: "user-ana".to_string(),
            rel_type: 1, // Friend
            username: "ana_rust".to_string(),
            global_name: None,
            avatar: None,
        },
    ];

    state.handle_transport_event(TransportEvent::RelationshipsUpdated(relationships));
    assert_eq!(state.friends.len(), 2);
    assert_eq!(state.friends[0].display_name, "Ferris");
    assert_eq!(state.friends[0].avatar_text, "F");
    assert_eq!(state.friends[0].presence, PresenceStatus::Online);
    assert_eq!(state.friends[1].display_name, "ana_rust");
    assert_eq!(state.friends[1].avatar_text, "A");
}

// =========================================================================
// 5. Envio de mensagens com estado otimista e confirmação
// =========================================================================

#[test]
fn test_e2e_optimistic_message_sending_and_gateway_confirmation() {
    let mut state = AppState::new();
    let channel_id = "chan-general";
    state.selected_channel_id = Some(channel_id.to_string());

    let initial_count = state.messages_by_channel.get(channel_id).map(|v| v.len()).unwrap_or(0);

    // 5.1 Send an empty or whitespace message (should be rejected)
    state.send_message(channel_id, "   ");
    assert_eq!(
        state.messages_by_channel.get(channel_id).map(|v| v.len()).unwrap_or(0),
        initial_count,
        "Empty message should not be sent or injected"
    );

    // 5.2 Optimistically send message
    let test_content = "Olá do teste E2E determinístico em Rust! 🚀";
    state.send_message(channel_id, test_content);

    let messages = state.messages_by_channel.get(channel_id).expect("Channel messages should exist");
    assert_eq!(messages.len(), initial_count + 1);

    let optimistic_msg = messages.last().expect("Last message should be the optimistic one");
    assert!(optimistic_msg.id.starts_with("temp-"), "Optimistic ID must start with temp-");
    assert!(optimistic_msg.is_pending, "Message must have pending state true");
    assert_eq!(optimistic_msg.content, test_content);
    assert_eq!(optimistic_msg.author.id, state.current_user.id);
    assert_eq!(optimistic_msg.author.username, state.current_user.username);
    assert_eq!(state.active_draft, "", "Draft must be cleared on send");
    assert!(state.replying_to_message.is_none());

    // 5.3 Gateway confirmation: Server acknowledges message with real Snowflake ID
    let confirmed_id = "134598765432109876";
    let confirmed_msg = Message::new(
        confirmed_id,
        channel_id,
        Author::new(&state.current_user.id, &state.current_user.username),
        test_content,
    )
    .with_pending(false);

    state.handle_transport_event(TransportEvent::MessageCreate(confirmed_msg));

    let messages_after_confirm = state.messages_by_channel.get(channel_id).unwrap();
    // Count must not duplicate! In-place replacement occurred
    assert_eq!(messages_after_confirm.len(), initial_count + 1);

    let replaced_msg = messages_after_confirm.last().unwrap();
    assert_eq!(replaced_msg.id, confirmed_id);
    assert!(!replaced_msg.is_pending, "Confirmed message must not be pending");
    assert_eq!(replaced_msg.content, test_content);
}

// =========================================================================
// 6. Reações a mensagens e respostas
// =========================================================================

#[test]
fn test_e2e_message_reactions_and_replies() {
    let mut state = AppState::new();
    let channel_id = "chan-general";
    state.selected_channel_id = Some(channel_id.to_string());

    let target_msg_id = "msg-gen-1";

    if state.messages_by_channel.get(channel_id).is_none() {
        let seed = Message::new(
            target_msg_id,
            channel_id,
            Author::new("user-ferran", "ferran_rs"),
            "Olá pessoal! Sejam muito bem-vindos ao Discord RS 🦀🚀",
        )
        .with_reactions(vec![
            Reaction::new("🦀", 8, true),
            Reaction::new("❤️", 5, false),
            Reaction::new("🚀", 7, false),
        ]);
        state.inject_message(channel_id, seed);
    }

    // 6.1 Add reaction to existing message
    state.add_reaction_to_message(channel_id, target_msg_id, "🦀");

    let messages = state.messages_by_channel.get(channel_id).unwrap();
    let msg = messages.iter().find(|m| m.id == target_msg_id).expect("Target message must exist");
    let crab_rx = msg.reactions.iter().find(|r| r.emoji == "🦀").expect("Reaction 🦀 must exist");
    assert!(crab_rx.count >= 1);
    assert!(crab_rx.me, "Current user must be marked as reacted");

    // Add a second distinct reaction emoji
    state.add_reaction_to_message(channel_id, target_msg_id, "⚡");
    let messages = state.messages_by_channel.get(channel_id).unwrap();
    let msg = messages.iter().find(|m| m.id == target_msg_id).unwrap();
    let lightning_rx = msg.reactions.iter().find(|r| r.emoji == "⚡").expect("Reaction ⚡ must exist");
    assert_eq!(lightning_rx.count, 1);
    assert!(lightning_rx.me);

    // 6.2 Simulate remote reaction via TransportEvent::ReactionAdd
    state.handle_transport_event(TransportEvent::ReactionAdd {
        channel_id: channel_id.to_string(),
        message_id: target_msg_id.to_string(),
        user_id: "user-remote-42".to_string(),
        emoji: "🔥".to_string(),
    });

    let messages = state.messages_by_channel.get(channel_id).unwrap();
    let msg = messages.iter().find(|m| m.id == target_msg_id).unwrap();
    assert!(msg.reactions.iter().any(|r| r.emoji == "🔥"));

    // 6.3 Message replies lifecycle
    let reply_target = msg.clone();
    state.replying_to_message = Some(reply_target.clone());
    assert_eq!(state.replying_to_message.as_ref().map(|m| m.id.as_str()), Some(target_msg_id));

    // Sending a message automatically resets the reply context
    state.send_message(channel_id, "Respondendo à sua mensagem!");
    assert!(state.replying_to_message.is_none(), "Replying context must be cleared after send");
}

// =========================================================================
// 7. Abertura e fechamento de modais
// =========================================================================

#[test]
fn test_e2e_modal_dialogs_lifecycle() {
    let mut state = AppState::new();

    // Initial state: no modal open
    assert_eq!(state.active_modal, None);

    // 7.1 User Settings Modal
    state.open_modal(ActiveModal::UserSettings);
    assert_eq!(state.active_modal, Some(ActiveModal::UserSettings));
    assert_eq!(state.user_settings_tab, SettingsTab::MyAccount);

    // Tab switching inside Settings
    state.set_settings_tab(SettingsTab::Appearance);
    assert_eq!(state.user_settings_tab, SettingsTab::Appearance);
    state.set_settings_tab(SettingsTab::VoiceVideo);
    assert_eq!(state.user_settings_tab, SettingsTab::VoiceVideo);

    state.close_modal();
    assert_eq!(state.active_modal, None);

    // 7.2 Add Server Modal
    state.open_modal(ActiveModal::AddServer);
    assert_eq!(state.active_modal, Some(ActiveModal::AddServer));
    state.close_modal();
    assert_eq!(state.active_modal, None);

    // 7.3 Explore Servers Modal
    state.open_modal(ActiveModal::ExploreServers);
    assert_eq!(state.active_modal, Some(ActiveModal::ExploreServers));
    state.close_modal();
    assert_eq!(state.active_modal, None);

    // 7.4 Login Modal and auto-close on Gateway Connected event
    state.open_modal(ActiveModal::Login);
    assert_eq!(state.active_modal, Some(ActiveModal::Login));

    state.handle_transport_event(TransportEvent::Connected);
    assert_eq!(state.active_modal, None, "Login modal must auto-close upon successful connection");
    assert!(state.is_connected_to_gateway);
    assert!(!state.is_connecting);
    assert_eq!(state.auth_error, None);
}

// =========================================================================
// 8. Teste de estresse com volume elevado (1.000 mensagens e 500 membros)
// =========================================================================

#[test]
fn test_e2e_stress_high_volume_messages_and_members() {
    let mut state = AppState::new();
    let stress_channel_id = "chan-stress-1000";

    // 8.1 Generate 1,000 realistic messages
    let start_messages = Instant::now();
    let now = Utc::now();
    let mut high_volume_messages = Vec::with_capacity(1000);

    for i in 0..1000 {
        let author_name = match i % 5 {
            0 => "ferris_rust",
            1 => "ana_dev",
            2 => "carlos_sys",
            3 => "bot_helper",
            _ => "gpui_enthusiast",
        };
        let is_bot = i % 5 == 3;
        let author = Author {
            id: format!("user-stress-{}", i % 20),
            username: author_name.to_string(),
            global_name: Some(format!("User #{}", i % 20)),
            avatar: None,
            bot: is_bot,
        };

        let content = format!(
            "Mensagem de estresse #{:04}: validando performance no buffer de chat do GPUI.",
            i
        );

        let mut msg = Message::new(format!("stress-msg-{}", i), stress_channel_id, author, content)
            .with_timestamp(now - Duration::seconds(1000 - i as i64));

        if i % 10 == 0 {
            msg.reactions.push(Reaction::new("🦀", (i % 25 + 1) as u32, i % 2 == 0));
        }

        high_volume_messages.push(msg);
    }

    let elapsed_gen_messages = start_messages.elapsed();
    assert!(elapsed_gen_messages.as_millis() < 50, "Message generation took too long: {:?}", elapsed_gen_messages);

    // Ingest messages into AppState via batch update
    let start_inject = Instant::now();
    state.set_channel_messages(stress_channel_id, high_volume_messages);
    let elapsed_inject = start_inject.elapsed();
    assert!(elapsed_inject.as_millis() < 100, "Batch injection took too long: {:?}", elapsed_inject);

    // Verify channel message integrity
    let messages = state.messages_by_channel.get(stress_channel_id).expect("Channel messages missing");
    assert_eq!(messages.len(), 1000);
    assert_eq!(messages.first().unwrap().id, "stress-msg-0");
    assert_eq!(messages.last().unwrap().id, "stress-msg-999");

    // 8.2 Generate 500 guild members with diverse roles and presence
    let start_members = Instant::now();
    let mut high_volume_members = Vec::with_capacity(500);

    for i in 0..500 {
        let (role_group, role_color) = match i {
            0..=4 => ("ADMINISTRADOR", 0x5865f2),
            5..=24 => ("MODERADOR", 0x23a55a),
            25..=49 => ("BOTS", 0x9b59b6),
            _ => ("MEMBROS", 0xdbdee1),
        };

        let presence = match i % 4 {
            0 => PresenceStatus::Online,
            1 => PresenceStatus::Idle,
            2 => PresenceStatus::DoNotDisturb,
            _ => PresenceStatus::Offline,
        };

        let member = MemberItemState {
            id: format!("member-stress-{}", i),
            name: format!("Rustacean #{:03}", i),
            avatar_text: format!("{}", (b'A' + (i % 26) as u8) as char),
            presence,
            role_group: role_group.to_string(),
            role_color,
            is_bot: (25..=49).contains(&i),
            activity: if i % 3 == 0 {
                Some(format!("Codando em Rust #{:02}", i % 10))
            } else {
                None
            },
        };
        high_volume_members.push(member);
    }

    state.members = high_volume_members;
    let elapsed_members = start_members.elapsed();
    assert!(elapsed_members.as_millis() < 30, "Member generation took too long: {:?}", elapsed_members);
    assert_eq!(state.members.len(), 500);

    // 8.3 Verify state queries under high load
    let online_members_count = state.members.iter().filter(|m| m.presence == PresenceStatus::Online).count();
    assert_eq!(online_members_count, 125);

    let admin_count = state.members.iter().filter(|m| m.role_group == "ADMINISTRADOR").count();
    assert_eq!(admin_count, 5);

    // Ingest 50 sequential rapid messages (simulating high-throughput chat during live event)
    let start_rapid = Instant::now();
    for i in 1000..1050 {
        let rapid_msg = Message::new(
            format!("rapid-msg-{}", i),
            stress_channel_id,
            Author::new("user-rapid", "rapid_bot"),
            format!("Live event stream message #{}", i),
        );
        state.handle_transport_event(TransportEvent::MessageCreate(rapid_msg));
    }
    let elapsed_rapid = start_rapid.elapsed();
    assert!(elapsed_rapid.as_millis() < 25, "Rapid sequential additions took too long: {:?}", elapsed_rapid);

    let final_messages = state.messages_by_channel.get(stress_channel_id).unwrap();
    assert_eq!(final_messages.len(), 1050);
}

// =========================================================================
// 9. Ordenação de DMs por fixadas (pinned) e recência (last_message_id)
// =========================================================================

#[test]
fn test_e2e_pinned_and_recency_sorting_dms() {
    let mut state = AppState::new();
    state.dm_channels.clear();
    state.select_guild(None);

    let mut dm_old = Channel::text("dm-old", "Old Friend");
    dm_old.last_message_id = Some("100000000000000000".to_string());
    dm_old.is_pinned = false;

    let mut dm_recent = Channel::text("dm-recent", "Recent Friend");
    dm_recent.last_message_id = Some("300000000000000000".to_string());
    dm_recent.is_pinned = false;

    let mut dm_mid = Channel::text("dm-mid", "Mid Friend");
    dm_mid.last_message_id = Some("200000000000000000".to_string());
    dm_mid.is_pinned = false;

    let mut dm_pinned = Channel::text("dm-pinned", "VIP Pinned Friend");
    dm_pinned.last_message_id = Some("150000000000000000".to_string());
    dm_pinned.is_pinned = true;

    let mut dm_pinned_recent = Channel::text("dm-pinned-recent", "VIP Super Pinned");
    dm_pinned_recent.last_message_id = Some("250000000000000000".to_string());
    dm_pinned_recent.is_pinned = true;

    let mut dm_no_messages = Channel::text("dm-no-msg", "New Friend No Msg");
    dm_no_messages.last_message_id = None;
    dm_no_messages.is_pinned = false;

    // Dispatch in unordered sequence
    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![
        dm_old,
        dm_no_messages,
        dm_pinned,
        dm_recent,
        dm_pinned_recent,
        dm_mid,
    ]));

    assert_eq!(state.dm_channels.len(), 6);

    // 1. Pinned DMs must be strictly at the top, ordered by recency amongst themselves
    assert!(state.dm_channels[0].is_pinned);
    assert_eq!(state.dm_channels[0].id, "dm-pinned-recent");

    assert!(state.dm_channels[1].is_pinned);
    assert_eq!(state.dm_channels[1].id, "dm-pinned");

    // 2. Non-pinned DMs must be ordered by last_message_id descending
    assert!(!state.dm_channels[2].is_pinned);
    assert_eq!(state.dm_channels[2].id, "dm-recent");

    assert!(!state.dm_channels[3].is_pinned);
    assert_eq!(state.dm_channels[3].id, "dm-mid");

    assert!(!state.dm_channels[4].is_pinned);
    assert_eq!(state.dm_channels[4].id, "dm-old");

    // 3. Channels with no messages (None) at the bottom
    assert!(!state.dm_channels[5].is_pinned);
    assert_eq!(state.dm_channels[5].id, "dm-no-msg");
}
