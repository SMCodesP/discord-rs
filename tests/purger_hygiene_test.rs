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
