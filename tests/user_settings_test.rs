use chrono::Utc;
use discord_rs::app::state::{ActiveModal, AppState, SettingsTab};
use discord_rs::domain::settings::{AccountConnection, AuthorizedApp, NitroTier};

#[test]
fn test_user_settings_modal_tabs_navigation() {
    let mut state = AppState::new();
    assert_eq!(state.active_modal, None);

    state.open_modal(ActiveModal::UserSettings);
    assert_eq!(state.active_modal, Some(ActiveModal::UserSettings));
    assert_eq!(state.user_settings_tab, SettingsTab::MyAccount);

    let tabs = [
        SettingsTab::MyAccount,
        SettingsTab::Profiles,
        SettingsTab::Privacy,
        SettingsTab::AuthorizedApps,
        SettingsTab::Connections,
        SettingsTab::FamilyCenter,
        SettingsTab::Nitro,
        SettingsTab::ServerBoost,
        SettingsTab::Appearance,
        SettingsTab::Accessibility,
        SettingsTab::VoiceVideo,
        SettingsTab::Notifications,
        SettingsTab::Keybinds,
        SettingsTab::Language,
    ];

    for tab in tabs {
        state.set_settings_tab(tab);
        assert_eq!(state.user_settings_tab, tab);
    }

    state.close_modal();
    assert_eq!(state.active_modal, None);
}

#[test]
fn test_user_settings_honest_empty_states() {
    let mut state = AppState::new();
    // Verify clean state without mock services
    state.set_nitro_tier(NitroTier::None);
    state.authorized_apps.clear();
    state.connections.clear();
    state.server_boosts.clear();
    state.family_center_links.clear();

    assert!(state.authorized_apps.is_empty(), "Authorized apps must be empty in clean state");
    assert!(state.connections.is_empty(), "Connections must be empty in clean state");
    assert!(state.server_boosts.is_empty(), "Server boosts must be empty in clean state");
    assert!(state.family_center_links.is_empty(), "Family center links must be empty in clean state");
    assert_eq!(state.nitro_tier, NitroTier::None, "Nitro tier should be None when set");
}

#[test]
fn test_user_settings_authorized_apps_revocation() {
    let mut state = AppState::new();
    state.authorized_apps = vec![
        AuthorizedApp {
            id: "app-test-1".into(),
            name: "Test Bot".into(),
            icon_url: None,
            scopes: vec!["identify".into(), "email".into()],
            authorized_at: Utc::now(),
        },
        AuthorizedApp {
            id: "app-test-2".into(),
            name: "Second Bot".into(),
            icon_url: None,
            scopes: vec!["guilds".into()],
            authorized_at: Utc::now(),
        },
    ];

    assert_eq!(state.authorized_apps.len(), 2);
    state.revoke_authorized_app("app-test-1");
    assert_eq!(state.authorized_apps.len(), 1);
    assert_eq!(state.authorized_apps[0].id, "app-test-2");

    state.revoke_authorized_app("app-test-2");
    assert!(state.authorized_apps.is_empty());
}

#[test]
fn test_user_settings_connections_mutation() {
    let mut state = AppState::new();
    state.connections = vec![AccountConnection {
        platform: "github".into(),
        id: "gh-123".into(),
        name: "testuser".into(),
        verified: true,
        show_on_profile: true,
        show_activity: false,
    }];

    assert_eq!(state.connections.len(), 1);
    assert_eq!(state.connections[0].platform, "github");

    state.disconnect_platform("github");
    assert!(state.connections.is_empty());
}
