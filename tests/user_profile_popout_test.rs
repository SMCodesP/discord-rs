use chrono::{Datelike, Timelike};
use discord_rs::app::state::AppState;
use discord_rs::domain::profile::{
    snowflake_to_datetime, ProfileBadge, ProfileTab, UserProfileData,
};
use discord_rs::protocol::discord_api::parse_user_profile_from_json;
use discord_rs::protocol::transport::TransportEvent;
use gpui_kit::{point, px};

#[test]
fn test_snowflake_to_datetime_calculation() {
    // Discord Epoch: 1420070400000 ms = 2015-01-01 00:00:00 UTC
    // Snowflake 0 -> 2015-01-01 00:00:00 UTC
    let dt_zero = snowflake_to_datetime("0").expect("valid epoch datetime");
    assert_eq!(dt_zero.year(), 2015);
    assert_eq!(dt_zero.month(), 1);
    assert_eq!(dt_zero.day(), 1);
    assert_eq!(dt_zero.hour(), 0);
    assert_eq!(dt_zero.minute(), 0);

    // Synthetic snowflake: 300000000000000001
    // (300000000000000001 >> 22) = 71525573730 ms since Discord Epoch
    // 1420070400000 + 71525573730 = 1491595973730 ms = 2017-04-07 20:12:53 UTC
    let dt_known = snowflake_to_datetime("300000000000000001").expect("valid snowflake");
    assert_eq!(dt_known.year(), 2017);
    assert_eq!(dt_known.month(), 4);
    assert_eq!(dt_known.day(), 7);

    // Invalid snowflake string returns None
    assert!(snowflake_to_datetime("invalid_snowflake").is_none());
}

#[test]
fn test_parse_user_profile_from_real_api_json() {
    let raw_json = serde_json::json!({
        "user": {
            "id": "123456789012345678",
            "username": "discord_dev",
            "global_name": "Developer",
            "avatar": "a_123456789abcdef",
            "banner": "b_123456789abcdef",
            "banner_color": "#5865f2",
            "accent_color": 5793266,
            "bio": "Building native apps with GPUI!"
        },
        "user_profile": {
            "bio": "Building native apps with GPUI!",
            "pronouns": "they/them",
            "accent_color": 5793266
        },
        "badges": [
            {
                "id": "hypesquad_bravery",
                "description": "HypeSquad Bravery",
                "icon": "8a88d63823d8a71cd5e390baa45efa02"
            },
            {
                "id": "early_supporter",
                "description": "Apoiador Inicial",
                "icon": "70d3562a96dfd1c78918bc2feb3503a1"
            }
        ],
        "connected_accounts": [
            {
                "type": "github",
                "id": "dev123",
                "name": "DevOnGitHub",
                "verified": true
            }
        ],
        "mutual_guilds": [
            { "id": "999888777666", "nick": "DevNick" }
        ],
        "mutual_friends_count": 5
    });

    let profile = parse_user_profile_from_json(&raw_json).expect("profile parses successfully");
    assert_eq!(profile.user.id, "123456789012345678");
    assert_eq!(profile.user.username, "discord_dev");
    assert_eq!(profile.user.global_name.as_deref(), Some("Developer"));
    assert_eq!(profile.pronouns.as_deref(), Some("they/them"));
    assert_eq!(profile.bio.as_deref(), Some("Building native apps with GPUI!"));
    assert_eq!(profile.banner_color.as_deref(), Some("#5865f2"));
    assert_eq!(profile.accent_color, Some(5793266));
    assert_eq!(profile.badges.len(), 2);
    assert_eq!(profile.badges[0].id, "hypesquad_bravery");
    assert_eq!(profile.badges[1].description, "Apoiador Inicial");
    assert_eq!(profile.connected_accounts.len(), 1);
    assert_eq!(profile.connected_accounts[0].account_type, "github");
    assert_eq!(profile.connected_accounts[0].name, "DevOnGitHub");
    assert_eq!(profile.mutual_guilds.len(), 1);
    assert_eq!(profile.mutual_guilds[0].id, "999888777666");
    assert_eq!(profile.mutual_friends_count, 5);
}

#[test]
fn test_parse_user_profile_minimal_missing_fields_zero_mocks() {
    // Handling missing optional fields gracefully without fake mocks
    let raw_json = serde_json::json!({
        "user": {
            "id": "987654321098765432",
            "username": "minimal_user"
        }
    });

    let profile = parse_user_profile_from_json(&raw_json).expect("minimal profile parses");
    assert_eq!(profile.user.id, "987654321098765432");
    assert_eq!(profile.user.username, "minimal_user");
    assert_eq!(profile.user.global_name, None);
    assert_eq!(profile.pronouns, None);
    assert_eq!(profile.bio, None);
    assert_eq!(profile.banner, None);
    assert_eq!(profile.banner_color, None);
    assert_eq!(profile.accent_color, None);
    assert!(profile.badges.is_empty());
    assert!(profile.connected_accounts.is_empty());
    assert!(profile.mutual_guilds.is_empty());
    assert_eq!(profile.mutual_friends_count, 0);
}

#[test]
fn test_badge_icon_url_resolution() {
    let hash_badge = ProfileBadge {
        id: "nitro".to_string(),
        description: "Nitro".to_string(),
        icon: "2ba85e8026a8614b640c2837bcdfe21b".to_string(),
        link: None,
    };
    assert_eq!(
        hash_badge.icon_url(),
        "https://cdn.discordapp.com/badge-icons/2ba85e8026a8614b640c2837bcdfe21b.png"
    );

    let full_url_badge = ProfileBadge {
        id: "custom".to_string(),
        description: "Custom".to_string(),
        icon: "https://example.com/badge.png".to_string(),
        link: None,
    };
    assert_eq!(full_url_badge.icon_url(), "https://example.com/badge.png");
}

#[test]
fn test_app_state_profile_popout_workflow() {
    let mut state = AppState::new();
    assert!(state.profile_popout.is_none());

    // 1. Open profile popout (initial loading state)
    state.open_user_profile("user_100", None);
    assert!(state.profile_popout.is_some());
    let popout = state.profile_popout.as_ref().unwrap();
    assert_eq!(popout.user_id, "user_100");
    assert!(popout.is_loading);
    assert!(popout.error.is_none());
    assert_eq!(popout.active_tab, ProfileTab::MutualGuilds);

    // 2. Supply loaded profile data
    let test_profile = UserProfileData {
        user: {
            let mut u = discord_rs::domain::user::User::new("user_100", "tester");
            u.global_name = Some("Tester".to_string());
            u
        },
        bio: Some("Hello from unit test".to_string()),
        pronouns: Some("he/him".to_string()),
        banner: None,
        banner_color: Some("#232428".to_string()),
        accent_color: None,
        profile_effect_id: None,
        badges: Vec::new(),
        connected_accounts: Vec::new(),
        mutual_guilds: Vec::new(),
        mutual_friends_count: 2,
        created_at: Some(chrono::Utc::now()),
        joined_server_at: None,
    };

    state.set_profile_popout_data("user_100", test_profile);
    let popout = state.profile_popout.as_ref().unwrap();
    assert!(!popout.is_loading);
    assert!(popout.data.is_some());
    assert_eq!(
        popout.data.as_ref().unwrap().bio.as_deref(),
        Some("Hello from unit test")
    );

    // 3. Error reporting for another user ID does not overwrite
    state.set_profile_popout_error("user_999", "Network error".to_string());
    assert_eq!(state.profile_popout.as_ref().unwrap().user_id, "user_100");
    assert!(state.profile_popout.as_ref().unwrap().error.is_none());

    // Error reporting for user with existing data preserves data and ignores error
    state.set_profile_popout_error("user_100", "404 Not Found".to_string());
    let popout = state.profile_popout.as_ref().unwrap();
    assert!(!popout.is_loading);
    assert!(popout.data.is_some());
    assert!(popout.error.is_none());

    // 4. Opening a new user profile without existing data where error occurs sets error state
    state.open_user_profile("user_200", None);
    let popout = state.profile_popout.as_ref().unwrap();
    assert_eq!(popout.user_id, "user_200");
    assert!(popout.is_loading);

    state.set_profile_popout_error("user_200", "404 Not Found".to_string());
    let popout = state.profile_popout.as_ref().unwrap();
    assert!(!popout.is_loading);
    assert_eq!(popout.error.as_deref(), Some("404 Not Found"));
    assert!(popout.data.is_none());

    // 5. Close profile popout
    state.close_profile_popout();
    assert!(state.profile_popout.is_none());
}

#[test]
fn test_single_fetch_and_concurrent_deduplication() {
    let mut state = AppState::new();

    // First open triggers loading
    state.open_user_profile("user_duplicate_check", None);
    assert!(state.profile_popout.is_some());
    assert!(state.profile_popout.as_ref().unwrap().is_loading);

    // Set quick message on active popout
    state.profile_popout.as_mut().unwrap().quick_message = "drafting...".to_string();

    // Second open for SAME user while is_loading is true:
    // MUST NOT reset popout state or spawn duplicate fetch
    state.open_user_profile("user_duplicate_check", None);
    let popout = state.profile_popout.as_ref().unwrap();
    assert_eq!(popout.user_id, "user_duplicate_check");
    assert_eq!(popout.quick_message, "drafting...");
    assert!(popout.is_loading);
}

#[test]
fn test_out_of_order_stale_response_discarded() {
    let mut state = AppState::new();

    // 1. User clicks User A
    state.open_user_profile("user_A", None);
    assert_eq!(state.profile_popout.as_ref().unwrap().user_id, "user_A");

    // 2. Before User A responds, User clicks User B
    state.open_user_profile("user_B", None);
    assert_eq!(state.profile_popout.as_ref().unwrap().user_id, "user_B");

    // 3. Stale response for User A arrives via TransportEvent
    let profile_a = UserProfileData {
        user: discord_rs::domain::user::User::new("user_A", "user_a"),
        bio: Some("User A bio".into()),
        pronouns: None,
        banner: None,
        banner_color: None,
        accent_color: None,
        profile_effect_id: None,
        badges: Vec::new(),
        connected_accounts: Vec::new(),
        mutual_guilds: Vec::new(),
        mutual_friends_count: 0,
        created_at: None,
        joined_server_at: None,
    };

    state.handle_transport_event(TransportEvent::UserProfileLoaded {
        user_id: "user_A".to_string(),
        profile: Box::new(profile_a),
    });

    // Current popout must STILL be User B, and data must NOT be User A's data
    let popout = state.profile_popout.as_ref().unwrap();
    assert_eq!(popout.user_id, "user_B");
    assert!(popout.data.is_none());
    assert!(popout.is_loading);

    // 4. Response for User B arrives
    let profile_b = UserProfileData {
        user: discord_rs::domain::user::User::new("user_B", "user_b"),
        bio: Some("User B bio".into()),
        pronouns: None,
        banner: None,
        banner_color: None,
        accent_color: None,
        profile_effect_id: None,
        badges: Vec::new(),
        connected_accounts: Vec::new(),
        mutual_guilds: Vec::new(),
        mutual_friends_count: 0,
        created_at: None,
        joined_server_at: None,
    };

    state.handle_transport_event(TransportEvent::UserProfileLoaded {
        user_id: "user_B".to_string(),
        profile: Box::new(profile_b),
    });

    let popout = state.profile_popout.as_ref().unwrap();
    assert_eq!(popout.user_id, "user_B");
    assert_eq!(popout.data.as_ref().unwrap().bio.as_deref(), Some("User B bio"));
    assert!(!popout.is_loading);
}

#[test]
fn test_zero_fake_date_non_snowflake() {
    // When user ID is not snowflake (e.g. non-numeric string), snowflake_to_datetime is None
    assert_eq!(snowflake_to_datetime("non-snowflake-id"), None);

    let raw_json = serde_json::json!({
        "user": {
            "id": "non-snowflake-id",
            "username": "custom_id_user"
        }
    });

    let profile = parse_user_profile_from_json(&raw_json).expect("parses");
    assert_eq!(profile.created_at, None);
}

#[tokio::test]
async fn test_profile_popout_quick_direct_message_send() {
    let mut state = AppState::new();
    let (cm, _rx) = discord_rs::protocol::ClientManager::new_mock();
    state.client_manager = Some(cm);

    // Open profile popout
    state.open_user_profile("recipient_99", None);

    // Populate profile data with target username
    let test_profile = UserProfileData {
        user: discord_rs::domain::user::User::new("recipient_99", "best_friend"),
        bio: None,
        pronouns: None,
        banner: None,
        banner_color: None,
        accent_color: None,
        profile_effect_id: None,
        badges: Vec::new(),
        connected_accounts: Vec::new(),
        mutual_guilds: Vec::new(),
        mutual_friends_count: 0,
        created_at: None,
        joined_server_at: None,
    };
    state.set_profile_popout_data("recipient_99", test_profile);

    // User types in quick direct message field
    state.profile_popout.as_mut().unwrap().quick_message = "Olá, vamos programar em Rust?".to_string();

    // Two-phase quick message send (begin + execute + finish)
    let req = state.begin_profile_popout_quick_message().expect("request bundle");
    let res = req.execute().await;
    state.finish_profile_popout_quick_message(&req, res);

    // Popout must be closed
    assert!(state.profile_popout.is_none());

    // DM channel must be selected
    let selected_cid = state.selected_channel_id.as_ref().expect("channel selected");
    assert!(selected_cid.contains("recipient_99") || selected_cid.starts_with("dm-") || selected_cid.starts_with("chan-dm-"));

    // Message must be dispatched into the channel
    let all_msgs = state.messages_by_channel.values().flatten().collect::<Vec<_>>();
    assert!(all_msgs.iter().any(|m| m.content == "Olá, vamos programar em Rust?"));
}

#[test]
fn test_profile_popout_quick_direct_message_send_disconnected() {
    let mut state = AppState::new();
    // client_manager is None -> sem conexão

    state.open_user_profile("recipient_disconnected", None);
    state.profile_popout.as_mut().unwrap().quick_message = "Tentando enviar sem rede".to_string();

    let req = state.begin_profile_popout_quick_message();
    assert!(req.is_none(), "Sem client_manager não pode gerar requisição de envio");

    let popout = state.profile_popout.as_ref().expect("popout continua aberto");
    assert_eq!(popout.error.as_deref(), Some("Não conectado ao Discord."));
    assert_eq!(popout.quick_message, "Tentando enviar sem rede", "Campo deve ser preservado");
    assert!(state.messages_by_channel.is_empty(), "Zero mocks: nenhuma mensagem pode ser injetada");
}

#[test]
fn test_profile_popout_live_presence_integration() {
    let mut state = AppState::new();

    let user_id = "user-presence-live-test";
    let cs = discord_rs::domain::user::CustomStatus::new(
        Some("Estudando Rust no discord-rs".to_string()),
        Some(discord_rs::domain::user::CustomStatusEmoji::unicode("🦀")),
    );
    let game = discord_rs::domain::user::Activity::playing("Zed");

    // Gateway sends live presence update
    state.handle_transport_event(TransportEvent::PresenceUpdate {
        user_id: user_id.to_string(),
        status: discord_rs::domain::user::PresenceStatus::DoNotDisturb,
        activity: Some(game.clone()),
        custom_status: Some(cs.clone()),
    });

    // Verify presence is stored in state.presences_by_user
    let live_presence = state.get_user_presence(user_id).expect("presence exists in state");
    assert_eq!(live_presence.status, discord_rs::domain::user::PresenceStatus::DoNotDisturb);
    assert_eq!(live_presence.custom_status, Some(cs));
    assert_eq!(live_presence.activity, Some(game));

    // Open profile
    state.open_user_profile(user_id, None);
    let popout = state.profile_popout.as_ref().unwrap();
    assert_eq!(popout.user_id, user_id);

    // Live presence is resolved for this user from state
    let resolved_p = state.get_user_presence(&popout.user_id);
    assert_eq!(resolved_p.unwrap().status, discord_rs::domain::user::PresenceStatus::DoNotDisturb);
}

#[test]
fn test_profile_popout_anchor_stored_and_cleared_on_close() {
    let mut state = AppState::new();
    assert!(state.profile_anchor.is_none());

    // Open popout with anchor coordinates
    let anchor_pt = point(px(1040.0), px(60.0));
    state.open_user_profile("user_anchor_test", Some(anchor_pt));

    assert!(state.profile_popout.is_some());
    assert_eq!(state.profile_anchor, Some(anchor_pt));

    // Close popout -> anchor is cleared
    state.close_profile_popout();
    assert!(state.profile_popout.is_none());
    assert!(state.profile_anchor.is_none());
}

#[test]
fn test_profile_modal_only_opens_with_popout_open() {
    let mut state = AppState::new();
    assert!(state.profile_popout.is_none());
    assert!(!state.is_profile_modal_open());

    // Attempting to open modal without popout must not open modal
    state.open_user_profile_modal();
    assert!(!state.is_profile_modal_open());

    // Open popout first
    state.open_user_profile("user_modal_test", None);
    assert!(state.profile_popout.is_some());
    assert!(!state.is_profile_modal_open());

    // Now open modal succeeds
    state.open_user_profile_modal();
    assert!(state.is_profile_modal_open());
}

#[test]
fn test_escape_closes_one_level_at_a_time() {
    let mut state = AppState::new();

    // With nothing open, Esc returns false
    assert!(!state.handle_escape());

    // Open popout and modal
    state.open_user_profile("user_esc_test", None);
    state.open_user_profile_modal();
    assert!(state.is_profile_modal_open());
    assert!(state.profile_popout.is_some());

    // First Esc: closes modal only, leaves popout open
    let handled_first = state.handle_escape();
    assert!(handled_first);
    assert!(!state.is_profile_modal_open());
    assert!(state.profile_popout.is_some());

    // Second Esc: closes popout
    let handled_second = state.handle_escape();
    assert!(handled_second);
    assert!(!state.is_profile_modal_open());
    assert!(state.profile_popout.is_none());

    // Third Esc: nothing to close, returns false
    let handled_third = state.handle_escape();
    assert!(!handled_third);
}

#[test]
fn test_close_profile_modal_leaves_popout_open() {
    let mut state = AppState::new();

    state.open_user_profile("user_modal_close_test", None);
    state.open_user_profile_modal();
    assert!(state.is_profile_modal_open());
    assert!(state.profile_popout.is_some());

    // Explicitly closing the modal dialog leaves the popout visible
    state.close_user_profile_modal();
    assert!(!state.is_profile_modal_open());
    assert!(state.profile_popout.is_some());
    assert_eq!(state.profile_popout.as_ref().unwrap().user_id, "user_modal_close_test");
}
