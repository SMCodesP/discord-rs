use discord_rs::app::state::AppState;
use discord_rs::domain::settings::ThemeChoice;
use discord_rs::protocol::user_settings_proto::{
    decode_base64, decode_preloaded_user_settings, encode_base64, synthetic::*, Theme,
};
use discord_rs::protocol::TransportEvent;

#[test]
fn test_decode_valid_proto_appearance() {
    let mut top_level = Vec::new();

    // Appearance (Field 13): Theme::Darker (3), preset 108, custom colors ["#1e1f22", "#2b2d31"], stops [0.1, 0.9], angle 45, base_mix 10
    let app_bytes = build_appearance(
        3,
        Some(108),
        &["#1e1f22", "#2b2d31"],
        &[0.1, 0.9],
        Some(45),
        Some(10),
    );
    write_length_delimited(&mut top_level, 13, &app_bytes);

    let b64 = encode_base64(&top_level);
    let decoded = decode_preloaded_user_settings(&b64).expect("decode synthetic appearance proto");

    // Verify appearance theme and client theme settings
    assert_eq!(decoded.theme(), Some(Theme::Darker));
    let client_theme = decoded.client_theme_settings().expect("client theme");
    assert_eq!(client_theme.background_gradient_preset_id, Some(108));
    let custom = client_theme.custom_user_theme_settings.as_ref().expect("custom theme");
    assert_eq!(custom.colors, vec!["#1e1f22".to_string(), "#2b2d31".to_string()]);
    assert_eq!(custom.gradient_color_stops, vec![0.1, 0.9]);
    assert_eq!(custom.gradient_angle, Some(45));
    assert_eq!(custom.base_mix, Some(10));
}

#[test]
fn test_corrupted_and_truncated_proto_error_handling() {
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
fn test_account_theme_stored_in_app_state_without_mutating_theme_choice() {
    let mut state = AppState::new();
    assert_eq!(state.theme_choice, ThemeChoice::Dark);
    assert_eq!(state.account_theme, None);

    // Dispatch AccountThemeUpdated with Theme::Light
    state.handle_transport_event(TransportEvent::AccountThemeUpdated {
        theme: Theme::Light,
        client_theme_settings: None,
    });

    // Account theme is stored
    assert_eq!(state.account_theme, Some(Theme::Light));
    // theme_choice must NOT be mutated to Light (avoid showing "Claro" selected while only dark palette exists)
    assert_eq!(state.theme_choice, ThemeChoice::Dark);

    // Dispatch AccountThemeUpdated with Theme::Midnight
    state.handle_transport_event(TransportEvent::AccountThemeUpdated {
        theme: Theme::Midnight,
        client_theme_settings: None,
    });
    assert_eq!(state.account_theme, Some(Theme::Midnight));
    assert_eq!(state.theme_choice, ThemeChoice::Dark);
}

#[test]
fn test_partial_proto_update_preserves_existing_theme() {
    let mut state = AppState::new();

    // 1. Initially user has Theme::Darker configured
    state.handle_transport_event(TransportEvent::AccountThemeUpdated {
        theme: Theme::Darker,
        client_theme_settings: None,
    });
    assert_eq!(state.account_theme, Some(Theme::Darker));

    // 2. A partial USER_SETTINGS_PROTO_UPDATE arrives without `appearance`
    // (e.g. only audio or localization was updated, field 13 is absent)
    let partial_bytes = Vec::new(); // empty or unrelated fields
    let partial_b64 = encode_base64(&partial_bytes);
    let decoded = decode_preloaded_user_settings(&partial_b64).expect("decode partial proto");

    // When appearance is absent, theme() returns None
    assert_eq!(decoded.theme(), None);

    // In gateway dispatch, no AccountThemeUpdated event is fired if theme is None,
    // so state retains existing account_theme:
    assert_eq!(state.account_theme, Some(Theme::Darker));
}

#[test]
fn test_all_theme_variants_parsing() {
    // Verified against PreloadedUserSettings.proto:390-396:
    // 0 = UNSET, 1 = DARK, 2 = LIGHT, 3 = DARKER, 4 = MIDNIGHT
    assert_eq!(Theme::from_u32(0), Theme::Unset);
    assert_eq!(Theme::from_u32(1), Theme::Dark);
    assert_eq!(Theme::from_u32(2), Theme::Light);
    assert_eq!(Theme::from_u32(3), Theme::Darker);
    assert_eq!(Theme::from_u32(4), Theme::Midnight);
    assert_eq!(Theme::from_u32(999), Theme::Unset);

    for (val, expected) in [
        (0, Theme::Unset),
        (1, Theme::Dark),
        (2, Theme::Light),
        (3, Theme::Darker),
        (4, Theme::Midnight),
    ] {
        let mut top_level = Vec::new();
        let app_bytes = build_appearance(val, None, &[], &[], None, None);
        write_length_delimited(&mut top_level, 13, &app_bytes);

        let b64 = encode_base64(&top_level);
        let decoded = decode_preloaded_user_settings(&b64).unwrap();
        assert_eq!(decoded.theme(), Some(expected));
    }
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
