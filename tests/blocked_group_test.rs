extern crate gpui_kit as gpui;

use std::collections::HashSet;

use discord_rs::chat::blocked_group::{
    render_blocked_group_notice, visible_message_indices, BlockedGroupKey, BlockedGroupLocale,
    BlockedGroupsState, BlockedStream, BlockedStreamItem,
};
use discord_rs::domain::message::{Author, Message};

const CHANNEL_A: &str = "900000000000000001";
const CHANNEL_B: &str = "900000000000000002";
const USER_VISIBLE: &str = "700000000000000001";
const USER_BLOCKED: &str = "700000000000000002";
const USER_BLOCKED_TWO: &str = "700000000000000003";

fn message(id: &str, channel_id: &str, author_id: &str, content: &str) -> Message {
    let mut author = Author::new(author_id, format!("user_{}", author_id));
    author.global_name = Some(format!("Nome {}", author_id));
    Message::new(id, channel_id, author, content)
}

fn blocked_set(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

#[test]
fn groups_only_consecutive_messages_from_the_same_blocked_author() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_VISIBLE, "primeira visível"),
        message("m2", CHANNEL_A, USER_BLOCKED, "bloqueada 1"),
        message("m3", CHANNEL_A, USER_BLOCKED, "bloqueada 2"),
        message("m4", CHANNEL_A, USER_VISIBLE, "visível no meio"),
        message("m5", CHANNEL_A, USER_BLOCKED_TWO, "outro bloqueado"),
        message("m6", CHANNEL_A, USER_BLOCKED, "bloqueada 3"),
    ];

    let stream = BlockedStream::build(&messages, &blocked_set(&[USER_BLOCKED, USER_BLOCKED_TWO]));

    assert_eq!(stream.groups.len(), 3);
    assert_eq!(stream.blocked_message_count, 4);

    assert_eq!(
        stream.groups[0].key,
        BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED)
    );
    assert_eq!(stream.groups[0].message_indices, vec![1, 2]);
    assert_eq!(stream.groups[0].count(), 2);
    assert_eq!(
        stream.groups[0].message_ids(&messages),
        vec!["m2", "m3"]
    );

    assert_eq!(
        stream.groups[1].key,
        BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED_TWO)
    );
    assert_eq!(stream.groups[1].message_indices, vec![4]);

    assert_eq!(stream.groups[2].message_indices, vec![5]);

    assert_eq!(
        stream.items,
        vec![
            BlockedStreamItem::Message { message_index: 0 },
            BlockedStreamItem::BlockedGroup { group_index: 0 },
            BlockedStreamItem::Message { message_index: 3 },
            BlockedStreamItem::BlockedGroup { group_index: 1 },
            BlockedStreamItem::BlockedGroup { group_index: 2 },
        ]
    );
}

#[test]
fn a_channel_change_breaks_grouping_even_with_the_same_author() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_BLOCKED, "canal A"),
        message("m2", CHANNEL_B, USER_BLOCKED, "canal B"),
        message("m3", CHANNEL_A, USER_BLOCKED, "canal A de novo"),
    ];

    let stream = BlockedStream::build(&messages, &blocked_set(&[USER_BLOCKED]));

    assert_eq!(stream.groups.len(), 3);
    assert_eq!(stream.blocked_message_count, 3);
    assert_eq!(stream.groups[0].message_indices, vec![0]);
    assert_eq!(stream.groups[1].message_indices, vec![1]);
    assert_eq!(stream.groups[2].message_indices, vec![2]);
}

#[test]
fn messages_from_non_blocked_authors_are_never_hidden() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_BLOCKED, "bloqueada"),
        message("m2", CHANNEL_A, USER_VISIBLE, "normal"),
    ];

    let stream = BlockedStream::build(&messages, &blocked_set(&[USER_BLOCKED]));
    let state = BlockedGroupsState::new(blocked_set(&[USER_BLOCKED]));

    assert_eq!(visible_message_indices(&stream, &state), vec![1]);
    assert_eq!(stream.blocked_message_count, 1);
}

#[test]
fn collapsed_group_hides_content_and_toggle_preserves_raw_messages() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_VISIBLE, "antes"),
        message("m2", CHANNEL_A, USER_BLOCKED, "conteúdo preservado 1"),
        message("m3", CHANNEL_A, USER_BLOCKED, "conteúdo preservado 2"),
        message("m4", CHANNEL_A, USER_VISIBLE, "depois"),
    ];

    let stream = BlockedStream::build(&messages, &blocked_set(&[USER_BLOCKED]));
    let mut state = BlockedGroupsState::new(blocked_set(&[USER_BLOCKED]));
    let key = BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED);

    assert_eq!(visible_message_indices(&stream, &state), vec![0, 3]);

    let expanded = state.toggle(key.clone());
    assert!(expanded);
    assert!(state.is_expanded(&key));
    assert_eq!(visible_message_indices(&stream, &state), vec![0, 1, 2, 3]);

    let group = stream.group_for_key(&key).unwrap();
    assert_eq!(group.first_message(&messages).unwrap().content, "conteúdo preservado 1");
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[1].content, "conteúdo preservado 1");
    assert_eq!(messages[2].content, "conteúdo preservado 2");

    let collapsed = state.toggle(key.clone());
    assert!(!collapsed);
    assert!(!state.is_expanded(&key));
    assert_eq!(visible_message_indices(&stream, &state), vec![0, 3]);
    assert_eq!(messages[1].content, "conteúdo preservado 1");
}

#[test]
fn expansion_is_scoped_by_channel_and_author() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_BLOCKED, "canal A"),
        message("m2", CHANNEL_B, USER_BLOCKED, "canal B"),
        message("m3", CHANNEL_A, USER_BLOCKED_TWO, "outro autor"),
    ];

    let stream = BlockedStream::build(&messages, &blocked_set(&[USER_BLOCKED, USER_BLOCKED_TWO]));
    let mut state = BlockedGroupsState::new(blocked_set(&[USER_BLOCKED, USER_BLOCKED_TWO]));

    let key_a = BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED);
    state.expand(key_a.clone());

    assert_eq!(visible_message_indices(&stream, &state), vec![0]);
    assert!(!state.is_expanded(&BlockedGroupKey::new(CHANNEL_B, USER_BLOCKED)));
    assert!(!state.is_expanded(&BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED_TWO)));

    assert_eq!(state.collapse_channel(CHANNEL_A), 1);
    assert_eq!(visible_message_indices(&stream, &state), Vec::<usize>::new());
}

#[test]
fn blocking_and_unblocking_update_state_without_dropping_cached_messages() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_VISIBLE, "visível"),
        message("m2", CHANNEL_A, USER_BLOCKED, "bloqueada"),
    ];

    let mut state = BlockedGroupsState::default();
    let key = BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED);

    let no_groups = BlockedStream::build(&messages, state.blocked_user_ids());
    assert!(no_groups.groups.is_empty());

    state.apply_user_relationship(USER_BLOCKED, true);
    let stream_after_block = BlockedStream::build(&messages, state.blocked_user_ids());
    assert_eq!(stream_after_block.groups.len(), 1);
    assert_eq!(visible_message_indices(&stream_after_block, &state), vec![0]);

    state.expand(key.clone());
    assert_eq!(visible_message_indices(&stream_after_block, &state), vec![0, 1]);

    let discarded = state.apply_user_relationship(USER_BLOCKED, false);
    assert_eq!(discarded, vec![key.clone()]);
    assert!(!state.is_expanded(&key));
    assert!(!state.is_user_blocked(USER_BLOCKED));

    let stream_after_unblock = BlockedStream::build(&messages, state.blocked_user_ids());
    assert!(stream_after_unblock.groups.is_empty());
    assert_eq!(visible_message_indices(&stream_after_unblock, &state), vec![0, 1]);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "bloqueada");
}

#[test]
fn blocking_an_expanded_author_collapses_it_before_content_is_exposed() {
    let mut state = BlockedGroupsState::new(blocked_set(&[USER_BLOCKED]));
    let key = BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED);
    state.expand(key.clone());

    let discarded = state.apply_user_relationship(USER_BLOCKED, true);

    assert_eq!(discarded, vec![key.clone()]);
    assert!(!state.is_expanded(&key));
    assert!(state.is_user_blocked(USER_BLOCKED));
}

#[test]
fn relationship_snapshot_discards_only_expansions_of_newly_unblocked_authors() {
    let mut state = BlockedGroupsState::new(blocked_set(&[USER_BLOCKED, USER_BLOCKED_TWO]));
    let blocked_in_a = BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED);
    let blocked_in_b = BlockedGroupKey::new(CHANNEL_B, USER_BLOCKED);
    let kept = BlockedGroupKey::new(CHANNEL_A, USER_BLOCKED_TWO);
    state.expand(blocked_in_a.clone());
    state.expand(blocked_in_b.clone());
    state.expand(kept.clone());

    let discarded = state.sync_blocked_users(blocked_set(&[USER_BLOCKED_TWO]));

    assert_eq!(discarded.len(), 2);
    assert!(discarded.contains(&blocked_in_a));
    assert!(discarded.contains(&blocked_in_b));
    assert!(!state.is_expanded(&blocked_in_a));
    assert!(!state.is_expanded(&blocked_in_b));
    assert!(state.is_expanded(&kept));
    assert!(!state.is_user_blocked(USER_BLOCKED));
    assert!(state.is_user_blocked(USER_BLOCKED_TWO));
}

#[test]
fn localized_labels_report_the_real_group_count_and_toggle_action() {
    assert_eq!(BlockedGroupLocale::PtBr.count_label(1), "1 Mensagem bloqueada");
    assert_eq!(BlockedGroupLocale::PtBr.count_label(3), "3 Mensagens bloqueadas");
    assert_eq!(BlockedGroupLocale::PtBr.action_label(false), "Mostrar mensagens");
    assert_eq!(BlockedGroupLocale::PtBr.action_label(true), "Recolher mensagens");

    assert_eq!(BlockedGroupLocale::EnUs.count_label(1), "1 Blocked Message");
    assert_eq!(BlockedGroupLocale::EnUs.count_label(3), "3 Blocked Messages");
    assert_eq!(BlockedGroupLocale::EnUs.action_label(false), "Show messages");
    assert_eq!(BlockedGroupLocale::EnUs.action_label(true), "Collapse messages");
}

#[test]
fn render_notice_builds_for_collapsed_and_expanded_groups() {
    let messages = vec![
        message("m1", CHANNEL_A, USER_BLOCKED, "um"),
        message("m2", CHANNEL_A, USER_BLOCKED, "dois"),
    ];
    let stream = BlockedStream::build(&messages, &blocked_set(&[USER_BLOCKED]));
    let group = stream.group(0).unwrap();

    let _collapsed = render_blocked_group_notice(group, false, BlockedGroupLocale::PtBr, None);
    let _expanded = render_blocked_group_notice(group, true, BlockedGroupLocale::EnUs, None);
}
