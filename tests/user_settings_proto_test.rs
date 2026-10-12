use discord_rs::app::state::AppState;
use discord_rs::domain::settings::ThemeChoice;
use discord_rs::domain::Channel;
use discord_rs::protocol::user_settings_proto::{
    decode_base64, decode_preloaded_user_settings, encode_base64, synthetic::*, Theme,
};
use discord_rs::protocol::TransportEvent;
use discord_rs::tokens::colors::DiscordPalette;

#[test]
fn test_decode_valid_proto_pinned_dms_and_appearance() {
    let mut top_level = Vec::new();

    // 1. Appearance (Field 13): Theme::Darker (3), preset 108, custom colors ["#1e1f22", "#2b2d31"]
    let app_bytes = build_appearance(3, Some(108), &["#1e1f22", "#2b2d31"]);
    write_length_delimited(&mut top_level, 13, &app_bytes);

    // 2. Favorites (Field 15) with multiple channels:
    // - DM 1: channel_id 101, position 5, channel_type 1 (DM)
    // - DM 2: channel_id 102, position 1, channel_type 1 (DM)
    // - Group DM: channel_id 103, position 3, channel_type 3 (GROUP_DM)
    // - Server Text Channel: channel_id 201, position 0, channel_type 0 (GUILD_TEXT - should be filtered out)
    // - Server Voice Channel: channel_id 202, position 2, channel_type 2 (GUILD_VOICE - should be filtered out)
    let mut favs_buf = Vec::new();
    let entry_dm1 = build_favorite_entry_fixed64(101, 5, Some(1));
    write_length_delimited(&mut favs_buf, 1, &entry_dm1);

    let entry_dm2 = build_favorite_entry_fixed64(102, 1, Some(1));
    write_length_delimited(&mut favs_buf, 1, &entry_dm2);

    let entry_group_dm = build_favorite_entry_fixed64(103, 3, Some(3));
    write_length_delimited(&mut favs_buf, 1, &entry_group_dm);

    let entry_guild_text = build_favorite_entry_fixed64(201, 0, Some(0));
    write_length_delimited(&mut favs_buf, 1, &entry_guild_text);

    let entry_guild_voice = build_favorite_entry_fixed64(202, 2, Some(2));
    write_length_delimited(&mut favs_buf, 1, &entry_guild_voice);

    write_length_delimited(&mut top_level, 15, &favs_buf);

    let b64 = encode_base64(&top_level);
    let decoded = decode_preloaded_user_settings(&b64).expect("decode synthetic proto");

    // Verify appearance theme and client theme settings
    assert_eq!(decoded.theme(), Some(Theme::Darker));
    let client_theme = decoded.client_theme_settings().expect("client theme");
    assert_eq!(client_theme.background_gradient_preset_id, Some(108));
    let custom = client_theme.custom_user_theme_settings.as_ref().expect("custom theme");
    assert_eq!(custom.colors, vec!["#1e1f22".to_string(), "#2b2d31".to_string()]);

    // Verify pinned DMs: only channel_type 1 and 3, ordered by position:
    // 102 (pos 1) -> 103 (pos 3) -> 101 (pos 5)
    let pinned_dms = decoded.pinned_dms();
    assert_eq!(pinned_dms, vec!["102".to_string(), "103".to_string(), "101".to_string()]);
}

#[test]
fn test_corrupted_and_missing_proto_error_handling() {
    // 1. Invalid base64 characters
    let bad_b64 = "!!!NotBase64@@@";
    assert!(decode_preloaded_user_settings(bad_b64).is_err());

    // 2. Corrupted protobuf varint (unexpected EOF while high bit set)
    let corrupted_proto = vec![0x80, 0x80, 0x80];
    let bad_proto_b64 = encode_base64(&corrupted_proto);
    assert!(decode_preloaded_user_settings(&bad_proto_b64).is_err());

    // 3. Length-delimited header with truncated payload
    let mut truncated_buf = Vec::new();
    write_tag(&mut truncated_buf, 13, 2);
    write_varint(&mut truncated_buf, 100); // Claims 100 bytes length
    truncated_buf.extend_from_slice(&[1, 2, 3]); // Only provides 3 bytes
    let truncated_b64 = encode_base64(&truncated_buf);
    assert!(decode_preloaded_user_settings(&truncated_b64).is_err());
}

#[test]
fn test_corrupted_or_missing_proto_preserves_local_pinned_dms() {
    let mut state = AppState::new();
    state.dm_channels.clear();

    // Set initial local pinned DMs (e.g. from disk cache)
    state.pinned_dms = vec!["dm-local-1".to_string(), "dm-local-2".to_string()];

    let mut dm1 = Channel::text("dm-local-1", "Alice");
    dm1.is_pinned = true;
    let mut dm2 = Channel::text("dm-local-2", "Bob");
    dm2.is_pinned = true;
    let mut dm3 = Channel::text("dm-local-3", "Charlie");
    dm3.is_pinned = false;

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![
        dm1,
        dm2,
        dm3,
    ]));

    // Corrupted proto string decoding attempt
    let corrupted_b64 = "CorruptedBase64Bytes!!";
    let decode_result = decode_preloaded_user_settings(corrupted_b64);
    assert!(decode_result.is_err());

    // On decode failure, handler does NOT emit PinnedDmsUpdated, so local state remains intact
    assert_eq!(state.pinned_dms, vec!["dm-local-1", "dm-local-2"]);
    assert!(state.dm_channels[0].is_pinned);
    assert!(state.dm_channels[1].is_pinned);
    assert!(!state.dm_channels[2].is_pinned);

    // Empty proto (missing favorites) decoding attempt
    let empty_b64 = encode_base64(&[]);
    let empty_settings = decode_preloaded_user_settings(&empty_b64).expect("empty proto");
    assert!(empty_settings.pinned_dms().is_empty());

    // Empty list does not overwrite local cache
    assert_eq!(state.pinned_dms, vec!["dm-local-1", "dm-local-2"]);
}

#[test]
fn test_user_settings_proto_update_reorders_dms() {
    let mut state = AppState::new();
    state.dm_channels.clear();

    // Channels initially unpinned or ordered differently
    let mut dm_alice = Channel::text("101", "Alice");
    dm_alice.is_pinned = false;
    let mut dm_bob = Channel::text("102", "Bob");
    dm_bob.is_pinned = false;
    let mut dm_carol = Channel::text("103", "Carol");
    dm_carol.is_pinned = false;

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![
        dm_alice,
        dm_bob,
        dm_carol,
    ]));

    // Build synthetic proto where Carol is position 0, Alice is position 1
    let mut top_level = Vec::new();
    let mut favs_buf = Vec::new();
    let entry_carol = build_favorite_entry_fixed64(103, 0, Some(1));
    write_length_delimited(&mut favs_buf, 1, &entry_carol);
    let entry_alice = build_favorite_entry_fixed64(101, 1, Some(1));
    write_length_delimited(&mut favs_buf, 1, &entry_alice);
    write_length_delimited(&mut top_level, 15, &favs_buf);

    let b64 = encode_base64(&top_level);
    let settings = decode_preloaded_user_settings(&b64).expect("decode proto");
    let new_pins = settings.pinned_dms();
    assert_eq!(new_pins, vec!["103".to_string(), "101".to_string()]);

    // Dispatch PinnedDmsUpdated (as happens when USER_SETTINGS_PROTO_UPDATE arrives)
    state.handle_transport_event(TransportEvent::PinnedDmsUpdated(new_pins));

    // Verify pinned_dms updated
    assert_eq!(state.pinned_dms, vec!["103".to_string(), "101".to_string()]);

    // Verify dm_channels re-sorted:
    // Index 0: 103 (Carol, pinned, pos 0)
    // Index 1: 101 (Alice, pinned, pos 1)
    // Index 2: 102 (Bob, unpinned)
    assert_eq!(state.dm_channels[0].id, "103");
    assert!(state.dm_channels[0].is_pinned);

    assert_eq!(state.dm_channels[1].id, "101");
    assert!(state.dm_channels[1].is_pinned);

    assert_eq!(state.dm_channels[2].id, "102");
    assert!(!state.dm_channels[2].is_pinned);
}

#[test]
fn test_account_theme_stored_in_app_state_and_fallback_to_dark() {
    let mut state = AppState::new();

    // 1. Test Theme::Light
    state.handle_transport_event(TransportEvent::AccountThemeUpdated {
        theme: Theme::Light,
        client_theme_settings: None,
    });
    assert_eq!(state.account_theme, Some(Theme::Light));
    assert_eq!(state.theme_choice, ThemeChoice::Light);

    // Fallback: DiscordPalette::for_theme falls back to dark palette (no fake light palette)
    let palette_light = DiscordPalette::for_theme(Theme::Light);
    assert_eq!(palette_light, DiscordPalette::dark());

    // 2. Test Theme::Midnight
    state.handle_transport_event(TransportEvent::AccountThemeUpdated {
        theme: Theme::Midnight,
        client_theme_settings: None,
    });
    assert_eq!(state.account_theme, Some(Theme::Midnight));
    assert_eq!(state.theme_choice, ThemeChoice::Midnight);

    let palette_midnight = DiscordPalette::for_theme(Theme::Midnight);
    assert_eq!(palette_midnight, DiscordPalette::dark());

    // 3. Test Theme::Darker
    state.handle_transport_event(TransportEvent::AccountThemeUpdated {
        theme: Theme::Darker,
        client_theme_settings: None,
    });
    assert_eq!(state.account_theme, Some(Theme::Darker));
    assert_eq!(state.theme_choice, ThemeChoice::Dark);
    assert_eq!(DiscordPalette::for_theme(Theme::Darker), DiscordPalette::dark());
}

#[test]
fn test_base64_decoding_variants() {
    let data = b"ProtobufSettingsBytesTest";
    let standard_b64 = encode_base64(data);
    let decoded = decode_base64(&standard_b64).expect("decode standard base64");
    assert_eq!(decoded.as_slice(), data.as_slice());

    // Test with whitespace and newlines interspersed
    let whitespace_b64 = format!("  \n{} \r\n ", standard_b64);
    let decoded_ws = decode_base64(&whitespace_b64).expect("decode base64 with whitespace");
    assert_eq!(decoded_ws.as_slice(), data.as_slice());
}
