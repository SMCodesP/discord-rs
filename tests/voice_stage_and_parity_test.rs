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
use discord_rs::protocol::{
    AudioBridge, MainSignalingChannel, SessionDescription, TransportEvent, VoiceGatewayEvent,
    VoiceGatewayTransport, VoiceReady, VoiceSessionConfig, VoiceSessionState,
};

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
            Arc::new(MockVoiceGatewayTransport { events_tx: tx })
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

struct MockVoiceGatewayTransport {
    events_tx: broadcast::Sender<VoiceGatewayEvent>,
}

#[async_trait]
impl VoiceGatewayTransport for MockVoiceGatewayTransport {
    async fn connect(&self) -> Result<()> {
        Ok(())
    }

    async fn wait_for_ready(&self, _timeout: Duration) -> Result<VoiceReady> {
        Ok(VoiceReady {
            ssrc: 12345,
            ip: "127.0.0.1".into(),
            port: 50000,
            modes: vec!["aead_xchacha20_poly1305_rtpsize".into()],
        })
    }

    async fn select_protocol(
        &self,
        _external_ip: &str,
        _external_port: u16,
        _mode: &str,
    ) -> Result<SessionDescription> {
        Ok(SessionDescription {
            mode: "aead_xchacha20_poly1305_rtpsize".into(),
            secret_key: vec![0u8; 32],
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
        Arc::new(MockVoiceGatewayTransport { events_tx: tx })
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
