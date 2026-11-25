use std::time::Instant;

use discord_rs::app::state::{CategoryItemState, ChannelItemState, ChannelKind, MemberItemState, PresenceStatus};
use discord_rs::app::view_memo::{CategoryChannelMemo, MemberRoleMemo};

fn channel(id: &str, name: &str, kind: ChannelKind) -> ChannelItemState {
    ChannelItemState {
        id: id.to_string(),
        name: name.to_string(),
        kind,
        topic: None,
        unread: false,
        mention_count: 0,
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
    }
}

fn member(id: &str, name: &str, role_group: &str, presence: PresenceStatus) -> MemberItemState {
    MemberItemState {
        id: id.to_string(),
        guild_id: Some("guild-memo".to_string()),
        name: name.to_string(),
        nickname: None,
        avatar_text: name.chars().next().unwrap_or('?').to_string(),
        presence,
        role_group: role_group.to_string(),
        role_color: 0,
        is_bot: false,
        activity: None,
        avatar_decoration_data: None,
    }
}

#[test]
fn category_channel_memo_recomputes_only_on_generation_change() {
    let categories = vec![
        CategoryItemState {
            id: "cat-a".into(),
            name: "ALPHA".into(),
            channels: vec![
                channel("c2", "zeta", ChannelKind::Text),
                channel("c1", "alpha", ChannelKind::Text),
                channel("c3", "voice-room", ChannelKind::Voice),
            ],
        },
        CategoryItemState {
            id: "cat-b".into(),
            name: "BETA".into(),
            channels: vec![channel("c4", "ann", ChannelKind::Announcement)],
        },
    ];

    let mut memo = CategoryChannelMemo::new();
    let first_ptr = memo.get_or_compute(1, &categories).as_ptr();
    let second_ptr = memo.get_or_compute(1, &categories).as_ptr();
    assert_eq!(first_ptr, second_ptr, "same generation must reuse cached slice");

    let first = memo.get_or_compute(1, &categories);
    assert_eq!(first[0].channels[0].id, "c1");
    assert_eq!(first[0].channels[1].id, "c2");
    assert_eq!(first[0].channels[2].id, "c3");

    let mut updated = categories.clone();
    updated[0].channels.push(channel("c0", "aaa", ChannelKind::Text));
    let third = memo.get_or_compute(2, &updated);
    assert_eq!(third[0].channels[0].id, "c0", "generation bump must recompute");
    assert_ne!(memo.get_or_compute(2, &updated).as_ptr() as usize, 0);
}

#[test]
fn member_role_memo_groups_and_sorts_by_role() {
    let members = vec![
        member("u3", "Zed", "MOD", PresenceStatus::Online),
        member("u1", "alice", "ADMIN", PresenceStatus::Online),
        member("u4", "bob", "ADMIN", PresenceStatus::Idle),
        member("u2", "charlie", "", PresenceStatus::Offline),
        member("u5", "dora", "MOD", PresenceStatus::DoNotDisturb),
    ];

    let mut memo = MemberRoleMemo::new();
    let groups = memo.get_or_compute(10, &members);
    let names: Vec<(String, Vec<String>)> = groups
        .iter()
        .map(|g| (g.role_group.clone(), g.members.iter().map(|m| m.name.clone()).collect()))
        .collect();

    assert_eq!(names[0].0, "MOD");
    assert_eq!(names[0].1, vec!["dora".to_string(), "Zed".to_string()]);
    assert_eq!(names[1].0, "ADMIN");
    assert_eq!(names[1].1, vec!["alice".to_string(), "bob".to_string()]);
    assert_eq!(names[2].0, "OFFLINE");
    assert_eq!(names[2].1, vec!["charlie".to_string()]);

    let cached_ptr = memo.get_or_compute(10, &members).as_ptr();
    let cached_ptr_2 = memo.get_or_compute(10, &members).as_ptr();
    assert_eq!(cached_ptr, cached_ptr_2, "same generation must hit memo cache");
}

#[test]
fn memoized_lookup_is_faster_than_recompute_for_large_input() {
    let mut categories = Vec::new();
    for cat_i in 0..64 {
        let mut channels = Vec::new();
        for ch_i in 0..128 {
            let kind = match ch_i % 3 {
                0 => ChannelKind::Text,
                1 => ChannelKind::Voice,
                _ => ChannelKind::Announcement,
            };
            channels.push(channel(
                &format!("c-{cat_i}-{ch_i}"),
                &format!("chan-{}-{}", cat_i, 128 - ch_i),
                kind,
            ));
        }
        categories.push(CategoryItemState {
            id: format!("cat-{cat_i}"),
            name: format!("CAT-{cat_i}"),
            channels,
        });
    }

    let mut memo = CategoryChannelMemo::new();
    let _ = memo.get_or_compute(1, &categories);

    let recompute_start = Instant::now();
    for g in 2..102 {
        let _ = memo.get_or_compute(g, &categories);
    }
    let recompute_elapsed = recompute_start.elapsed();

    let cached_start = Instant::now();
    for _ in 0..10_000 {
        let _ = memo.get_or_compute(101, &categories);
    }
    let cached_elapsed = cached_start.elapsed();

    // Cached lookups should be substantially cheaper than repeated recomputes.
    // Use a generous bound to avoid flakiness in CI.
    assert!(
        cached_elapsed < recompute_elapsed,
        "cached lookups should be faster: cached={cached_elapsed:?} recompute={recompute_elapsed:?}"
    );
}
