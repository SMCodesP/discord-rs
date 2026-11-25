use discord_rs::app::channel_sidebar::{
    close_dm_channel, is_channel_muted, mark_channel_read, open_nitro_settings,
    toggle_channel_mute,
};
use discord_rs::app::friends_hub::{open_server_discovery, should_show_active_now};
use discord_rs::app::member_sidebar::{mention_member, open_member_dm};
use discord_rs::app::state::{
    ActiveModal, ActiveNowItem, ActiveNowKind, AppState, CategoryItemState, ChannelItemState,
    ChannelKind, GuildItemState, PresenceStatus, SettingsTab,
};

fn channel(id: &str, unread: bool, mentions: u32) -> ChannelItemState {
    ChannelItemState {
        id: id.into(),
        name: id.into(),
        kind: ChannelKind::Text,
        topic: None,
        unread,
        mention_count: mentions,
        icon_url: None,
        is_pinned: false,
        last_message_id: None,
        last_read_message_id: None,
        is_group_dm: false,
        recipient_ids: Vec::new(),
        recipient_names: Vec::new(),
        user_limit: None,
        avatar_decoration_data: None,
        thread_metadata: None,
        default_tag_ids: Vec::new(),
        available_tags: Vec::new(),
        flags: 0,
        rate_limit_per_user: 0,
    }
}

fn dm(id: &str, name: &str) -> ChannelItemState {
    let mut ch = channel(id, false, 0);
    ch.name = name.into();
    ch.recipient_ids = vec![format!("user-{id}")];
    ch.recipient_names = vec![name.into()];
    ch
}

fn active_now(id: &str) -> ActiveNowItem {
    ActiveNowItem {
        id: id.into(),
        user_id: format!("user-{id}"),
        username: id.into(),
        display_name: id.into(),
        avatar_url: None,
        avatar_text: "A".into(),
        presence: PresenceStatus::Online,
        kind: ActiveNowKind::Game,
        app_name: "Game".into(),
        header_subtitle: "Game".into(),
        app_icon_symbol: String::new(),
        details_title: "Playing".into(),
        details_state: None,
        elapsed_time_text: None,
        spotify_track: None,
        spotify_artist: None,
        spotify_album: None,
        spotify_progress_text: None,
        spotify_progress_pct: None,
        button_text: None,
        large_image_url: None,
        large_image_text: None,
        small_image_url: None,
        small_image_text: None,
        application_id: None,
        timestamps_start: None,
        timestamps_end: None,
        avatar_decoration_data: None,
    }
}

#[test]
fn nitro_nav_opens_the_real_nitro_settings_tab() {
    let mut state = AppState::new();
    open_nitro_settings(&mut state);
    assert_eq!(state.active_modal, Some(ActiveModal::UserSettings));
    assert_eq!(state.user_settings_tab, SettingsTab::Nitro);
}

#[test]
fn close_dm_removes_row_pin_and_clears_selection() {
    let mut state = AppState::new();
    state.dm_channels = vec![dm("dm-1", "One"), dm("dm-2", "Two")];
    state.pinned_dms = vec!["dm-1".into(), "dm-2".into()];
    state.selected_channel_id = Some("dm-1".into());

    assert!(close_dm_channel(&mut state, "dm-1"));
    assert_eq!(state.dm_channels.len(), 1);
    assert_eq!(state.dm_channels[0].id, "dm-2");
    assert_eq!(state.pinned_dms, vec!["dm-2".to_string()]);
    assert_eq!(state.selected_channel_id, None);

    assert!(!close_dm_channel(&mut state, "missing"));
}

#[test]
fn mark_read_clears_channel_and_guild_badges() {
    let mut state = AppState::new();
    state.guilds = vec![GuildItemState {
        id: "guild-1".into(),
        name: "Guild".into(),
        acronym: "G".into(),
        icon_url: None,
        icon_hash: None,
        unread: true,
        mention_count: 3,
        categories: vec![CategoryItemState {
            id: "cat-1".into(),
            name: "Text".into(),
            channels: vec![channel("chan-1", true, 3), channel("chan-2", false, 0)],
        }],
        emojis: Vec::new(),
        stickers: Vec::new(),
    }];
    state.dm_channels = vec![{
        let mut d = dm("dm-3", "Three");
        d.unread = true;
        d.mention_count = 2;
        d
    }];

    assert!(mark_channel_read(&mut state, "chan-1"));
    let ch = &state.guilds[0].categories[0].channels[0];
    assert!(!ch.unread);
    assert_eq!(ch.mention_count, 0);
    assert!(!state.guilds[0].unread);
    assert_eq!(state.guilds[0].mention_count, 0);

    assert!(mark_channel_read(&mut state, "dm-3"));
    assert!(!state.dm_channels[0].unread);
    assert_eq!(state.dm_channels[0].mention_count, 0);
    assert!(!mark_channel_read(&mut state, "unknown"));
}

#[test]
fn channel_mute_toggles_local_state() {
    let mut state = AppState::new();
    assert!(!is_channel_muted(&state, "chan-1"));
    assert!(toggle_channel_mute(&mut state, "chan-1"));
    assert!(is_channel_muted(&state, "chan-1"));
    assert!(!toggle_channel_mute(&mut state, "chan-1"));
    assert!(!is_channel_muted(&state, "chan-1"));
}

#[test]
fn member_message_opens_dm_and_mention_uses_real_id() {
    let mut state = AppState::new();
    open_member_dm(&mut state, "user-42", "Member");
    assert!(state
        .dm_channels
        .iter()
        .any(|d| d.recipient_ids.contains(&"user-42".to_string())));

    state.active_draft = "hello".into();
    mention_member(&mut state, "user-42");
    assert_eq!(state.active_draft, "hello <@user-42>");

    mention_member(&mut state, "");
    assert_eq!(state.active_draft, "hello <@user-42>");
}

#[test]
fn active_now_requires_content_and_official_breakpoint() {
    let mut state = AppState::new();
    state.active_now_items.clear();
    assert!(!should_show_active_now(&state, 1280.0));

    state.active_now_items.push(active_now("activity-1"));
    assert!(!should_show_active_now(&state, 1199.0));
    assert!(should_show_active_now(&state, 1200.0));
    assert!(should_show_active_now(&state, 1600.0));
}

#[test]
fn discover_servers_opens_explore_modal() {
    let mut state = AppState::new();
    open_server_discovery(&mut state);
    assert_eq!(state.active_modal, Some(ActiveModal::ExploreServers));
}
