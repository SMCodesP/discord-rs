use discord_rs::app::state::AppState;
use discord_rs::domain::channel::Channel;
use discord_rs::domain::voice::VoiceState;
use discord_rs::protocol::TransportEvent;

#[test]
fn test_channel_user_limit_serde() {
    // 1. Channel without user_limit defaults to None
    let json_no_limit = r#"{
        "id": "chan-voice-1",
        "name": "General Voice",
        "type": 2
    }"#;
    let ch: Channel = serde_json::from_str(json_no_limit).expect("Failed to deserialize Channel");
    assert_eq!(ch.user_limit, None);
    assert!(ch.is_voice());

    // 2. Channel with user_limit is parsed correctly
    let json_with_limit = r#"{
        "id": "chan-voice-2",
        "name": "Duo 1",
        "type": 2,
        "user_limit": 2
    }"#;
    let ch2: Channel = serde_json::from_str(json_with_limit).expect("Failed to deserialize Channel");
    assert_eq!(ch2.user_limit, Some(2));

    // 3. Round-trip serialization
    let serialized = serde_json::to_string(&ch2).expect("Failed to serialize Channel");
    let ch2_back: Channel = serde_json::from_str(&serialized).expect("Round-trip failed");
    assert_eq!(ch2_back.user_limit, Some(2));
}

#[test]
fn test_voice_state_self_stream_and_serde() {
    let json_vs = r#"{
        "guild_id": "guild-1",
        "channel_id": "chan-voice-1",
        "user_id": "user-streamer",
        "session_id": "sess-xyz",
        "deaf": false,
        "mute": false,
        "self_deaf": false,
        "self_mute": true,
        "self_video": false,
        "self_stream": true,
        "suppress": false
    }"#;

    let vs: VoiceState = serde_json::from_str(json_vs).expect("Failed to deserialize VoiceState");
    assert_eq!(vs.user_id, "user-streamer");
    assert_eq!(vs.channel_id.as_deref(), Some("chan-voice-1"));
    assert!(vs.self_stream);
    assert!(vs.is_muted());
    assert!(!vs.is_deafened());
    assert!(!vs.is_speaking);
}

#[test]
fn test_app_state_voice_states_by_channel_lifecycle() {
    let mut state = AppState::new();

    // Verify initially empty voice states (Zero Mocks)
    assert!(state.voice_states_by_channel.is_empty());
    assert_eq!(state.voice_states_for_channel("chan-voice-1").len(), 0);

    // 1. User 1 joins channel 1
    let vs1 = VoiceState::new("user-1", "sess-1")
        .with_channel_id("chan-voice-1")
        .with_guild_id("guild-1")
        .with_user_name("Alice");

    state.update_voice_state(vs1);

    assert_eq!(state.voice_states_for_channel("chan-voice-1").len(), 1);
    assert_eq!(state.voice_states_for_channel("chan-voice-1")[0].user_id, "user-1");
    assert_eq!(state.voice_states_for_channel("chan-voice-1")[0].display_name(), "Alice");

    // 2. User 2 joins channel 1 with stream and mute
    let vs2 = VoiceState::new("user-2", "sess-2")
        .with_channel_id("chan-voice-1")
        .with_guild_id("guild-1")
        .with_user_name("Bob")
        .with_self_stream(true)
        .with_mute(true);

    state.update_voice_state(vs2);

    assert_eq!(state.voice_states_for_channel("chan-voice-1").len(), 2);

    // 3. User 1 moves from channel 1 to channel 2
    let vs1_moved = VoiceState::new("user-1", "sess-1")
        .with_channel_id("chan-voice-2")
        .with_guild_id("guild-1")
        .with_user_name("Alice");

    state.update_voice_state(vs1_moved);

    // Channel 1 should now have only User 2
    assert_eq!(state.voice_states_for_channel("chan-voice-1").len(), 1);
    assert_eq!(state.voice_states_for_channel("chan-voice-1")[0].user_id, "user-2");

    // Channel 2 should now have User 1
    assert_eq!(state.voice_states_for_channel("chan-voice-2").len(), 1);
    assert_eq!(state.voice_states_for_channel("chan-voice-2")[0].user_id, "user-1");

    // 4. User 2 disconnects (channel_id == None)
    let vs2_disc = VoiceState::new("user-2", "sess-2"); // channel_id is None
    state.update_voice_state(vs2_disc);

    assert_eq!(state.voice_states_for_channel("chan-voice-1").len(), 0);
    assert_eq!(state.voice_states_for_channel("chan-voice-2").len(), 1);

    // 5. User 1 disconnects
    let vs1_disc = VoiceState::new("user-1", "sess-1");
    state.update_voice_state(vs1_disc);

    assert_eq!(state.voice_states_for_channel("chan-voice-2").len(), 0);
}

#[test]
fn test_transport_event_voice_state_update_integration() {
    let mut state = AppState::new();

    let vs = VoiceState::new("user-carol", "sess-c")
        .with_channel_id("chan-voice-lounge")
        .with_user_name("Carol")
        .with_speaking(true);

    state.handle_transport_event(TransportEvent::VoiceStateUpdate(vs));

    let states = state.voice_states_for_channel("chan-voice-lounge");
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].user_id, "user-carol");
    assert!(states[0].is_speaking);
}

#[test]
fn test_capacity_badge_and_user_limits() {
    let mut state = AppState::new();

    // Set channel user limit
    state.set_channel_user_limit("chan-voice-trio", 3);
    state.set_channel_user_limit("chan-voice-unlimited", 0);

    assert_eq!(state.get_channel_user_limit("chan-voice-trio"), Some(3));
    assert_eq!(state.get_channel_user_limit("chan-voice-unlimited"), Some(0));

    // Initially 0 users
    let states_trio = state.voice_states_for_channel("chan-voice-trio");
    assert_eq!(states_trio.len(), 0);

    // Add 2 users to trio: count=2, limit=3 -> not full
    state.update_voice_state(VoiceState::new("u1", "s1").with_channel_id("chan-voice-trio"));
    state.update_voice_state(VoiceState::new("u2", "s2").with_channel_id("chan-voice-trio"));

    let count = state.voice_states_for_channel("chan-voice-trio").len();
    let limit = state.get_channel_user_limit("chan-voice-trio").unwrap_or(0);
    assert_eq!(count, 2);
    assert_eq!(limit, 3);
    assert!(count < limit as usize); // not full

    // Add 3rd user -> full (count >= limit)
    state.update_voice_state(VoiceState::new("u3", "s3").with_channel_id("chan-voice-trio"));
    let count = state.voice_states_for_channel("chan-voice-trio").len();
    assert_eq!(count, 3);
    assert!(count >= limit as usize); // full!

    // Add 1 user to unlimited channel -> count=1, limit=0
    state.update_voice_state(VoiceState::new("u4", "s4").with_channel_id("chan-voice-unlimited"));
    let count_unlimited = state.voice_states_for_channel("chan-voice-unlimited").len();
    let limit_unlimited = state.get_channel_user_limit("chan-voice-unlimited").unwrap_or(0);
    assert_eq!(count_unlimited, 1);
    assert_eq!(limit_unlimited, 0);
}

#[test]
fn test_guild_initial_snapshot_population() {
    let mut state = AppState::new();

    let initial_states = vec![
        VoiceState::new("user-10", "sess-10").with_channel_id("chan-v1").with_user_name("Dev 1"),
        VoiceState::new("user-20", "sess-20").with_channel_id("chan-v1").with_user_name("Dev 2"),
        VoiceState::new("user-30", "sess-30").with_channel_id("chan-v2").with_user_name("Dev 3"),
    ];

    state.populate_guild_voice_states(initial_states);

    assert_eq!(state.voice_states_for_channel("chan-v1").len(), 2);
    assert_eq!(state.voice_states_for_channel("chan-v2").len(), 1);
}
