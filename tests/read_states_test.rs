use discord_rs::protocol::{
    compare_snowflakes, is_message_unread, is_newer_snowflake, normalize_snowflake, AckPayload,
    ChannelReadState, ReadAckDebouncer, ReadState, ReadStateEntry, ReadStateStore,
};
use std::cmp::Ordering;
use std::time::{Duration, Instant};

fn sample_ready_read_state() -> ReadState {
    serde_json::from_value(serde_json::json!({
        "version": 42,
        "partial": false,
        "entries": [
            {
                "id": "100000000000000001",
                "last_message_id": "200000000000000010",
                "mention_count": 2,
                "flags": 0,
                "last_pin_timestamp": null
            },
            {
                "id": "100000000000000002",
                "last_message_id": "200000000000000020",
                "mention_count": 0
            }
        ]
    }))
    .expect("valid READY read_state fixture")
}

#[test]
fn ready_read_state_deserializes_and_store_tracks_entries() {
    let read_state = sample_ready_read_state();
    assert_eq!(read_state.version, 42);
    assert!(!read_state.partial);
    assert_eq!(read_state.entries.len(), 2);

    let store = ReadStateStore::from_ready(&read_state);
    let ch1 = store.get("100000000000000001").expect("channel entry");
    assert_eq!(
        ch1.last_read_message_id.as_deref(),
        Some("200000000000000010")
    );
    assert_eq!(ch1.mention_count, 2);
    assert_eq!(ch1.read_state_version, 42);
    assert!(!ch1.has_unread(), "no last_message_id observed yet");

    let ch2 = store.get("100000000000000002").expect("channel entry");
    assert_eq!(ch2.mention_count, 0);
}

#[test]
fn snowflake_ordering_uses_discord_chronology_not_lexical_string_order() {
    // Same numeric length but different values; also ensure numeric comparison.
    let older = "200000000000000009";
    let newer = "200000000000000010";
    assert_eq!(compare_snowflakes(Some(older), Some(newer)), Ordering::Less);
    assert_eq!(compare_snowflakes(Some(newer), Some(older)), Ordering::Greater);
    assert_eq!(compare_snowflakes(Some(newer), Some(newer)), Ordering::Equal);
    assert!(is_newer_snowflake(Some(newer), Some(older)));
    assert!(!is_newer_snowflake(Some(older), Some(newer)));

    // Lexical trap: "100" vs "99" as strings would be wrong lexically in naive compare;
    // snowflake numeric ordering should treat 100 as newer.
    assert_eq!(compare_snowflakes(Some("100"), Some("99")), Ordering::Greater);
    assert!(is_message_unread(Some("100"), Some("99")));
    assert!(!is_message_unread(Some("99"), Some("100")));

    // Missing/invalid IDs are not considered unread evidence.
    assert!(!is_message_unread(None, Some("100")));
    assert!(is_message_unread(Some("100"), None));
    assert!(!is_message_unread(Some("not-a-snowflake"), Some("100")));
}

#[test]
fn normalize_snowflake_rejects_invalid_values() {
    assert_eq!(normalize_snowflake(" 123 "), Some("123".to_string()));
    assert_eq!(normalize_snowflake(""), None);
    assert_eq!(normalize_snowflake("  "), None);
    assert_eq!(normalize_snowflake("abc"), None);
    assert_eq!(normalize_snowflake("-1"), None);
}

#[test]
fn channel_read_state_tracks_unread_and_mentions_from_messages() {
    let entry = ReadStateEntry {
        id: "100000000000000001".to_string(),
        last_message_id: Some("200000000000000010".to_string()),
        mention_count: 0,
        flags: 0,
        last_pin_timestamp: None,
    };
    let mut state = ChannelReadState::from_ready_entry(&entry, 7);

    // Older/equal message does not create unread.
    assert!(!state.observe_message("200000000000000010", false, 0));
    assert!(!state.has_unread());

    // Newer message from another user creates unread.
    assert!(state.observe_message("200000000000000011", false, 0));
    assert!(state.has_unread());
    assert_eq!(state.mention_count, 0);

    // Mention increments are tracked.
    assert!(state.observe_message("200000000000000012", false, 1));
    assert_eq!(state.mention_count, 1);
    assert!(state.observe_message("200000000000000013", false, 2));
    assert_eq!(state.mention_count, 3);

    // Own message advances read marker and clears mentions.
    assert!(!state.observe_message("200000000000000014", true, 0));
    assert!(!state.has_unread());
    assert_eq!(
        state.last_read_message_id.as_deref(),
        Some("200000000000000014")
    );
    assert_eq!(state.mention_count, 0);
}

#[test]
fn mark_read_up_to_is_monotonic_and_clears_mentions_when_caught_up() {
    let mut state = ChannelReadState::new("100000000000000001");
    state.last_message_id = Some("200000000000000020".to_string());
    state.last_read_message_id = Some("200000000000000010".to_string());
    state.mention_count = 5;

    // Older read ack does not regress.
    assert!(!state.mark_read_up_to("200000000000000005"));
    assert_eq!(
        state.last_read_message_id.as_deref(),
        Some("200000000000000010")
    );
    assert_eq!(state.mention_count, 5);

    // Partial read advances marker but may still be unread; mentions remain until caught up.
    assert!(state.mark_read_up_to("200000000000000015"));
    assert_eq!(
        state.last_read_message_id.as_deref(),
        Some("200000000000000015")
    );
    assert!(state.has_unread());
    assert_eq!(state.mention_count, 5);

    // Caught up clears mentions.
    assert!(state.mark_read_up_to("200000000000000020"));
    assert!(!state.has_unread());
    assert_eq!(state.mention_count, 0);
}

#[test]
fn read_ack_payload_serializes_expected_shape() {
    let payload = AckPayload::manual(3);
    let json = serde_json::to_value(&payload).expect("serialize ack payload");
    assert_eq!(json["manual"], true);
    assert_eq!(json["mention_count"], 3);
    assert!(json.get("token").is_none(), "token should be omitted when absent");
}

#[test]
fn read_ack_debouncer_coalesces_to_newest_message_and_debounces() {
    let mut debouncer = ReadAckDebouncer::new(Duration::from_millis(500));
    let t0 = Instant::now();

    assert!(debouncer.schedule("chan1", "200000000000000010", 1, true, t0));
    assert!(debouncer.schedule(
        "chan1",
        "200000000000000020",
        0,
        true,
        t0 + Duration::from_millis(100)
    ));
    // Older message should not regress the coalesced ACK target.
    assert!(debouncer.schedule(
        "chan1",
        "200000000000000015",
        2,
        true,
        t0 + Duration::from_millis(200)
    ));
    // Invalid IDs are ignored.
    assert!(!debouncer.schedule("chan1", "not-a-snowflake", 0, true, t0));

    // Not due before debounce window elapses from last schedule.
    assert!(debouncer
        .drain_due(t0 + Duration::from_millis(650))
        .is_empty());
    assert_eq!(debouncer.pending_len(), 1);

    let due = debouncer.drain_due(t0 + Duration::from_millis(701));
    assert_eq!(due.len(), 1);
    let ack = &due[0];
    assert_eq!(ack.channel_id, "chan1");
    assert_eq!(ack.message_id, "200000000000000020");
    assert_eq!(ack.payload.manual, Some(true));
    assert_eq!(ack.payload.mention_count, Some(2));
    assert_eq!(debouncer.pending_len(), 0);
}

#[test]
fn read_ack_debouncer_drain_all_returns_stable_channel_order() {
    let mut debouncer = ReadAckDebouncer::new(Duration::from_secs(2));
    let now = Instant::now();
    assert!(debouncer.schedule("chan-b", "200000000000000002", 0, true, now));
    assert!(debouncer.schedule("chan-a", "200000000000000001", 1, true, now));

    let all = debouncer.drain_all();
    let channels: Vec<&str> = all.iter().map(|a| a.channel_id.as_str()).collect();
    assert_eq!(channels, vec!["chan-a", "chan-b"]);
}

#[test]
fn read_state_store_apply_ready_updates_versions_and_mention_counts() {
    let mut store = ReadStateStore::new();
    store.apply_ready(&sample_ready_read_state());

    // Simulate newer READY/read_state with updated mention count and newer read marker.
    let updated = ReadState {
        version: 43,
        partial: false,
        entries: vec![ReadStateEntry {
            id: "100000000000000001".to_string(),
            last_message_id: Some("200000000000000015".to_string()),
            mention_count: 1,
            flags: 0,
            last_pin_timestamp: None,
        }],
    };
    store.apply_ready(&updated);

    let ch1 = store.get("100000000000000001").expect("channel");
    assert_eq!(ch1.read_state_version, 43);
    assert_eq!(ch1.mention_count, 1);
    assert_eq!(
        ch1.last_read_message_id.as_deref(),
        Some("200000000000000015")
    );
}
