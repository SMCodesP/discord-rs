//! Integration tests for Search State, Engine, and Faceted Filter Parsing
//!
//! Tests search opening, toggling, closing, clearing, query execution,
//! scoped search (guild vs DM isolation), chronological ordering,
//! jumping to messages with guild switching, and REST API payload conversions.

use chrono::{Duration, NaiveDate, Utc};
use discord_rs::app::{AppState, CategoryItemState, ChannelItemState, ChannelKind, GuildItemState};
use discord_rs::chat::search::{
    ApiSearchMessage, HasFilter, SearchApiResponse, SearchPayload, SearchQuery, SearchSortOrder,
};
use discord_rs::domain::message::{Attachment, Message, MessageStickerItem};
use discord_rs::domain::Author;

fn create_test_author(id: &str, username: &str) -> Author {
    Author::new(id, username)
}

fn create_test_message(id: &str, channel_id: &str, author: Author, content: &str) -> Message {
    Message::new(id, channel_id, author, content)
}

fn setup_test_app_state() -> AppState {
    let mut state = AppState::new();

    // Guild 1 with two text channels
    let g1 = GuildItemState {
        id: "guild-1".to_string(),
        name: "Servidor Alpha".to_string(),
        acronym: "SA".to_string(),
        icon_url: None,
        icon_hash: None,
        unread: false,
        mention_count: 0,
        categories: vec![CategoryItemState {
            id: "cat-1".to_string(),
            name: "Canais".to_string(),
            channels: vec![
                ChannelItemState {
                    id: "c-geral".to_string(),
                    name: "geral".to_string(),
                    kind: ChannelKind::Text,
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
                    id: "c-dev".to_string(),
                    name: "desenvolvimento".to_string(),
                    kind: ChannelKind::Text,
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
        }],
        emojis: Vec::new(),
        stickers: Vec::new(),
    };

    // Guild 2 with one channel
    let g2 = GuildItemState {
        id: "guild-2".to_string(),
        name: "Servidor Beta".to_string(),
        acronym: "SB".to_string(),
        icon_url: None,
        icon_hash: None,
        unread: false,
        mention_count: 0,
        categories: vec![CategoryItemState {
            id: "cat-2".to_string(),
            name: "Geral".to_string(),
            channels: vec![ChannelItemState {
                id: "c-beta-chat".to_string(),
                name: "chat".to_string(),
                kind: ChannelKind::Text,
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
        }],
        emojis: Vec::new(),
        stickers: Vec::new(),
    };

    state.guilds = vec![g1, g2];
    state.selected_guild_id = Some("guild-1".to_string());
    state.selected_channel_id = Some("c-geral".to_string());

    // Populate messages in channels with timestamps
    let author_alice = create_test_author("u-alice", "alice");
    let author_bob = create_test_author("u-bob", "bob");

    let now = Utc::now();

    // Messages in c-geral
    let mut m1 = create_test_message("m-1", "c-geral", author_alice.clone(), "Primeira mensagem sobre rust e gpui");
    m1.timestamp = now - Duration::minutes(20);

    let mut m2 = create_test_message("m-2", "c-geral", author_bob.clone(), "Concordo totalmente sobre rust");
    m2.timestamp = now - Duration::minutes(10);

    // Message in c-dev (Guild 1) - newest
    let mut m3 = create_test_message("m-3", "c-dev", author_alice.clone(), "Build de release do rust concluída");
    m3.timestamp = now - Duration::minutes(5);

    // Message in c-beta-chat (Guild 2)
    let mut m4 = create_test_message("m-4", "c-beta-chat", author_bob.clone(), "Rust no servidor beta");
    m4.timestamp = now - Duration::minutes(2);

    state.messages_by_channel.insert("c-geral".to_string(), vec![m1, m2]);
    state.messages_by_channel.insert("c-dev".to_string(), vec![m3]);
    state.messages_by_channel.insert("c-beta-chat".to_string(), vec![m4]);

    state
}

#[test]
fn test_search_open_close_toggle() {
    let mut state = setup_test_app_state();
    assert!(!state.is_search_open);

    state.open_search();
    assert!(state.is_search_open);

    state.close_search();
    assert!(!state.is_search_open);

    state.toggle_search();
    assert!(state.is_search_open);

    state.toggle_search();
    assert!(!state.is_search_open);
}

#[test]
fn test_search_clear() {
    let mut state = setup_test_app_state();
    state.search_query = "rust".to_string();
    state.search_total_results = 10;
    state.search_offset = 25;
    state.is_searching = true;
    state.search_error = Some("Erro anterior".to_string());
    state.is_search_indexing = true;
    state.search_retry_after = Some(2.5);

    state.clear_search();
    assert!(state.search_query.is_empty());
    assert!(state.search_results.is_empty());
    assert!(!state.is_searching);
    assert_eq!(state.search_offset(), 0);
    assert_eq!(state.search_total_results(), 0);
    assert!(state.search_error().is_none());
    assert!(!state.is_search_indexing());
    assert_eq!(state.search_retry_after(), None);
}

#[test]
fn test_scoped_search_request_construction() {
    let mut state = setup_test_app_state();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);

    // Active guild is guild-1, active channel is c-geral
    let req = state.begin_search("rust release").expect("SearchRequest should be created");
    assert_eq!(req.guild_id.as_deref(), Some("guild-1"));
    assert_eq!(req.channel_id.as_deref(), Some("c-geral"));
    assert_eq!(req.query.text_terms, vec!["rust", "release"]);
    assert_eq!(req.offset, 0);
    assert_eq!(req.sort_order, SearchSortOrder::Desc);
    assert_eq!(req.generation, 1);
    assert!(state.is_searching);

    // Switching active scope changes subsequent SearchRequest
    state.selected_channel_id = Some("c-dev".to_string());
    let req2 = state.begin_search("gpui").expect("SearchRequest for c-dev");
    assert_eq!(req2.channel_id.as_deref(), Some("c-dev"));
    assert_eq!(req2.generation, 2);
}

#[test]
fn test_jump_to_message_syncs_channel_and_guild() {
    let mut state = setup_test_app_state();
    assert_eq!(state.selected_guild_id.as_deref(), Some("guild-1"));
    assert_eq!(state.selected_channel_id.as_deref(), Some("c-geral"));

    // Jump to m-4 in c-beta-chat (which belongs to guild-2)
    state.jump_to_message("c-beta-chat", "m-4");

    assert_eq!(state.selected_guild_id.as_deref(), Some("guild-2"));
    assert_eq!(state.selected_channel_id.as_deref(), Some("c-beta-chat"));
    assert_eq!(state.target_message_id.as_deref(), Some("m-4"));
}

#[test]
fn test_faceted_query_spaced_filters() {
    let q = SearchQuery::parse("from: alice in: geral has: link before: 2026-12-31 pinned: true");
    assert_eq!(q.from.as_deref(), Some("alice"));
    assert_eq!(q.in_channel.as_deref(), Some("geral"));
    assert_eq!(q.has_filters, vec![HasFilter::Link]);
    assert_eq!(q.before, Some(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()));
    assert_eq!(q.pinned, Some(true));
}

#[test]
fn test_faceted_query_quotes_and_text_terms() {
    let q = SearchQuery::parse("from: \"alice smith\" \"gpui engine\" release");
    assert_eq!(q.from.as_deref(), Some("alice smith"));
    assert_eq!(q.text_terms, vec!["gpui engine", "release"]);
}

#[test]
fn test_has_filter_stickers_and_media() {
    let author = create_test_author("u-1", "testuser");
    let mut msg = create_test_message("m-1", "c-1", author, "Olá mundo");

    assert!(!HasFilter::Sticker.matches(&msg));
    assert!(!HasFilter::File.matches(&msg));

    msg.sticker_items.push(MessageStickerItem::new("s-1", "cat_wave", 1));
    assert!(HasFilter::Sticker.matches(&msg));

    let att = Attachment {
        id: "att-1".to_string(),
        filename: "demo.png".to_string(),
        size: 500,
        url: "http://example.com/demo.png".to_string(),
        proxy_url: None,
        content_type: Some("image/png".to_string()),
        width: Some(100),
        height: Some(100),
    };
    msg.attachments.push(att);
    assert!(HasFilter::File.matches(&msg));
    assert!(HasFilter::Image.matches(&msg));
}

#[test]
fn test_search_payload_conversion_and_api_params() {
    let author = create_test_author("u-1", "api_user");
    let m1 = create_test_message("m-ctx", "c-1", author.clone(), "contexto");
    let m2 = create_test_message("m-hit", "c-1", author.clone(), "resultado principal");

    let payload = SearchPayload {
        total_results: 1,
        messages: vec![vec![
            ApiSearchMessage { message: m1.clone(), hit: false },
            ApiSearchMessage { message: m2.clone(), hit: true },
        ]],
        analytics_id: Some("id-test".to_string()),
    };

    let groups = payload.into_result_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].hit_message.id, "m-hit");
    assert_eq!(groups[0].context_before.len(), 1);
    assert_eq!(groups[0].context_before[0].id, "m-ctx");

    let query = SearchQuery::parse("from:123 in:456 has:sticker crash");
    let params = query.to_api_query_params();
    assert!(params.contains(&("author_id", "123".to_string())));
    assert!(params.contains(&("channel_id", "456".to_string())));
    assert!(params.contains(&("has", "sticker".to_string())));
    assert!(params.contains(&("content", "crash".to_string())));
}

#[test]
fn test_disconnected_search_sets_error_state() {
    let mut state = setup_test_app_state();
    // Initially no error
    assert!(state.search_error().is_none());

    // Execute search when not connected to gateway/API (client_manager is None)
    let req = state.begin_search("termo de teste");
    assert!(req.is_none());

    // Must transition to error state without mocking results (Zero Mocks)
    assert_eq!(
        state.search_error().as_deref(),
        Some("Não conectado à API do Discord")
    );
    assert!(state.search_results.is_empty());
    assert!(!state.is_searching);
    assert!(!state.is_search_indexing());
}

#[test]
fn test_two_phase_real_path_synthetic_response_and_finish() {
    let mut state = setup_test_app_state();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);

    let req = state.begin_search("resultado da busca").expect("should create SearchRequest");
    assert!(state.is_searching);
    assert_eq!(state.search_generation, 1);

    let synthetic_json = r#"{
        "total_results": 42,
        "messages": [
            [
                {
                    "id": "1000",
                    "channel_id": "c-geral",
                    "author": { "id": "u-alice", "username": "alice" },
                    "content": "Contexto anterior",
                    "timestamp": "2026-10-11T12:00:00Z",
                    "hit": false
                },
                {
                    "id": "1001",
                    "channel_id": "c-geral",
                    "author": { "id": "u-bob", "username": "bob" },
                    "content": "Resultado da busca real",
                    "timestamp": "2026-10-11T12:01:00Z",
                    "hit": true
                }
            ]
        ],
        "analytics_id": "synthetic-analytics-id-123"
    }"#;

    let payload: SearchPayload = serde_json::from_str(synthetic_json).expect("valid synthetic json");
    assert_eq!(payload.total_results, 42);
    assert_eq!(payload.messages.len(), 1);

    // Consume through the real finish_search method
    state.finish_search(&req, Ok(SearchApiResponse::Ready(payload)));

    assert_eq!(state.search_total_results(), 42);
    assert_eq!(state.search_offset(), 0);
    assert_eq!(state.search_results.len(), 1);
    assert_eq!(state.search_results[0].hit_message.id, "1001");
    assert_eq!(state.search_results[0].hit_message.content, "Resultado da busca real");
    assert_eq!(state.search_results[0].context_before.len(), 1);
    assert_eq!(state.search_results[0].context_before[0].id, "1000");
    assert!(state.search_error().is_none());
    assert!(!state.is_search_indexing());
    assert!(!state.is_searching);
}

#[test]
fn test_two_phase_stale_response_discarded() {
    let mut state = setup_test_app_state();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);

    // Search 1 started
    let req1 = state.begin_search("busca 1").unwrap();
    assert_eq!(req1.generation, 1);

    // Search 2 started while search 1 was still in-flight
    let req2 = state.begin_search("busca 2").unwrap();
    assert_eq!(req2.generation, 2);

    let synthetic_payload_1 = SearchPayload {
        total_results: 10,
        messages: vec![vec![ApiSearchMessage {
            message: create_test_message("m-old", "c-geral", create_test_author("u-1", "user1"), "resposta antiga"),
            hit: true,
        }]],
        analytics_id: None,
    };

    // Stale response from req1 arrives late
    state.finish_search(&req1, Ok(SearchApiResponse::Ready(synthetic_payload_1)));

    // Must be discarded: search_results should NOT be populated by stale response
    assert!(state.search_results.is_empty());
    assert_eq!(state.search_total_results(), 0);
    assert!(state.is_searching); // Still searching for req2

    let synthetic_payload_2 = SearchPayload {
        total_results: 5,
        messages: vec![vec![ApiSearchMessage {
            message: create_test_message("m-new", "c-geral", create_test_author("u-2", "user2"), "resposta nova"),
            hit: true,
        }]],
        analytics_id: None,
    };

    // Active response for req2 arrives
    state.finish_search(&req2, Ok(SearchApiResponse::Ready(synthetic_payload_2)));

    assert_eq!(state.search_results.len(), 1);
    assert_eq!(state.search_results[0].hit_message.id, "m-new");
    assert_eq!(state.search_total_results(), 5);
    assert!(!state.is_searching);
}

#[test]
fn test_two_phase_http_202_indexing_and_retry() {
    let mut state = setup_test_app_state();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);

    let req = state.begin_search("query indexing").unwrap();

    // 1. API returns 202 Accepted (indexing in progress)
    state.finish_search(&req, Ok(SearchApiResponse::Indexing { retry_after: 3.5 }));

    assert!(state.is_search_indexing());
    assert_eq!(state.search_retry_after(), Some(3.5));
    assert!(state.is_searching);
    assert!(state.search_error().is_none());

    // 2. record_search_indexing also updates retry duration
    state.record_search_indexing(&req, 4.0);
    assert_eq!(state.search_retry_after(), Some(4.0));

    // Stale request indexing is ignored by generation safety
    let mut stale_req = req.clone();
    stale_req.generation = 999;
    state.record_search_indexing(&stale_req, 10.0);
    assert_eq!(state.search_retry_after(), Some(4.0));

    // 3. Ready response clears indexing and marks search complete
    let ready_payload = SearchPayload {
        total_results: 1,
        messages: vec![vec![ApiSearchMessage {
            message: create_test_message("m-indexed", "c-geral", create_test_author("u-1", "user"), "pronto"),
            hit: true,
        }]],
        analytics_id: None,
    };
    state.finish_search(&req, Ok(SearchApiResponse::Ready(ready_payload)));

    assert!(!state.is_search_indexing());
    assert_eq!(state.search_retry_after(), None);
    assert!(!state.is_searching);
    assert_eq!(state.search_total_results(), 1);
}

#[test]
fn test_two_phase_api_error_state_handling() {
    let mut state = setup_test_app_state();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);

    let req = state.begin_search("falha").unwrap();

    state.finish_search(&req, Err("Rate limited: HTTP 429".to_string()));

    assert_eq!(
        state.search_error().as_deref(),
        Some("Falha na busca: Rate limited: HTTP 429")
    );
    assert!(!state.is_searching);
    assert!(!state.is_search_indexing());
    assert!(state.search_results.is_empty());

    // Clearing search clears error
    state.clear_search();
    assert!(state.search_error().is_none());
}

#[test]
fn test_pagination_and_sort_order_refetches() {
    let mut state = setup_test_app_state();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    state.client_manager = Some(cm);

    // Default pagination offset is 0, default order is Desc ("desc")
    assert_eq!(state.search_offset(), 0);
    assert_eq!(state.search_sort_order(), SearchSortOrder::Desc);
    assert_eq!(state.search_sort_order().as_api_str(), "desc");

    let req1 = state.begin_search("paginacao").unwrap();
    assert_eq!(req1.offset, 0);
    assert_eq!(req1.sort_order, SearchSortOrder::Desc);

    // Advance to page 2 (offset 25)
    state.set_search_offset(25);
    assert_eq!(state.search_offset(), 25);
    let req2 = state.begin_search("paginacao").unwrap();
    assert_eq!(req2.offset, 25);
    assert_eq!(req2.sort_order, SearchSortOrder::Desc);

    // Advance to page 3 (offset 50)
    state.set_search_offset(50);
    assert_eq!(state.search_offset(), 50);
    let req3 = state.begin_search("paginacao").unwrap();
    assert_eq!(req3.offset, 50);

    // Toggle sort order to Asc ("asc") resets offset to 0 and refetches
    state.set_search_sort_order(SearchSortOrder::Asc);
    assert_eq!(state.search_sort_order(), SearchSortOrder::Asc);
    assert_eq!(state.search_sort_order().as_api_str(), "asc");
    assert_eq!(state.search_offset(), 0);
    let req4 = state.begin_search("paginacao").unwrap();
    assert_eq!(req4.offset, 0);
    assert_eq!(req4.sort_order, SearchSortOrder::Asc);

    // Close and clear resets offset and order
    state.clear_search();
    assert_eq!(state.search_offset(), 0);
    assert_eq!(state.search_sort_order(), SearchSortOrder::Desc);
}
