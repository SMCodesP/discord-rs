use discord_rs::app::state::AppState;
use discord_rs::protocol::transport::TransportEvent;

#[test]
fn test_voice_call_starts_with_only_current_user_no_ferris() {
    let mut state = AppState::new();
    state.current_user.id = "user-me-123".into();
    state.current_user.display_name = "MyUser".into();

    state.connect_voice("chan-voice-1", "Voz Geral", "Guild Test");

    let call = state.active_voice_call.as_ref().expect("Voice call should be active");
    assert_eq!(call.participants.len(), 1, "Call must start with only the current user");
    assert_eq!(call.participants[0].id, "user-me-123");
    assert_eq!(call.participants[0].name, "MyUser");
    assert!(
        !call.participants.iter().any(|p| p.id == "user-ferris" || p.name.to_lowercase().contains("ferris")),
        "Mock participant Ferris must not be present in voice call"
    );
}

#[test]
fn test_active_now_starts_empty_zero_mocks() {
    let state = AppState::new();
    assert!(
        state.active_now_items.is_empty(),
        "active_now_items must start empty with no hardcoded mocks (VS Code, Roblox, Spotify)"
    );
}

#[test]
fn test_connection_status_purged_of_gateway_conectado() {
    let mut state = AppState::new();
    state.handle_transport_event(TransportEvent::Connected);

    assert!(state.is_connected_to_gateway);
    assert_ne!(
        state.connection_status, "Gateway Conectado",
        "connection_status must not be 'Gateway Conectado'"
    );
    assert_eq!(state.connection_status, "Conectado");
}

#[test]
fn test_app_state_new_empty_for_production() {
    let state = AppState::new();
    assert!(state.guilds.is_empty(), "guilds must be empty in AppState::new()");
    assert!(state.friends.is_empty(), "friends must be empty in AppState::new()");
    assert!(state.dm_channels.is_empty(), "dm_channels must be empty in AppState::new()");
    assert!(state.messages_by_channel.is_empty(), "messages_by_channel must be empty in AppState::new()");
    assert!(state.connections.is_empty(), "connections must be empty in AppState::new()");
    assert!(state.active_subscription.is_none(), "active_subscription must be None in AppState::new()");
    assert!(state.server_boosts.is_empty(), "server_boosts must be empty in AppState::new()");
    assert!(state.authorized_apps.is_empty(), "authorized_apps must be empty in AppState::new()");
    assert!(state.family_center_links.is_empty(), "family_center_links must be empty in AppState::new()");
    assert_eq!(
        state.nitro_tier,
        discord_rs::domain::settings::NitroTier::None,
        "nitro_tier must be None in AppState::new()"
    );
    assert!(state.current_user.id.is_empty(), "current_user.id must be empty in AppState::new()");
    assert!(state.current_user.username.is_empty(), "current_user.username must be empty in AppState::new()");
    assert!(state.current_user.display_name.is_empty(), "current_user.display_name must be empty in AppState::new()");
}

#[test]
fn test_hygiene_no_invented_mocks_outside_allowed() {
    use std::fs;
    use std::path::Path;

    // Verify seeded_for_tests still preserves the test seed
    let seed = AppState::seeded_for_tests();
    assert_eq!(seed.current_user.display_name, "Vanguard");
    assert!(seed.guilds.iter().any(|g| g.name == "Rust Brasil"));
    assert_eq!(
        seed.active_subscription.as_ref().map(|s| s.payment_gateway.as_str()),
        Some("Stripe")
    );

    let forbidden = [
        "Vanguard",
        "Rust Brasil",
        "Stripe",
        "R$ ",
        "Nexus",
        "Ferran",
        "GPUI Hub",
    ];
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src_dir = Path::new(manifest_dir).join("src");

    fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&Path)) {
        if dir.is_dir() {
            for entry in fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, cb);
                } else if path.extension().map_or(false, |ext| ext == "rs") {
                    cb(&path);
                }
            }
        }
    }

    let mut violations = Vec::new();

    visit_dirs(&src_dir, &mut |path| {
        let relative = path.strip_prefix(manifest_dir).unwrap().to_str().unwrap();
        // Allowed: mock_provider.rs (mock transport for tests)
        if relative == "src/protocol/mock_provider.rs" {
            return;
        }
        // Allowed: seed.rs (holds seeded_for_tests)
        if relative == "src/app/state/seed.rs" {
            return;
        }

        let content = fs::read_to_string(path).unwrap();
        let mut in_test_mod = false;
        let mut brace_depth: i32 = 0;
        let mut target_depth: i32 = 0;

        for (line_no, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.contains("#[cfg(test)]") {
                in_test_mod = true;
                target_depth = brace_depth;
            }

            let open_braces = line.matches('{').count() as i32;
            let close_braces = line.matches('}').count() as i32;
            brace_depth += open_braces - close_braces;

            if in_test_mod && brace_depth <= target_depth && close_braces > 0 {
                in_test_mod = false;
            }

            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
                continue;
            }

            // Strip trailing comments (e.g. `pub payment_gateway: String, // "Stripe"...`)
            let code_part = if let Some((code, _)) = line.split_once("//") {
                code.trim()
            } else {
                trimmed
            };

            if !in_test_mod {
                for &word in &forbidden {
                    if code_part.contains(word) {
                        violations.push(format!(
                            "{}:{}: contains forbidden word '{}': {}",
                            relative,
                            line_no + 1,
                            word,
                            trimmed
                        ));
                    }
                }

                // new_mock( is only allowed in client_manager.rs, mock_provider.rs, or #[cfg(test)]
                if relative != "src/protocol/client_manager.rs" && code_part.contains("new_mock(") {
                    violations.push(format!(
                        "{}:{}: contains forbidden 'new_mock(' in production code: {}",
                        relative,
                        line_no + 1,
                        trimmed
                    ));
                }
            }
        }
    });

    assert!(
        violations.is_empty(),
        "Found forbidden strings outside seeded_for_tests/mock_provider/testes:\n{}",
        violations.join("\n")
    );
}

#[tokio::test]
async fn test_client_manager_new_starts_idle_disconnected() {
    let (cm, _rx) = discord_rs::protocol::ClientManager::new();
    assert_eq!(
        cm.current_mode().await,
        discord_rs::protocol::ConnectionMode::Disconnected("Não conectado".to_string())
    );
    assert!(cm.list_guilds().await.unwrap().is_empty());
    assert!(cm.list_dm_channels().await.unwrap().is_empty());
    assert!(cm.list_relationships().await.unwrap().is_empty());
    assert!(cm.get_messages("chan-1", 10).await.unwrap().is_empty());
    assert!(cm.send_message("chan-1", "hello").await.is_err());
}
