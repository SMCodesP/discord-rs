//! UI screenshot harness (capture-only, never shipped in production).
//!
//! Renders the app in a deterministic state loaded from a synthetic JSON
//! fixture (`tests/fixtures/ui_shot/<screen>.json`) and opens a window with a
//! unique title so `scripts/ui-shot.sh` can find it and photograph it with
//! ImageMagick (`import -window <id>`).
//!
//! Offline guarantees (see docs/UI_SHOT.md for the full network-path audit):
//! - `XDG_CONFIG_HOME` is pointed at a fresh temp dir before any state is
//!   built, so no token (`auth.json`) and no guild cache is ever read.
//! - `DISCORD_TOKEN` / `DISCORD_TOKEN_IS_USER` are removed from the env.
//! - `AppState::new()` starts empty in production; `apply_fixture` populates
//!   fixture content and ensures `auth_token`, `client_manager` stay `None`.
//! - The view is mounted with `RootView::with_state` (no `ClientManager`,
//!   no event loop, no `connect_with_token` spawn).
//! - Fixtures carry no URLs at all (no avatar/icon/attachment URLs), so the
//!   media/image pipeline has nothing to fetch.
//!
//! Usage (normally via scripts/ui-shot.sh):
//!   ui_shot <fixture.json> <title> <WIDTHxHEIGHT>

extern crate gpui_kit as gpui;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use gpui::*;
use gpui::component::input::InputState;
use serde::Deserialize;

use discord_rs::app::{
    ActiveModal, AppState, CategoryItemState, ChannelItemState, ChannelKind, FriendItemState,
    FriendsTab, GuildItemState, MemberItemState, PresenceStatus, RootView, SettingsTab,
};
use discord_rs::chat::media_picker::MediaPickerTab;
use discord_rs::components::{Avatar, AvatarSize, Banner, BannerVariant, Button, ButtonSize,
    ButtonVariant, IconButton, TextInput};
use discord_rs::domain::message::{Attachment, Author, Message, Reaction};
use discord_rs::domain::profile::{MutualGuild, ProfilePopoutState, UserProfileData};
use discord_rs::domain::settings::{AccountConnection, NitroTier};
use discord_rs::domain::user::{AvatarDecorationData, User};
use discord_rs::protocol::ConnectionMode;
use discord_rs::tokens::colors::{
    text_muted, text_normal, text_titles, HEX_BG_CHAT,
};
use discord_rs::tokens::spacing::{space_2, space_3, space_4};
use discord_rs::tokens::typography::{font_size_sm, font_size_xs, WEIGHT_BOLD};

// Fixed clock for deterministic message timestamps: every fixture message
// timestamp is BASE minus `minutes_ago`.
const BASE_RFC3339: &str = "2026-01-05T12:00:00Z";

fn base_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(BASE_RFC3339)
        .expect("const base time parses")
        .with_timezone(&Utc)
}

// ---------------------------------------------------------------------------
// Fixture schema (synthetic capture data only)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
struct FixtureUser {
    #[serde(default)]
    id: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    presence: String,
    #[serde(default)]
    custom_status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureChannel {
    id: String,
    name: String,
    kind: String,
    #[serde(default)]
    topic: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureCategory {
    id: String,
    name: String,
    #[serde(default)]
    channels: Vec<FixtureChannel>,
}

#[derive(Debug, Deserialize)]
struct FixtureEmoji {
    id: String,
    name: String,
    #[serde(default)]
    animated: bool,
}

#[derive(Debug, Deserialize)]
struct FixtureGuild {
    id: String,
    name: String,
    acronym: String,
    #[serde(default)]
    categories: Vec<FixtureCategory>,
    #[serde(default)]
    emojis: Vec<FixtureEmoji>,
}

#[derive(Debug, Deserialize)]
struct FixtureDm {
    id: String,
    name: String,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    recipient_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureAttachment {
    filename: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureReaction {
    emoji: String,
    #[serde(default)]
    count: u32,
    #[serde(default)]
    me: bool,
}

#[derive(Debug, Deserialize)]
struct FixtureMessage {
    id: String,
    author_id: String,
    author_username: String,
    #[serde(default)]
    author_display: String,
    #[serde(default)]
    bot: bool,
    #[serde(default)]
    text: String,
    #[serde(default)]
    minutes_ago: i64,
    #[serde(default)]
    attachments: Vec<FixtureAttachment>,
    #[serde(default)]
    reactions: Vec<FixtureReaction>,
}

#[derive(Debug, Deserialize)]
struct FixtureMember {
    id: String,
    name: String,
    #[serde(default)]
    presence: String,
    #[serde(default)]
    role_group: String,
    #[serde(default)]
    role_color: u32,
    #[serde(default)]
    is_bot: bool,
    #[serde(default)]
    activity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureFriend {
    id: String,
    username: String,
    display_name: String,
    #[serde(default)]
    presence: String,
    #[serde(default)]
    custom_status: Option<String>,
    /// Discord relationship type: 1 friend (Online/All tabs), 2 blocked,
    /// 3 incoming request, 4 outgoing request (Pending tab).
    #[serde(default = "default_rel_type")]
    relationship_type: u32,
}

fn default_rel_type() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
struct FixturePopout {
    user_id: String,
    username: String,
    display_name: String,
    #[serde(default)]
    presence: String,
    #[serde(default)]
    bio: Option<String>,
    #[serde(default)]
    mutual_guilds: Vec<FixtureMutualGuild>,
    #[serde(default)]
    mutual_friends_count: u32,
    #[serde(default)]
    anchor_x: Option<f32>,
    #[serde(default)]
    anchor_y: Option<f32>,
    #[serde(default)]
    modal_open: bool,
}

#[derive(Debug, Deserialize)]
struct FixtureMutualGuild {
    id: String,
    #[serde(default)]
    nick: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureSearchGroup {
    channel_id: String,
    #[serde(default)]
    channel_name: Option<String>,
    hit: FixtureMessage,
    #[serde(default)]
    context_before: Vec<FixtureMessage>,
    #[serde(default)]
    context_after: Vec<FixtureMessage>,
}

#[derive(Debug, Deserialize)]
struct FixtureMediaPicker {
    #[serde(default)]
    tab: String,
}

#[derive(Debug, Deserialize)]
struct FixtureConnection {
    #[serde(default)]
    platform: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    show_on_profile: bool,
    #[serde(default)]
    show_activity: bool,
}

#[derive(Debug, Deserialize)]
struct ShotFixture {
    screen: String,
    #[serde(default)]
    connected: bool,
    #[serde(default)]
    connection_status: Option<String>,
    #[serde(default)]
    selected_guild_id: Option<String>,
    #[serde(default)]
    selected_channel_id: Option<String>,
    #[serde(default = "default_true")]
    show_members: bool,
    #[serde(default = "default_online_tab")]
    friends_tab: String,
    #[serde(default)]
    current_user: FixtureUser,
    #[serde(default)]
    guilds: Vec<FixtureGuild>,
    #[serde(default)]
    dm_channels: Vec<FixtureDm>,
    #[serde(default)]
    messages: HashMap<String, Vec<FixtureMessage>>,
    #[serde(default)]
    members: Vec<FixtureMember>,
    #[serde(default)]
    friends: Vec<FixtureFriend>,
    #[serde(default)]
    active_modal: Option<String>,
    #[serde(default = "default_settings_tab")]
    user_settings_tab: String,
    #[serde(default)]
    profile_popout: Option<FixturePopout>,
    #[serde(default)]
    is_search_open: bool,
    #[serde(default)]
    search_query: String,
    #[serde(default)]
    search_results: Vec<FixtureSearchGroup>,
    #[serde(default)]
    media_picker: Option<FixtureMediaPicker>,
    #[serde(default)]
    connections: Vec<FixtureConnection>,
    /// Prefill for the AddFriend tab input (friends_add screen).
    #[serde(default)]
    add_friend_input: String,
    /// Selects the "Conta de Usuário" account type (login modal).
    #[serde(default)]
    is_user_token: bool,
}

fn default_true() -> bool {
    true
}

fn default_online_tab() -> String {
    "Online".to_string()
}

fn default_settings_tab() -> String {
    "MyAccount".to_string()
}

// ---------------------------------------------------------------------------
// Mapping helpers (fixture JSON -> public AppState APIs)
// ---------------------------------------------------------------------------

fn parse_presence(s: &str) -> PresenceStatus {
    match s {
        "Online" => PresenceStatus::Online,
        "Idle" => PresenceStatus::Idle,
        "DoNotDisturb" | "Dnd" | "DoNotDisturb " => PresenceStatus::DoNotDisturb,
        _ => PresenceStatus::Offline,
    }
}

fn parse_domain_presence(s: &str) -> discord_rs::domain::user::PresenceStatus {
    match s {
        "Online" => discord_rs::domain::user::PresenceStatus::Online,
        "Idle" => discord_rs::domain::user::PresenceStatus::Idle,
        "DoNotDisturb" | "Dnd" => discord_rs::domain::user::PresenceStatus::DoNotDisturb,
        _ => discord_rs::domain::user::PresenceStatus::Offline,
    }
}

fn parse_channel_kind(s: &str) -> ChannelKind {
    match s {
        "Voice" => ChannelKind::Voice,
        "Announcement" => ChannelKind::Announcement,
        _ => ChannelKind::Text,
    }
}

fn parse_friends_tab(s: &str) -> FriendsTab {
    match s {
        "All" => FriendsTab::All,
        "Pending" => FriendsTab::Pending,
        "Blocked" => FriendsTab::Blocked,
        "AddFriend" => FriendsTab::AddFriend,
        "MessageRequests" => FriendsTab::MessageRequests,
        _ => FriendsTab::Online,
    }
}

fn parse_settings_tab(s: &str) -> SettingsTab {
    match s {
        "Profiles" => SettingsTab::Profiles,
        "Privacy" => SettingsTab::Privacy,
        "AuthorizedApps" => SettingsTab::AuthorizedApps,
        "Connections" => SettingsTab::Connections,
        "Nitro" => SettingsTab::Nitro,
        "ServerBoost" => SettingsTab::ServerBoost,
        "FamilyCenter" => SettingsTab::FamilyCenter,
        "Appearance" => SettingsTab::Appearance,
        "Accessibility" => SettingsTab::Accessibility,
        "VoiceVideo" => SettingsTab::VoiceVideo,
        "Notifications" => SettingsTab::Notifications,
        "Keybinds" => SettingsTab::Keybinds,
        "Language" => SettingsTab::Language,
        _ => SettingsTab::MyAccount,
    }
}

fn parse_media_tab(s: &str) -> MediaPickerTab {
    match s {
        "Stickers" => MediaPickerTab::Stickers,
        "Gifs" => MediaPickerTab::Gifs,
        "Soundboard" => MediaPickerTab::Soundboard,
        _ => MediaPickerTab::Emojis,
    }
}

fn build_message(channel_id: &str, f: &FixtureMessage, base: DateTime<Utc>) -> Message {
    let mut author = Author::new(f.author_id.clone(), f.author_username.clone());
    if !f.author_display.is_empty() {
        author.global_name = Some(f.author_display.clone());
    }
    author.bot = f.bot;
    let attachments = f
        .attachments
        .iter()
        .enumerate()
        .map(|(i, a)| Attachment {
            id: format!("att-{}-{}", f.id, i),
            filename: a.filename.clone(),
            size: a.size,
            // Never fetched: the chat renderer only reads filename/size for
            // file cards (message_list.rs render_attachments), and fixtures
            // use non-image content types so no preview is attempted.
            url: String::new(),
            proxy_url: None,
            content_type: a.content_type.clone(),
            width: None,
            height: None,
        })
        .collect::<Vec<_>>();
    let reactions = f
        .reactions
        .iter()
        .map(|r| Reaction::new(r.emoji.clone(), r.count, r.me))
        .collect::<Vec<_>>();
    Message::new(f.id.clone(), channel_id, author, f.text.clone())
        .with_timestamp(base - chrono::Duration::minutes(f.minutes_ago))
        .with_attachments(attachments)
        .with_reactions(reactions)
}

/// Builds the full AppState from a fixture, starting from the empty production
/// state `AppState::new()`: every collection is populated from fixture content.
fn apply_fixture(f: &ShotFixture) -> AppState {
    let mut s = AppState::new();

    // --- Hard offline: no token, no gateway, no disk persistence -----------
    s.auth_token = None;
    s.token_input = String::new();
    s.is_user_token = f.is_user_token;
    s.save_token_to_disk = false;
    s.is_connecting = false;
    s.is_connected_to_gateway = f.connected;
    s.connection_status = f.connection_status.clone().unwrap_or_else(|| {
        if f.connected {
            "Conectado (captura local)".to_string()
        } else {
            "Não conectado (captura local)".to_string()
        }
    });
    s.connection_mode = ConnectionMode::Mock;
    s.client_manager = None;
    // None disables pinned-DM disk persistence.
    s.pinned_dms_path = None;

    // --- Replace seed collections with fixture content --------------------
    s.guilds = f
        .guilds
        .iter()
        .map(|g| GuildItemState {
            id: g.id.clone(),
            name: g.name.clone(),
            acronym: g.acronym.clone(),
            icon_url: None,
            icon_hash: None,
            unread: false,
            mention_count: 0,
            categories: g
                .categories
                .iter()
                .map(|c| CategoryItemState {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    channels: c
                        .channels
                        .iter()
                        .map(|ch| ChannelItemState {
                            avatar_decoration_data: None,
                            id: ch.id.clone(),
                            name: ch.name.clone(),
                            kind: parse_channel_kind(&ch.kind),
                            topic: ch.topic.clone(),
                            unread: false,
                            mention_count: 0,
                            icon_url: None,
                            is_pinned: false,
                            last_message_id: None,
                            is_group_dm: false,
                            recipient_ids: Vec::new(),
                            recipient_names: Vec::new(),
                            user_limit: None,
                        })
                        .collect(),
                })
                .collect(),
            emojis: g
                .emojis
                .iter()
                .map(|e| {
                    discord_rs::protocol::media_cache::DiscordEmoji::new_custom(
                        e.id.clone(),
                        e.name.clone(),
                        e.animated,
                        Some(g.id.clone()),
                    )
                })
                .collect(),
            stickers: Vec::new(),
        })
        .collect();

    s.dm_channels = f
        .dm_channels
        .iter()
        .map(|d| ChannelItemState {
            avatar_decoration_data: None,
            id: d.id.clone(),
            name: d.name.clone(),
            kind: ChannelKind::Text,
            topic: d.topic.clone(),
            unread: false,
            mention_count: 0,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            is_group_dm: d.recipient_names.len() > 1,
            recipient_ids: Vec::new(),
            recipient_names: d.recipient_names.clone(),
            user_limit: None,
        })
        .collect();

    let base = base_time();
    s.messages_by_channel = HashMap::new();
    for (channel_id, msgs) in &f.messages {
        s.messages_by_channel.insert(
            channel_id.clone(),
            msgs.iter()
                .map(|m| build_message(channel_id, m, base))
                .collect(),
        );
    }

    s.members = f
        .members
        .iter()
        .map(|m| MemberItemState {
            avatar_decoration_data: None,
            id: m.id.clone(),
            name: m.name.clone(),
            avatar_text: m.name.chars().next().unwrap_or('?').to_string(),
            presence: parse_presence(&m.presence),
            role_group: m.role_group.clone(),
            role_color: m.role_color,
            is_bot: m.is_bot,
            activity: m.activity.clone(),
        })
        .collect();

    s.friends = f
        .friends
        .iter()
        .map(|fr| FriendItemState {
            avatar_decoration_data: None,
            id: fr.id.clone(),
            username: fr.username.clone(),
            display_name: fr.display_name.clone(),
            avatar_url: None,
            avatar_text: fr.display_name.chars().next().unwrap_or('?').to_string(),
            presence: parse_presence(&fr.presence),
            custom_status: fr.custom_status.clone(),
            relationship_type: fr.relationship_type,
        })
        .collect();
    s.friends_tab = parse_friends_tab(&f.friends_tab);

    // Deselected seed lists that have no fixture source on these screens.
    s.message_requests = Vec::new();
    s.active_now_items = Vec::new();
    s.presences_by_user = HashMap::new();
    s.pinned_dms = Vec::new();
    s.guild_folders = Vec::new();
    s.expanded_folders = HashSet::new();
    s.collapsed_categories = HashSet::new();
    s.typing_tracker = HashMap::new();
    s.typing_users = Vec::new();
    s.voice_states_by_channel = HashMap::new();
    s.active_voice_call = None;

    // --- Navigation / overlays --------------------------------------------
    s.selected_guild_id = f.selected_guild_id.clone();
    s.selected_channel_id = f.selected_channel_id.clone();
    s.show_members = f.show_members;
    s.active_modal = match f.active_modal.as_deref() {
        Some("UserSettings") => Some(ActiveModal::UserSettings),
        Some("AddServer") => Some(ActiveModal::AddServer),
        Some("ExploreServers") => Some(ActiveModal::ExploreServers),
        Some("Login") => Some(ActiveModal::Login),
        Some("LoginDiscord") => Some(ActiveModal::LoginDiscord),
        Some("CreateGroupDm") => Some(ActiveModal::CreateGroupDm),
        // "AddFriendToGroup:<channel_id>" — needs the DM channel to exist in
        // the fixture so the modal can list recipients.
        Some(other) => match other.strip_prefix("AddFriendToGroup:") {
            Some(channel_id) => Some(ActiveModal::AddFriendToGroup(channel_id.to_string())),
            None => panic!("ui_shot: unsupported active_modal {:?}", other),
        },
        None => None,
    };
    s.user_settings_tab = parse_settings_tab(&f.user_settings_tab);

    s.current_user = discord_rs::app::CurrentUserProfile {
        avatar_decoration_data: None,
        id: if f.current_user.id.is_empty() {
            "u-me".to_string()
        } else {
            f.current_user.id.clone()
        },
        username: f.current_user.username.clone(),
        display_name: if f.current_user.display_name.is_empty() {
            f.current_user.username.clone()
        } else {
            f.current_user.display_name.clone()
        },
        discriminator: None,
        avatar_text: f
            .current_user
            .display_name
            .chars()
            .next()
            .unwrap_or('U')
            .to_string(),
        avatar_url: None,
        custom_status: f.current_user.custom_status.clone(),
        presence: parse_presence(&f.current_user.presence),
        is_muted: false,
        is_deafened: false,
    };

    // Seed settings subsystems carry developer-machine data (connections with
    // real handles, boosts pointing at seed guilds): drop them, keep only
    // fixture-provided synthetic connections.
    s.connections = f
        .connections
        .iter()
        .map(|c| AccountConnection {
            platform: c.platform.clone(),
            id: c.id.clone(),
            name: c.name.clone(),
            verified: c.verified,
            show_on_profile: c.show_on_profile,
            show_activity: c.show_activity,
        })
        .collect();
    s.authorized_apps = Vec::new();
    s.server_boosts = Vec::new();
    s.family_center_links = Vec::new();
    // No subscription in fixtures: tier None keeps the Nitro tab coherent
    // ("Sem Assinatura") instead of mixing seed Full with subscription None.
    s.nitro_tier = NitroTier::None;
    s.active_subscription = None;

    s.profile_popout = f.profile_popout.as_ref().map(|p| {
        let mut user = User::new(p.user_id.clone(), p.username.clone());
        user.global_name = Some(p.display_name.clone());
        user.presence = parse_domain_presence(&p.presence);
        ProfilePopoutState {
            user_id: p.user_id.clone(),
            guild_id: f.selected_guild_id.clone(),
            data: Some(UserProfileData {
                user,
                bio: p.bio.clone(),
                pronouns: None,
                banner: None,
                banner_color: None,
                accent_color: None,
                profile_effect_id: None,
                badges: Vec::new(),
                connected_accounts: Vec::new(),
                mutual_guilds: p
                    .mutual_guilds
                    .iter()
                    .map(|g| MutualGuild {
                        id: g.id.clone(),
                        nick: g.nick.clone(),
                    })
                    .collect(),
                mutual_friends_count: p.mutual_friends_count,
                created_at: None,
                joined_server_at: None,
            }),
            is_loading: false,
            error: None,
            active_tab: Default::default(),
            quick_message: String::new(),
        }
    });

    if let Some(ref p) = f.profile_popout {
        if let (Some(ax), Some(ay)) = (p.anchor_x, p.anchor_y) {
            s.profile_anchor = Some(point(px(ax), px(ay)));
        }
        if p.modal_open || f.screen == "profile_modal" {
            s.profile_modal_open = true;
        }
    }

    s.is_search_open = f.is_search_open;
    s.search_query = f.search_query.clone();
    s.is_searching = false;
    s.search_results = f
        .search_results
        .iter()
        .map(|g| discord_rs::chat::search::SearchResultGroup {
            channel_id: g.channel_id.clone(),
            channel_name: g.channel_name.clone(),
            hit_message: build_message(&g.channel_id, &g.hit, base),
            context_before: g
                .context_before
                .iter()
                .map(|m| build_message(&g.channel_id, m, base))
                .collect(),
            context_after: g
                .context_after
                .iter()
                .map(|m| build_message(&g.channel_id, m, base))
                .collect(),
        })
        .collect();

    s.target_message_id = None;
    s.replying_to_message = None;
    s.active_draft = String::new();
    s.auth_error = None;
    s.add_friend_input = f.add_friend_input.clone();
    s.add_friend_feedback = None;
    s.is_sending_friend_request = false;
    s.pending_relationship_actions = HashSet::new();

    s
}

// ---------------------------------------------------------------------------
// Gallery screen: deterministic component sheet (fixture screen "gallery").
//
// Uses only discord_rs::components public APIs; every item carries a small
// label with its exact variant name. No hover states (static capture).
// No Modal: src/components/modal.rs is dead code being deleted elsewhere.
// No avatar decoration: AvatarDecorationData::url() always builds a CDN
// https URL (src/domain/user.rs), so offline only undecorated avatars can
// be shown.
// ---------------------------------------------------------------------------

fn gallery_label(name: &str) -> AnyElement {
    div()
        .text_size(font_size_xs())
        .text_color(text_muted())
        .child(name.to_string())
        .into_any_element()
}

fn gallery_section(title: &str, body: AnyElement) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(space_2())
        .child(
            div()
                .text_size(font_size_sm())
                .font_weight(WEIGHT_BOLD)
                .text_color(text_titles())
                .child(title.to_string()),
        )
        .child(body)
        .into_any_element()
}

fn gallery_labeled(name: String, item: AnyElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(space_2())
        .child(gallery_label(&name))
        .child(item)
        .into_any_element()
}

fn gallery_buttons() -> AnyElement {
    let variants = [
        ("Blurple", ButtonVariant::Blurple),
        ("Gray", ButtonVariant::Gray),
        ("Danger", ButtonVariant::Danger),
        ("Ghost", ButtonVariant::Ghost),
        ("Link", ButtonVariant::Link),
    ];
    let sizes = [
        ("Sm", ButtonSize::Sm),
        ("Md", ButtonSize::Md),
        ("Lg", ButtonSize::Lg),
    ];
    let mut columns = Vec::new();
    for (vname, variant) in variants {
        let mut cells = Vec::new();
        for (sname, size) in sizes {
            for disabled in [false, true] {
                for with_icon in [false, true] {
                    let id = format!("gal-btn-{}-{}-{}-{}", vname, sname, disabled, with_icon);
                    let label = format!(
                        "{} {} {}{}",
                        vname,
                        sname,
                        if disabled { "dis" } else { "ok" },
                        if with_icon { " ★" } else { "" }
                    );
                    let mut btn = Button::new(id, label)
                        .variant(variant)
                        .size(size)
                        .disabled(disabled);
                    if with_icon {
                        btn = btn.icon("✚");
                    }
                    cells.push(btn.into_any_element());
                }
            }
        }
        let mut col = div().flex_1().flex().flex_col().gap(px(2.0));
        col = col.child(
            div()
                .text_size(font_size_xs())
                .font_weight(WEIGHT_BOLD)
                .text_color(text_muted())
                .child(vname.to_string()),
        );
        for c in cells {
            col = col.child(c);
        }
        columns.push(col.into_any_element());
    }
    let mut row = div().w_full().flex().flex_row().gap(space_2());
    for c in columns {
        row = row.child(c);
    }
    gallery_section(
        "Button — 5 variantes x 3 tamanhos x normal/disabled x com/sem ícone",
        row.into_any_element(),
    )
}

fn gallery_icon_buttons() -> AnyElement {
    let states = [
        ("Normal", false, false, false),
        ("Active", true, false, false),
        ("Danger", false, true, false),
        ("Disabled", false, false, true),
    ];
    let mut col = div().w_full().flex().flex_col().gap(space_2());

    // Fileira 1: Ícone por glifo de texto
    let mut row_glyph = div().w_full().flex().flex_row().gap(space_4()).items_center();
    for (i, (name, active, danger, disabled)) in states.iter().enumerate() {
        let btn = IconButton::new(format!("gal-iconbtn-{}", i), "✕")
            .active(*active)
            .danger(*danger)
            .disabled(*disabled);
        row_glyph = row_glyph.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space_2())
                .child(btn)
                .child(
                    div()
                        .text_size(font_size_xs())
                        .text_color(text_muted())
                        .child(name.to_string()),
                ),
        );
    }
    col = col.child(row_glyph);

    // Fileira 2: Ícone por elemento SVG (with_icon)
    let mut row_svg = div().w_full().flex().flex_row().gap(space_4()).items_center();
    for (i, (name, active, danger, disabled)) in states.iter().enumerate() {
        let btn = IconButton::with_icon(format!("gal-iconbtn-svg-{}", i), |sz, col| {
            discord_rs::tokens::icons::icon_bell(sz, col).into_any_element()
        })
        .active(*active)
        .danger(*danger)
        .disabled(*disabled);
        row_svg = row_svg.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space_2())
                .child(btn)
                .child(
                    div()
                        .text_size(font_size_xs())
                        .text_color(text_muted())
                        .child(format!("{} (SVG)", name)),
                ),
        );
    }
    col = col.child(row_svg);

    gallery_section(
        "IconButton — normal / active / danger / disabled (glifo + SVG)",
        col.into_any_element(),
    )
}

fn gallery_text_inputs(
    empty: &Entity<InputState>,
    with_text: &Entity<InputState>,
    with_error: &Entity<InputState>,
    masked: &Entity<InputState>,
) -> AnyElement {
    let empty_inp = TextInput::new("gal-input-empty", empty)
        .label("TEXTINPUT VAZIO")
        .placeholder("Placeholder de exemplo");
    let text_inp = TextInput::new("gal-input-text", with_text)
        .label("TEXTINPUT PREENCHIDO")
        .placeholder("Placeholder de exemplo");
    let err_inp = TextInput::new("gal-input-error", with_error)
        .label("TEXTINPUT COM ERRO")
        .placeholder("Placeholder de exemplo")
        .error("Campo obrigatório ou formato inválido");
    let secret_inp = TextInput::new("gal-input-masked", masked)
        .label("TEXTINPUT SECRETO")
        .placeholder("Placeholder de exemplo")
        .secret(true);

    let mut col = div().w_full().flex().flex_col().gap(space_2());
    col = col.child(
        div()
            .flex()
            .flex_row()
            .gap(space_2())
            .child(div().flex_1().child(empty_inp))
            .child(div().flex_1().child(text_inp)),
    );
    col = col.child(
        div()
            .flex()
            .flex_row()
            .gap(space_2())
            .child(div().flex_1().child(err_inp))
            .child(div().flex_1().child(secret_inp)),
    );
    gallery_section("TextInput — vazio / preenchido / erro / secreto", col.into_any_element())
}

fn gallery_avatars() -> AnyElement {
    let decor = AvatarDecorationData::new("decor_preset_gallery");
    let mut col = div().w_full().flex().flex_col().gap(space_3());

    // Px80
    let mut row80 = div().w_full().flex().flex_row().gap(space_4()).items_center();
    row80 = row80.child(gallery_labeled("80 sem status".to_string(), Avatar::new("UA").size(AvatarSize::Px80).into_any_element()));
    row80 = row80.child(gallery_labeled("80 online".to_string(), Avatar::new("UA").size(AvatarSize::Px80).online().into_any_element()));
    row80 = row80.child(gallery_labeled("80 dnd".to_string(), Avatar::new("UA").size(AvatarSize::Px80).dnd().into_any_element()));
    row80 = row80.child(gallery_labeled("80 decor".to_string(), Avatar::new("UA").size(AvatarSize::Px80).decoration(decor.clone()).into_any_element()));
    row80 = row80.child(gallery_labeled("80 online+decor".to_string(), Avatar::new("UA").size(AvatarSize::Px80).online().decoration(decor.clone()).into_any_element()));
    col = col.child(row80);

    // Px40
    let mut row40 = div().w_full().flex().flex_row().gap(space_3()).items_center();
    row40 = row40.child(gallery_labeled("40 sem status".to_string(), Avatar::new("UA").size(AvatarSize::Px40).into_any_element()));
    row40 = row40.child(gallery_labeled("40 online".to_string(), Avatar::new("UA").size(AvatarSize::Px40).online().into_any_element()));
    row40 = row40.child(gallery_labeled("40 idle".to_string(), Avatar::new("UA").size(AvatarSize::Px40).idle().into_any_element()));
    row40 = row40.child(gallery_labeled("40 dnd".to_string(), Avatar::new("UA").size(AvatarSize::Px40).dnd().into_any_element()));
    row40 = row40.child(gallery_labeled("40 offline".to_string(), Avatar::new("UA").size(AvatarSize::Px40).offline().into_any_element()));
    row40 = row40.child(gallery_labeled("40 decor".to_string(), Avatar::new("UA").size(AvatarSize::Px40).decoration(decor.clone()).into_any_element()));
    row40 = row40.child(gallery_labeled("40 on+decor".to_string(), Avatar::new("UA").size(AvatarSize::Px40).online().decoration(decor.clone()).into_any_element()));
    col = col.child(row40);

    // Px32 & Px24 side-by-side
    let mut row_compact = div().w_full().flex().flex_row().gap(space_3()).items_center();
    row_compact = row_compact.child(gallery_labeled("32 sem status".to_string(), Avatar::new("UA").size(AvatarSize::Px32).into_any_element()));
    row_compact = row_compact.child(gallery_labeled("32 online".to_string(), Avatar::new("UA").size(AvatarSize::Px32).online().into_any_element()));
    row_compact = row_compact.child(gallery_labeled("32 decor".to_string(), Avatar::new("UA").size(AvatarSize::Px32).decoration(decor.clone()).into_any_element()));
    row_compact = row_compact.child(gallery_labeled("24 sem status".to_string(), Avatar::new("UA").size(AvatarSize::Px24).into_any_element()));
    row_compact = row_compact.child(gallery_labeled("24 online".to_string(), Avatar::new("UA").size(AvatarSize::Px24).online().into_any_element()));
    row_compact = row_compact.child(gallery_labeled("24 decor".to_string(), Avatar::new("UA").size(AvatarSize::Px24).decoration(decor.clone()).into_any_element()));
    col = col.child(row_compact);

    gallery_section("Avatar — 4 tamanhos x com/sem status x com/sem decoração", col.into_any_element())
}

fn gallery_banners() -> AnyElement {
    let mut grid = div().w_full().flex().flex_col().gap(space_2());
    let b1 = Banner::new("gal-b-info", "Mensagem de informação da variante")
        .variant(BannerVariant::Info)
        .title("Banner Info");
    let b2 = Banner::new("gal-b-succ", "Operação concluída com sucesso")
        .variant(BannerVariant::Success)
        .title("Banner Success");
    let b3 = Banner::new("gal-b-warn", "Atenção: verifique suas permissões")
        .variant(BannerVariant::Warning)
        .title("Banner Warning");
    let b4 = Banner::new("gal-b-dang", "Erro crítico: falha na conexão")
        .variant(BannerVariant::Danger)
        .title("Banner Danger");

    grid = grid.child(
        div()
            .flex()
            .flex_row()
            .gap(space_2())
            .child(div().flex_1().child(b1))
            .child(div().flex_1().child(b2)),
    );
    grid = grid.child(
        div()
            .flex()
            .flex_row()
            .gap(space_2())
            .child(div().flex_1().child(b3))
            .child(div().flex_1().child(b4)),
    );
    gallery_section("Banner — 4 variantes", grid.into_any_element())
}

struct GalleryView {
    input_empty: Entity<InputState>,
    input_text: Entity<InputState>,
    input_error: Entity<InputState>,
    input_masked: Entity<InputState>,
}

fn gallery_content(gal: &GalleryView) -> AnyElement {
    div()
        .size_full()
        .bg(rgb(HEX_BG_CHAT))
        .text_color(text_normal())
        .overflow_hidden()
        .flex()
        .flex_col()
        .p(space_3())
        .gap(space_2())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(font_size_sm())
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_titles())
                        .child("Galeria de componentes próprios (comparação visual com referências button-*.png)"),
                )
                .child(
                    div()
                        .text_size(font_size_xs())
                        .text_color(text_muted())
                        .child("Button | IconButton | TextInput | Banner | Avatar (sem Modal)"),
                ),
        )
        .child(
            div()
                .flex_1()
                .w_full()
                .flex()
                .flex_row()
                .gap(space_4())
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(space_3())
                        .child(gallery_buttons())
                        .child(gallery_icon_buttons())
                        .child(gallery_banners()),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(space_3())
                        .child(gallery_text_inputs(
                            &gal.input_empty,
                            &gal.input_text,
                            &gal.input_error,
                            &gal.input_masked,
                        ))
                        .child(gallery_avatars()),
                ),
        )
        .into_any_element()
}

/// Single view type for `open_window`: either the app shell or the gallery.
enum ShotRoot {
    App(Entity<RootView>),
    Gallery(GalleryView),
}

impl Render for ShotRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self {
            ShotRoot::App(root) => root.clone().into_any_element(),
            ShotRoot::Gallery(gal) => gallery_content(gal),
        }
    }
}

impl Render for GalleryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gallery_content(self)
    }
}

fn parse_size(arg: &str) -> (f32, f32) {
    let (w, h) = arg
        .split_once('x')
        .unwrap_or_else(|| panic!("ui_shot: size must look like 1280x800, got {:?}", arg));
    let w: f32 = w
        .parse()
        .unwrap_or_else(|_| panic!("ui_shot: bad width {:?}", w));
    let h: f32 = h
        .parse()
        .unwrap_or_else(|_| panic!("ui_shot: bad height {:?}", h));
    (w, h)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: ui_shot <fixture.json> <window-title> <WIDTHxHEIGHT>");
        std::process::exit(2);
    }
    let fixture_path = args[1].clone();
    let title = args[2].clone();
    let (width, height) = parse_size(&args[3]);
    let screen = std::path::Path::new(&fixture_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // --- Isolate from the user's disk and from any real account ------------
    // Must happen before AppState::new() (reads TokenStorage + guild cache).
    let sandbox = std::env::temp_dir().join(format!("discord-rs-uishot-{}", std::process::id()));
    std::fs::create_dir_all(&sandbox).expect("ui_shot: cannot create sandbox dir");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &sandbox);
    }
    unsafe {
        std::env::remove_var("DISCORD_TOKEN");
    }
    unsafe {
        std::env::remove_var("DISCORD_TOKEN_IS_USER");
    }

    let raw = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("ui_shot: cannot read {:?}: {}", fixture_path, e));
    let fixture: ShotFixture = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("ui_shot: bad fixture {:?}: {}", fixture_path, e));
    if fixture.screen != screen {
        eprintln!(
            "ui_shot: warning: fixture screen {:?} != file name {:?}",
            fixture.screen, screen
        );
    }
    let media_tab = fixture
        .media_picker
        .as_ref()
        .map(|m| parse_media_tab(&m.tab));

    let app_state = apply_fixture(&fixture);
    assert!(
        app_state.auth_token.is_none() && app_state.client_manager.is_none(),
        "ui_shot: must stay offline (no token, no client manager)"
    );

    eprintln!(
        "ui_shot: screen={} connected={} guilds={} dms={} msgs={} title={:?}",
        fixture.screen,
        fixture.connected,
        app_state.guilds.len(),
        app_state.dm_channels.len(),
        app_state
            .messages_by_channel
            .values()
            .map(|v| v.len())
            .sum::<usize>(),
        title,
    );

    platform::application()
        .with_http_client(Arc::new(discord_rs::protocol::ReqwestHttpClient::new()))
        .with_assets(discord_rs::tokens::icons::AppAssetSource)
        .run(move |cx: &mut App| {
            gpui::init(cx);
            // Same theme the app installs.
            discord_rs::tokens::kit_theme::install(cx);

            let window_bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.clone().into()),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                is_resizable: true,
                ..Default::default()
            };

            // with_state (not RootView::new): mounts a prebuilt fixture state
            // without creating a ClientManager or spawning any auto-login.
            // Screen "gallery" mounts the component sheet instead (no state).
            let moved_state = app_state;
            let moved_tab = media_tab;
            let is_gallery = fixture.screen == "gallery";
            let (_handle, _view) = gpui::open_window(window_options, cx, |window, cx| {
                if is_gallery {
                    let input_empty = cx.new(|cx| InputState::new(window, cx));
                    let input_text = cx.new(|cx| {
                        InputState::new(window, cx).default_value("Texto de exemplo")
                    });
                    let input_error = cx.new(|cx| {
                        InputState::new(window, cx).default_value("Valor inválido")
                    });
                    let input_masked = cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value("segredo-exemplo")
                            .masked(true)
                    });
                    cx.new(|_cx| {
                        ShotRoot::Gallery(GalleryView {
                            input_empty,
                            input_text,
                            input_error,
                            input_masked,
                        })
                    })
                } else {
                    let state_entity = cx.new(|_cx| moved_state);
                    let root = cx.new(|cx| RootView::with_state(state_entity, cx));
                    if let Some(tab) = moved_tab {
                        let chat = root.read(cx).chat_view.clone();
                        chat.update(cx, |v, _cx| {
                            v.media_picker_open = true;
                            v.media_picker_tab = tab;
                        });
                    }
                    cx.new(|_cx| ShotRoot::App(root))
                }
            })
            .expect("ui_shot: failed to open window");
            println!("UI_SHOT_WINDOW_OPEN {}", fixture.screen);
        });
}
