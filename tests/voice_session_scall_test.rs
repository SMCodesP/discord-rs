//! Offline unit tests for SCall (DM voice call ringing 403, voice server caching,
//! fast close-code reporting on wait_for_ready, and secret-free VoiceDiag logs).
//!
//! Strictly network-free (Zero mocks em produção - apenas testes offline em tests/).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::broadcast;

use discord_rs::domain::{Channel, Guild, Message, User, VoiceState};
use discord_rs::protocol::{
    AudioBridge, AudioBridgeCommand, DiscordTransport, IdleTransport, MainSignalingChannel,
    Relationship, TransportEvent, VoiceGatewayConfig, VoiceGatewayEvent,
    VoiceGatewayTransport, VoiceReady, VoiceServerData, VoiceSession,
    VoiceSessionConfig, VoiceSessionState,
};
use discord_rs::protocol::client_manager::ClientManager;
use discord_rs::protocol::voice_gateway::VoiceGateway;

// ============================================================================
// Global Log Capturer for Secret Leak Assertion
// ============================================================================

static INIT_LOGGER: Once = Once::new();
static CAPTURED_LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct TestCapturingLogger;

impl log::Log for TestCapturingLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if let Ok(mut logs) = CAPTURED_LOGS.lock() {
            logs.push(format!("{}", record.args()));
        }
    }
    fn flush(&self) {}
}

fn init_test_logger() {
    INIT_LOGGER.call_once(|| {
        let _ = log::set_logger(&TestCapturingLogger);
        log::set_max_level(log::LevelFilter::Debug);
    });
}

fn clear_captured_logs() {
    if let Ok(mut logs) = CAPTURED_LOGS.lock() {
        logs.clear();
    }
}

fn get_captured_logs() -> Vec<String> {
    if let Ok(logs) = CAPTURED_LOGS.lock() {
        logs.clone()
    } else {
        Vec::new()
    }
}

// ============================================================================
// Test Transport & Signaling Mock (Delegates to IdleTransport for boilerplate)
// ============================================================================

struct MockTransport {
    idle: IdleTransport,
    event_tx: broadcast::Sender<TransportEvent>,
    voice_state_sent: Arc<tokio::sync::Mutex<Vec<(Option<String>, Option<String>, bool, bool)>>>,
    ring_calls: Arc<tokio::sync::Mutex<Vec<(String, Option<Vec<String>>)>>>,
    ring_should_fail: Arc<AtomicBool>,
    cached_server: Arc<tokio::sync::Mutex<Option<VoiceServerData>>>,
}

impl MockTransport {
    fn new() -> Self {
        let (event_tx, _) = broadcast::channel(128);
        Self {
            idle: IdleTransport::new(),
            event_tx,
            voice_state_sent: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            ring_calls: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            ring_should_fail: Arc::new(AtomicBool::new(false)),
            cached_server: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

#[async_trait]
impl DiscordTransport for MockTransport {
    async fn connect(&self) -> Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn get_current_user(&self) -> Result<User> {
        self.idle.get_current_user().await
    }

    async fn list_guilds(&self) -> Result<Vec<Guild>> {
        self.idle.list_guilds().await
    }

    async fn get_guild(&self, guild_id: &str) -> Result<Option<Guild>> {
        self.idle.get_guild(guild_id).await
    }

    async fn get_guild_emojis(&self, guild_id: &str) -> Result<Vec<discord_rs::protocol::media_cache::DiscordEmoji>> {
        self.idle.get_guild_emojis(guild_id).await
    }

    async fn get_guild_stickers(&self, guild_id: &str) -> Result<Vec<discord_rs::protocol::media_cache::DiscordSticker>> {
        self.idle.get_guild_stickers(guild_id).await
    }

    async fn list_channels(&self, guild_id: &str) -> Result<Vec<Channel>> {
        self.idle.list_channels(guild_id).await
    }

    async fn list_dm_channels(&self) -> Result<Vec<Channel>> {
        self.idle.list_dm_channels().await
    }

    async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<Message>> {
        self.idle.get_messages(channel_id, limit).await
    }

    async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        self.idle.send_message(channel_id, content).await
    }

    async fn send_sticker(&self, channel_id: &str, sticker_id: &str) -> Result<Message> {
        self.idle.send_sticker(channel_id, sticker_id).await
    }

    async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        self.idle.delete_message(channel_id, message_id).await
    }

    async fn add_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        self.idle.add_reaction(channel_id, message_id, emoji).await
    }

    async fn remove_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        self.idle.remove_reaction(channel_id, message_id, emoji).await
    }

    async fn list_relationships(&self) -> Result<Vec<Relationship>> {
        self.idle.list_relationships().await
    }

    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        let mut list = self.voice_state_sent.lock().await;
        list.push((
            guild_id.map(String::from),
            channel_id.map(String::from),
            self_mute,
            self_deaf,
        ));
        Ok(())
    }

    async fn ring_call(
        &self,
        channel_id: &str,
        recipients: Option<Vec<String>>,
    ) -> Result<()> {
        let mut calls = self.ring_calls.lock().await;
        calls.push((channel_id.to_string(), recipients));

        if self.ring_should_fail.load(Ordering::SeqCst) {
            anyhow::bail!("Discord API returned HTTP 403 Forbidden: {{\"message\": \"Missing Permissions\", \"code\": 50013}}");
        }
        Ok(())
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }
}

#[async_trait]
impl MainSignalingChannel for MockTransport {
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        <Self as DiscordTransport>::send_voice_state_update(self, guild_id, channel_id, self_mute, self_deaf).await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }

    async fn current_voice_server(&self) -> Option<VoiceServerData> {
        self.cached_server.lock().await.clone()
    }

    async fn clear_voice_server(&self) {
        let mut s = self.cached_server.lock().await;
        *s = None;
    }
}

// Dummy AudioBridge for offline VoiceSession tests
fn dummy_audio_bridge() -> AudioBridge {
    let (_out_tx, out_rx) = tokio::sync::mpsc::channel(16);
    let (_spk_tx, spk_rx) = tokio::sync::mpsc::channel(16);
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(16);

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                AudioBridgeCommand::StartSession { responder } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    AudioBridge::from_channels(out_rx, spk_rx, cmd_tx)
}

// ============================================================================
// TEST 1: Toque só é chamado DEPOIS da confirmação de entrada (VoiceStateConfirmed)
//         Se a confirmação não vier, o toque NÃO é chamado.
// ============================================================================

#[tokio::test]
async fn test_ring_called_only_after_voice_state_confirmed() {
    init_test_logger();
    let transport = Arc::new(MockTransport::new());
    let (cm, _rx) = ClientManager::new_with_transport(transport.clone());

    let (gw_event_tx, _) = broadcast::channel(16);
    let gw_tx = gw_event_tx.clone();

    // Custom voice gateway factory that hangs in wait_for_ready until test finishes
    cm.set_voice_gateway_factory(Some(Arc::new(move |_cfg| {
        struct HangingGateway {
            event_tx: broadcast::Sender<VoiceGatewayEvent>,
        }
        #[async_trait]
        impl VoiceGatewayTransport for HangingGateway {
            async fn connect(&self) -> Result<()> { Ok(()) }
            async fn wait_for_ready(&self, _timeout: Duration) -> Result<VoiceReady> {
                tokio::time::sleep(Duration::from_secs(60)).await;
                anyhow::bail!("Timeout")
            }
            async fn select_protocol(&self, _: &str, _: u16, _: &str) -> Result<discord_rs::protocol::SessionDescription> {
                anyhow::bail!("not reached")
            }
            async fn set_speaking(&self, _: bool, _: u32, _: u32) -> Result<()> { Ok(()) }
            fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
                self.event_tx.subscribe()
            }
            async fn disconnect(&self) -> Result<()> { Ok(()) }
        }
        Arc::new(HangingGateway { event_tx: gw_tx.clone() })
    }))).await;

    let target_channel_id = 987654321u64;
    let target_user_id = 123456789u64;
    let recips = vec!["555444333".to_string()];

    // Inicia sessão de voz com ring_recipients
    let session = cm
        .start_voice_session(
            None, // DM call
            target_channel_id,
            target_user_id,
            false,
            false,
            Some(recips.clone()),
        )
        .await
        .expect("start_voice_session should succeed");

    // Aguarda um instante para garantir que Opcode 4 foi despachado
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 1. ANTES da confirmação de entrada: toque NUNCA pode ter saído!
    {
        let calls = transport.ring_calls.lock().await;
        assert_eq!(
            calls.len(),
            0,
            "ring_call NÃO deve ser chamado antes da confirmação de entrada"
        );
    }

    // 2. Agora despachamos o evento de confirmação de entrada do nosso usuário na chamada
    let mut my_voice_state = VoiceState::new(&target_user_id.to_string(), "sess-abc-123");
    my_voice_state.channel_id = Some(target_channel_id.to_string());
    my_voice_state.guild_id = None;
    transport
        .event_tx
        .send(TransportEvent::VoiceStateUpdate(my_voice_state))
        .expect("send VoiceStateUpdate");

    // Aguarda o background task processar o evento de confirmação e disparar ring_call
    tokio::time::sleep(Duration::from_millis(80)).await;

    // 3. APÓS confirmação de entrada: ring_call DEVE ter sido disparado exatamente 1 vez com os destinatários!
    {
        let calls = transport.ring_calls.lock().await;
        assert_eq!(
            calls.len(),
            1,
            "ring_call DEVE ser chamado após confirmação de entrada"
        );
        assert_eq!(calls[0].0, target_channel_id.to_string());
        assert_eq!(calls[0].1, Some(recips));
    }

    let _ = session.disconnect().await;
}

// Subteste: Se a confirmação de entrada nunca vier, o toque NUNCA é chamado.
#[tokio::test]
async fn test_ring_never_called_if_voice_state_confirmation_does_not_arrive() {
    init_test_logger();
    let transport = Arc::new(MockTransport::new());
    let (cm, _rx) = ClientManager::new_with_transport(transport.clone());

    let target_channel_id = 111222333u64;
    let target_user_id = 444555666u64;
    let recips = vec!["999888777".to_string()];

    let session = cm
        .start_voice_session(
            None,
            target_channel_id,
            target_user_id,
            false,
            false,
            Some(recips),
        )
        .await
        .expect("start_voice_session should succeed");

    // Espera tempo significativo sem despachar evento de entrada
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Encerra a sessão
    let _ = session.disconnect().await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Confirma que ring_call NUNCA foi chamado
    let calls = transport.ring_calls.lock().await;
    assert_eq!(
        calls.len(),
        0,
        "ring_call NUNCA deve ser disparado se a entrada não for confirmada"
    );
}

// ============================================================================
// TEST 2: Toque com erro NÃO muda o estado da sessão para falha
// ============================================================================

#[tokio::test]
async fn test_ring_error_does_not_fail_session() {
    init_test_logger();
    let transport = Arc::new(MockTransport::new());
    transport.ring_should_fail.store(true, Ordering::SeqCst);

    let (cm, _rx) = ClientManager::new_with_transport(transport.clone());
    let _event_sub = transport.event_tx.subscribe();

    // Custom voice gateway factory that hangs in wait_for_ready so session stays in Connecting
    cm.set_voice_gateway_factory(Some(Arc::new(move |_cfg| {
        struct HangingGateway;
        #[async_trait]
        impl VoiceGatewayTransport for HangingGateway {
            async fn connect(&self) -> Result<()> { Ok(()) }
            async fn wait_for_ready(&self, _timeout: Duration) -> Result<VoiceReady> {
                tokio::time::sleep(Duration::from_secs(60)).await;
                anyhow::bail!("Timeout")
            }
            async fn select_protocol(&self, _: &str, _: u16, _: &str) -> Result<discord_rs::protocol::SessionDescription> {
                anyhow::bail!("not reached")
            }
            async fn set_speaking(&self, _: bool, _: u32, _: u32) -> Result<()> { Ok(()) }
            fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
                let (_, rx) = broadcast::channel(1);
                rx
            }
            async fn disconnect(&self) -> Result<()> { Ok(()) }
        }
        Arc::new(HangingGateway)
    }))).await;

    let target_channel_id = 777888999u64;
    let target_user_id = 111222333u64;
    let recips = vec!["444333222".to_string()];

    let session = cm
        .start_voice_session(
            None,
            target_channel_id,
            target_user_id,
            false,
            false,
            Some(recips),
        )
        .await
        .expect("start_voice_session should succeed");

    // Aguarda um instante para garantir que Opcode 4 foi despachado e session subscreveu eventos
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Despacha confirmação de entrada
    let mut my_voice_state = VoiceState::new(&target_user_id.to_string(), "sess-err-ring");
    my_voice_state.channel_id = Some(target_channel_id.to_string());
    transport
        .event_tx
        .send(TransportEvent::VoiceStateUpdate(my_voice_state))
        .expect("send VoiceStateUpdate");

    // Aguarda o processamento do ring_call com falha
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 1. O toque foi tentado
    {
        let calls = transport.ring_calls.lock().await;
        assert_eq!(calls.len(), 1, "ring_call deve ter sido tentado");
    }

    // 2. A sessão NÃO foi derrubada nem marcada como falha fatal!
    assert!(
        !matches!(session.state(), VoiceSessionState::Disconnected { .. }),
        "A sessão NÃO deve ir para Disconnected devido à falha no toque"
    );

    // 3. O aviso não-fatal amigável foi registrado na sessão
    let warning = session.call_warning();
    assert_eq!(
        warning,
        Some("Não foi possível tocar a chamada".to_string()),
        "Aviso não-fatal deve ser registrado na sessão com mensagem amigável"
    );

    let _ = session.disconnect().await;
}

// ============================================================================
// TEST 3: Segunda sessão NÃO usa servidor de voz da primeira
// ============================================================================

#[tokio::test]
async fn test_second_session_does_not_use_first_session_voice_server() {
    init_test_logger();
    let transport = Arc::new(MockTransport::new());

    // Configura um servidor antigo em cache no signaling (como se fosse de uma chamada anterior)
    *transport.cached_server.lock().await = Some(VoiceServerData {
        token: "stale-token-111".to_string(),
        guild_id: None,
        channel_id: Some("old-channel-999".to_string()),
        endpoint: Some("stale-voice.discord.gg".to_string()),
    });

    let config1 = VoiceSessionConfig {
        guild_id: None,
        channel_id: 111111,
        user_id: 999999,
        self_mute: false,
        self_deaf: false,
    };

    let session1 = VoiceSession::new(config1, transport.clone(), dummy_audio_bridge());

    // Inicia sessão 1
    let (gw1_created_tx, mut gw1_created_rx) = tokio::sync::mpsc::channel::<VoiceGatewayConfig>(1);
    let session1 = session1.with_gateway_factory(move |cfg| {
        let _ = gw1_created_tx.try_send(cfg);
        struct DummyGw;
        #[async_trait]
        impl VoiceGatewayTransport for DummyGw {
            async fn connect(&self) -> Result<()> { Ok(()) }
            async fn wait_for_ready(&self, _: Duration) -> Result<VoiceReady> {
                tokio::time::sleep(Duration::from_secs(60)).await;
                anyhow::bail!("timeout")
            }
            async fn select_protocol(&self, _: &str, _: u16, _: &str) -> Result<discord_rs::protocol::SessionDescription> {
                anyhow::bail!("timeout")
            }
            async fn set_speaking(&self, _: bool, _: u32, _: u32) -> Result<()> { Ok(()) }
            fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
                let (_, rx) = broadcast::channel(1);
                rx
            }
            async fn disconnect(&self) -> Result<()> { Ok(()) }
        }
        Arc::new(DummyGw)
    });

    session1.start().await.expect("start session1");

    // Aguarda um pouco: a sessão 1 NÃO pode ter criado gateway usando o stale-token porque
    // não aceita servidor em cache de antes do Opcode 4 e com outro canal!
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        gw1_created_rx.try_recv().is_err(),
        "Sessão 1 NÃO deve criar gateway usando token/servidor stale"
    );

    // Agora despacha os dados reais para a sessão 1
    let mut vs1 = VoiceState::new("999999", "session-id-1");
    vs1.channel_id = Some("111111".to_string());
    let _ = transport.event_tx.send(TransportEvent::VoiceStateUpdate(vs1));

    let _ = transport.event_tx.send(TransportEvent::VoiceServerUpdate {
        token: "fresh-token-session1".to_string(),
        guild_id: None,
        channel_id: Some("111111".to_string()),
        endpoint: Some("fresh1.discord.gg".to_string()),
    });

    // Sessão 1 deve receber o seu servidor correto
    let gw1_cfg = tokio::time::timeout(Duration::from_millis(200), gw1_created_rx.recv())
        .await
        .expect("timeout waiting for gw1")
        .expect("gw1 created");
    assert_eq!(gw1_cfg.token, "fresh-token-session1");
    assert_eq!(gw1_cfg.session_id, "session-id-1");

    // Desconecta sessão 1 (isso DEVE limpar o cache de voice server)
    session1.disconnect().await.expect("disconnect session1");
    assert!(
        transport.cached_server.lock().await.is_none(),
        "Desconexão deve limpar o cache de voice server"
    );

    // ------------------------------------------------------------------------
    // Cria Sessão 2 para outro canal (channel_id 222222)
    // ------------------------------------------------------------------------
    let config2 = VoiceSessionConfig {
        guild_id: None,
        channel_id: 222222,
        user_id: 999999,
        self_mute: false,
        self_deaf: false,
    };

    let session2 = VoiceSession::new(config2, transport.clone(), dummy_audio_bridge());
    let (gw2_created_tx, mut gw2_created_rx) = tokio::sync::mpsc::channel::<VoiceGatewayConfig>(1);
    let session2 = session2.with_gateway_factory(move |cfg| {
        let _ = gw2_created_tx.try_send(cfg);
        struct DummyGw;
        #[async_trait]
        impl VoiceGatewayTransport for DummyGw {
            async fn connect(&self) -> Result<()> { Ok(()) }
            async fn wait_for_ready(&self, _: Duration) -> Result<VoiceReady> {
                tokio::time::sleep(Duration::from_secs(60)).await;
                anyhow::bail!("timeout")
            }
            async fn select_protocol(&self, _: &str, _: u16, _: &str) -> Result<discord_rs::protocol::SessionDescription> {
                anyhow::bail!("timeout")
            }
            async fn set_speaking(&self, _: bool, _: u32, _: u32) -> Result<()> { Ok(()) }
            fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
                let (_, rx) = broadcast::channel(1);
                rx
            }
            async fn disconnect(&self) -> Result<()> { Ok(()) }
        }
        Arc::new(DummyGw)
    });

    session2.start().await.expect("start session2");

    // Garante que a sessão 2 NÃO pega dados da sessão 1
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        gw2_created_rx.try_recv().is_err(),
        "Sessão 2 NÃO deve reaproveitar dados da sessão 1"
    );

    // Despacha evento de outro canal (111111) para testar o filtro de canal
    let _ = transport.event_tx.send(TransportEvent::VoiceServerUpdate {
        token: "token-para-canal-antigo".to_string(),
        guild_id: None,
        channel_id: Some("111111".to_string()),
        endpoint: Some("endpoint-errado.discord.gg".to_string()),
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        gw2_created_rx.try_recv().is_err(),
        "Sessão 2 DEVE ignorar VoiceServerUpdate direcionado para canal diferente"
    );

    // Agora despacha os dados exclusivos da Sessão 2
    let mut vs2 = VoiceState::new("999999", "session-id-2");
    vs2.channel_id = Some("222222".to_string());
    let _ = transport.event_tx.send(TransportEvent::VoiceStateUpdate(vs2));

    let _ = transport.event_tx.send(TransportEvent::VoiceServerUpdate {
        token: "fresh-token-session2".to_string(),
        guild_id: None,
        channel_id: Some("222222".to_string()),
        endpoint: Some("fresh2.discord.gg".to_string()),
    });

    let gw2_cfg = tokio::time::timeout(Duration::from_millis(200), gw2_created_rx.recv())
        .await
        .expect("timeout waiting for gw2")
        .expect("gw2 created");
    assert_eq!(gw2_cfg.token, "fresh-token-session2");
    assert_eq!(gw2_cfg.session_id, "session-id-2");

    let _ = session2.disconnect().await;
}

// ============================================================================
// TEST 4: Fechamento com código antes do Ready devolve erro em menos de 1 s
// ============================================================================

#[tokio::test]
async fn test_voice_gateway_close_code_before_ready_returns_under_1_second() {
    init_test_logger();
    let gw_config = VoiceGatewayConfig {
        endpoint: "voice.discord.media".to_string(),
        server_id: "server-123".to_string(),
        user_id: "user-456".to_string(),
        session_id: "sess-789".to_string(),
        token: "token-abc".to_string(),
        max_dave_protocol_version: Some(1),
    };

    let gw = VoiceGateway::new(gw_config);

    // Dispara fechamento com código 4004 (Token não reconhecido) após 30ms
    let gw_clone = Arc::new(gw);
    let gw_task = gw_clone.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        gw_task.inject_close(Some(4004), "Token não reconhecido");
    });

    let start = Instant::now();
    let result = gw_clone.wait_for_ready(Duration::from_secs(10)).await;
    let elapsed = start.elapsed();

    // Deve retornar em muito menos de 1 segundo (não esperar os 10s)
    assert!(
        elapsed < Duration::from_secs(1),
        "wait_for_ready demorou {:?}, deve retornar em menos de 1 segundo",
        elapsed
    );

    // O erro deve conter o código 4004 e a descrição
    assert!(result.is_err(), "wait_for_ready deve retornar erro ao fechar");
    let err_str = result.err().unwrap().to_string();
    assert!(
        err_str.contains("4004"),
        "Erro deve conter código 4004. Obtido: {}",
        err_str
    );
    assert!(
        err_str.contains("Authentication failed: token is invalid"),
        "Erro deve conter descrição de fechamento. Obtido: {}",
        err_str
    );
}

// Subteste: Fechamento que já ocorreu ANTES de wait_for_ready ser chamado
#[tokio::test]
async fn test_voice_gateway_already_closed_returns_immediately() {
    init_test_logger();
    let gw_config = VoiceGatewayConfig {
        endpoint: "voice.discord.media".to_string(),
        server_id: "server-123".to_string(),
        user_id: "user-456".to_string(),
        session_id: "sess-789".to_string(),
        token: "token-abc".to_string(),
        max_dave_protocol_version: Some(1),
    };

    let gw = VoiceGateway::new(gw_config);
    // Injeta fechamento 4014 (Canal de voz excluído ou expulso) imediatamente
    gw.inject_close(Some(4014), "Desconectado do canal");

    let start = Instant::now();
    let result = gw.wait_for_ready(Duration::from_secs(10)).await;
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_millis(100), "Deve retornar instantaneamente");
    assert!(result.is_err());
    let err_str = result.err().unwrap().to_string();
    assert!(err_str.contains("4014"));
}

// ============================================================================
// TEST 5: Log NÃO contém segredos (token, session_id, IP, ids de snowflake)
// ============================================================================

#[tokio::test]
async fn test_voice_logs_do_not_contain_secrets() {
    init_test_logger();
    clear_captured_logs();

    // Valores sintéticos secretos
    let secret_session_id = "synthetic_secret_session_id_xyz999";
    let secret_token = "synthetic_secret_token_abc888";
    let secret_ip = "198.51.100.77";
    let secret_user_id = 998877665544332211u64;
    let secret_channel_id = 112233445566778899u64;
    let secret_user_str = secret_user_id.to_string();
    let secret_channel_str = secret_channel_id.to_string();

    let transport = Arc::new(MockTransport::new());

    let config = VoiceSessionConfig {
        guild_id: None,
        channel_id: secret_channel_id,
        user_id: secret_user_id,
        self_mute: false,
        self_deaf: false,
    };

    let session = VoiceSession::new(config, transport.clone(), dummy_audio_bridge());

    let session = session.with_gateway_factory(move |_cfg| {
        struct MockDiagGateway {
            event_tx: broadcast::Sender<VoiceGatewayEvent>,
        }
        #[async_trait]
        impl VoiceGatewayTransport for MockDiagGateway {
            async fn connect(&self) -> Result<()> {
                let _ = self.event_tx.send(VoiceGatewayEvent::Connected);
                Ok(())
            }
            async fn wait_for_ready(&self, _: Duration) -> Result<VoiceReady> {
                Ok(VoiceReady {
                    ssrc: 12345,
                    ip: "127.0.0.1".to_string(),
                    port: 50000,
                    modes: vec!["aead_aes256_gcm_rtpsize".to_string()],
                })
            }
            async fn select_protocol(&self, _: &str, _: u16, mode: &str) -> Result<discord_rs::protocol::SessionDescription> {
                Ok(discord_rs::protocol::SessionDescription {
                    mode: mode.to_string(),
                    secret_key: vec![0u8; 32],
                    dave_protocol_version: None,
                })
            }
            async fn set_speaking(&self, _: bool, _: u32, _: u32) -> Result<()> { Ok(()) }
            fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
                self.event_tx.subscribe()
            }
            async fn disconnect(&self) -> Result<()> { Ok(()) }
        }
        let (tx, _) = broadcast::channel(16);
        Arc::new(MockDiagGateway { event_tx: tx })
    });

    session.start().await.expect("start session");

    // Despacha eventos com dados sintéticos secretos
    let mut vs = VoiceState::new(&secret_user_str, secret_session_id);
    vs.channel_id = Some(secret_channel_str.clone());
    let _ = transport.event_tx.send(TransportEvent::VoiceStateUpdate(vs));

    let _ = transport.event_tx.send(TransportEvent::VoiceServerUpdate {
        token: secret_token.to_string(),
        guild_id: None,
        channel_id: Some(secret_channel_str.clone()),
        endpoint: Some(format!("{}:443", secret_ip)),
    });

    // Aguarda um instante para que todos os logs assíncronos sejam gravados
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = session.disconnect().await;

    // Coleta todos os logs capturados durante o fluxo
    let logs = get_captured_logs();
    assert!(!logs.is_empty(), "Logs devem ter sido capturados durante o fluxo de voz");

    // Verifica que logs de diagnóstico estruturados [VoiceDiag] foram emitidos
    let has_voice_diag = logs.iter().any(|l| l.contains("[VoiceDiag]"));
    assert!(has_voice_diag, "Logs devem conter entradas estruturadas [VoiceDiag]");

    // REGRA 26: Nenhum segredo pode aparecer em nenhuma linha de log!
    for line in &logs {
        assert!(
            !line.contains(secret_token),
            "VIOLAÇÃO DE SEGURANÇA: Token secreto vazou no log: {}",
            line
        );
        assert!(
            !line.contains(secret_session_id),
            "VIOLAÇÃO DE SEGURANÇA: Session ID secreto vazou no log: {}",
            line
        );
        assert!(
            !line.contains(secret_ip),
            "VIOLAÇÃO DE SEGURANÇA: IP secreto vazou no log: {}",
            line
        );
        assert!(
            !line.contains(&secret_user_str),
            "VIOLAÇÃO DE SEGURANÇA: User Snowflake secreto vazou no log: {}",
            line
        );
        assert!(
            !line.contains(&secret_channel_str),
            "VIOLAÇÃO DE SEGURANÇA: Channel Snowflake secreto vazou no log: {}",
            line
        );
    }
}
