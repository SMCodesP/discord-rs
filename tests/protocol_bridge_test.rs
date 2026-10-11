use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::{AuthToken, ProtocolBridge, TokenStorage, TokenType, TransportEvent};
use std::fs;
use std::time::Duration;

fn make_temp_auth_storage(test_name: &str) -> TokenStorage {
    let dir = std::env::temp_dir().join(format!("discord_rs_test_{}_{}", test_name, std::process::id()));
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

    // Small delay to ensure forwarder task is active
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Emit Connected event
    bridge.emit_event(TransportEvent::Connected);

    // Verify broadcast received
    let event1 = broadcast_rx.recv().await?;
    matches!(event1, TransportEvent::Connected);

    // Verify primary unbounded received
    let event2 = primary_unbounded_rx.recv().await.expect("Unbounded event expected");
    matches!(event2, TransportEvent::Connected);

    // Verify secondary unbounded received
    let event3 = secondary_unbounded_rx.recv().await.expect("Secondary unbounded event expected");
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

    let event_msg = broadcast_rx.recv().await?;
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

    let event_typing = broadcast_rx.recv().await?;
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
    let bridge = ProtocolBridge::with_storage(storage);

    // Short invalid token
    let res = bridge.connect_raw("short", TokenType::Bot, false).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_connect_with_unauthorized_token_emits_disconnected() {
    let (cm, mut rx) = discord_rs::protocol::ClientManager::new_mock();
    let token = AuthToken::user("BQBIQvAmXjxFaDK1WqqMbC-DNfYpe8sPr7UOpX-Dqc7BTTtjrufGtOSLbCWOxfp4x2GPYNBpoNn0UMZErC3laefVsUnk15rirXcJ0c6a2TuCnIMDco_CdRh52B40t-6bzc6KXqanRq8G83xCLw2XUQJRwTLdyZWcTYfRAkQvi46sxU5iosw7xNA0sqSzCx5soDp2VECkhiauMBFQ-Us_-NjlvNIq75iMUsXsOU8ySVsTVXbheKOifymc2WGkHKOQaq-w");

    let _ = rustls::crypto::ring::default_provider().install_default();

    let _ = cm.connect_with_token(token).await;

    let event = tokio::time::timeout(Duration::from_secs(5), async {
        while let Ok(evt) = rx.recv().await {
            if let TransportEvent::Disconnected(reason) = evt {
                return Some(reason);
            }
        }
        None
    }).await;

    assert!(event.is_ok(), "Expected Disconnected event before timeout");
    let reason = event.unwrap().expect("Should have received Disconnected reason");
    assert!(reason.contains("Falha na autenticação") || reason.contains("401") || reason.contains("Gateway"));
}
