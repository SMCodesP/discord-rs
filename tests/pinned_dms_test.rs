use discord_rs::app::state::AppState;
use discord_rs::domain::Channel;
use discord_rs::protocol::pinned_dms::{
    load_pinned_dms_from_path, pinned_dms_cache_path, save_pinned_dms_to_path,
};
use discord_rs::protocol::TransportEvent;

#[test]
fn test_pinned_dms_cache_path_validity() {
    let path = pinned_dms_cache_path();
    assert!(path.to_string_lossy().contains("pinned_dms.json"));
    assert!(path.to_string_lossy().contains("discord-rs/cache"));
}

#[test]
fn test_pinned_dms_cache_save_and_load() {
    let temp_dir = std::env::temp_dir().join(format!("discord_rs_pins_test_{}", std::process::id()));
    let cache_file = temp_dir.join("pinned_dms.json");

    let original = vec![
        "1233438711597891736".to_string(),
        "987654321098765432".to_string(),
    ];
    save_pinned_dms_to_path(&cache_file, &original);

    let loaded = load_pinned_dms_from_path(&cache_file);
    assert_eq!(loaded, original);

    // Test JSON wrapper format as well (e.g. {"pinned_dms": [...]})
    let wrapper_json = r#"{"pinned_dms": ["dm-custom-1", "dm-custom-2"]}"#;
    std::fs::write(&cache_file, wrapper_json).unwrap();
    let loaded_wrapper = load_pinned_dms_from_path(&cache_file);
    assert_eq!(loaded_wrapper, vec!["dm-custom-1", "dm-custom-2"]);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_pinned_dms_strict_array_ordering() {
    let mut state = AppState::new();
    state.dm_channels.clear();

    // Define 2 pinned DMs and 2 unpinned DMs
    // dm-b has older message but is first in pinned_dms!
    // dm-a has newer message but is second in pinned_dms!
    state.pinned_dms = vec!["dm-b".to_string(), "dm-a".to_string()];

    let mut ch_a = Channel::text("dm-a", "Channel A");
    ch_a.is_pinned = true;
    ch_a.last_message_id = Some("500000000000000000".to_string());

    let mut ch_b = Channel::text("dm-b", "Channel B");
    ch_b.is_pinned = true;
    ch_b.last_message_id = Some("100000000000000000".to_string());

    let mut ch_unpinned_recent = Channel::text("dm-recent", "Channel Recent");
    ch_unpinned_recent.is_pinned = false;
    ch_unpinned_recent.last_message_id = Some("900000000000000000".to_string());

    let mut ch_unpinned_old = Channel::text("dm-old", "Channel Old");
    ch_unpinned_old.is_pinned = false;
    ch_unpinned_old.last_message_id = Some("200000000000000000".to_string());

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![
        ch_unpinned_old,
        ch_a,
        ch_unpinned_recent,
        ch_b,
    ]));

    assert_eq!(state.dm_channels.len(), 4);

    // 1. Pinned DMs must be at the absolute top, strictly in the order of state.pinned_dms:
    // "dm-b" is first in pinned_dms, so it MUST be index 0
    assert_eq!(state.dm_channels[0].id, "dm-b");
    assert!(state.dm_channels[0].is_pinned);

    // "dm-a" is second in pinned_dms, so it MUST be index 1
    assert_eq!(state.dm_channels[1].id, "dm-a");
    assert!(state.dm_channels[1].is_pinned);

    // 2. Unpinned DMs must follow below, ordered by last_message_id descending
    assert_eq!(state.dm_channels[2].id, "dm-recent");
    assert!(!state.dm_channels[2].is_pinned);

    assert_eq!(state.dm_channels[3].id, "dm-old");
    assert!(!state.dm_channels[3].is_pinned);
}

#[test]
fn test_pin_unpin_and_toggle_methods() {
    let mut state = AppState::new();
    state.dm_channels.clear();
    state.pinned_dms.clear();

    let mut ch1 = Channel::text("dm-1", "Channel 1");
    ch1.last_message_id = Some("100".to_string());

    let mut ch2 = Channel::text("dm-2", "Channel 2");
    ch2.last_message_id = Some("200".to_string());

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![ch1, ch2]));

    // Initially sorted by recency: dm-2 (200), dm-1 (100)
    assert_eq!(state.dm_channels[0].id, "dm-2");
    assert_eq!(state.dm_channels[1].id, "dm-1");

    // Pin dm-1: it should jump to top
    state.pin_dm_channel("dm-1");
    assert_eq!(state.dm_channels[0].id, "dm-1");
    assert!(state.dm_channels[0].is_pinned);
    assert!(state.pinned_dms.contains(&"dm-1".to_string()));

    // Toggle pin on dm-2: it should also become pinned
    state.toggle_pin_dm_channel("dm-2");
    assert!(state.dm_channels.iter().all(|d| d.is_pinned));
    // Since dm-1 was pinned first, dm-1 is index 0 in pinned_dms, dm-2 is index 1
    assert_eq!(state.dm_channels[0].id, "dm-1");
    assert_eq!(state.dm_channels[1].id, "dm-2");

    // Unpin dm-1
    state.unpin_dm_channel("dm-1");
    assert!(!state.pinned_dms.contains(&"dm-1".to_string()));
    assert_eq!(state.dm_channels[0].id, "dm-2"); // only dm-2 is pinned
    assert_eq!(state.dm_channels[1].id, "dm-1"); // dm-1 is unpinned
}

#[test]
fn test_pinned_dms_updated_event() {
    let mut state = AppState::new();
    state.dm_channels.clear();
    state.pinned_dms.clear();

    let ch1 = Channel::text("dm-1", "Channel 1");
    let ch2 = Channel::text("dm-2", "Channel 2");
    let ch3 = Channel::text("dm-3", "Channel 3");

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![ch1, ch2, ch3]));

    // Dispatch PinnedDmsUpdated event with order ["dm-3", "dm-1"]
    state.handle_transport_event(TransportEvent::PinnedDmsUpdated(vec![
        "dm-3".to_string(),
        "dm-1".to_string(),
    ]));

    assert_eq!(state.pinned_dms, vec!["dm-3", "dm-1"]);
    assert_eq!(state.dm_channels[0].id, "dm-3");
    assert!(state.dm_channels[0].is_pinned);
    assert_eq!(state.dm_channels[1].id, "dm-1");
    assert!(state.dm_channels[1].is_pinned);
    assert_eq!(state.dm_channels[2].id, "dm-2");
    assert!(!state.dm_channels[2].is_pinned);
}
