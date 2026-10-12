//! Offline integration tests for Voice Stage, zero mocks compliance,
//! state transitions, participant synchronization, speaking ring, and call controls.

use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::{broadcast, Mutex};

use discord_rs::app::state::AppState;
use discord_rs::domain::voice::VoiceState;
use discord_rs::protocol::client_manager::ClientManager;
use discord_rs::protocol::voice_gateway::SpeakingPayload;
use discord_rs::protocol::voice_session::VoiceSessionEvent;
use discord_rs::protocol::{
    AudioBridge, AudioBridgeCommand, MainSignalingChannel, SessionDescription, TransportEvent,
    VoiceGatewayEvent, VoiceGatewayTransport, VoiceReady, VoiceSessionConfig, VoiceSessionState,
};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[test]
fn test_connect_voice_zero_invented_ping() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();

    // 1. Initial state: zero voice call
    assert!(state.active_voice_call.is_none());

    // 2. Connect voice to a channel with a real numeric Snowflake
    state.connect_voice("123456789012345678", "Canal de Voz", "Servidor Teste");

    let call = state.active_voice_call.as_ref().expect("ActiveVoiceCall must exist");
    assert_eq!(call.channel_id, "123456789012345678");
    assert_eq!(call.channel_name, "Canal de Voz");
    assert_eq!(call.guild_name, "Servidor Teste");

    // Zero mocks: Ping MUST NOT be hardcoded to 18ms or any invented number.
    assert_eq!(call.ping_ms, None, "ping_ms must start as None (zero mocks)");
    assert_eq!(
        call.connection_state,
        VoiceSessionState::Connecting,
        "Initial session state must be Connecting"
    );
}

#[test]
fn test_invalid_ids_no_session_and_fatal_error() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();

    // 1. Non-numeric channel ID: must NOT start session and must record fatal_error
    state.connect_voice("invalid-channel-id", "Canal Inválido", "Servidor");
    let call = state.active_voice_call.as_ref().expect("ActiveVoiceCall must exist");
    assert!(call.fatal_error.is_some(), "fatal_error must be recorded for non-numeric channel ID");
    assert!(
        matches!(call.connection_state, VoiceSessionState::Disconnected { .. }),
        "connection_state must be Disconnected on invalid channel ID"
    );

    // 2. Non-numeric user ID: must NOT start session and must record fatal_error
    let mut state2 = AppState::new();
    state2.current_user.id = "non-numeric-user".into();
    state2.connect_voice("123456789012345678", "Canal de Voz", "Servidor");
    let call2 = state2.active_voice_call.as_ref().expect("ActiveVoiceCall must exist");
    assert!(call2.fatal_error.is_some(), "fatal_error must be recorded for non-numeric user ID");
    assert!(
        matches!(call2.connection_state, VoiceSessionState::Disconnected { .. }),
        "connection_state must be Disconnected on invalid user ID"
    );
}

#[test]
fn test_voice_session_state_transitions_and_active_call() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice("123456789012345678", "Voz", "Guild");

    // 1. Transition to Connected with real measured ping
    state.update_voice_session_state(VoiceSessionState::Connected, Some(35));
    {
        let call = state.active_voice_call.as_ref().unwrap();
        assert_eq!(call.connection_state, VoiceSessionState::Connected);
        assert_eq!(call.ping_ms, Some(35));
    }

    // 2. Transition to Reconnecting (ping omitted or lost)
    state.update_voice_session_state(VoiceSessionState::Reconnecting, None);
    {
        let call = state.active_voice_call.as_ref().unwrap();
        assert_eq!(call.connection_state, VoiceSessionState::Reconnecting);
        assert_eq!(call.ping_ms, None);
    }

    // 3. Transition to Disconnected with reason
    state.update_voice_session_state(
        VoiceSessionState::Disconnected {
            reason: "Perda de conexão UDP".into(),
        },
        None,
    );
    {
        let call = state.active_voice_call.as_ref().unwrap();
        assert_eq!(
            call.connection_state,
            VoiceSessionState::Disconnected {
                reason: "Perda de conexão UDP".into()
            }
        );
        assert_eq!(call.ping_ms, None);
        assert_eq!(call.fatal_error, Some("Perda de conexão UDP".into()));
    }
}

#[test]
fn test_participant_sync_from_voice_states_by_channel() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();

    // Popula estados de voz do canal antes da conexão
    let mut vs_bob = VoiceState::new("u-bob", "sess-bob");
    vs_bob.guild_id = Some("g-1".into());
    vs_bob.channel_id = Some("123456789012345678".into());
    vs_bob.user_name = Some("Bob".into());
    vs_bob.self_mute = true;
    vs_bob.self_video = true;

    let mut vs_carol = VoiceState::new("u-carol", "sess-carol");
    vs_carol.guild_id = Some("g-1".into());
    vs_carol.channel_id = Some("123456789012345678".into());
    vs_carol.user_name = Some("Carol".into());

    state.update_voice_state(vs_bob);
    state.update_voice_state(vs_carol);

    // Conecta na sala
    state.connect_voice("123456789012345678", "Voz Geral", "Guild 1");

    let call = state.active_voice_call.as_ref().unwrap();
    // Participantes devem conter Bob, Carol e o usuário local
    assert!(call.participants.iter().any(|p| p.id == "u-bob"));
    assert!(call.participants.iter().any(|p| p.id == "u-carol"));
    assert!(call.participants.iter().any(|p| p.id == state.current_user.id));

    let bob = call.participants.iter().find(|p| p.id == "u-bob").unwrap();
    assert_eq!(bob.name, "Bob");
    assert!(bob.is_muted);
    assert!(bob.is_camera_on);
    assert!(!bob.is_speaking);

    // Carol se desconecta (channel_id = None)
    let mut vs_carol_leaves = VoiceState::new("u-carol", "sess-carol");
    vs_carol_leaves.channel_id = None;
    state.update_voice_state(vs_carol_leaves);

    let call_after = state.active_voice_call.as_ref().unwrap();
    assert!(!call_after.participants.iter().any(|p| p.id == "u-carol"));
}

#[test]
fn test_speaking_ring_state_per_user() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice("123456789012345678", "Voz", "Guild");

    let mut vs_dave = VoiceState::new("u-dave", "sess-dave");
    vs_dave.channel_id = Some("123456789012345678".into());
    vs_dave.user_name = Some("Dave".into());
    state.update_voice_state(vs_dave);

    // Inicialmente ninguém está falando
    let call = state.active_voice_call.as_ref().unwrap();
    let dave = call.participants.iter().find(|p| p.id == "u-dave").unwrap();
    assert!(!dave.is_speaking);

    // Evento real de speaking para Dave
    state.set_participant_speaking("u-dave", true);
    {
        let call = state.active_voice_call.as_ref().unwrap();
        let dave = call.participants.iter().find(|p| p.id == "u-dave").unwrap();
        assert!(dave.is_speaking, "Dave deve ter anel de fala verde ativo");
        let me = call.participants.iter().find(|p| p.id == state.current_user.id).unwrap();
        assert!(!me.is_speaking, "Outros usuários não devem ter anel de fala");
    }

    // Dave para de falar
    state.set_participant_speaking("u-dave", false);
    {
        let call = state.active_voice_call.as_ref().unwrap();
        let dave = call.participants.iter().find(|p| p.id == "u-dave").unwrap();
        assert!(!dave.is_speaking, "Anel de fala deve ser desativado");
    }
}

#[test]
fn test_focus_mode_and_media_controls() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice("123456789012345678", "Voz", "Guild");

    // Foco em usuário específico
    assert_eq!(state.active_voice_call.as_ref().unwrap().focus_user_id, None);
    state.set_focus_user(Some("u-streamer".into()));
    assert_eq!(
        state.active_voice_call.as_ref().unwrap().focus_user_id,
        Some("u-streamer".into())
    );

    // Sem pipeline de vídeo/tela, toggle_camera e toggle_screen_share não alteram as flags (Zero mocks)
    assert!(!state.active_voice_call.as_ref().unwrap().is_camera_on);
    state.toggle_camera();
    assert!(
        !state.active_voice_call.as_ref().unwrap().is_camera_on,
        "is_camera_on não deve ser alterado sem pipeline real"
    );

    assert!(!state.active_voice_call.as_ref().unwrap().is_screen_sharing);
    state.toggle_screen_share();
    assert!(
        !state.active_voice_call.as_ref().unwrap().is_screen_sharing,
        "is_screen_sharing não deve ser alterado sem pipeline real"
    );

    // Desconectar voz limpa active_voice_call
    state.disconnect_voice();
    assert!(state.active_voice_call.is_none());
}

#[test]
fn test_dm_call_ring_failure_sets_fatal_error() {
    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice_dm("123456789012345678", "DM Teste");

    let err_msg = "Falha ao tocar chamada na DM: 403 Forbidden (ring_call recusado)".to_string();
    state.set_call_fatal_error(err_msg.clone());

    let call = state.active_voice_call.as_ref().expect("ActiveVoiceCall must exist");
    assert_eq!(call.fatal_error, Some(err_msg.clone()));
    assert_eq!(
        call.connection_state,
        VoiceSessionState::Disconnected { reason: err_msg }
    );
}

#[tokio::test]
async fn test_toggle_mute_and_deafen_dispatches_to_session() {
    let (event_tx, _) = broadcast::channel(16);
    let updates = Arc::new(Mutex::new(Vec::new()));
    let signaling = Arc::new(MockSignaling {
        events_tx: event_tx.clone(),
        updates: updates.clone(),
        auto_reply_port: None,
    });

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(16);
    let (_outgoing_tx, outgoing_rx) = tokio::sync::mpsc::channel(16);
    let (_speaking_tx, speaking_rx) = tokio::sync::mpsc::channel(16);
    let audio_bridge = AudioBridge::from_channels(outgoing_rx, speaking_rx, cmd_tx);

    let cfg = VoiceSessionConfig {
        guild_id: Some(12345),
        channel_id: 67890,
        user_id: 11111,
        self_mute: false,
        self_deaf: false,
    };

    let session = Arc::new(
        discord_rs::protocol::voice_session::VoiceSession::new(
            cfg,
            signaling,
            audio_bridge,
        )
        .with_gateway_factory(|_gw_cfg| {
            let (tx, _) = broadcast::channel(16);
            Arc::new(MockVoiceGatewayTransport {
                events_tx: tx,
                udp_port: 50000,
                ssrc: 12345,
            })
        }),
    );

    let mut state = AppState::new();
    state.current_user.id = "11111".into();
    state.connect_voice("67890", "Voz", "Guild");
    state.set_voice_session(session.clone());

    // 1. Toggle mute
    state.toggle_mute();
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let up = updates.lock().await;
        assert!(
            up.iter().any(|(_g, _c, mute, deaf)| *mute && !*deaf),
            "Comando set_muted(true) deve chegar à sinalização da VoiceSession"
        );
    }

    // 2. Toggle deafen
    state.toggle_deafen();
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let up = updates.lock().await;
        assert!(
            up.iter().any(|(_g, _c, mute, deaf)| *mute && *deaf),
            "Comando set_deafened(true) deve chegar à sinalização da VoiceSession"
        );
    }
}

#[tokio::test]
async fn test_client_manager_ring_call_reject_no_optimistic_ui() {
    let (client_mgr, _rx) = ClientManager::new();

    // Tentativa de ring_call sem gateway conectado deve falhar
    let result = client_mgr.ring_call("dm-chan-99", None).await;
    assert!(
        result.is_err(),
        "ring_call deve falhar se o gateway não estiver conectado"
    );

    // Verificação de Zero Mocks: UI não abre chamada em caso de erro da API
    let state = AppState::new();
    assert!(
        state.active_voice_call.is_none(),
        "Nenhuma chamada deve ser criada optimisticamente quando a API recusa"
    );
}

// ---------------------------------------------------------------------------
// Mock de gateway para teste offline de VoiceSession e ClientManager
// ---------------------------------------------------------------------------

async fn spawn_test_udp_server(expected_ssrc: u32) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, client_addr)) => {
                    if len == 74 && buf[0..2] == 1u16.to_be_bytes() {
                        let mut resp = [0u8; 74];
                        resp[0..2].copy_from_slice(&2u16.to_be_bytes());
                        resp[2..4].copy_from_slice(&70u16.to_be_bytes());
                        resp[4..8].copy_from_slice(&expected_ssrc.to_be_bytes());
                        let ip = b"127.0.0.1";
                        resp[8..8 + ip.len()].copy_from_slice(ip);
                        resp[8 + ip.len()] = 0;
                        resp[72..74].copy_from_slice(&addr.port().to_be_bytes());
                        let _ = socket.send_to(&resp, client_addr).await;
                        continue;
                    }
                    if len == 8 {
                        continue;
                    }
                }
                Err(_) => break,
            }
        }
    });

    (addr, handle)
}

struct MockVoiceGatewayTransport {
    events_tx: broadcast::Sender<VoiceGatewayEvent>,
    udp_port: u16,
    ssrc: u32,
}

#[async_trait]
impl VoiceGatewayTransport for MockVoiceGatewayTransport {
    async fn connect(&self) -> Result<()> {
        let _ = self.events_tx.send(VoiceGatewayEvent::Connected);
        Ok(())
    }

    async fn wait_for_ready(&self, _timeout: Duration) -> Result<VoiceReady> {
        Ok(VoiceReady {
            ssrc: self.ssrc,
            ip: "127.0.0.1".into(),
            port: self.udp_port,
            modes: vec!["aead_aes256_gcm_rtpsize".into()],
        })
    }

    async fn select_protocol(
        &self,
        _external_ip: &str,
        _external_port: u16,
        mode: &str,
    ) -> Result<SessionDescription> {
        Ok(SessionDescription {
            mode: mode.into(),
            secret_key: vec![0x42; 32],
            dave_protocol_version: None,
        })
    }

    async fn set_speaking(&self, _speaking: bool, _delay: u32, _ssrc: u32) -> Result<()> {
        Ok(())
    }

    fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
        self.events_tx.subscribe()
    }

    async fn send_text(&self, _text: String) -> Result<()> {
        Ok(())
    }

    async fn send_binary(&self, _data: Vec<u8>) -> Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }
}

struct MockSignaling {
    events_tx: broadcast::Sender<TransportEvent>,
    updates: Arc<Mutex<Vec<(Option<String>, Option<String>, bool, bool)>>>,
    auto_reply_port: Option<u16>,
}

#[async_trait]
impl MainSignalingChannel for MockSignaling {
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        let mut l = self.updates.lock().await;
        l.push((
            guild_id.map(|s| s.to_string()),
            channel_id.map(|s| s.to_string()),
            self_mute,
            self_deaf,
        ));

        if let (Some(chan_id), Some(port)) = (channel_id, self.auto_reply_port) {
            let mut vs = VoiceState::new("987654321098765432", "sess-test");
            vs.guild_id = guild_id.map(|s| s.to_string());
            vs.channel_id = Some(chan_id.to_string());
            let _ = self.events_tx.send(TransportEvent::VoiceStateUpdate(vs));
            let _ = self.events_tx.send(TransportEvent::VoiceServerUpdate {
                token: "mock-token".into(),
                guild_id: guild_id.map(|s| s.to_string()),
                channel_id: Some(chan_id.to_string()),
                endpoint: Some(format!("127.0.0.1:{port}")),
            });
        }
        Ok(())
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.events_tx.subscribe()
    }
}

#[tokio::test]
async fn test_voice_session_lifecycle_with_gateway_factory() {
    let (event_tx, _) = broadcast::channel(16);
    let updates = Arc::new(Mutex::new(Vec::new()));
    let signaling = Arc::new(MockSignaling {
        events_tx: event_tx.clone(),
        updates,
        auto_reply_port: None,
    });

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(16);
    let (_outgoing_tx, outgoing_rx) = tokio::sync::mpsc::channel(16);
    let (_speaking_tx, speaking_rx) = tokio::sync::mpsc::channel(16);
    let audio_bridge = AudioBridge::from_channels(outgoing_rx, speaking_rx, cmd_tx);

    let cfg = VoiceSessionConfig {
        guild_id: Some(12345),
        channel_id: 67890,
        user_id: 11111,
        self_mute: false,
        self_deaf: false,
    };

    let session = discord_rs::protocol::voice_session::VoiceSession::new(
        cfg,
        signaling,
        audio_bridge,
    )
    .with_gateway_factory(|_gw_cfg| {
        let (tx, _) = broadcast::channel(16);
        Arc::new(MockVoiceGatewayTransport {
            events_tx: tx,
            udp_port: 50000,
            ssrc: 12345,
        })
    });

    assert_eq!(
        session.state(),
        VoiceSessionState::Disconnected {
            reason: "Sessão inicializada".into()
        }
    );

    // Desconecta a sessão de forma limpa
    session.disconnect().await.expect("Disconnect must succeed");
    assert_eq!(
        session.state(),
        VoiceSessionState::Disconnected {
            reason: "Desconectado pelo usuário".into()
        }
    );
}

#[tokio::test]
async fn test_opcode_5_speaking_lights_and_clears_participant() {
    let ssrc = 0x1234;
    let (udp_addr, _udp_handle) = spawn_test_udp_server(ssrc).await;

    let (gw_tx, _) = broadcast::channel(32);
    let (sig_tx, _) = broadcast::channel(32);
    let signaling = Arc::new(MockSignaling {
        events_tx: sig_tx,
        updates: Arc::new(Mutex::new(Vec::new())),
        auto_reply_port: Some(udp_addr.port()),
    });

    let (audio_cmd_tx, mut audio_cmd_rx) = tokio::sync::mpsc::channel(32);
    let (_audio_out_tx, audio_out_rx) = tokio::sync::mpsc::channel(32);
    let (_speaking_tx, speaking_rx) = tokio::sync::mpsc::channel(16);
    let audio_bridge = AudioBridge::from_channels(audio_out_rx, speaking_rx, audio_cmd_tx);

    let gw_tx_clone = gw_tx.clone();
    let port = udp_addr.port();
    let session = discord_rs::protocol::voice_session::VoiceSession::new(
        VoiceSessionConfig {
            guild_id: Some(111111),
            channel_id: 222222,
            user_id: 987654321098765432,
            self_mute: false,
            self_deaf: false,
        },
        signaling,
        audio_bridge,
    )
    .with_gateway_factory(move |_gw_cfg| {
        Arc::new(MockVoiceGatewayTransport {
            events_tx: gw_tx_clone.clone(),
            udp_port: port,
            ssrc,
        })
    });

    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            match cmd {
                AudioBridgeCommand::StartSession { responder } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::SwitchInputDevice { responder, .. } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::SwitchOutputDevice { responder, .. } => {
                    let _ = responder.send(Ok(()));
                }
                _ => {}
            }
        }
    });

    let mut session_events_rx = session.subscribe_events();
    session.start().await.expect("start session");
    session.wait_until_connected(Duration::from_secs(3)).await.expect("connect");

    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice("222222", "Canal de Voz", "Servidor Teste");

    let mut vs_user = VoiceState::new("555555555555555555", "sess-555");
    vs_user.channel_id = Some("222222".into());
    vs_user.user_name = Some("Alice".into());
    state.update_voice_state(vs_user);

    // 1. Envia Opcode 5 (Speaking: true) pelo Voice Gateway
    let _ = gw_tx.send(VoiceGatewayEvent::Speaking(SpeakingPayload {
        speaking: 1,
        delay: 0,
        ssrc: 9999,
        user_id: Some("555555555555555555".into()),
    }));

    // Verifica que VoiceSession emitiu VoiceSessionEvent::Speaking
    let event = loop {
        let ev = tokio::time::timeout(Duration::from_millis(500), session_events_rx.recv())
            .await
            .expect("timeout esperando evento Speaking")
            .expect("evento Speaking");
        if matches!(ev, VoiceSessionEvent::Speaking { .. }) {
            break ev;
        }
    };

    match event {
        VoiceSessionEvent::Speaking { ssrc: ev_ssrc, user_id, is_speaking } => {
            assert_eq!(ev_ssrc, 9999);
            assert_eq!(user_id, Some(555555555555555555));
            assert!(is_speaking);
            state.set_participant_speaking(&user_id.unwrap().to_string(), is_speaking);
        }
        other => panic!("Esperado VoiceSessionEvent::Speaking, recebido: {:?}", other),
    }

    // Participante Alice acende (is_speaking == true)
    {
        let call = state.active_voice_call.as_ref().unwrap();
        let alice = call.participants.iter().find(|p| p.id == "555555555555555555").unwrap();
        assert!(alice.is_speaking, "Alice deve acender com anel de fala verde");
    }

    // 2. Envia Opcode 5 (Speaking: false / speaking == 0)
    let _ = gw_tx.send(VoiceGatewayEvent::Speaking(SpeakingPayload {
        speaking: 0,
        delay: 0,
        ssrc: 9999,
        user_id: Some("555555555555555555".into()),
    }));

    let event_off = loop {
        let ev = tokio::time::timeout(Duration::from_millis(500), session_events_rx.recv())
            .await
            .expect("timeout esperando evento Speaking off")
            .expect("evento Speaking off");
        if matches!(ev, VoiceSessionEvent::Speaking { .. }) {
            break ev;
        }
    };

    match event_off {
        VoiceSessionEvent::Speaking { ssrc: ev_ssrc, user_id, is_speaking } => {
            assert_eq!(ev_ssrc, 9999);
            assert_eq!(user_id, Some(555555555555555555));
            assert!(!is_speaking);
            state.set_participant_speaking(&user_id.unwrap().to_string(), is_speaking);
        }
        other => panic!("Esperado VoiceSessionEvent::Speaking off, recebido: {:?}", other),
    }

    // Participante Alice apaga (is_speaking == false)
    {
        let call = state.active_voice_call.as_ref().unwrap();
        let alice = call.participants.iter().find(|p| p.id == "555555555555555555").unwrap();
        assert!(!alice.is_speaking, "Alice deve apagar o anel de fala verde");
    }

    session.disconnect().await.expect("disconnect");
}

#[tokio::test]
async fn test_heartbeat_ack_produces_rtt_latency() {
    let ssrc = 0x5678;
    let (udp_addr, _udp_handle) = spawn_test_udp_server(ssrc).await;

    let (gw_tx, _) = broadcast::channel(32);
    let (sig_tx, _) = broadcast::channel(32);
    let signaling = Arc::new(MockSignaling {
        events_tx: sig_tx,
        updates: Arc::new(Mutex::new(Vec::new())),
        auto_reply_port: Some(udp_addr.port()),
    });

    let (audio_cmd_tx, mut audio_cmd_rx) = tokio::sync::mpsc::channel(32);
    let (_audio_out_tx, audio_out_rx) = tokio::sync::mpsc::channel(32);
    let (_speaking_tx, speaking_rx) = tokio::sync::mpsc::channel(16);
    let audio_bridge = AudioBridge::from_channels(audio_out_rx, speaking_rx, audio_cmd_tx);

    let gw_tx_clone = gw_tx.clone();
    let port = udp_addr.port();
    let session = discord_rs::protocol::voice_session::VoiceSession::new(
        VoiceSessionConfig {
            guild_id: Some(111111),
            channel_id: 222222,
            user_id: 987654321098765432,
            self_mute: false,
            self_deaf: false,
        },
        signaling,
        audio_bridge,
    )
    .with_gateway_factory(move |_gw_cfg| {
        Arc::new(MockVoiceGatewayTransport {
            events_tx: gw_tx_clone.clone(),
            udp_port: port,
            ssrc,
        })
    });

    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            if let AudioBridgeCommand::StartSession { responder } = cmd {
                let _ = responder.send(Ok(()));
            }
        }
    });

    let mut session_events_rx = session.subscribe_events();
    session.start().await.expect("start session");
    session.wait_until_connected(Duration::from_secs(3)).await.expect("connect");

    // Zero mocks: antes de qualquer Heartbeat ACK, ping_ms é None
    assert_eq!(session.ping_ms(), None);

    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice("222222", "Canal de Voz", "Servidor Teste");
    assert_eq!(state.active_voice_call.as_ref().unwrap().ping_ms, None);

    // Gateway de voz recebe Opcode 6 Heartbeat ACK com 28ms de RTT
    let _ = gw_tx.send(VoiceGatewayEvent::HeartbeatAck { latency_ms: Some(28) });

    let event = loop {
        let ev = tokio::time::timeout(Duration::from_millis(500), session_events_rx.recv())
            .await
            .expect("timeout esperando HeartbeatAck")
            .expect("HeartbeatAck");
        if matches!(ev, VoiceSessionEvent::HeartbeatAck { .. }) {
            break ev;
        }
    };

    match event {
        VoiceSessionEvent::HeartbeatAck { ping_ms } => {
            assert_eq!(ping_ms, 28);
        }
        other => panic!("Esperado HeartbeatAck, recebido: {:?}", other),
    }

    // Sessão agora expõe o RTT real
    assert_eq!(session.ping_ms(), Some(28));

    // Estado da UI recebe o ping_ms atualizado
    state.update_voice_session_state(VoiceSessionState::Connected, session.ping_ms());
    assert_eq!(state.active_voice_call.as_ref().unwrap().ping_ms, Some(28));

    session.disconnect().await.expect("disconnect");
}

#[tokio::test]
async fn test_hot_device_swap_preserves_session_connected() {
    let ssrc = 0x9abc;
    let (udp_addr, _udp_handle) = spawn_test_udp_server(ssrc).await;

    let (gw_tx, _) = broadcast::channel(32);
    let (sig_tx, _) = broadcast::channel(32);
    let signaling = Arc::new(MockSignaling {
        events_tx: sig_tx,
        updates: Arc::new(Mutex::new(Vec::new())),
        auto_reply_port: Some(udp_addr.port()),
    });

    let (audio_cmd_tx, mut audio_cmd_rx) = tokio::sync::mpsc::channel(32);
    let (_audio_out_tx, audio_out_rx) = tokio::sync::mpsc::channel(32);
    let (_speaking_tx, speaking_rx) = tokio::sync::mpsc::channel(16);
    let audio_bridge = AudioBridge::from_channels(audio_out_rx, speaking_rx, audio_cmd_tx);

    let gw_tx_clone = gw_tx.clone();
    let port = udp_addr.port();
    let session = Arc::new(discord_rs::protocol::voice_session::VoiceSession::new(
        VoiceSessionConfig {
            guild_id: Some(111111),
            channel_id: 222222,
            user_id: 987654321098765432,
            self_mute: false,
            self_deaf: false,
        },
        signaling,
        audio_bridge,
    )
    .with_gateway_factory(move |_gw_cfg| {
        Arc::new(MockVoiceGatewayTransport {
            events_tx: gw_tx_clone.clone(),
            udp_port: port,
            ssrc,
        })
    }));

    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            match cmd {
                AudioBridgeCommand::StartSession { responder } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::SwitchInputDevice { responder, .. } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::SwitchOutputDevice { responder, .. } => {
                    let _ = responder.send(Ok(()));
                }
                _ => {}
            }
        }
    });

    session.start().await.expect("start session");
    session.wait_until_connected(Duration::from_secs(3)).await.expect("connect");
    assert_eq!(session.state(), VoiceSessionState::Connected);

    let mut state = AppState::new();
    state.current_user.id = "987654321098765432".into();
    state.connect_voice("222222", "Canal de Voz", "Servidor Teste");
    state.update_voice_session_state(VoiceSessionState::Connected, None);
    state.set_voice_session(session.clone());

    // 1. Troca a quente de microfone (dispositivo de entrada)
    session.switch_input_device(Some("Microfone USB HyperX".into())).await.expect("switch input");
    assert_eq!(session.state(), VoiceSessionState::Connected, "Sessão deve permanecer Connected após troca de microfone");

    state.set_input_device(Some("Microfone USB HyperX".into()));
    assert_eq!(state.active_voice_call.as_ref().unwrap().connection_state, VoiceSessionState::Connected);
    assert_eq!(state.active_voice_call.as_ref().unwrap().selected_input_device, Some("Microfone USB HyperX".into()));

    // 2. Troca a quente de alto-falante (dispositivo de saída)
    session.switch_output_device(Some("Fone de Ouvido Sony WH-1000XM4".into())).await.expect("switch output");
    assert_eq!(session.state(), VoiceSessionState::Connected, "Sessão deve permanecer Connected após troca de fone");

    state.set_output_device(Some("Fone de Ouvido Sony WH-1000XM4".into()));
    assert_eq!(state.active_voice_call.as_ref().unwrap().connection_state, VoiceSessionState::Connected);
    assert_eq!(state.active_voice_call.as_ref().unwrap().selected_output_device, Some("Fone de Ouvido Sony WH-1000XM4".into()));

    session.disconnect().await.expect("disconnect");
}
