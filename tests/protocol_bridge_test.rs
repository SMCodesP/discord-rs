use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::{AuthToken, ProtocolBridge, TokenStorage, TokenType, TransportEvent};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_AUTH_DIR: AtomicU64 = AtomicU64::new(0);

fn make_temp_auth_storage(test_name: &str) -> TokenStorage {
    let unique = NEXT_AUTH_DIR.fetch_add(1, Ordering::Relaxed);
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let dir = std::env::temp_dir().join(format!(
        "discord_rs_test_{}_{}_{}_{}",
        test_name,
        std::process::id(),
        timestamp,
        unique
    ));
    let _ = fs::create_dir_all(&dir);
    TokenStorage::with_path(dir.join("auth.json"))
}

#[tokio::test]
async fn test_protocol_bridge_token_storage_lifecycle() -> anyhow::Result<()> {
    let storage = make_temp_auth_storage("token_storage");
    let auth_path = storage.path().to_path_buf();

    let bridge = ProtocolBridge::with_storage(storage);

    // Initial state: no saved token
    assert_eq!(bridge.load_saved_token()?, None);
    assert!(!bridge.is_connected());

    // Save a token
    let sample_token = AuthToken::bot("MTAwMDAwMDAwMDAwMDAwMDAw.GxYxYz.abcdefghijklmnopqrstuvwxyz123456");
    bridge.save_token(&sample_token)?;
    assert!(auth_path.exists());

    // Load saved token
    let loaded = bridge.load_saved_token()?.expect("Expected token to be loaded");
    assert_eq!(loaded.secret(), sample_token.secret());
    assert_eq!(loaded.token_type, TokenType::Bot);

    // Clear token
    bridge.clear_token()?;
    assert!(!auth_path.exists());
    assert_eq!(bridge.load_saved_token()?, None);

    Ok(())
}

#[tokio::test]
async fn test_protocol_bridge_event_channels() -> anyhow::Result<()> {
    let storage = make_temp_auth_storage("event_channels");
    let bridge = ProtocolBridge::with_storage(storage);

    // 1. Subscribe to broadcast channel
    let mut broadcast_rx = bridge.subscribe();

    // 2. Claim initial unbounded receiver
    let mut primary_unbounded_rx = bridge
        .take_unbounded_receiver()
        .await
        .expect("Primary unbounded receiver should be present");

    // 3. Subscribe secondary unbounded receiver
    let mut secondary_unbounded_rx = bridge.subscribe_unbounded();

    // Sem sleep fixo: cada receptor é verificado com timeout de eventos.
    // Emit Connected event
    bridge.emit_event(TransportEvent::Connected);

    // Verify broadcast received
    let event1 = tokio::time::timeout(Duration::from_secs(1), broadcast_rx.recv()).await??;
    matches!(event1, TransportEvent::Connected);

    // Verify primary unbounded received
    let event2 = tokio::time::timeout(Duration::from_secs(1), primary_unbounded_rx.recv())
        .await?
        .expect("Unbounded event expected");
    matches!(event2, TransportEvent::Connected);

    // Verify secondary unbounded received
    let event3 = tokio::time::timeout(Duration::from_secs(1), secondary_unbounded_rx.recv())
        .await?
        .expect("Secondary unbounded event expected");
    matches!(event3, TransportEvent::Connected);

    // Emit MessageCreate event
    let author = Author {
        id: "user-alice".into(),
        username: "alice".into(),
        global_name: Some("Alice".into()),
        avatar: None,
        bot: false,
        avatar_decoration_data: None,
    };
    let msg = Message::new("msg-101", "chan-test", author, "Hello GPUI from ProtocolBridge!");
    bridge.emit_event(TransportEvent::MessageCreate(msg.clone()));

    let event_msg = tokio::time::timeout(Duration::from_secs(1), broadcast_rx.recv()).await??;
    match event_msg {
        TransportEvent::MessageCreate(m) => {
            assert_eq!(m.id, "msg-101");
            assert_eq!(m.content, "Hello GPUI from ProtocolBridge!");
        }
        other => panic!("Expected MessageCreate, got {:?}", other),
    }

    // Emit TypingStart event
    bridge.emit_event(TransportEvent::TypingStart {
        channel_id: "chan-test".into(),
        user_id: "user-123".into(),
        timestamp: 123456789,
    });

    let event_typing = tokio::time::timeout(Duration::from_secs(1), broadcast_rx.recv()).await??;
    match event_typing {
        TransportEvent::TypingStart { channel_id, user_id, timestamp } => {
            assert_eq!(channel_id, "chan-test");
            assert_eq!(user_id, "user-123");
            assert_eq!(timestamp, 123456789);
        }
        other => panic!("Expected TypingStart, got {:?}", other),
    }

    Ok(())
}

#[tokio::test]
async fn test_protocol_bridge_disconnected_guards() {
    let storage = make_temp_auth_storage("disconnected_guards");
    let bridge = ProtocolBridge::with_storage(storage);

    assert!(!bridge.is_connected());

    // Calling send_message when disconnected should fail gracefully with context error
    let send_res = bridge.send_message("chan-1", "Test content").await;
    assert!(send_res.is_err());
    let err_msg = format!("{:#}", send_res.unwrap_err());
    assert!(err_msg.contains("not connected"));

    // Calling get_messages when disconnected should also fail gracefully
    let get_res = bridge.get_messages("chan-1", 50).await;
    assert!(get_res.is_err());

    // Calling disconnect when already disconnected should succeed cleanly
    assert!(bridge.disconnect().await.is_ok());
}

#[tokio::test]
async fn test_protocol_bridge_connect_raw_validation() {
    let storage = make_temp_auth_storage("connect_raw");
    let auth_path = storage.path().to_path_buf();
    let bridge = ProtocolBridge::with_storage(storage);

    // Token curto é rejeitado antes de persistir credenciais ou iniciar rede.
    let res = bridge.connect_raw("short", TokenType::Bot, true).await;
    assert!(res.is_err());
    assert!(!auth_path.exists(), "token inválido não deve ser gravado");
    assert!(!bridge.is_connected());
    assert!(bridge.active_token().await.is_none());
}

#[tokio::test]
async fn test_idle_transport_network_failure_emits_no_success_event() {
    let transport = Arc::new(discord_rs::protocol::IdleTransport::new());
    let (manager, mut events) = discord_rs::protocol::ClientManager::new_with_transport(transport);

    let error = manager
        .send_message("chan-offline", "conteudo sintetico")
        .await
        .expect_err("transporte desconectado não pode fabricar envio bem-sucedido");
    assert!(format!("{:#}", error).contains("Não conectado"));
    assert!(
        events.try_recv().is_err(),
        "falha de rede não deve emitir MessageCreate/Connected"
    );
}

#[test]
fn test_token_storage_rejects_corrupt_json_without_leaking_contents() -> anyhow::Result<()> {
    let storage = make_temp_auth_storage("corrupt_token");
    let secret_fragment = "SEGREDO_SINTETICO_NAO_PODE_VAZAR";
    fs::write(
        storage.path(),
        format!("{{\"token\":\"{}\"", secret_fragment),
    )?;

    let error = storage.load().expect_err("JSON corrompido deve falhar");
    let message = format!("{:#}", error);
    assert!(message.contains("Failed to parse stored authentication token JSON"));
    assert!(
        !message.contains(secret_fragment),
        "erro não pode vazar conteúdo do token"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_token_storage_uses_owner_only_permissions() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let storage = make_temp_auth_storage("secure_permissions");
    let token = AuthToken::bot("token_sintetico_1234567890");
    storage.save(&token)?;
    let mode = fs::metadata(storage.path())?.permissions().mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "auth.json deve ser legível apenas pelo dono"
    );
    Ok(())
}
