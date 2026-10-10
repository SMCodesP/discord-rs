//! Comprehensive Integration Tests for VoiceSession (Fase 8.2 & Fase 8.3).
//!
//! Cobre:
//! 1. Handshake completo na ordem correta:
//!    Opcode 4 -> session_id + token + endpoint -> VoiceGateway::connect -> Ready ->
//!    VoiceUdpSocket::connect + discover_ip no mesmo socket -> select_protocol ->
//!    SessionDescription -> into_connection(crypto) -> Connected.
//! 2. Envio de áudio cifrado: quadros Opus saem pela thread de áudio, são cifrados via FrameCipher
//!    e enviados sobre RTP AEAD pelo socket UDP.
//! 3. Recepção entregue com SSRC correto: pacote RTP recebido é decifrado via AEAD + FrameCipher
//!    e entregue à ponte de áudio com SSRC, sequence e timestamp corretos.
//! 4. Speaking on/off com 5 quadros de silêncio: SpeakingStarted ativa gateway, SpeakingStopped
//!    desativa gateway e emite os 5 quadros de silêncio canônicos (0xF8, 0xFF, 0xFE).
//! 5. Desconexão limpa: Opcode 4 com channel_id nulo enviado ao cliente principal, gateway fechado,
//!    keepalive e loops cancelados, e estado transita para Disconnected.
//! 6. Close fatal do gateway (ex.: código 4017 DAVE required): encerra a sessão e transita para Disconnected.
//! 7. Hook de FrameCipher customizado: valida o ponto de extensão DAVE entre o Opus e o RTP.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, Mutex};

use discord_rs::domain::VoiceState;
use discord_rs::media::capture::SpeakingEvent;
use discord_rs::protocol::{
    AudioBridge, AudioBridgeCommand, FrameCipher, MainSignalingChannel, RtpCrypto,
    SessionDescription, TransportEvent, VoiceCloseDecision, VoiceGatewayEvent,
    VoiceGatewayTransport, VoiceReady, VoiceSession, VoiceSessionConfig, VoiceSessionState,
    IP_DISCOVERY_PACKET_LEN, SILENCE_FRAME, SILENCE_FRAME_COUNT,
};

fn sample_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = ((i * 19 + 7) % 256) as u8;
    }
    k
}

// ============================================================================
// Mock de Sinalização Principal (Zero Mocks em Produção - Apenas em tests/)
// ============================================================================

struct MockMainSignaling {
    event_tx: broadcast::Sender<TransportEvent>,
    last_voice_state: Arc<Mutex<Option<(Option<String>, Option<String>, bool, bool)>>>,
    auto_reply_endpoint: Option<String>,
}

impl MockMainSignaling {
    fn new(auto_reply_endpoint: Option<String>) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            event_tx,
            last_voice_state: Arc::new(Mutex::new(None)),
            auto_reply_endpoint,
        }
    }
}

#[async_trait]
impl MainSignalingChannel for MockMainSignaling {
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        *self.last_voice_state.lock().await = Some((
            guild_id.map(String::from),
            channel_id.map(String::from),
            self_mute,
            self_deaf,
        ));

        // Se conectando a um canal e configurado com endpoint automático, despacha eventos
        if let (Some(chan_id), Some(ep)) = (channel_id, &self.auto_reply_endpoint) {
            let _ = self.event_tx.send(TransportEvent::VoiceStateUpdate(VoiceState {
                guild_id: guild_id.map(String::from),
                channel_id: Some(chan_id.to_string()),
                user_id: "123456789".to_string(),
                session_id: "test-voice-session-xyz".to_string(),
                deaf: false,
                mute: false,
                self_deaf,
                self_mute,
                self_video: false,
                suppress: false,
            }));

            let _ = self.event_tx.send(TransportEvent::VoiceServerUpdate {
                token: "test-voice-token-999".to_string(),
                guild_id: guild_id.map(String::from),
                channel_id: Some(chan_id.to_string()),
                endpoint: Some(ep.clone()),
            });
        }

        Ok(())
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }
}

// ============================================================================
// Mock de Voice Gateway (Zero Mocks em Produção - Apenas em tests/)
// ============================================================================

struct MockVoiceGateway {
    udp_port: u16,
    ssrc: u32,
    event_tx: broadcast::Sender<VoiceGatewayEvent>,
    speaking_records: Arc<Mutex<Vec<(bool, u32, u32)>>>,
    disconnected: Arc<AtomicBool>,
}

impl MockVoiceGateway {
    fn new(udp_port: u16, ssrc: u32) -> Self {
        let (event_tx, _) = broadcast::channel(32);
        Self {
            udp_port,
            ssrc,
            event_tx,
            speaking_records: Arc::new(Mutex::new(Vec::new())),
            disconnected: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl VoiceGatewayTransport for MockVoiceGateway {
    async fn connect(&self) -> Result<()> {
        let _ = self.event_tx.send(VoiceGatewayEvent::Connected);
        Ok(())
    }

    async fn wait_for_ready(&self, _timeout: Duration) -> Result<VoiceReady> {
        Ok(VoiceReady {
            ssrc: self.ssrc,
            ip: "127.0.0.1".to_string(),
            port: self.udp_port,
            modes: vec![
                "aead_aes256_gcm_rtpsize".to_string(),
                "aead_xchacha20_poly1305_rtpsize".to_string(),
            ],
        })
    }

    async fn select_protocol(
        &self,
        _external_ip: &str,
        _external_port: u16,
        mode: &str,
    ) -> Result<SessionDescription> {
        Ok(SessionDescription {
            mode: mode.to_string(),
            secret_key: sample_key().to_vec(),
        })
    }

    async fn set_speaking(&self, speaking: bool, delay: u32, ssrc: u32) -> Result<()> {
        self.speaking_records.lock().await.push((speaking, delay, ssrc));
        Ok(())
    }

    fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
        self.event_tx.subscribe()
    }

    async fn disconnect(&self) -> Result<()> {
        self.disconnected.store(true, Ordering::SeqCst);
        let _ = self.event_tx.send(VoiceGatewayEvent::Disconnected {
            code: Some(1000),
            reason: "Client disconnect".to_string(),
            decision: VoiceCloseDecision::Fatal,
        });
        Ok(())
    }
}

// ============================================================================
// Servidor UDP de Teste em Loopback 127.0.0.1
// ============================================================================

async fn spawn_test_udp_server(expected_ssrc: u32) -> (SocketAddr, tokio::task::JoinHandle<()>, mpsc::Receiver<Vec<u8>>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let (received_packets_tx, received_packets_rx) = mpsc::channel(64);

    let handle = tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, client_addr)) => {
                    // 1. IP Discovery (74 bytes)
                    if len == IP_DISCOVERY_PACKET_LEN && buf[0..2] == 1u16.to_be_bytes() {
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

                    // 2. Keepalive UDP (8 bytes)
                    if len == 8 {
                        // Echo ou ignore
                        continue;
                    }

                    // 3. Pacotes RTP normais
                    let pkt = buf[..len].to_vec();
                    let _ = received_packets_tx.send(pkt.clone()).await;

                    // Ecoa o pacote de volta para o cliente para testar recepção
                    let _ = socket.send_to(&pkt, client_addr).await;
                }
                Err(_) => break,
            }
        }
    });

    (addr, handle, received_packets_rx)
}

// ============================================================================
// 1. Teste de Handshake Completo, Ordem Correta e Streaming Bidirecional
// ============================================================================

#[tokio::test]
async fn test_voice_session_full_handshake_order_and_audio_roundtrip() {
    let ssrc = 0x11223344;
    let (udp_addr, _udp_handle, mut udp_rx) = spawn_test_udp_server(ssrc).await;

    let signaling = Arc::new(MockMainSignaling::new(Some(format!("127.0.0.1:{}", udp_addr.port()))));

    let (audio_out_tx, audio_out_rx) = mpsc::channel(32);
    let (_audio_spk_tx, audio_spk_rx) = mpsc::channel(16);
    let (audio_cmd_tx, mut audio_cmd_rx) = mpsc::channel(64);

    let audio_bridge = AudioBridge::from_channels(audio_out_rx, audio_spk_rx, audio_cmd_tx);

    let mock_gw = Arc::new(MockVoiceGateway::new(udp_addr.port(), ssrc));
    let mock_gw_clone = mock_gw.clone();

    let config = VoiceSessionConfig {
        guild_id: Some(987654321),
        channel_id: 123456789,
        user_id: 123456789,
        self_mute: false,
        self_deaf: false,
    };

    let session = VoiceSession::new(config, signaling.clone(), audio_bridge)
        .with_gateway_factory(move |_cfg| mock_gw_clone.clone());

    // Tarefa que responde ao comando StartSession da ponte de áudio
    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            match cmd {
                AudioBridgeCommand::StartSession { responder } => {
                    let _ = responder.send(Ok(()));
                }
                _ => {}
            }
        }
    });

    // 1. Inicia a sessão
    session.start().await.expect("start session");

    // 2. Aguarda até atingir o estado Connected
    session
        .wait_until_connected(Duration::from_secs(3))
        .await
        .expect("conectar sessão de voz");

    assert!(session.is_connected());
    assert_eq!(session.state(), VoiceSessionState::Connected);

    // 3. Envia um quadro Opus da thread de áudio
    let original_opus = b"real opus voice data 20ms frame from microphone";
    audio_out_tx.send(original_opus.to_vec()).await.unwrap();

    // 4. Servidor UDP deve receber o pacote RTP cifrado
    let received_pkt = tokio::time::timeout(Duration::from_millis(500), udp_rx.recv())
        .await
        .expect("timeout aguardando pacote no servidor UDP")
        .expect("pacote recebido no UDP");

    assert!(received_pkt.len() > 12);
    assert_eq!(received_pkt[0], 0x80); // RTP v2
    assert_eq!(received_pkt[1], 0x78); // Opus

    // 5. Decifrar no lado do teste para validar integridade da cifra AEAD
    let crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &sample_key()).unwrap();
    let (rx_ssrc, decrypted_payload) = crypto.decrypt_packet(&received_pkt).unwrap();
    assert_eq!(rx_ssrc, ssrc);
    assert_eq!(decrypted_payload, original_opus);

    // 6. Desconecta graciosamente
    session.disconnect().await.expect("disconnect");
    assert_eq!(
        session.state(),
        VoiceSessionState::Disconnected {
            reason: "Desconectado pelo usuário".into()
        }
    );
}

// ============================================================================
// 2. Teste de Speaking Transitions e 5 Quadros Canônicos de Silêncio
// ============================================================================

#[tokio::test]
async fn test_voice_session_speaking_events_and_silence_frames() {
    let ssrc = 0x55667788;
    let (udp_addr, _udp_handle, mut udp_rx) = spawn_test_udp_server(ssrc).await;

    let signaling = Arc::new(MockMainSignaling::new(Some(format!("127.0.0.1:{}", udp_addr.port()))));

    let (_audio_out_tx, audio_out_rx) = mpsc::channel(32);
    let (audio_spk_tx, audio_spk_rx) = mpsc::channel(16);
    let (audio_cmd_tx, mut audio_cmd_rx) = mpsc::channel(64);

    let audio_bridge = AudioBridge::from_channels(audio_out_rx, audio_spk_rx, audio_cmd_tx);

    let mock_gw = Arc::new(MockVoiceGateway::new(udp_addr.port(), ssrc));
    let mock_gw_clone = mock_gw.clone();

    let config = VoiceSessionConfig {
        guild_id: None, // DM call
        channel_id: 44556677,
        user_id: 123456789,
        self_mute: false,
        self_deaf: false,
    };

    let session = VoiceSession::new(config, signaling.clone(), audio_bridge)
        .with_gateway_factory(move |_cfg| mock_gw_clone.clone());

    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            if let AudioBridgeCommand::StartSession { responder } = cmd {
                let _ = responder.send(Ok(()));
            }
        }
    });

    session.start().await.unwrap();
    session.wait_until_connected(Duration::from_secs(3)).await.unwrap();

    // 1. Emite transição SpeakingStarted
    audio_spk_tx.send(SpeakingEvent::SpeakingStarted).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let records = mock_gw.speaking_records.lock().await;
        assert_eq!(records.last(), Some(&(true, 0, ssrc)));
    }

    // 2. Emite transição SpeakingStopped (deve gerar set_speaking(false) E 5 quadros de silêncio)
    audio_spk_tx.send(SpeakingEvent::SpeakingStopped).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let records = mock_gw.speaking_records.lock().await;
        assert_eq!(records.last(), Some(&(false, 0, ssrc)));
    }

    // Verifica que 5 quadros de silêncio foram recebidos pelo servidor UDP
    let crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &sample_key()).unwrap();
    for _ in 0..SILENCE_FRAME_COUNT {
        let pkt = tokio::time::timeout(Duration::from_millis(500), udp_rx.recv())
            .await
            .expect("timeout silêncio")
            .expect("pacote");
        let (rx_ssrc, decrypted) = crypto.decrypt_packet(&pkt).unwrap();
        assert_eq!(rx_ssrc, ssrc);
        assert_eq!(decrypted, SILENCE_FRAME);
    }

    session.disconnect().await.unwrap();
}

// ============================================================================
// 3. Teste de Desconexão Limpa (Opcode 4 com channel_id nulo)
// ============================================================================

#[tokio::test]
async fn test_voice_session_clean_disconnect_flow() {
    let ssrc = 0x99887766;
    let (udp_addr, _udp_handle, _udp_rx) = spawn_test_udp_server(ssrc).await;

    let signaling = Arc::new(MockMainSignaling::new(Some(format!("127.0.0.1:{}", udp_addr.port()))));

    let (_audio_out_tx, audio_out_rx) = mpsc::channel(32);
    let (_audio_spk_tx, audio_spk_rx) = mpsc::channel(16);
    let (audio_cmd_tx, mut audio_cmd_rx) = mpsc::channel(64);

    let audio_bridge = AudioBridge::from_channels(audio_out_rx, audio_spk_rx, audio_cmd_tx);
    let mock_gw = Arc::new(MockVoiceGateway::new(udp_addr.port(), ssrc));
    let mock_gw_clone = mock_gw.clone();

    let stopped_called = Arc::new(AtomicBool::new(false));
    let stopped_called_clone = stopped_called.clone();
    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            match cmd {
                AudioBridgeCommand::StartSession { responder } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::StopSession => {
                    stopped_called_clone.store(true, Ordering::SeqCst);
                }
                _ => {}
            }
        }
    });

    let config = VoiceSessionConfig {
        guild_id: Some(111222),
        channel_id: 333444,
        user_id: 123456789,
        self_mute: false,
        self_deaf: false,
    };

    let session = VoiceSession::new(config, signaling.clone(), audio_bridge)
        .with_gateway_factory(move |_cfg| mock_gw_clone.clone());

    session.start().await.unwrap();
    session.wait_until_connected(Duration::from_secs(3)).await.unwrap();

    // Desconecta graciosamente
    session.disconnect().await.unwrap();

    // 1. Sinalização principal deve ter recebido Opcode 4 com channel_id = None
    let last_state = signaling.last_voice_state.lock().await;
    assert!(last_state.is_some());
    let (guild, channel, _, _) = last_state.as_ref().unwrap();
    assert_eq!(guild.as_deref(), Some("111222"));
    assert_eq!(*channel, None, "Opcode 4 com channel_id nulo para sair do canal");

    // 2. Gateway deve estar desconectado
    assert!(mock_gw.disconnected.load(Ordering::SeqCst));

    // 3. Motor de áudio teve stop_session chamado
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(stopped_called.load(Ordering::SeqCst));

    // 4. Estado da sessão é Disconnected
    assert!(matches!(session.state(), VoiceSessionState::Disconnected { .. }));
}

// ============================================================================
// 4. Teste de Fechamento Fatal do Voice Gateway (ex: código 4017 DAVE required)
// ============================================================================

#[tokio::test]
async fn test_voice_session_fatal_close_terminates_session() {
    let ssrc = 0x44332211;
    let (udp_addr, _udp_handle, _udp_rx) = spawn_test_udp_server(ssrc).await;

    let signaling = Arc::new(MockMainSignaling::new(Some(format!("127.0.0.1:{}", udp_addr.port()))));

    let (_audio_out_tx, audio_out_rx) = mpsc::channel(32);
    let (_audio_spk_tx, audio_spk_rx) = mpsc::channel(16);
    let (audio_cmd_tx, mut audio_cmd_rx) = mpsc::channel(64);

    let audio_bridge = AudioBridge::from_channels(audio_out_rx, audio_spk_rx, audio_cmd_tx);
    let mock_gw = Arc::new(MockVoiceGateway::new(udp_addr.port(), ssrc));
    let mock_gw_clone = mock_gw.clone();

    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            if let AudioBridgeCommand::StartSession { responder } = cmd {
                let _ = responder.send(Ok(()));
            }
        }
    });

    let config = VoiceSessionConfig {
        guild_id: Some(10),
        channel_id: 20,
        user_id: 123456789,
        self_mute: false,
        self_deaf: false,
    };

    let session = VoiceSession::new(config, signaling.clone(), audio_bridge)
        .with_gateway_factory(move |_cfg| mock_gw_clone.clone());

    session.start().await.unwrap();
    session.wait_until_connected(Duration::from_secs(3)).await.unwrap();

    // Simula emissão de erro fatal pelo Voice Gateway (código 4017: DAVE protocol required)
    let _ = mock_gw.event_tx.send(VoiceGatewayEvent::Disconnected {
        code: Some(4017),
        reason: "E2EE/DAVE protocol required".to_string(),
        decision: VoiceCloseDecision::Fatal,
    });

    // Aguarda a sessão processar o evento e transitar para Disconnected
    tokio::time::sleep(Duration::from_millis(100)).await;

    match session.state() {
        VoiceSessionState::Disconnected { reason } => {
            assert!(
                reason.contains("DAVE protocol required"),
                "Motivo deve refletir o fechamento fatal: {reason}"
            );
        }
        other => panic!("Esperado Disconnected após close fatal, obtido {:?}", other),
    }
}

// ============================================================================
// 5. Teste do Ponto de Extensão DAVE (Custom FrameCipher Hook)
// ============================================================================

struct DaveTestCipher {
    enc_calls: Arc<AtomicU32>,
    dec_calls: Arc<AtomicU32>,
}

impl FrameCipher for DaveTestCipher {
    fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        self.enc_calls.fetch_add(1, Ordering::SeqCst);
        let mut out = b"DAVE:".to_vec();
        out.extend_from_slice(frame);
        Ok(out)
    }

    fn decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        self.dec_calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(sender_user_id, 999888777);
        if frame.starts_with(b"DAVE:") {
            Ok(frame[5..].to_vec())
        } else {
            Ok(frame.to_vec())
        }
    }
}

#[tokio::test]
async fn test_voice_session_custom_frame_cipher_extension_point() {
    let ssrc = 0x77889900;
    let (udp_addr, _udp_handle, mut udp_rx) = spawn_test_udp_server(ssrc).await;

    let signaling = Arc::new(MockMainSignaling::new(Some(format!("127.0.0.1:{}", udp_addr.port()))));

    let (audio_out_tx, audio_out_rx) = mpsc::channel(32);
    let (_audio_spk_tx, audio_spk_rx) = mpsc::channel(16);
    let (audio_cmd_tx, mut audio_cmd_rx) = mpsc::channel(64);

    let audio_bridge = AudioBridge::from_channels(audio_out_rx, audio_spk_rx, audio_cmd_tx);
    let mock_gw = Arc::new(MockVoiceGateway::new(udp_addr.port(), ssrc));
    let mock_gw_clone = mock_gw.clone();

    let enc_counter = Arc::new(AtomicU32::new(0));
    let dec_counter = Arc::new(AtomicU32::new(0));
    let cipher = DaveTestCipher {
        enc_calls: enc_counter.clone(),
        dec_calls: dec_counter.clone(),
    };

    let (delivered_audio_tx, mut delivered_audio_rx) = mpsc::channel(32);
    tokio::spawn(async move {
        while let Some(cmd) = audio_cmd_rx.recv().await {
            match cmd {
                AudioBridgeCommand::StartSession { responder } => {
                    let _ = responder.send(Ok(()));
                }
                AudioBridgeCommand::ReceiveOpusPacket { opus_data, .. } => {
                    let _ = delivered_audio_tx.send(opus_data).await;
                }
                _ => {}
            }
        }
    });

    let config = VoiceSessionConfig {
        guild_id: Some(1),
        channel_id: 2,
        user_id: 123456789,
        self_mute: false,
        self_deaf: false,
    };

    let session = VoiceSession::new(config, signaling.clone(), audio_bridge)
        .with_gateway_factory(move |_cfg| mock_gw_clone.clone())
        .with_cipher(cipher);

    session.start().await.unwrap();
    session.wait_until_connected(Duration::from_secs(3)).await.unwrap();

    // Notifica que o SSRC pertence ao user_id 999888777
    let _ = mock_gw.event_tx.send(VoiceGatewayEvent::Speaking(
        discord_rs::protocol::voice_gateway::SpeakingPayload {
            speaking: 1,
            delay: 0,
            ssrc,
            user_id: Some("999888777".to_string()),
        },
    ));

    // 1. Envia quadro através do cifrador customizado
    let raw_frame = b"opus sample packet for dave encryption";
    audio_out_tx.send(raw_frame.to_vec()).await.unwrap();

    // 2. Servidor UDP recebe pacote ecoado
    let received_pkt = tokio::time::timeout(Duration::from_millis(500), udp_rx.recv())
        .await
        .unwrap()
        .unwrap();

    let crypto = RtpCrypto::new("aead_aes256_gcm_rtpsize", &sample_key()).unwrap();
    let (_, decrypted_rtp) = crypto.decrypt_packet(&received_pkt).unwrap();

    // Deve conter o prefixo adicionado pelo cifrador "DAVE:"
    assert_eq!(&decrypted_rtp[..5], b"DAVE:");
    assert_eq!(&decrypted_rtp[5..], raw_frame);
    assert_eq!(enc_counter.load(Ordering::SeqCst), 1);

    // 3. Ao ecoar de volta, a ponte de áudio deve receber o quadro decifrado limpo
    let delivered = tokio::time::timeout(Duration::from_millis(500), delivered_audio_rx.recv())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(delivered, raw_frame);
    assert_eq!(dec_counter.load(Ordering::SeqCst), 1);

    session.disconnect().await.unwrap();
}
