use discord_rs::app::state::AppState;
use discord_rs::protocol::{ClientManager, ConnectionMode};

#[test]
fn test_authenticate_empty_or_whitespace_token_fails() {
    let mut state = AppState::new();

    // Empty token
    state.authenticate("".to_string(), true);
    assert_eq!(
        state.auth_error,
        Some("O token não pode estar vazio.".to_string())
    );
    assert!(!state.is_connecting);
    assert!(!state.is_connected_to_gateway);

    // Whitespace token
    state.authenticate("   \t\n  ".to_string(), false);
    assert_eq!(
        state.auth_error,
        Some("O token não pode estar vazio.".to_string())
    );
    assert!(!state.is_connecting);
    assert!(!state.is_connected_to_gateway);
}

#[test]
fn test_authenticate_mock_tokens_do_not_bypass_zero_mocks() {
    let mut state = AppState::new();

    // Anteriormente, tokens começando com "mock_", "test" ou "demo" ativavam bypass imediato
    // forjando login conectado como "rustacean" e fechando o modal sem consultar API.
    for mock_token in &["mock_12345", "mock_developer_key", "test", "demo"] {
        state.authenticate(mock_token.to_string(), true);

        // Sem client_manager, deve falhar explicitamente e nunca forjar usuário "rustacean"
        assert_ne!(
            state.connection_status, "Conectado (Ambiente de Testes)",
            "Token '{}' não pode forjar 'Conectado (Ambiente de Testes)'",
            mock_token
        );
        assert_ne!(
            state.current_user.username, "rustacean",
            "Token '{}' não pode forjar usuário 'rustacean'",
            mock_token
        );
        assert!(!state.is_connected_to_gateway);
        assert!(!state.is_connecting);
        assert_eq!(
            state.auth_error,
            Some("Gerenciador de conexão não inicializado.".to_string())
        );
    }
}

#[tokio::test]
async fn test_authenticate_with_client_manager_sets_connecting_state() {
    let mut state = AppState::new();
    let (cm, _rx) = ClientManager::new();
    state.client_manager = Some(cm);

    state.authenticate("valid_format_test_token_12345".to_string(), true);

    // Inicia processo real de conexão
    assert!(state.is_connecting);
    assert!(!state.is_connected_to_gateway);
    assert_eq!(state.connection_status, "Conectando ao Gateway v10...");
    assert_eq!(state.auth_error, None);
    assert_eq!(
        state.auth_token,
        Some("valid_format_test_token_12345".to_string())
    );
    assert_eq!(state.token_input, "valid_format_test_token_12345");
}

#[tokio::test]
async fn test_disconnect_gateway_resets_connection_state() {
    let mut state = AppState::new();
    let (cm, _rx) = ClientManager::new();
    state.client_manager = Some(cm);

    state.is_connected_to_gateway = true;
    state.auth_token = Some("some_token".to_string());
    state.connection_mode = ConnectionMode::LiveDiscord("user1".to_string());

    state.disconnect_gateway();

    assert!(!state.is_connected_to_gateway);
    assert_eq!(state.auth_token, None);
    assert_eq!(
        state.connection_mode,
        ConnectionMode::Disconnected("Desconectado".to_string())
    );
    assert_eq!(state.connection_status, "Desconectado");
    assert!(state.messages_by_channel.is_empty());
    assert!(state.dm_channels.is_empty());
}
