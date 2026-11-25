use discord_rs::app::state::{AppState, ChannelKind, CategoryItemState, ChannelItemState, GuildItemState};

fn mk_guild(id: &str, name: &str, channels: Vec<(&str, &str)>) -> GuildItemState {
    GuildItemState {
        id: id.to_string(),
        name: name.to_string(),
        acronym: name.chars().take(2).collect(),
        icon_url: None,
        icon_hash: None,
        unread: false,
        mention_count: 0,
        categories: vec![CategoryItemState {
            id: format!("cat-{id}"),
            name: "Text".to_string(),
            channels: channels
                .into_iter()
                .map(|(cid, cname)| ChannelItemState {
                    id: cid.to_string(),
                    name: cname.to_string(),
                    kind: ChannelKind::Text,
                    topic: None,
                    unread: false,
                    mention_count: 0,
                    icon_url: None,
                    is_pinned: false,
                    last_message_id: None,
                    last_read_message_id: None,
                    is_group_dm: false,
                    recipient_ids: vec![],
                    recipient_names: vec![],
                    user_limit: None,
                    avatar_decoration_data: None,
                    thread_metadata: None,
                    default_tag_ids: Vec::new(),
                    available_tags: Vec::new(),
                    flags: 0,
                })
                .collect(),
        }],
        emojis: vec![],
        stickers: vec![],
    }
}

fn mk_dm(id: &str, name: &str) -> ChannelItemState {
    ChannelItemState {
        id: id.to_string(),
        name: name.to_string(),
        kind: ChannelKind::Text,
        topic: None,
        unread: false,
        mention_count: 0,
        icon_url: None,
        is_pinned: false,
        last_message_id: None,
        last_read_message_id: None,
        is_group_dm: false,
        recipient_ids: vec![format!("u-{id}")],
        recipient_names: vec![name.to_string()],
        user_limit: None,
        avatar_decoration_data: None,
        thread_metadata: None,
        default_tag_ids: Vec::new(),
        available_tags: Vec::new(),
        flags: 0,
    }
}

#[test]
fn quick_switcher_ctrlk_style_open_and_close() {
    let mut state = AppState::default();
    assert!(!state.quick_switcher.open);
    state.open_quick_switcher();
    assert!(state.quick_switcher.open);
    state.close_quick_switcher();
    assert!(!state.quick_switcher.open);
    assert!(state.quick_switcher.query.is_empty());
}

#[test]
fn quick_switcher_fuzzy_ranking_and_empty_result() {
    let mut state = AppState::default();
    state.guilds = vec![mk_guild(
        "g1",
        "Alpha Guild",
        vec![("c1", "general"), ("c2", "gaming")],
    )];
    state.dm_channels = vec![mk_dm("d1", "alice"), mk_dm("d2", "gabi")];

    state.set_quick_switcher_query("ga".to_string());
    let labels: Vec<String> = state
        .quick_switcher
        .results
        .iter()
        .map(|r| r.label.to_lowercase())
        .collect();
    assert!(labels.iter().any(|l| l.contains("gabi") || l.contains("gaming")));

    state.set_quick_switcher_query("zzzz-no-match".to_string());
    assert!(state.quick_switcher.results.is_empty());
}

#[test]
fn quick_switcher_excludes_inaccessible_channel() {
    let mut state = AppState::default();
    state.guilds = vec![mk_guild(
        "g1",
        "Alpha Guild",
        vec![("c1", "public"), ("c2", "secret")],
    )];
    state
        .quick_switcher_inaccessible_channel_ids
        .insert("c2".to_string());
    state.set_quick_switcher_query("sec".to_string());
    assert!(state.quick_switcher.results.iter().all(|r| r.channel_id.as_deref() != Some("c2")));
    assert!(state.quick_switcher.results.is_empty());
}

#[test]
fn quick_switcher_keyboard_navigation_and_confirm() {
    let mut state = AppState::default();
    state.dm_channels = vec![mk_dm("d1", "alice"), mk_dm("d2", "bob")];
    state.open_quick_switcher();
    assert!(!state.quick_switcher.results.is_empty());
    let len = state.quick_switcher.results.len();
    state.quick_switcher_move_selection(-1);
    assert_eq!(state.quick_switcher.selected_index, len - 1);
    state.quick_switcher_move_selection(1);
    assert_eq!(state.quick_switcher.selected_index, 0);

    state.set_quick_switcher_query("bob".to_string());
    let ok = state.confirm_quick_switcher();
    assert!(ok);
    assert!(!state.quick_switcher.open);
    assert_eq!(state.selected_channel_id.as_deref(), Some("d2"));
}

#[test]
fn quick_switcher_recent_history_is_prioritized() {
    let mut state = AppState::default();
    state.guilds = vec![mk_guild("g1", "Alpha", vec![("c1", "general")])];
    state.dm_channels = vec![mk_dm("d1", "alice")];
    state.quick_switcher.recent_channel_ids = vec!["d1".to_string()];
    state.set_quick_switcher_query(String::new());
    assert_eq!(state.quick_switcher.results.first().and_then(|r| r.channel_id.as_deref()), Some("d1"));
}
