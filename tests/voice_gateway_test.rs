//! Voice Gateway v8 Integration Tests
//!
//! Validates:
//! - Serialization/deserialization of all Voice Gateway Opcodes (0, 1, 2, 3, 4, 5, 6, 7, 8, 9).
//! - VoiceUDP contract: VoiceReady, SessionDescription, preferred and fallback encryption modes.
//! - Pure state machine transitions without network.
//! - Buffered resume logic and heartbeat `seq_ack` tracking.
//! - Main Gateway Voice State Update Opcode 4 and VOICE_SERVER_UPDATE handling.

use discord_rs::protocol::voice_gateway::{
    describe_voice_close_code, format_voice_gateway_url, HelloPayload, ReadyPayload,
    SessionDescription, SessionDescriptionPayload, SpeakingFlags, SpeakingPayload,
    VoiceCloseDecision, VoiceConnectionState, VoiceGatewayAction, VoiceGatewayConfig,
    VoiceGatewayEvent, VoiceGatewayPayload, VoiceGatewayStateMachine, VoiceReady,
    VoiceServerData, FALLBACK_ENCRYPTION_MODE, PREFERRED_ENCRYPTION_MODE,
};

fn mock_voice_config() -> VoiceGatewayConfig {
    VoiceGatewayConfig {
        endpoint: "us-east-1.discord.media:443".to_string(),
        server_id: "guild_888999".to_string(),
        user_id: "user_111222".to_string(),
        session_id: "gateway_session_999".to_string(),
        token: "voice_secret_token_123".to_string(),
        max_dave_protocol_version: Some(0),
    }
}

#[test]
fn test_voice_gateway_url_formatting() {
    assert_eq!(
        format_voice_gateway_url("us-east1.discord.media"),
        "wss://us-east1.discord.media/?v=8"
    );
    assert_eq!(
        format_voice_gateway_url("us-east1.discord.media:443"),
        "wss://us-east1.discord.media/?v=8"
    );
    assert_eq!(
        format_voice_gateway_url("wss://us-east1.discord.media:443"),
        "wss://us-east1.discord.media/?v=8"
    );
    assert_eq!(
        format_voice_gateway_url("ws://us-east1.discord.media:80"),
        "wss://us-east1.discord.media/?v=8"
    );
}

#[test]
fn test_voice_udp_contract_types() {
    // 1. VoiceReady contract check
    let ready = VoiceReady {
        ssrc: 1337,
        ip: "192.0.2.1".to_string(),
        port: 50000,
        modes: vec![
            "aead_aes256_gcm_rtpsize".to_string(),
            "aead_xchacha20_poly1305_rtpsize".to_string(),
        ],
    };
    assert_eq!(ready.ssrc, 1337);
    assert_eq!(ready.ip, "192.0.2.1");
    assert_eq!(ready.port, 50000);
    assert_eq!(ready.select_best_mode(), PREFERRED_ENCRYPTION_MODE);

    // 2. Fallback mode check
    let ready_fallback = VoiceReady {
        ssrc: 2000,
        ip: "192.0.2.2".to_string(),
        port: 50001,
        modes: vec!["aead_xchacha20_poly1305_rtpsize".to_string()],
    };
    assert_eq!(ready_fallback.select_best_mode(), FALLBACK_ENCRYPTION_MODE);

    // 3. SessionDescription contract check
    let secret = vec![42u8; 32];
    let session_desc = SessionDescription {
        mode: PREFERRED_ENCRYPTION_MODE.to_string(),
        secret_key: secret.clone(),
        dave_protocol_version: None,
    };
    assert_eq!(session_desc.mode, "aead_aes256_gcm_rtpsize");
    assert_eq!(session_desc.secret_key.len(), 32);
    assert_eq!(session_desc.secret_key, secret);
}

#[test]
fn test_opcode_0_identify_payload_format() {
    let config = mock_voice_config();
    let sm = VoiceGatewayStateMachine::new(config);
    let identify = sm.generate_identify();

    let value = serde_json::to_value(&identify).expect("Identify serialization failed");
    assert_eq!(value["op"], 0);
    assert_eq!(value["d"]["server_id"], "guild_888999");
    assert_eq!(value["d"]["user_id"], "user_111222");
    assert_eq!(value["d"]["session_id"], "gateway_session_999");
    assert_eq!(value["d"]["token"], "voice_secret_token_123");
    // Verify DAVE protocol signaling
    assert_eq!(value["d"]["max_dave_protocol_version"], 0);
}

#[test]
fn test_opcode_1_select_protocol_payload_format() {
    let config = mock_voice_config();
    let sm = VoiceGatewayStateMachine::new(config);
    let select = sm.generate_select_protocol("203.0.113.10", 41234, PREFERRED_ENCRYPTION_MODE);

    let value = serde_json::to_value(&select).expect("Select Protocol serialization failed");
    assert_eq!(value["op"], 1);
    assert_eq!(value["d"]["protocol"], "udp");
    assert_eq!(value["d"]["data"]["address"], "203.0.113.10");
    assert_eq!(value["d"]["data"]["port"], 41234);
    assert_eq!(value["d"]["data"]["mode"], "aead_aes256_gcm_rtpsize");
}

#[test]
fn test_opcode_2_ready_payload_format() {
    let raw = r#"{
        "op": 2,
        "d": {
            "ssrc": 4242,
            "ip": "203.0.113.5",
            "port": 51234,
            "modes": [
                "aead_aes256_gcm_rtpsize",
                "aead_xchacha20_poly1305_rtpsize"
            ],
            "experiments": [],
            "streams": []
        }
    }"#;

    let payload: VoiceGatewayPayload<ReadyPayload> = serde_json::from_str(raw).unwrap();
    assert_eq!(payload.op, 2);
    let ready = payload.d.unwrap();
    assert_eq!(ready.ssrc, 4242);
    assert_eq!(ready.ip, "203.0.113.5");
    assert_eq!(ready.port, 51234);
    assert_eq!(ready.modes.len(), 2);
}

#[test]
fn test_opcode_3_heartbeat_payload_format_v8() {
    let config = mock_voice_config();
    let mut sm = VoiceGatewayStateMachine::new(config);
    sm.set_last_seq(77);

    let hb = sm.generate_heartbeat(Some(1700000000000));
    let val = serde_json::to_value(&hb).unwrap();
    assert_eq!(val["op"], 3);
    assert_eq!(val["d"]["seq_ack"], 77);
    assert_eq!(val["d"]["t"], 1700000000000u64);
}

#[test]
fn test_opcode_4_session_description_payload_format() {
    let key: Vec<u8> = (0..32).collect();
    let key_json = serde_json::to_string(&key).unwrap();
    let raw = format!(
        r#"{{
            "op": 4,
            "d": {{
                "mode": "aead_aes256_gcm_rtpsize",
                "secret_key": {},
                "dave_protocol_version": 0,
                "media_session_id": "media-sess-1"
            }}
        }}"#,
        key_json
    );

    let payload: VoiceGatewayPayload<SessionDescriptionPayload> = serde_json::from_str(&raw).unwrap();
    assert_eq!(payload.op, 4);
    let data = payload.d.unwrap();
    assert_eq!(data.mode, "aead_aes256_gcm_rtpsize");
    assert_eq!(data.secret_key.len(), 32);
    assert_eq!(data.secret_key[10], 10);
    assert_eq!(data.dave_protocol_version, Some(0));
}

#[test]
fn test_opcode_5_speaking_flags_and_payload() {
    assert_eq!(SpeakingFlags::MICROPHONE, 1);
    assert_eq!(SpeakingFlags::SOUNDSHARE, 2);
    assert_eq!(SpeakingFlags::PRIORITY, 4);

    let config = mock_voice_config();
    let sm = VoiceGatewayStateMachine::new(config);
    let speaking = sm.generate_speaking(true, 5, 1234);
    let val = serde_json::to_value(&speaking).unwrap();
    assert_eq!(val["op"], 5);
    assert_eq!(val["d"]["speaking"], 1);
    assert_eq!(val["d"]["delay"], 5);
    assert_eq!(val["d"]["ssrc"], 1234);

    // Incoming speaking from server
    let raw = r#"{"op": 5, "d": {"speaking": 1, "delay": 0, "ssrc": 1234, "user_id": "user_other"}}"#;
    let payload: VoiceGatewayPayload<SpeakingPayload> = serde_json::from_str(raw).unwrap();
    let sp = payload.d.unwrap();
    assert_eq!(sp.user_id.as_deref(), Some("user_other"));
    assert_eq!(sp.speaking, 1);
}

#[test]
fn test_opcode_6_heartbeat_ack_payload() {
    let raw = r#"{"op": 6, "d": 1700000000000}"#;
    let payload: VoiceGatewayPayload<u64> = serde_json::from_str(raw).unwrap();
    assert_eq!(payload.op, 6);
    assert_eq!(payload.d, Some(1700000000000));
}

#[test]
fn test_opcode_7_resume_and_opcode_9_resumed() {
    let config = mock_voice_config();
    let mut sm = VoiceGatewayStateMachine::new(config);
    sm.set_last_seq(99);

    let resume = sm.generate_resume();
    let val = serde_json::to_value(&resume).unwrap();
    assert_eq!(val["op"], 7);
    assert_eq!(val["d"]["server_id"], "guild_888999");
    assert_eq!(val["d"]["session_id"], "gateway_session_999");
    assert_eq!(val["d"]["token"], "voice_secret_token_123");
    assert_eq!(val["d"]["seq_ack"], 99);

    // Opcode 9 Resumed
    let raw_resumed = r#"{"op": 9, "d": null}"#;
    let payload_resumed: VoiceGatewayPayload<serde_json::Value> = serde_json::from_str(raw_resumed).unwrap();
    assert_eq!(payload_resumed.op, 9);
}

#[test]
fn test_opcode_8_hello_payload() {
    let raw = r#"{"op": 8, "d": {"v": 8, "heartbeat_interval": 41250.0}}"#;
    let payload: VoiceGatewayPayload<HelloPayload> = serde_json::from_str(raw).unwrap();
    assert_eq!(payload.op, 8);
    let hello = payload.d.unwrap();
    assert_eq!(hello.v, Some(8));
    assert_eq!(hello.heartbeat_interval, 41250.0);
}

#[test]
fn test_state_machine_full_offline_handshake() {
    let mut sm = VoiceGatewayStateMachine::new(mock_voice_config());
    assert_eq!(*sm.current_state(), VoiceConnectionState::Disconnected);

    // 1. Client begins connection
    sm.start().unwrap();
    assert_eq!(*sm.current_state(), VoiceConnectionState::WaitingForHello);

    // 2. Gateway delivers Op 8 Hello
    let hello_raw = r#"{"op": 8, "d": {"v": 8, "heartbeat_interval": 25000.0}}"#;
    let actions = sm.handle_incoming_text(hello_raw).unwrap();
    assert_eq!(*sm.current_state(), VoiceConnectionState::Identifying);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], VoiceGatewayAction::StartHeartbeat { interval_ms: 25000 });

    // Verify Op 0 Identify payload was emitted
    if let VoiceGatewayAction::SendMessage(identify_str) = &actions[1] {
        let val: serde_json::Value = serde_json::from_str(identify_str).unwrap();
        assert_eq!(val["op"], 0);
        assert_eq!(val["d"]["max_dave_protocol_version"], 0);
    } else {
        panic!("Expected SendMessage(Identify)");
    }

    // 3. Gateway delivers Op 2 Ready with sequence number 10
    let ready_raw = r#"{
        "op": 2,
        "seq": 10,
        "d": {
            "ssrc": 9999,
            "ip": "198.51.100.10",
            "port": 50000,
            "modes": ["aead_aes256_gcm_rtpsize", "aead_xchacha20_poly1305_rtpsize"]
        }
    }"#;
    let actions = sm.handle_incoming_text(ready_raw).unwrap();
    assert_eq!(sm.last_seq(), 10);
    let expected_ready = VoiceReady {
        ssrc: 9999,
        ip: "198.51.100.10".to_string(),
        port: 50000,
        modes: vec![
            "aead_aes256_gcm_rtpsize".to_string(),
            "aead_xchacha20_poly1305_rtpsize".to_string(),
        ],
    };
    assert_eq!(*sm.current_state(), VoiceConnectionState::Ready(expected_ready.clone()));
    assert_eq!(actions, vec![VoiceGatewayAction::ReadyReceived(expected_ready)]);

    // 4. IP discovery completed; client selects protocol (Op 1)
    let action = sm
        .select_protocol("203.0.113.88", 55555, PREFERRED_ENCRYPTION_MODE)
        .unwrap();
    assert_eq!(*sm.current_state(), VoiceConnectionState::SelectingProtocol);
    if let VoiceGatewayAction::SendMessage(sel_str) = action {
        let val: serde_json::Value = serde_json::from_str(&sel_str).unwrap();
        assert_eq!(val["op"], 1);
        assert_eq!(val["d"]["data"]["address"], "203.0.113.88");
        assert_eq!(val["d"]["data"]["port"], 55555);
        assert_eq!(val["d"]["data"]["mode"], "aead_aes256_gcm_rtpsize");
    } else {
        panic!("Expected SendMessage(SelectProtocol)");
    }

    // 5. Gateway delivers Op 4 Session Description with sequence number 11
    let secret = vec![15u8; 32];
    let secret_json = serde_json::to_string(&secret).unwrap();
    let session_raw = format!(
        r#"{{"op": 4, "seq": 11, "d": {{"mode": "aead_aes256_gcm_rtpsize", "secret_key": {}}}}}"#,
        secret_json
    );
    let actions = sm.handle_incoming_text(&session_raw).unwrap();
    assert_eq!(sm.last_seq(), 11);
    let expected_desc = SessionDescription {
        mode: "aead_aes256_gcm_rtpsize".to_string(),
        secret_key: secret,
        dave_protocol_version: None,
    };
    assert_eq!(*sm.current_state(), VoiceConnectionState::Connected(expected_desc.clone()));
    assert_eq!(actions, vec![VoiceGatewayAction::SessionDescriptionReceived(expected_desc)]);

    // 6. Client sends Heartbeat with seq_ack = 11
    let hb_action = sm.trigger_heartbeat(Some(9999999)).unwrap();
    if let VoiceGatewayAction::SendMessage(hb_str) = hb_action {
        let val: serde_json::Value = serde_json::from_str(&hb_str).unwrap();
        assert_eq!(val["op"], 3);
        assert_eq!(val["d"]["seq_ack"], 11);
        assert_eq!(val["d"]["t"], 9999999);
    } else {
        panic!("Expected SendMessage(Heartbeat)");
    }

    // 7. Gateway sends Heartbeat ACK
    let ack_raw = r#"{"op": 6, "d": 9999999}"#;
    let actions = sm.handle_incoming_text(ack_raw).unwrap();
    assert_eq!(actions, vec![VoiceGatewayAction::HeartbeatAckReceived { timestamp: Some(9999999) }]);
}

#[test]
fn test_state_machine_buffered_resume() {
    let mut sm = VoiceGatewayStateMachine::new(mock_voice_config());
    sm.set_last_seq(45);
    let session = SessionDescription {
        mode: "aead_aes256_gcm_rtpsize".to_string(),
        secret_key: vec![3u8; 32],
        dave_protocol_version: None,
    };
    sm.set_session_description(session.clone());
    sm.set_state(VoiceConnectionState::Connected(session.clone()));

    // Connection drop (can_resume = true)
    let action = sm.handle_disconnect(true);
    assert_eq!(action, None);
    assert_eq!(*sm.current_state(), VoiceConnectionState::Resuming);

    // Reconnected: Gateway sends Op 8 Hello
    let hello_raw = r#"{"op": 8, "d": {"v": 8, "heartbeat_interval": 30000.0}}"#;
    let actions = sm.handle_incoming_text(hello_raw).unwrap();
    assert_eq!(actions.len(), 2);
    // Action 1: StartHeartbeat
    assert_eq!(actions[0], VoiceGatewayAction::StartHeartbeat { interval_ms: 30000 });
    // Action 2: Send Op 7 Resume with seq_ack = 45
    if let VoiceGatewayAction::SendMessage(resume_str) = &actions[1] {
        let val: serde_json::Value = serde_json::from_str(resume_str).unwrap();
        assert_eq!(val["op"], 7);
        assert_eq!(val["d"]["seq_ack"], 45);
        assert_eq!(val["d"]["session_id"], "gateway_session_999");
    } else {
        panic!("Expected SendMessage(Resume)");
    }

    // Gateway responds with Op 9 Resumed
    let resumed_raw = r#"{"op": 9, "d": null}"#;
    let actions = sm.handle_incoming_text(resumed_raw).unwrap();
    assert_eq!(actions, vec![VoiceGatewayAction::Resumed]);
    assert_eq!(*sm.current_state(), VoiceConnectionState::Connected(session));
}

#[test]
fn test_main_gateway_voice_state_and_server_update() {
    // Test VoiceServerData construction and serde
    let v_server = VoiceServerData {
        token: "token_abc".to_string(),
        guild_id: Some("guild_123".to_string()),
        channel_id: Some("channel_456".to_string()),
        endpoint: Some("voice.discord.media:443".to_string()),
    };
    let json_val = serde_json::to_value(&v_server).unwrap();
    assert_eq!(json_val["token"], "token_abc");
    assert_eq!(json_val["guild_id"], "guild_123");
    assert_eq!(json_val["channel_id"], "channel_456");
    assert_eq!(json_val["endpoint"], "voice.discord.media:443");
}

#[test]
fn test_voice_gateway_close_codes_decisions() {
    // 1. Resumable
    assert_eq!(VoiceCloseDecision::from_close_code(4015), VoiceCloseDecision::Resume);
    assert!(VoiceCloseDecision::from_close_code(4015).can_resume());
    assert_eq!(VoiceCloseDecision::from_close_code(1001), VoiceCloseDecision::Resume);
    assert_eq!(VoiceCloseDecision::from_close_code(1006), VoiceCloseDecision::Resume);

    // 2. NewSession (reconnect with Opcode 0 Identify)
    assert_eq!(VoiceCloseDecision::from_close_code(4003), VoiceCloseDecision::NewSession);
    assert_eq!(VoiceCloseDecision::from_close_code(4005), VoiceCloseDecision::NewSession);
    assert_eq!(VoiceCloseDecision::from_close_code(4006), VoiceCloseDecision::NewSession);
    assert_eq!(VoiceCloseDecision::from_close_code(4009), VoiceCloseDecision::NewSession);
    assert!(!VoiceCloseDecision::from_close_code(4006).can_resume());
    assert!(VoiceCloseDecision::from_close_code(4006).should_reconnect());

    // 3. Fatal (stop and do not reconnect)
    assert_eq!(VoiceCloseDecision::from_close_code(4001), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4002), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4004), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4011), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4012), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4014), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4016), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4017), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4020), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(4022), VoiceCloseDecision::Fatal);
    assert_eq!(VoiceCloseDecision::from_close_code(1000), VoiceCloseDecision::Fatal);
    assert!(!VoiceCloseDecision::from_close_code(4017).should_reconnect());
    assert!(!VoiceCloseDecision::from_close_code(4014).should_reconnect());
}

#[test]
fn test_state_machine_close_code_handling_offline() {
    let mut sm = VoiceGatewayStateMachine::new(mock_voice_config());
    sm.set_last_seq(100);

    // Test 1: Code 4015 (Server crashed) -> Resume, keeps sequence
    let (decision, action) = sm.handle_close_code(4015, "Server died");
    assert_eq!(decision, VoiceCloseDecision::Resume);
    assert_eq!(action, None);
    assert_eq!(*sm.current_state(), VoiceConnectionState::Resuming);
    assert_eq!(sm.last_seq(), 100);

    // Test 2: Code 4006 (Session invalid) -> NewSession, sequence reset to -1
    let (decision, action) = sm.handle_close_code(4006, "Invalidated");
    assert_eq!(decision, VoiceCloseDecision::NewSession);
    assert_eq!(action, None);
    assert_eq!(*sm.current_state(), VoiceConnectionState::WaitingForHello);
    assert_eq!(sm.last_seq(), -1);

    // Test 3: Code 4017 (DAVE Required) -> Fatal, state Failed, action CloseConnection
    let (decision, action) = sm.handle_close_code(4017, "");
    assert_eq!(decision, VoiceCloseDecision::Fatal);
    assert!(matches!(*sm.current_state(), VoiceConnectionState::Failed(_)));
    if let Some(VoiceGatewayAction::CloseConnection { reason, can_resume }) = action {
        assert!(!can_resume);
        assert!(reason.contains("DAVE"));
    } else {
        panic!("Expected CloseConnection action");
    }

    // Test 4: Code 4014 (Kicked/Channel Deleted) -> Fatal, state Failed, action CloseConnection
    let (decision, action) = sm.handle_close_code(4014, "Channel removed");
    assert_eq!(decision, VoiceCloseDecision::Fatal);
    assert!(matches!(*sm.current_state(), VoiceConnectionState::Failed(_)));
    if let Some(VoiceGatewayAction::CloseConnection { reason, can_resume }) = action {
        assert!(!can_resume);
        assert!(reason.contains("kicked"));
        assert!(reason.contains("Channel removed"));
    } else {
        panic!("Expected CloseConnection action");
    }
}

#[test]
fn test_voice_gateway_event_disconnected_structure() {
    let event = VoiceGatewayEvent::Disconnected {
        code: Some(4017),
        reason: describe_voice_close_code(4017).to_string(),
        decision: VoiceCloseDecision::Fatal,
    };

    if let VoiceGatewayEvent::Disconnected { code, reason, decision } = event {
        assert_eq!(code, Some(4017));
        assert!(reason.contains("DAVE"));
        assert_eq!(decision, VoiceCloseDecision::Fatal);
    } else {
        panic!("Expected Disconnected event");
    }
}

// ============================================================================
// DAVE PROTOCOL & E2EE MEDIA CONTRACT TESTS
// ============================================================================

#[test]
fn test_dave_opcodes_and_framing() {
    use discord_rs::protocol::{
        format_client_binary_message, parse_server_binary_message, DaveOpcode,
    };

    // Test opcode byte mappings and classification
    assert_eq!(DaveOpcode::from_u8(21), Some(DaveOpcode::PrepareTransition));
    assert_eq!(DaveOpcode::from_u8(22), Some(DaveOpcode::ExecuteTransition));
    assert_eq!(DaveOpcode::from_u8(23), Some(DaveOpcode::TransitionReady));
    assert_eq!(DaveOpcode::from_u8(24), Some(DaveOpcode::PrepareEpoch));
    assert_eq!(DaveOpcode::from_u8(25), Some(DaveOpcode::MlsExternalSender));
    assert_eq!(DaveOpcode::from_u8(26), Some(DaveOpcode::MlsKeyPackage));
    assert_eq!(DaveOpcode::from_u8(27), Some(DaveOpcode::MlsProposals));
    assert_eq!(DaveOpcode::from_u8(28), Some(DaveOpcode::MlsCommitWelcome));
    assert_eq!(DaveOpcode::from_u8(29), Some(DaveOpcode::MlsAnnounceCommitTransition));
    assert_eq!(DaveOpcode::from_u8(30), Some(DaveOpcode::MlsWelcome));
    assert_eq!(DaveOpcode::from_u8(31), Some(DaveOpcode::MlsInvalidCommitWelcome));
    assert_eq!(DaveOpcode::from_u8(100), None);

    assert!(DaveOpcode::MlsExternalSender.is_binary());
    assert!(DaveOpcode::MlsKeyPackage.is_binary());
    assert!(DaveOpcode::MlsProposals.is_binary());
    assert!(DaveOpcode::MlsCommitWelcome.is_binary());
    assert!(DaveOpcode::MlsWelcome.is_binary());

    assert!(DaveOpcode::PrepareTransition.is_json());
    assert!(DaveOpcode::ExecuteTransition.is_json());
    assert!(DaveOpcode::TransitionReady.is_json());
    assert!(DaveOpcode::PrepareEpoch.is_json());
    assert!(DaveOpcode::MlsAnnounceCommitTransition.is_json());
    assert!(DaveOpcode::MlsInvalidCommitWelcome.is_json());

    // Test client binary formatting: [opcode (1 byte), payload...]
    let client_msg = format_client_binary_message(DaveOpcode::MlsKeyPackage, &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(client_msg.len(), 5);
    assert_eq!(client_msg[0], 26);
    assert_eq!(&client_msg[1..], &[0xDE, 0xAD, 0xBE, 0xEF]);

    // Test server binary parsing: [seq (2 bytes BE), opcode (1 byte), payload...]
    let raw_server = vec![0x01, 0x02, 25, 0xCA, 0xFE, 0xBA, 0xBE];
    let (seq, op, payload) = parse_server_binary_message(&raw_server).expect("Failed to parse binary message");
    assert_eq!(seq, 0x0102);
    assert_eq!(op, DaveOpcode::MlsExternalSender);
    assert_eq!(payload, &[0xCA, 0xFE, 0xBA, 0xBE]);

    // Truncated binary frame error handling
    assert!(parse_server_binary_message(&[0x01, 0x02]).is_err());
}

#[test]
fn test_dave_json_payloads_serialization() {
    use discord_rs::protocol::{
        DaveAnnounceCommitTransitionPayload, DaveExecuteTransitionPayload,
        DaveInvalidCommitWelcomePayload, DavePrepareEpochPayload, DavePrepareTransitionPayload,
        DaveTransitionReadyPayload,
    };

    // Opcode 21
    let prep = DavePrepareTransitionPayload { transition_id: 42, protocol_version: 1 };
    let json = serde_json::to_string(&prep).unwrap();
    let parsed: DavePrepareTransitionPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, prep);

    // Opcode 22
    let exec = DaveExecuteTransitionPayload { transition_id: 42 };
    let json = serde_json::to_string(&exec).unwrap();
    let parsed: DaveExecuteTransitionPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, exec);

    // Opcode 23
    let ready = DaveTransitionReadyPayload { transition_id: 42 };
    let json = serde_json::to_string(&ready).unwrap();
    let parsed: DaveTransitionReadyPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, ready);

    // Opcode 24
    let epoch = DavePrepareEpochPayload { epoch: 100, protocol_version: 1 };
    let json = serde_json::to_string(&epoch).unwrap();
    let parsed: DavePrepareEpochPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, epoch);

    // Opcode 29
    let announce = DaveAnnounceCommitTransitionPayload {
        transition_id: 42,
        commit: Some("base64commit".to_string()),
    };
    let json = serde_json::to_string(&announce).unwrap();
    let parsed: DaveAnnounceCommitTransitionPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, announce);

    // Opcode 31
    let invalid = DaveInvalidCommitWelcomePayload { transition_id: 42 };
    let json = serde_json::to_string(&invalid).unwrap();
    let parsed: DaveInvalidCommitWelcomePayload = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, invalid);
}

#[test]
fn test_dave_default_identify_version_1() {
    let mut config = mock_voice_config();
    config.max_dave_protocol_version = None;
    let sm = VoiceGatewayStateMachine::new(config);
    let identify = sm.generate_identify();

    let value = serde_json::to_value(&identify).unwrap();
    assert_eq!(value["d"]["max_dave_protocol_version"], 1);
}

#[test]
fn test_dave_signaling_and_media_contract_flow() {
    let mut sm = VoiceGatewayStateMachine::new(mock_voice_config());

    // 1. SessionDescription arrives with negotiated DAVE version 1
    let session_msg = r#"{
        "op": 4,
        "d": {
            "mode": "aead_aes256_gcm_rtpsize",
            "secret_key": [1,2,3,4,5,6,7,8,9,1,2,3,4,5,6,7,8,9,1,2,3,4,5,6,7,8,9,1,2,3,4,5],
            "dave_protocol_version": 1
        }
    }"#;
    let actions = sm.handle_incoming_text(session_msg).unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(sm.dave_session().protocol_version(), 1);

    // 2. Server sends Opcode 21 Prepare Transition
    let prep_msg = r#"{"op": 21, "d": {"transition_id": 5, "protocol_version": 1}}"#;
    let actions = sm.handle_incoming_text(prep_msg).unwrap();
    assert_eq!(actions.len(), 1);
    // Client must automatically reply with Opcode 23 Transition Ready
    if let VoiceGatewayAction::SendMessage(json_str) = &actions[0] {
        let val: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(val["op"], 23);
        assert_eq!(val["d"]["transition_id"], 5);
    } else {
        panic!("Expected SendMessage(TransitionReady)");
    }

    // 3. Server sends Opcode 22 Execute Transition
    let exec_msg = r#"{"op": 22, "d": {"transition_id": 5}}"#;
    let actions = sm.handle_incoming_text(exec_msg).unwrap();
    assert!(actions.is_empty());

    // 4. Remote participant speaking mapping (SSRC -> User ID)
    let speaking_msg = r#"{"op": 5, "d": {"speaking": 1, "ssrc": 7777, "user_id": "999888777"}}"#;
    sm.handle_incoming_text(speaking_msg).unwrap();
    assert_eq!(sm.dave_session().get_user_id_by_ssrc(7777), Some(999888777));

    // 5. Media contract verification: Passthrough while MLS key negotiation proceeds
    let raw_opus_audio = vec![0xF8, 0xFF, 0xFE, 0x01, 0x02, 0x03];
    let encrypted = sm.dave_session_mut().encrypt_opus_frame(&raw_opus_audio).unwrap();
    assert_eq!(encrypted, raw_opus_audio); // Passthrough intact

    let decrypted = sm.dave_session_mut().decrypt_opus_frame_by_ssrc(7777, &raw_opus_audio).unwrap();
    assert_eq!(decrypted, raw_opus_audio); // Passthrough intact
}
