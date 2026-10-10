use chrono::{Datelike, Timelike};
use discord_rs::app::state::AppState;
use discord_rs::app::user_profile_popout::{
    snowflake_to_datetime, ProfileBadge, ProfileTab, UserProfileData,
};
use discord_rs::protocol::discord_api::parse_user_profile_from_json;

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

    // Known Discord Snowflake: 218348062828003328
    // (218348062828003328 >> 22) = 52058145749 ms since Discord Epoch
    // 1420070400000 + 52058145749 = 1472128545749 ms = 2016-08-25 15:15:45 UTC
    let dt_known = snowflake_to_datetime("218348062828003328").expect("valid snowflake");
    assert_eq!(dt_known.year(), 2016);
    assert_eq!(dt_known.month(), 8);
    assert_eq!(dt_known.day(), 25);

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
    state.open_user_profile("user_100");
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
        badges: Vec::new(),
        connected_accounts: Vec::new(),
        mutual_guilds: Vec::new(),
        mutual_friends_count: 2,
        created_at: chrono::Utc::now(),
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
    state.open_user_profile("user_200");
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
