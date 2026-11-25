extern crate gpui_kit as gpui;

use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Duration};

use discord_rs::app::state::{AppState, HISTORY_PAGE_SIZE};
use discord_rs::chat::message_list::{build_chat_list_items, ChatListItem};
use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::{ClientManager, DiscordTransport, MockDiscordProvider};

fn synthetic_history(channel_id: &str, count: usize) -> Vec<Message> {
    // Noon avoids a date-separator boundary inside the fixture.
    let base = DateTime::from_timestamp(1_700_000_000, 0).unwrap()
        + Duration::hours(12);
    (0..count)
        .map(|index| {
            Message::new(
                format!("msg-{:03}", index),
                channel_id,
                Author::new("user-synthetic", "synthetic"),
                format!("Mensagem sintética {}", index),
            )
            .with_timestamp(base + Duration::seconds(index as i64))
        })
        .collect()
}

fn assert_unique_ascending(messages: &[Message], expected: usize) {
    assert_eq!(messages.len(), expected);
    let ids: HashSet<&str> = messages.iter().map(|message| message.id.as_str()).collect();
    assert_eq!(ids.len(), expected, "fixture must not contain duplicate IDs");
    assert!(
        messages
            .windows(2)
            .all(|pair| pair[0].timestamp <= pair[1].timestamp),
        "merged history must stay in ascending chronological order"
    );
}

#[tokio::test]
async fn before_cursor_reads_three_pages_without_duplicates() {
    let provider = MockDiscordProvider::new();
    provider
        .seed_messages_for_test("chan-history", synthetic_history("chan-history", 150))
        .await;

    let latest = provider
        .get_messages("chan-history", HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(latest.len(), HISTORY_PAGE_SIZE);
    assert_eq!(latest.first().unwrap().id, "msg-100");
    assert_eq!(latest.last().unwrap().id, "msg-149");

    let page_two = provider
        .get_messages_before("chan-history", &latest[0].id, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(page_two.len(), HISTORY_PAGE_SIZE);
    assert_eq!(page_two.first().unwrap().id, "msg-050");
    assert_eq!(page_two.last().unwrap().id, "msg-099");

    let page_three = provider
        .get_messages_before("chan-history", &page_two[0].id, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(page_three.len(), HISTORY_PAGE_SIZE);
    assert_eq!(page_three.first().unwrap().id, "msg-000");
    assert_eq!(page_three.last().unwrap().id, "msg-049");

    let terminal = provider
        .get_messages_before("chan-history", &page_three[0].id, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert!(terminal.is_empty(), "start of history must terminate");

    let mut all = latest;
    all.extend(page_two);
    all.extend(page_three);
    all.sort_by_key(|message| message.timestamp);
    assert_unique_ascending(&all, 150);
}

#[tokio::test]
async fn around_cursor_returns_search_context_in_chronological_order() {
    let provider = MockDiscordProvider::new();
    provider
        .seed_messages_for_test("chan-history", synthetic_history("chan-history", 150))
        .await;

    let around = provider
        .get_messages_around("chan-history", "msg-075", 50)
        .await
        .unwrap();
    assert_eq!(around.first().unwrap().id, "msg-050");
    assert_eq!(around.last().unwrap().id, "msg-099");
    assert!(around.iter().any(|message| message.id == "msg-075"));
    assert_unique_ascending(&around, 50);

    let missing = provider
        .get_messages_around("chan-history", "msg-does-not-exist", 50)
        .await
        .unwrap();
    assert!(missing.is_empty());
}

#[tokio::test]
async fn state_pagination_merges_three_pages_with_single_loading_guard() {
    let provider = MockDiscordProvider::new();
    provider
        .seed_messages_for_test("chan-history", synthetic_history("chan-history", 150))
        .await;
    let latest = provider
        .get_messages("chan-history", HISTORY_PAGE_SIZE)
        .await
        .unwrap();

    let mut state = AppState::default();
    state.selected_channel_id = Some("chan-history".to_string());
    state.set_channel_messages("chan-history", latest);

    let pagination = state.history_pagination.get("chan-history").unwrap();
    assert!(pagination.has_more);
    assert_eq!(pagination.oldest_id.as_deref(), Some("msg-100"));
    assert!(!pagination.loading);

    // First request starts and exposes exactly one in-flight spinner.
    let cursor = state.begin_history_load("chan-history").unwrap();
    assert_eq!(cursor, "msg-100");
    assert!(state.history_pagination["chan-history"].loading);
    assert!(
        state.begin_history_load("chan-history").is_none(),
        "a second scroll event must not start a duplicate spinner"
    );

    let page_two = provider
        .get_messages_before("chan-history", &cursor, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(
        state.finish_history_load("chan-history", page_two),
        HISTORY_PAGE_SIZE
    );
    let pagination = &state.history_pagination["chan-history"];
    assert!(!pagination.loading);
    assert!(pagination.has_more);
    assert_eq!(pagination.oldest_id.as_deref(), Some("msg-050"));
    assert_eq!(pagination.pages_loaded, 1);

    let cursor = state.begin_history_load("chan-history").unwrap();
    assert_eq!(cursor, "msg-050");
    let page_three = provider
        .get_messages_before("chan-history", &cursor, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(
        state.finish_history_load("chan-history", page_three),
        HISTORY_PAGE_SIZE
    );
    assert_eq!(
        state.history_pagination["chan-history"]
            .oldest_id
            .as_deref(),
        Some("msg-000")
    );
    assert_eq!(
        state.history_pagination["chan-history"].pages_loaded,
        2
    );

    // Discord does not return a total count, so a full third page still asks
    // once more; the empty terminal page marks the start as reached.
    let cursor = state.begin_history_load("chan-history").unwrap();
    assert_eq!(cursor, "msg-000");
    let terminal = provider
        .get_messages_before("chan-history", &cursor, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(state.finish_history_load("chan-history", terminal), 0);
    let pagination = &state.history_pagination["chan-history"];
    assert!(!pagination.loading);
    assert!(!pagination.has_more);
    assert_eq!(pagination.pages_loaded, 2);
    assert!(state.begin_history_load("chan-history").is_none());

    assert_unique_ascending(&state.messages_by_channel["chan-history"], 150);
}

#[tokio::test]
async fn client_manager_paginates_resolved_dm_channel() {
    let provider = Arc::new(MockDiscordProvider::new());
    provider
        .seed_messages_for_test(
            "chan-dm-ferris",
            synthetic_history("chan-dm-ferris", 120),
        )
        .await;
    let (client_manager, _events) = ClientManager::new_with_transport(provider);

    let latest = client_manager
        .get_messages("dm-ferris", HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(latest.len(), HISTORY_PAGE_SIZE);
    assert_eq!(latest.first().unwrap().id, "msg-070");
    assert_eq!(latest.last().unwrap().id, "msg-119");
    assert!(latest
        .iter()
        .all(|message| message.channel_id == "chan-dm-ferris"));

    let older = client_manager
        .get_messages_before("dm-ferris", &latest[0].id, HISTORY_PAGE_SIZE)
        .await
        .unwrap();
    assert_eq!(older.len(), HISTORY_PAGE_SIZE);
    assert_eq!(older.first().unwrap().id, "msg-020");
    assert_eq!(older.last().unwrap().id, "msg-069");
    assert!(older
        .iter()
        .all(|message| message.channel_id == "chan-dm-ferris"));

    let around = client_manager
        .get_messages_around("dm-ferris", "msg-045", 50)
        .await
        .unwrap();
    assert!(around.iter().any(|message| message.id == "msg-045"));
    assert_unique_ascending(&around, 50);
}

#[test]
fn merging_older_page_keeps_anchor_message_identity() {
    let history = synthetic_history("chan-history", 150);
    let mut state = AppState::default();
    state.selected_channel_id = Some("chan-history".to_string());
    state.set_channel_messages("chan-history", history[100..].to_vec());

    let anchor_id = "msg-120";
    let old_items = build_chat_list_items(state.get_active_messages());
    let old_index = old_items
        .iter()
        .position(|item| match item {
            ChatListItem::Message { message_idx, .. } => state.get_active_messages()
                .get(*message_idx)
                .map(|message| message.id == anchor_id)
                .unwrap_or(false),
            _ => false,
        })
        .expect("anchor must exist in the latest page");

    state.merge_older_messages("chan-history", history[50..100].to_vec());
    let new_items = build_chat_list_items(state.get_active_messages());
    let new_index = new_items
        .iter()
        .position(|item| match item {
            ChatListItem::Message { message_idx, .. } => state.get_active_messages()
                .get(*message_idx)
                .map(|message| message.id == anchor_id)
                .unwrap_or(false),
            _ => false,
        })
        .expect("anchor must still exist after prepend");

    // Same date, so the 50 prepended messages shift the anchor by exactly 50
    // virtual items; scroll_to_reveal_item(new_index) keeps it in place.
    assert_eq!(new_index - old_index, 50);
    assert_unique_ascending(state.get_active_messages(), 100);
}
