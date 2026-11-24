use chrono::{Duration, TimeZone, Utc};
use discord_rs::components::{
    build_gateway_presence_update_payload, build_settings_custom_status_payload,
    CustomStatusDraft, CustomStatusExpiration, StatusPickerState, UserStatusSelection,
};
use discord_rs::domain::user::{CustomStatusEmoji, PresenceStatus};

#[test]
fn models_user_status_selection_and_invisible_mapping() {
    assert_eq!(UserStatusSelection::ALL.len(), 4);
    assert_eq!(UserStatusSelection::Online.as_gateway_str(), "online");
    assert_eq!(UserStatusSelection::Idle.as_gateway_str(), "idle");
    assert_eq!(UserStatusSelection::Dnd.as_gateway_str(), "dnd");
    assert_eq!(UserStatusSelection::Invisible.as_gateway_str(), "invisible");
    assert!(UserStatusSelection::Idle.is_afk());
    assert!(!UserStatusSelection::Invisible.is_afk());

    // Domain currently represents invisible as Offline presence for UI/other users.
    assert_eq!(
        UserStatusSelection::Invisible.to_presence_status(),
        PresenceStatus::Offline
    );
    assert_eq!(
        UserStatusSelection::from_presence_status(PresenceStatus::Offline),
        UserStatusSelection::Invisible
    );
    assert_eq!(
        UserStatusSelection::from_gateway_str("invisible"),
        Some(UserStatusSelection::Invisible)
    );
}

#[test]
fn custom_status_draft_handles_text_emoji_and_empty() {
    let draft = CustomStatusDraft::new()
        .with_text("  coding  ")
        .with_emoji(CustomStatusEmoji::unicode("🦀"))
        .with_expiration(CustomStatusExpiration::Never);

    assert_eq!(draft.text.as_deref(), Some("coding"));
    assert_eq!(draft.emoji.as_ref().unwrap().name, "🦀");
    assert!(!draft.is_empty());
    assert!(draft.to_custom_status().is_some());

    let cleared = CustomStatusDraft::new().with_text("   ");
    assert!(cleared.is_empty());
    assert!(cleared.to_custom_status().is_none());
}

#[test]
fn expiration_presets_cover_requested_options() {
    let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 34, 56).unwrap();
    let labels: Vec<_> = CustomStatusExpiration::ALL
        .iter()
        .map(|p| p.label())
        .collect();
    assert_eq!(labels, vec!["Today", "4 hours", "1 hour", "30 minutes", "Never"]);

    assert_eq!(
        CustomStatusExpiration::ThirtyMinutes.expires_at(now),
        Some(now + Duration::minutes(30))
    );
    assert_eq!(
        CustomStatusExpiration::OneHour.expires_at(now),
        Some(now + Duration::hours(1))
    );
    assert_eq!(
        CustomStatusExpiration::FourHours.expires_at(now),
        Some(now + Duration::hours(4))
    );
    assert_eq!(
        CustomStatusExpiration::Today.expires_at(now),
        Some(Utc.with_ymd_and_hms(2026, 11, 25, 0, 0, 0).unwrap())
    );
    assert_eq!(CustomStatusExpiration::Never.expires_at(now), None);
}

#[test]
fn builds_gateway_presence_payload_for_status_and_custom_status() {
    let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let draft = CustomStatusDraft::new()
        .with_text("In focus mode")
        .with_emoji(CustomStatusEmoji::custom("1020304050", "ferris_party", true))
        .with_expiration(CustomStatusExpiration::FourHours);

    let payload = build_gateway_presence_update_payload(UserStatusSelection::Idle, &draft, now);
    assert_eq!(payload["op"], 3);
    assert_eq!(payload["d"]["status"], "idle");
    assert_eq!(payload["d"]["afk"], true);
    assert!(payload["d"]["since"].as_i64().unwrap() > 0);

    let activity = &payload["d"]["activities"][0];
    assert_eq!(activity["type"], 4);
    assert_eq!(activity["name"], "Custom Status");
    assert_eq!(activity["state"], "In focus mode");
    assert_eq!(activity["emoji"]["name"], "ferris_party");
    assert_eq!(activity["emoji"]["id"], "1020304050");
    assert_eq!(activity["emoji"]["animated"], true);
    assert_eq!(
        activity["timestamps"]["end"],
        (now + Duration::hours(4)).timestamp_millis()
    );
}

#[test]
fn builds_gateway_payload_without_custom_status_activity() {
    let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let payload = build_gateway_presence_update_payload(
        UserStatusSelection::Online,
        &CustomStatusDraft::default(),
        now,
    );

    assert_eq!(payload["op"], 3);
    assert_eq!(payload["d"]["status"], "online");
    assert_eq!(payload["d"]["afk"], false);
    assert_eq!(payload["d"]["since"], 0);
    assert_eq!(payload["d"]["activities"].as_array().unwrap().len(), 0);
}

#[test]
fn builds_settings_payload_and_clear_payload() {
    let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let draft = CustomStatusDraft::new()
        .with_text("Lunch")
        .with_emoji(CustomStatusEmoji::unicode("🍱"))
        .with_expiration(CustomStatusExpiration::ThirtyMinutes);

    let payload = build_settings_custom_status_payload(&draft, now);
    assert_eq!(payload["custom_status"]["text"], "Lunch");
    assert_eq!(payload["custom_status"]["emoji_name"], "🍱");
    assert!(payload["custom_status"]["emoji_id"].is_null());
    assert_eq!(
        payload["custom_status"]["expires_at"],
        (now + Duration::minutes(30)).to_rfc3339()
    );

    let clear = build_settings_custom_status_payload(&CustomStatusDraft::default(), now);
    assert!(clear["custom_status"].is_null());
}

#[test]
fn status_picker_state_builds_both_payloads() {
    let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let mut state = StatusPickerState::new(UserStatusSelection::Invisible);
    state.set_custom_status(
        CustomStatusDraft::new()
            .with_text("Away")
            .with_expiration(CustomStatusExpiration::OneHour),
    );

    let gateway = state.gateway_presence_update(now);
    assert_eq!(gateway["d"]["status"], "invisible");
    assert_eq!(gateway["d"]["activities"][0]["state"], "Away");

    let settings = state.settings_custom_status_payload(now);
    assert_eq!(settings["custom_status"]["text"], "Away");

    state.clear_custom_status();
    let cleared = state.settings_custom_status_payload(now);
    assert!(cleared["custom_status"].is_null());
}
