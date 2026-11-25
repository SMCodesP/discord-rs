//! Discord End-to-End Voice Session Orchestrator (Fase 8.2 & Fase 8.3).
//!
//! Conduz uma chamada de voz real do início ao fim integrando todas as camadas:
//! 1. **Sinalização Principal**: Opcode 4 (`send_voice_state_update`) sobre o Gateway principal
//!    do Discord e captura de `VOICE_STATE_UPDATE` (`session_id`) e `VOICE_SERVER_UPDATE` (`token`, `endpoint`).
//! 2. **Voice Gateway v8**: Conexão WebSocket com `wss://{endpoint}/?v=8`, identificação Opcode 0,
//!    recebimento de Opcode 2 `Ready` (SSRC, IP, porta, modos suportados).
//! 3. **Transporte UDP**: Descoberta pública via IP Discovery no mesmo socket UDP
//!    ([`VoiceUdpSocket`]), seleção de protocolo Opcode 1 (`select_protocol`), recepção de
//!    Opcode 4 `SessionDescription` com a chave secreta, e transição para [`VoiceUdpConnection`].
//! 4. **Keepalive UDP**: Emissão periódica a cada 5 segundos de pacotes de 8 bytes e descarte transparente.
//! 5. **Motor de Áudio em Thread Dedicada**: [`AudioEngine`] operando em thread dedicada do SO
//!    (isolando APIs de hardware de áudio `cpal`/ALSA do runtime Tokio), comunicando-se via canais.
//! 6. **DAVE (E2EE com MLS)**: Trait [`FrameCipher`] entre a codificação Opus e a
//!    cifragem RTP. O padrão de produção é [`DaveFrameCipher`], compartilhando a
//!    mesma [`VoiceDaveSession`] com o gateway. [`PassthroughFrameCipher`] só deve
//!    ser selecionado explicitamente em testes offline.
//!
//! # Aviso de Segurança
//!
//! Desde 1º de março de 2026, chamadas elegíveis exigem DAVE. O caminho de
//! produção falha fechado quando a versão negociada é maior que zero e o MLS
//! ainda não possui ratchet ativo; somente o silêncio canônico e os estados
//! oficiais de passthrough são liberados.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, oneshot, watch};

use crate::domain::VoiceState;
use crate::media::capture::SpeakingEvent;
use crate::media::{AudioEngine, AudioError};
use crate::protocol::discord_api::DiscordApiClient;
use crate::protocol::transport::{DiscordTransport, TransportEvent};
use crate::protocol::voice_crypto::RtpCrypto;
use crate::protocol::voice_dave::{
    DaveInvalidCommitWelcomePayload, DaveOpcode, VoiceDaveSession, format_client_binary_message,
    parse_server_binary_message,
};
use crate::protocol::voice_gateway::{
    DEFAULT_MAX_DAVE_PROTOCOL_VERSION, SessionDescription, VoiceCloseDecision, VoiceGateway,
    VoiceGatewayConfig, VoiceGatewayEvent, VoiceReady, VoiceServerData,
};
use crate::protocol::voice_udp::{
    SILENCE_FRAME, VoicePacket, VoiceUdpReceiver, VoiceUdpSocket, VoiceUdpSender,
};

/// Limite defensivo para mensagens binárias DAVE antes de qualquer parse.
/// Pacotes MLS legítimos são pequenos (KBs); 1 MiB evita alocações hostis.
const MAX_DAVE_BINARY_PAYLOAD: usize = 1024 * 1024;

// ============================================================================
// 1. Ponto de Extensão DAVE (FrameCipher)
// ============================================================================

/// Contrato para cifradores de quadros Opus entre a camada de áudio e o transporte RTP.
pub trait FrameCipher: Send + Sync {
    /// Cifra um quadro Opus antes de envelopá-lo no cabeçalho RTP e na criptografia de transporte AEAD.
    fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>>;

    /// Decifra um quadro Opus recebido de um participante específico após a decifração RTP AEAD.
    fn decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>>;
}

/// Implementação passthrough para desenvolvimento, testes offline e ambientes sem imposição de DAVE.
#[derive(Debug, Default, Clone)]
pub struct PassthroughFrameCipher;

impl FrameCipher for PassthroughFrameCipher {
    fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        Ok(frame.to_vec())
    }

    fn decrypt_opus_frame(&mut self, _sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        Ok(frame.to_vec())
    }
}

/// Cifrador de quadros de mídia usando o protocolo DAVE (E2EE com MLS / RFC 9420).
///
/// Encapsula uma [`VoiceDaveSession`] compartilhada via `Arc<std::sync::Mutex<...>>` para sincronizar
/// o estado criptográfico de chaves e ratchets MLS com o laço de sinalização do gateway.
#[derive(Clone)]
pub struct DaveFrameCipher {
    session: Arc<std::sync::Mutex<VoiceDaveSession>>,
}

impl DaveFrameCipher {
    /// Cria uma nova instância de `DaveFrameCipher` a partir de uma sessão DAVE compartilhada.
    pub fn new(session: Arc<std::sync::Mutex<VoiceDaveSession>>) -> Self {
        Self { session }
    }

    /// Cria uma nova instância inicializando internamente uma [`VoiceDaveSession`].
    pub fn for_user_and_channel(user_id: u64, channel_id: u64) -> Self {
        Self {
            session: Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(
                user_id, channel_id,
            ))),
        }
    }

    /// Retorna a referência para a sessão DAVE subjacente.
    pub fn session(&self) -> Arc<std::sync::Mutex<VoiceDaveSession>> {
        self.session.clone()
    }
}

impl FrameCipher for DaveFrameCipher {
    fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        let mut sess = self
            .session
            .lock()
            .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?;
        sess.encrypt_opus_frame_strict(frame)
    }

    fn decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        let mut sess = self
            .session
            .lock()
            .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?;
        sess.decrypt_opus_frame_strict(sender_user_id, frame)
    }
}

impl FrameCipher for VoiceDaveSession {
    fn encrypt_opus_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>> {
        VoiceDaveSession::encrypt_opus_frame_strict(self, frame)
    }

    fn decrypt_opus_frame(&mut self, sender_user_id: u64, frame: &[u8]) -> Result<Vec<u8>> {
        VoiceDaveSession::decrypt_opus_frame_strict(self, sender_user_id, frame)
    }
}

// ============================================================================
// 2. Estados Observáveis do Ciclo de Vida da Sessão
// ============================================================================

/// Estados observáveis de uma sessão de voz do Discord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceSessionState {
    /// Sessão desconectada ou finalizada.
    Disconnected { reason: String },
    /// Conectando (aguardando sinalização, WebSocket e transporte UDP).
    Connecting,
    /// Conexão de voz estabelecida e ativa (áudio bidirecional em streaming).
    Connected,
    /// Tentando reconectar após perda transitória de rede ou indicação do servidor.
    Reconnecting,
}

/// Estatísticas periódicas de transmissão e recepção de pacotes RTP.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VoicePacketStats {
    pub sent_packets: u64,
    pub recv_packets: u64,
    pub lost_packets: u64,
}

impl VoicePacketStats {
    pub fn packet_loss_pct(&self) -> Option<f32> {
        let expected = self.recv_packets.saturating_add(self.lost_packets);
        if expected == 0 {
            None
        } else {
            Some(self.lost_packets as f32 / expected as f32 * 100.0)
        }
    }
}

/// Eventos observáveis do ciclo de vida e progresso da sessão de voz.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceSessionEvent {
    /// Opcode 4 enviado ao Gateway principal.
    VoiceStateSent,
    /// Metadados de servidor recebidos (VOICE_SERVER_UPDATE).
    VoiceServerReceived { endpoint: String },
    /// Voice Gateway autenticado e Opcode 2 Ready recebido com SSRC.
    ReadyReceived { ssrc: u32, ip: String, port: u16 },
    /// IP discovery concluído sobre o mesmo socket UDP.
    IpDiscovered { ip: String, port: u16 },
    /// Opcode 1 enviado e Opcode 4 Session Description recebido com chave de voz.
    SessionDescriptionReceived {
        mode: String,
        dave_protocol_version: Option<u32>,
    },
    /// Transição de época DAVE executada.
    DaveTransitionExecuted { transition_id: u16 },
    /// Conexão desconectada ou encerrada.
    Disconnected { code: Option<u16>, reason: String },
    /// Evento de fala por participante (local ou remoto) com SSRC e ID de usuário mapeado.
    Speaking {
        ssrc: u32,
        user_id: Option<u64>,
        is_speaking: bool,
    },
    /// Latência RTT medida a partir do Heartbeat ACK do Voice Gateway.
    HeartbeatAck { ping_ms: u32 },
}

// ============================================================================
// 3. Abstração do Canal de Sinalização Principal (Zero Mocks em Produção)
// ============================================================================

/// Abstração do canal de sinalização do cliente Discord principal.
///
/// Permite enviar Opcode 4 (`send_voice_state_update`) e observar os eventos
/// `VoiceServerUpdate` e `VoiceStateUpdate`.
#[async_trait]
pub trait MainSignalingChannel: Send + Sync {
    /// Envia o Opcode 4 (Voice State Update) ao Gateway do Discord.
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()>;

    /// Subscreve eventos em tempo real do transporte do Discord.
    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent>;

    /// Retorna o último `session_id` de voz capturado, se disponível.
    async fn current_voice_session_id(&self) -> Option<String> {
        None
    }

    /// Retorna os últimos metadados de servidor de voz capturados, se disponíveis.
    async fn current_voice_server(&self) -> Option<VoiceServerData> {
        None
    }

    /// Limpa o cache de servidor de voz capturado (ao desconectar ou sair).
    async fn clear_voice_server(&self) {}
}

#[async_trait]
impl MainSignalingChannel for DiscordApiClient {
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        <Self as DiscordTransport>::send_voice_state_update(
            self, guild_id, channel_id, self_mute, self_deaf,
        )
        .await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        <Self as DiscordTransport>::subscribe_events(self)
    }

    async fn current_voice_session_id(&self) -> Option<String> {
        self.current_voice_session_id().await
    }

    async fn current_voice_server(&self) -> Option<VoiceServerData> {
        self.current_voice_server().await
    }

    async fn clear_voice_server(&self) {
        self.clear_voice_server().await;
    }
}

#[async_trait]
impl<T: DiscordTransport + ?Sized> MainSignalingChannel for Arc<T> {
    async fn send_voice_state_update(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<()> {
        self.as_ref()
            .send_voice_state_update(guild_id, channel_id, self_mute, self_deaf)
            .await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> {
        self.as_ref().subscribe_events()
    }
}

// ============================================================================
// 4. Abstração do Voice Gateway (Zero Mocks em Produção)
// ============================================================================

/// Abstração do transporte do Voice Gateway v8 para desacoplamento e testes locais.
#[async_trait]
pub trait VoiceGatewayTransport: Send + Sync {
    /// Inicia a conexão WebSocket com o gateway de voz.
    async fn connect(&self) -> Result<()>;

    /// Aguarda o recebimento do payload `Ready` (Opcode 2).
    async fn wait_for_ready(&self, timeout: Duration) -> Result<VoiceReady>;

    /// Envia o payload `Select Protocol` (Opcode 1) e aguarda `Session Description` (Opcode 4).
    async fn select_protocol(
        &self,
        external_ip: &str,
        external_port: u16,
        mode: &str,
    ) -> Result<SessionDescription>;

    /// Define o estado de fala do usuário local (Opcode 5 Speaking).
    async fn set_speaking(&self, speaking: bool, delay: u32, ssrc: u32) -> Result<()>;

    /// Subscreve eventos em tempo real do Voice Gateway.
    fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent>;

    /// Envia uma mensagem de texto (JSON) sobre o WebSocket do Voice Gateway.
    async fn send_text(&self, text: String) -> Result<()> {
        let _ = text;
        Ok(())
    }

    /// Envia uma mensagem binária sobre o WebSocket do Voice Gateway.
    async fn send_binary(&self, data: Vec<u8>) -> Result<()> {
        let _ = data;
        Ok(())
    }

    /// Desconecta graciosamente do Voice Gateway.
    async fn disconnect(&self) -> Result<()>;
}

#[async_trait]
impl VoiceGatewayTransport for VoiceGateway {
    async fn connect(&self) -> Result<()> {
        VoiceGateway::connect(self).await
    }

    async fn wait_for_ready(&self, timeout: Duration) -> Result<VoiceReady> {
        VoiceGateway::wait_for_ready(self, timeout).await
    }

    async fn select_protocol(
        &self,
        external_ip: &str,
        external_port: u16,
        mode: &str,
    ) -> Result<SessionDescription> {
        VoiceGateway::select_protocol(self, external_ip, external_port, mode).await
    }

    async fn set_speaking(&self, speaking: bool, delay: u32, ssrc: u32) -> Result<()> {
        VoiceGateway::set_speaking(self, speaking, delay, ssrc).await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
        VoiceGateway::subscribe_events(self)
    }

    async fn send_text(&self, text: String) -> Result<()> {
        VoiceGateway::send_text(self, text).await
    }

    async fn send_binary(&self, data: Vec<u8>) -> Result<()> {
        VoiceGateway::send_binary(self, data).await
    }

    async fn disconnect(&self) -> Result<()> {
        VoiceGateway::disconnect(self).await
    }
}

/// Tipo da função construtora do Voice Gateway.
pub type VoiceGatewayFactory =
    Arc<dyn Fn(VoiceGatewayConfig) -> Arc<dyn VoiceGatewayTransport> + Send + Sync>;

// ============================================================================
// 5. Ponte de Áudio (AudioBridge com Thread Dedicada do SO)
// ============================================================================

/// Comandos despachados para a thread dedicada do `AudioEngine`.
pub enum AudioBridgeCommand {
    StartSession {
        responder: oneshot::Sender<Result<(), AudioError>>,
    },
    StopSession,
    SetMuted(bool),
    SetDeafened(bool),
    SwitchInputDevice {
        device_id: Option<String>,
        responder: oneshot::Sender<Result<(), AudioError>>,
    },
    SwitchOutputDevice {
        device_id: Option<String>,
        responder: oneshot::Sender<Result<(), AudioError>>,
    },
    ReceiveOpusPacket {
        ssrc: u32,
        sequence: u16,
        timestamp: u32,
        opus_data: Vec<u8>,
    },
    Shutdown,
}

/// Ponte assíncrona entre o runtime Tokio e o [`AudioEngine`] isolado em thread de SO.
pub struct AudioBridge {
    pub outgoing_rx: Option<mpsc::Receiver<Vec<u8>>>,
    pub speaking_rx: Option<mpsc::Receiver<SpeakingEvent>>,
    command_tx: mpsc::Sender<AudioBridgeCommand>,
    audio_level: Option<Arc<AtomicU32>>,
    input_gain: Option<Arc<AtomicU32>>,
    output_gain: Option<Arc<AtomicU32>>,
}

impl AudioBridge {
    /// Constrói a ponte executando o [`AudioEngine`] em uma thread dedicada do SO.
    pub fn from_engine_factory<F>(factory: F) -> Self
    where
        F: FnOnce() -> AudioEngine + Send + 'static,
    {
        let (init_tx, init_rx) = std::sync::mpsc::channel();
        let (command_tx, mut command_rx) = mpsc::channel::<AudioBridgeCommand>(256);

        let spawn_result = std::thread::Builder::new()
            .name("discord-audio-engine".into())
            .spawn(move || {
                let mut engine = factory();
                let outgoing_rx = engine.take_outgoing_receiver();
                let speaking_rx = engine.take_speaking_receiver();
                let meter_handles = (
                    Some(engine.audio_level_handle()),
                    Some(engine.input_gain_handle()),
                    Some(engine.output_gain_handle()),
                );
                let _ = init_tx.send((outgoing_rx, speaking_rx, meter_handles));

                while let Some(cmd) = command_rx.blocking_recv() {
                    match cmd {
                        AudioBridgeCommand::StartSession { responder } => {
                            let res = engine.start_session();
                            let _ = responder.send(res);
                        }
                        AudioBridgeCommand::StopSession => {
                            engine.stop_session();
                        }
                        AudioBridgeCommand::SetMuted(muted) => {
                            engine.set_muted(muted);
                        }
                        AudioBridgeCommand::SetDeafened(deafened) => {
                            engine.set_deafened(deafened);
                        }
                        AudioBridgeCommand::SwitchInputDevice {
                            device_id,
                            responder,
                        } => {
                            let res = engine.switch_input_device(device_id.as_deref());
                            let _ = responder.send(res);
                        }
                        AudioBridgeCommand::SwitchOutputDevice {
                            device_id,
                            responder,
                        } => {
                            let res = engine.switch_output_device(device_id.as_deref());
                            let _ = responder.send(res);
                        }
                        AudioBridgeCommand::ReceiveOpusPacket {
                            ssrc,
                            sequence,
                            timestamp,
                            opus_data,
                        } => {
                            let _ = engine.receive_opus_packet(ssrc, sequence, timestamp, &opus_data);
                        }
                        AudioBridgeCommand::Shutdown => {
                            engine.stop_session();
                            break;
                        }
                    }
                }
            });
        if let Err(err) = spawn_result {
            log::error!("Falha ao iniciar thread dedicada do AudioEngine: {err}");
        }

        let (outgoing_rx, speaking_rx, (audio_level, input_gain, output_gain)) = init_rx
            .recv()
            .unwrap_or((None, None, (None, None, None)));

        Self {
            outgoing_rx,
            speaking_rx,
            command_tx,
            audio_level,
            input_gain,
            output_gain,
        }
    }

    /// Cria uma nova thread de áudio com [`AudioEngine::new`].
    pub fn new_thread() -> Self {
        Self::from_engine_factory(AudioEngine::new)
    }

    /// Cria uma ponte com canais explícitos para ambientes de teste ou pipelines customizados.
    pub fn from_channels(
        outgoing_rx: mpsc::Receiver<Vec<u8>>,
        speaking_rx: mpsc::Receiver<SpeakingEvent>,
        command_tx: mpsc::Sender<AudioBridgeCommand>,
    ) -> Self {
        Self {
            outgoing_rx: Some(outgoing_rx),
            speaking_rx: Some(speaking_rx),
            command_tx,
            audio_level: None,
            input_gain: None,
            output_gain: None,
        }
    }

    /// Real microphone RMS for Mic Test. `None` means this bridge has no audio
    /// engine (for example a test channel), not a fabricated zero level.
    pub fn audio_level(&self) -> Option<f32> {
        self.audio_level
            .as_ref()
            .map(|level| f32::from_bits(level.load(Ordering::Relaxed)))
    }

    pub fn input_gain(&self) -> Option<f32> {
        self.gain_value(&self.input_gain)
    }

    pub fn output_gain(&self) -> Option<f32> {
        self.gain_value(&self.output_gain)
    }

    pub fn set_input_gain(&self, gain: f32) -> Result<()> {
        self.set_gain(&self.input_gain, gain)
    }

    pub fn set_output_gain(&self, gain: f32) -> Result<()> {
        self.set_gain(&self.output_gain, gain)
    }

    fn gain_value(&self, handle: &Option<Arc<AtomicU32>>) -> Option<f32> {
        handle
            .as_ref()
            .map(|gain| f32::from_bits(gain.load(Ordering::Relaxed)))
    }

    fn set_gain(&self, handle: &Option<Arc<AtomicU32>>, gain: f32) -> Result<()> {
        let handle = handle
            .as_ref()
            .ok_or_else(|| anyhow!("Bridge de áudio sem engine real"))?;
        let gain = if gain.is_finite() {
            gain.clamp(0.0, 2.0)
        } else {
            1.0
        };
        handle.store(gain.to_bits(), Ordering::Relaxed);
        Ok(())
    }

    /// Inicia a captura e reprodução de hardware através da thread dedicada.
    pub async fn start_session(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(AudioBridgeCommand::StartSession { responder: tx })
            .await
            .map_err(|_| anyhow!("Thread de áudio encerrada inesperadamente"))?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(anyhow!("Falha ao iniciar hardware de áudio: {err}")),
            Err(_) => Err(anyhow!("Sem resposta da thread de áudio")),
        }
    }

    /// Para as streams de hardware de áudio.
    pub async fn stop_session(&self) {
        let _ = self.command_tx.send(AudioBridgeCommand::StopSession).await;
    }

    /// Atualiza mute local.
    pub async fn set_muted(&self, muted: bool) {
        let _ = self
            .command_tx
            .send(AudioBridgeCommand::SetMuted(muted))
            .await;
    }

    /// Atualiza ensurdecimento local.
    pub async fn set_deafened(&self, deafened: bool) {
        let _ = self
            .command_tx
            .send(AudioBridgeCommand::SetDeafened(deafened))
            .await;
    }

    /// Entrega pacote Opus decifrado para mixagem e reprodução.
    pub async fn receive_opus_packet(
        &self,
        ssrc: u32,
        sequence: u16,
        timestamp: u32,
        opus_data: Vec<u8>,
    ) {
        let _ = self
            .command_tx
            .send(AudioBridgeCommand::ReceiveOpusPacket {
                ssrc,
                sequence,
                timestamp,
                opus_data,
            })
            .await;
    }

    /// Troca dinamicamente o microfone ativo sem reiniciar a chamada.
    pub async fn switch_input_device(&self, device_id: Option<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(AudioBridgeCommand::SwitchInputDevice {
                device_id,
                responder: tx,
            })
            .await
            .map_err(|_| anyhow!("Thread de áudio encerrada inesperadamente"))?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(anyhow!("Falha ao trocar dispositivo de entrada: {err}")),
            Err(_) => Err(anyhow!("Sem resposta da thread de áudio")),
        }
    }

    /// Troca dinamicamente o dispositivo de saída ativo sem reiniciar a chamada.
    pub async fn switch_output_device(&self, device_id: Option<String>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(AudioBridgeCommand::SwitchOutputDevice {
                device_id,
                responder: tx,
            })
            .await
            .map_err(|_| anyhow!("Thread de áudio encerrada inesperadamente"))?;

        match rx.await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(anyhow!("Falha ao trocar dispositivo de saída: {err}")),
            Err(_) => Err(anyhow!("Sem resposta da thread de áudio")),
        }
    }

    /// Encerra a thread de áudio.
    pub async fn shutdown(&self) {
        let _ = self.command_tx.send(AudioBridgeCommand::Shutdown).await;
    }
}

impl Drop for AudioBridge {
    fn drop(&mut self) {
        // Se a ponte for destruída sem um shutdown explícito (por exemplo no
        // fechamento da janela), pede a parada do engine sem bloquear a UI. O
        // drop do `command_tx` fecha o canal e encerra a thread dedicada mesmo
        // que este `try_send` não encontre espaço no buffer.
        let _ = self.command_tx.try_send(AudioBridgeCommand::Shutdown);
    }
}

// ============================================================================
// 6. Configuração e Estrutura Principal da Sessão de Voz (VoiceSession)
// ============================================================================

/// Estado único de mute/deafen local.
///
/// Invariantes (T-098):
/// - `deafened` implica `muted` (ensurdecer silencia o microfone);
/// - não é possível reativar o microfone enquanto `deafened` for verdadeiro;
/// - ao desensurdecer, o mute anterior ao deafen é restaurado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VoiceControlState {
    /// Microfone silenciado.
    pub muted: bool,
    /// Saída de áudio ensurdecida.
    pub deafened: bool,
    /// Mute que existia imediatamente antes de ativar o deafen.
    previous_mute_before_deafen: bool,
}

impl VoiceControlState {
    /// Cria o estado normalizando a invariante deafen ⇒ mute.
    pub fn new(muted: bool, deafened: bool) -> Self {
        let previous = muted;
        Self {
            muted: muted || deafened,
            deafened,
            previous_mute_before_deafen: previous,
        }
    }

    /// Alterna o mute. Enquanto ensurdecido, o comando é ignorado para não
    /// reativar o microfone.
    pub fn toggle_mute(&mut self) {
        if self.deafened {
            return;
        }
        self.muted = !self.muted;
        self.previous_mute_before_deafen = self.muted;
    }

    /// Alterna o deafen preservando/restaurando o mute anterior.
    pub fn toggle_deafen(&mut self) {
        if self.deafened {
            self.deafened = false;
            self.muted = self.previous_mute_before_deafen;
        } else {
            self.previous_mute_before_deafen = self.muted;
            self.deafened = true;
            self.muted = true;
        }
    }

    /// Aplica um mute explícito respeitando a invariante deafen ⇒ mute.
    pub fn set_muted(&mut self, muted: bool) {
        if self.deafened {
            // Deafened mantém o microfone silenciado; unmute é recusado.
            self.muted = true;
            return;
        }
        self.muted = muted;
        self.previous_mute_before_deafen = muted;
    }

    /// Aplica um deafen explícito preservando/restaurando o mute anterior.
    pub fn set_deafened(&mut self, deafened: bool) {
        if deafened {
            if !self.deafened {
                self.previous_mute_before_deafen = self.muted;
            }
            self.deafened = true;
            self.muted = true;
        } else {
            self.deafened = false;
            self.muted = self.previous_mute_before_deafen;
        }
    }

    /// Sincroniza com o eco do Gateway (fonte da verdade) normalizando a
    /// invariante. Mudanças externas atualizam também o mute de restauração.
    pub fn observe_gateway_echo(&mut self, muted: bool, deafened: bool) {
        if deafened {
            if !self.deafened {
                self.previous_mute_before_deafen = self.muted;
            }
            self.deafened = true;
            self.muted = true;
        } else {
            self.deafened = false;
            self.muted = muted;
            self.previous_mute_before_deafen = muted;
        }
    }
}

/// Parâmetros de entrada para estabelecer uma sessão de voz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSessionConfig {
    /// ID do servidor / guilda (`None` no caso de DM ou chamada privada).
    pub guild_id: Option<u64>,
    /// ID do canal de voz ou canal de DM.
    pub channel_id: u64,
    /// ID do usuário autenticado no Discord.
    pub user_id: u64,
    /// Mute local inicial.
    pub self_mute: bool,
    /// Ensurdecimento local inicial.
    pub self_deaf: bool,
}

/// Orquestrador de alto nível da sessão de voz do Discord.
///
/// Conduz o ciclo de vida completo: sinalização principal, Voice Gateway v8,
/// transporte UDP/RTP com criptografia, keepalive, DAVE passthrough e áudio via hardware.
pub struct VoiceSession {
    config: VoiceSessionConfig,
    state_tx: watch::Sender<VoiceSessionState>,
    state_rx: watch::Receiver<VoiceSessionState>,
    signaling: Arc<dyn MainSignalingChannel>,
    audio_bridge: Arc<Mutex<AudioBridge>>,
    frame_cipher: Arc<Mutex<Box<dyn FrameCipher>>>,
    dave_session: Arc<std::sync::Mutex<VoiceDaveSession>>,
    events_tx: broadcast::Sender<VoiceSessionEvent>,
    sent_packets: Arc<AtomicU64>,
    recv_packets: Arc<AtomicU64>,
    lost_packets: Arc<AtomicU64>,
    gateway_factory: VoiceGatewayFactory,
    session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    active_gateway: Arc<Mutex<Option<Arc<dyn VoiceGatewayTransport>>>>,
    self_mute: Arc<AtomicBool>,
    self_deaf: Arc<AtomicBool>,
    /// Estado canônico desejado de mute/deafen.
    control_state: Arc<std::sync::Mutex<VoiceControlState>>,
    /// Último estado confirmado por Opcode 4 ou pelo eco do Gateway.
    confirmed_control: Arc<std::sync::Mutex<VoiceControlState>>,
    /// Geração monotônica dos comandos de controle planejados.
    control_generation: Arc<AtomicU64>,
    /// Maior geração já processada (sucesso, falha ou descarte).
    control_last_applied: Arc<AtomicU64>,
    /// Serializa o envio dos comandos de controle (um Opcode 4 por vez).
    control_gate: Arc<Mutex<()>>,
    /// Último Opcode 4 de controle confirmado como enviado.
    last_opcode4: Arc<std::sync::RwLock<Option<VoiceControlState>>>,
    ssrc_to_user: Arc<RwLock<HashMap<u32, u64>>>,
    ping_ms: Arc<AtomicU32>,
    call_warning: Arc<std::sync::RwLock<Option<String>>>,
    entry_confirmed_tx: watch::Sender<bool>,
    entry_confirmed_rx: watch::Receiver<bool>,
    shutdown_started: Arc<AtomicBool>,
}

impl VoiceSession {
    /// Cria uma nova instância de `VoiceSession` a partir de uma ponte de áudio existente.
    pub fn new(
        config: VoiceSessionConfig,
        signaling: Arc<dyn MainSignalingChannel>,
        audio_bridge: AudioBridge,
    ) -> Self {
        let (state_tx, state_rx) = watch::channel(VoiceSessionState::Disconnected {
            reason: "Sessão inicializada".into(),
        });
        let (events_tx, _) = broadcast::channel(64);
        let (entry_confirmed_tx, entry_confirmed_rx) = watch::channel(false);

        let self_mute = Arc::new(AtomicBool::new(config.self_mute));
        let self_deaf = Arc::new(AtomicBool::new(config.self_deaf));
        let initial_control = VoiceControlState::new(config.self_mute, config.self_deaf);
        let control_state = Arc::new(std::sync::Mutex::new(initial_control));
        let confirmed_control = Arc::new(std::sync::Mutex::new(initial_control));
        let control_generation = Arc::new(AtomicU64::new(0));
        let control_last_applied = Arc::new(AtomicU64::new(0));
        let control_gate = Arc::new(Mutex::new(()));
        let last_opcode4 = Arc::new(std::sync::RwLock::new(None));
        let sent_packets = Arc::new(AtomicU64::new(0));
        let recv_packets = Arc::new(AtomicU64::new(0));
        let lost_packets = Arc::new(AtomicU64::new(0));
        let ssrc_to_user = Arc::new(RwLock::new(HashMap::new()));
        let ping_ms = Arc::new(AtomicU32::new(u32::MAX));
        let call_warning = Arc::new(std::sync::RwLock::new(None));

        let dave_session = Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(
            config.user_id,
            config.channel_id,
        )));
        let default_cipher = DaveFrameCipher::new(dave_session.clone());
        let gateway_dave_session = dave_session.clone();

        Self {
            config,
            state_tx,
            state_rx,
            signaling,
            audio_bridge: Arc::new(Mutex::new(audio_bridge)),
            frame_cipher: Arc::new(Mutex::new(Box::new(default_cipher))),
            dave_session,
            events_tx,
            sent_packets,
            recv_packets,
            lost_packets,
            gateway_factory: Arc::new(move |cfg| {
                Arc::new(VoiceGateway::new_with_dave_session(
                    cfg,
                    gateway_dave_session.clone(),
                ))
            }),
            session_task: Arc::new(Mutex::new(None)),
            active_gateway: Arc::new(Mutex::new(None)),
            self_mute,
            self_deaf,
            control_state,
            confirmed_control,
            control_generation,
            control_last_applied,
            control_gate,
            last_opcode4,
            ssrc_to_user,
            ping_ms,
            call_warning,
            entry_confirmed_tx,
            entry_confirmed_rx,
            shutdown_started: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Subscreve ao sinal de entrada confirmada no canal de voz (próprio VOICE_STATE_UPDATE ou CALL_CREATE).
    pub fn subscribe_entry_confirmed(&self) -> watch::Receiver<bool> {
        self.entry_confirmed_rx.clone()
    }

    /// Retorna aviso de chamada atual se houver (aviso não-fatal)
    pub fn call_warning(&self) -> Option<String> {
        self.call_warning.read().ok()?.clone()
    }

    /// Define ou limpa o aviso não-fatal de chamada
    pub fn set_call_warning(&self, warning: Option<String>) {
        if let Ok(mut w) = self.call_warning.write() {
            *w = warning;
        }
    }

    /// Cria uma nova instância de `VoiceSession` inicializando o [`AudioEngine`] em uma thread dedicada.
    pub fn new_with_engine(
        config: VoiceSessionConfig,
        signaling: Arc<dyn MainSignalingChannel>,
    ) -> Self {
        let bridge = AudioBridge::new_thread();
        Self::new(config, signaling, bridge)
    }

    /// Cria uma nova instância de `VoiceSession` usando uma fábrica de [`AudioEngine`].
    pub fn from_audio_engine_factory<F>(
        config: VoiceSessionConfig,
        signaling: Arc<dyn MainSignalingChannel>,
        factory: F,
    ) -> Self
    where
        F: FnOnce() -> AudioEngine + Send + 'static,
    {
        let bridge = AudioBridge::from_engine_factory(factory);
        Self::new(config, signaling, bridge)
    }

    /// Configura um cifrador de quadros customizado (ex.: fallback para testes unitários/offline).
    pub fn with_cipher<C: FrameCipher + 'static>(self, cipher: C) -> Self {
        match self.frame_cipher.try_lock() {
            Ok(mut guard) => *guard = Box::new(cipher),
            Err(err) => log::error!("Cifrador de quadros não substituído: {err}"),
        }
        self
    }

    /// Retorna a sessão DAVE subjacente da chamada.
    pub fn dave_session(&self) -> Arc<std::sync::Mutex<VoiceDaveSession>> {
        self.dave_session.clone()
    }

    /// Subscreve aos eventos observáveis do ciclo de vida da chamada.
    pub fn subscribe_events(&self) -> broadcast::Receiver<VoiceSessionEvent> {
        self.events_tx.subscribe()
    }

    /// Retorna as estatísticas de transmissão e recepção de pacotes RTP.
    pub fn packet_stats(&self) -> VoicePacketStats {
        VoicePacketStats {
            sent_packets: self.sent_packets.load(Ordering::Relaxed),
            recv_packets: self.recv_packets.load(Ordering::Relaxed),
            lost_packets: self.lost_packets.load(Ordering::Relaxed),
        }
    }

    /// Codec de áudio do pipeline RTP ativo. O Discord Voice negocia Opus.
    pub fn audio_codec(&self) -> Option<&'static str> {
        self.is_connected().then_some("Opus")
    }

    /// Configura uma fábrica customizada para o Voice Gateway (útil para testes unitários/offline).
    pub fn with_gateway_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn(VoiceGatewayConfig) -> Arc<dyn VoiceGatewayTransport> + Send + Sync + 'static,
    {
        self.gateway_factory = Arc::new(factory);
        self
    }

    /// Retorna o estado atual da sessão de voz.
    pub fn state(&self) -> VoiceSessionState {
        self.state_rx.borrow().clone()
    }

    /// Subscreve a mudanças de estado observáveis da sessão.
    pub fn subscribe_state(&self) -> watch::Receiver<VoiceSessionState> {
        self.state_rx.clone()
    }

    /// Indica se a sessão está no estado `Connected`.
    pub fn is_connected(&self) -> bool {
        matches!(self.state(), VoiceSessionState::Connected)
    }

    /// Aguarda até que a sessão alcance o estado `Connected` com um limite de tempo.
    pub async fn wait_until_connected(&self, timeout: Duration) -> Result<()> {
        let mut rx = self.state_rx.clone();
        if matches!(*rx.borrow(), VoiceSessionState::Connected) {
            return Ok(());
        }

        tokio::time::timeout(timeout, async {
            while rx.changed().await.is_ok() {
                match *rx.borrow() {
                    VoiceSessionState::Connected => return Ok(()),
                    VoiceSessionState::Disconnected { ref reason } => {
                        bail!("Sessão falhou antes de conectar: {reason}")
                    }
                    _ => continue,
                }
            }
            bail!("Canal de estado da sessão fechado");
        })
        .await
        .map_err(|_| anyhow!("Timeout aguardando conexão da sessão de voz"))?
    }

    /// Retorna a latência RTT atual medida via Heartbeat ACK (ou None se nenhum ACK recebido ainda).
    pub fn ping_ms(&self) -> Option<u32> {
        let val = self.ping_ms.load(Ordering::Relaxed);
        if val == u32::MAX { None } else { Some(val) }
    }

    /// Retorna o ID de usuário mapeado para um dado SSRC, se conhecido.
    pub async fn user_id_by_ssrc(&self, ssrc: u32) -> Option<u64> {
        self.ssrc_to_user.read().await.get(&ssrc).copied()
    }

    /// Retorna uma referência para o mapa SSRC -> User ID.
    pub fn ssrc_to_user_map(&self) -> Arc<RwLock<HashMap<u32, u64>>> {
        self.ssrc_to_user.clone()
    }

    /// Real microphone RMS for Mic Test; `None` when no real engine exists.
    pub async fn audio_level(&self) -> Option<f32> {
        self.audio_bridge.lock().await.audio_level()
    }

    pub async fn set_input_gain(&self, gain: f32) -> Result<()> {
        self.audio_bridge.lock().await.set_input_gain(gain)
    }

    pub async fn input_gain(&self) -> Option<f32> {
        self.audio_bridge.lock().await.input_gain()
    }

    pub async fn set_output_gain(&self, gain: f32) -> Result<()> {
        self.audio_bridge.lock().await.set_output_gain(gain)
    }

    pub async fn output_gain(&self) -> Option<f32> {
        self.audio_bridge.lock().await.output_gain()
    }

    /// Troca dinamicamente o microfone ativo na sessão de áudio em andamento.
    pub async fn switch_input_device(&self, device_id: Option<String>) -> Result<()> {
        let bridge = self.audio_bridge.lock().await;
        bridge.switch_input_device(device_id).await
    }

    /// Troca dinamicamente o dispositivo de saída ativo na sessão de áudio em andamento.
    pub async fn switch_output_device(&self, device_id: Option<String>) -> Result<()> {
        let bridge = self.audio_bridge.lock().await;
        bridge.switch_output_device(device_id).await
    }

    fn lock_control(state: &std::sync::Mutex<VoiceControlState>) -> std::sync::MutexGuard<'_, VoiceControlState> {
        state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Estado canônico desejado de mute/deafen (inclui restauração de mute).
    pub fn control_state(&self) -> VoiceControlState {
        *Self::lock_control(&self.control_state)
    }

    /// Último estado confirmado por Opcode 4 ou pelo eco do Gateway.
    pub fn confirmed_control(&self) -> VoiceControlState {
        *Self::lock_control(&self.confirmed_control)
    }

    /// Último Opcode 4 de controle efetivamente enviado, se houver.
    pub fn last_opcode4(&self) -> Option<VoiceControlState> {
        self.last_opcode4
            .read()
            .ok()
            .and_then(|guard| *guard)
    }

    /// Indicadores derivados do estado canônico (espelho do bridge de áudio).
    pub fn is_self_muted(&self) -> bool {
        self.self_mute.load(Ordering::SeqCst)
    }

    /// Indicadores derivados do estado canônico (espelho do bridge de áudio).
    pub fn is_self_deafened(&self) -> bool {
        self.self_deaf.load(Ordering::SeqCst)
    }

    fn plan_control<F>(&self, mutate: F) -> (VoiceControlState, Option<u64>)
    where
        F: FnOnce(&mut VoiceControlState),
    {
        let mut state = Self::lock_control(&self.control_state);
        let before = *state;
        mutate(&mut state);
        let desired = *state;
        if desired == before {
            (desired, None)
        } else {
            let generation = self.control_generation.fetch_add(1, Ordering::SeqCst) + 1;
            (desired, Some(generation))
        }
    }

    /// Planeja um toggle de mute sem enviar nada. Retorna o estado desejado e a
    /// geração do comando (None quando o toggle é recusado pela invariante).
    pub fn plan_toggle_mute(&self) -> (VoiceControlState, Option<u64>) {
        self.plan_control(VoiceControlState::toggle_mute)
    }

    /// Planeja um toggle de deafen sem enviar nada.
    pub fn plan_toggle_deafen(&self) -> (VoiceControlState, Option<u64>) {
        self.plan_control(VoiceControlState::toggle_deafen)
    }

    /// Envia o estado planejado de forma serializada. Comandos de gerações
    /// anteriores à última processada são descartados, então cliques rápidos
    /// nunca deixam o último Opcode 4 divergente do estado local.
    pub async fn apply_control(&self, desired: VoiceControlState, generation: u64) -> Result<()> {
        let _gate = self.control_gate.lock().await;

        if generation <= self.control_last_applied.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.self_mute.store(desired.muted, Ordering::SeqCst);
        self.self_deaf.store(desired.deafened, Ordering::SeqCst);
        {
            let bridge = self.audio_bridge.lock().await;
            bridge.set_muted(desired.muted).await;
            bridge.set_deafened(desired.deafened).await;
        }

        let guild_id_str = self.config.guild_id.map(|g| g.to_string());
        let channel_id_str = self.config.channel_id.to_string();
        let result = self
            .signaling
            .send_voice_state_update(
                guild_id_str.as_deref(),
                Some(&channel_id_str),
                desired.muted,
                desired.deafened,
            )
            .await;

        match result {
            Ok(()) => {
                self.control_last_applied.store(generation, Ordering::SeqCst);
                *Self::lock_control(&self.confirmed_control) = desired;
                if let Ok(mut last) = self.last_opcode4.write() {
                    *last = Some(desired);
                }
                let _ = self.events_tx.send(VoiceSessionEvent::VoiceStateSent);
                Ok(())
            }
            Err(err) => {
                // Só faz rollback se este ainda é o comando mais recente.
                // Com um comando mais novo na fila, ele decide o estado final.
                if generation == self.control_generation.load(Ordering::SeqCst) {
                    let confirmed = self.confirmed_control();
                    *Self::lock_control(&self.control_state) = confirmed;
                    self.self_mute.store(confirmed.muted, Ordering::SeqCst);
                    self.self_deaf.store(confirmed.deafened, Ordering::SeqCst);
                    let bridge = self.audio_bridge.lock().await;
                    bridge.set_muted(confirmed.muted).await;
                    bridge.set_deafened(confirmed.deafened).await;
                }
                self.control_last_applied.store(generation, Ordering::SeqCst);
                Err(err)
            }
        }
    }

    /// Altera o estado de microfone local (mute) respeitando deafen ⇒ mute.
    pub async fn set_muted(&self, muted: bool) -> Result<()> {
        let (desired, generation) = self.plan_control(|state| state.set_muted(muted));
        match generation {
            Some(generation) => self.apply_control(desired, generation).await,
            None => Ok(()),
        }
    }

    /// Altera o estado de ensurdecimento local (deafen) restaurando o mute
    /// anterior ao desensurdecer.
    pub async fn set_deafened(&self, deafened: bool) -> Result<()> {
        let (desired, generation) = self.plan_control(|state| state.set_deafened(deafened));
        match generation {
            Some(generation) => self.apply_control(desired, generation).await,
            None => Ok(()),
        }
    }

    /// Sincroniza o estado canônico com o eco autoritativo do Gateway.
    pub fn observe_gateway_echo(&self, muted: bool, deafened: bool) {
        let observed = {
            let mut state = Self::lock_control(&self.control_state);
            state.observe_gateway_echo(muted, deafened);
            *state
        };
        *Self::lock_control(&self.confirmed_control) = observed;
        self.self_mute.store(observed.muted, Ordering::SeqCst);
        self.self_deaf.store(observed.deafened, Ordering::SeqCst);
    }

    /// Inicia o processo de conexão em segundo plano.
    pub async fn start(&self) -> Result<()> {
        self.shutdown_started.store(false, Ordering::SeqCst);
        let _ = self.state_tx.send(VoiceSessionState::Connecting);

        let config = self.config.clone();
        let signaling = self.signaling.clone();
        let audio_bridge = self.audio_bridge.clone();
        let frame_cipher = self.frame_cipher.clone();
        let dave_session = self.dave_session.clone();
        let events_tx = self.events_tx.clone();
        let sent_packets = self.sent_packets.clone();
        let recv_packets = self.recv_packets.clone();
        let lost_packets = self.lost_packets.clone();
        let gateway_factory = self.gateway_factory.clone();
        let state_tx = self.state_tx.clone();
        let active_gateway = self.active_gateway.clone();
        let self_mute = self.self_mute.clone();
        let self_deaf = self.self_deaf.clone();
        let ssrc_to_user = self.ssrc_to_user.clone();
        let ping_ms = self.ping_ms.clone();
        let entry_confirmed_tx = self.entry_confirmed_tx.clone();

        let handle = tokio::spawn(async move {
            let res = run_voice_session(
                config,
                signaling,
                audio_bridge,
                frame_cipher,
                dave_session,
                events_tx,
                sent_packets,
                recv_packets,
                lost_packets,
                gateway_factory,
                state_tx.clone(),
                active_gateway.clone(),
                self_mute,
                self_deaf,
                ssrc_to_user,
                ping_ms,
                entry_confirmed_tx,
            )
            .await;

            if let Err(err) = res {
                log::error!("[VoiceSession] Sessão encerrou com erro: {err:?}");
                let _ = state_tx.send(VoiceSessionState::Disconnected {
                    reason: format!("{err:#}"),
                });
            }

            // Garante parada definitiva do gateway de voz da sessão
            let mut gw_lock = active_gateway.lock().await;
            if let Some(gw) = gw_lock.take() {
                let _ = gw.disconnect().await;
            }
        });

        let mut lock = self.session_task.lock().await;
        if let Some(prev) = lock.take() {
            prev.abort();
        }
        *lock = Some(handle);

        Ok(())
    }

    /// Executa uma desconexão limpa da sessão de voz.
    ///
    /// 1. Envia Opcode 4 com `channel_id: None` (null channel) para deixar o canal;
    /// 2. Cancela tarefas de streaming e keepalive;
    /// 3. Desconecta o Voice Gateway;
    /// 4. Para o motor de áudio;
    /// 5. Atualiza o estado para `Disconnected`.
    pub async fn disconnect(&self) -> Result<()> {
        self.shutdown().await;
        Ok(())
    }

    /// Encerra a sessão de voz, o Voice Gateway e a thread de áudio de forma
    /// idempotente e ordenada.
    pub async fn shutdown(&self) {
        self.shutdown_with_timeout(VOICE_SESSION_SHUTDOWN_TIMEOUT)
            .await;
    }

    /// Encerramento com orçamento compartilhado entre sinalização, gateway,
    /// task de sessão e áudio. A primeira chamada executa; as seguintes são
    /// no-op.
    pub async fn shutdown_with_timeout(&self, timeout: Duration) {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return;
        }
        log::info!("[VoiceSession] Desconectando sessão de voz...");
        let deadline = tokio::time::Instant::now() + timeout;

        // 1. Sai do canal na sinalização principal sem deixar a UI presa.
        let guild_id_str = self.config.guild_id.map(|g| g.to_string());
        let _ = tokio::time::timeout(
            remaining_budget(deadline),
            self.signaling
                .send_voice_state_update(guild_id_str.as_deref(), None, false, false),
        )
        .await;
        let _ = tokio::time::timeout(
            remaining_budget(deadline),
            self.signaling.clear_voice_server(),
        )
        .await;

        // 2. Para o gateway de voz antes de abortar a task que o possui.
        let gateway = {
            let mut gw_lock = self.active_gateway.lock().await;
            gw_lock.take()
        };
        if let Some(gw) = gateway {
            let _ = tokio::time::timeout(remaining_budget(deadline), gw.disconnect()).await;
        }

        // 3. Aborta e aguarda a task de sessão (join com timeout).
        let task = {
            let mut task_lock = self.session_task.lock().await;
            task_lock.take()
        };
        if let Some(mut task) = task {
            task.abort();
            let _ = tokio::time::timeout(remaining_budget(deadline), &mut task).await;
        }

        // 4. Para as streams e encerra a thread dedicada do AudioEngine.
        let audio = self.audio_bridge.clone();
        let _ = tokio::time::timeout(remaining_budget(deadline), async move {
            let bridge = audio.lock().await;
            bridge.stop_session().await;
            bridge.shutdown().await;
        })
        .await;

        // 5. Transita o estado para Disconnected.
        let _ = self.state_tx.send(VoiceSessionState::Disconnected {
            reason: "Desconectado pelo usuário".into(),
        });
    }
}

fn remaining_budget(deadline: tokio::time::Instant) -> Duration {
    deadline
        .saturating_duration_since(tokio::time::Instant::now())
        .max(Duration::from_millis(1))
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        self.shutdown_started.store(true, Ordering::SeqCst);
        if let Ok(mut lock) = self.session_task.try_lock() {
            if let Some(task) = lock.take() {
                task.abort();
            }
        }
        if let Ok(bridge) = self.audio_bridge.try_lock() {
            let _ = bridge.command_tx.try_send(AudioBridgeCommand::StopSession);
            let _ = bridge.command_tx.try_send(AudioBridgeCommand::Shutdown);
        }
    }
}

struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);
impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

const VOICE_CREDENTIALS_TIMEOUT: Duration = Duration::from_secs(15);
const MEDIA_READY_TIMEOUT: Duration = Duration::from_secs(10);
const DAVE_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Orçamento padrão para encerrar a sessão de voz abaixo do timeout de quit do GPUI.
pub const VOICE_SESSION_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(Debug, Clone)]
struct VoiceCredentials {
    session_id: String,
    token: String,
    endpoint: String,
}

/// One transport generation. Resume keeps this value intact; NewSession drops
/// it (aborting both background tasks and closing the old UDP socket) before a
/// replacement is built.
struct MediaGeneration {
    id: u64,
    gateway: Arc<dyn VoiceGatewayTransport>,
    udp_sender: VoiceUdpSender,
    local_ssrc: u32,
    dave_version: u16,
    _keepalive_task: AbortOnDrop<()>,
    _recv_task: AbortOnDrop<()>,
}

/// Mantém o acesso ergonômico à geração atual e permite removê-la durante
/// NewSession sem deixar um valor fictício no lugar.
struct ActiveGeneration(Option<MediaGeneration>);

impl ActiveGeneration {
    fn new(generation: MediaGeneration) -> Self {
        Self(Some(generation))
    }

    fn current(&self) -> Option<&MediaGeneration> {
        self.0.as_ref()
    }

    fn current_mut(&mut self) -> Option<&mut MediaGeneration> {
        self.0.as_mut()
    }

    fn take(&mut self) -> Option<MediaGeneration> {
        self.0.take()
    }
}

fn drain_broadcast<T: Clone>(receiver: &mut broadcast::Receiver<T>) {
    while let Ok(_) = receiver.try_recv() {}
}

async fn request_voice_credentials(
    signaling: &Arc<dyn MainSignalingChannel>,
    transport_rx: &mut broadcast::Receiver<TransportEvent>,
    config: &VoiceSessionConfig,
    previous: Option<&VoiceCredentials>,
    self_mute: bool,
    self_deaf: bool,
) -> Result<VoiceCredentials> {
    // Discard queued events from the previous generation before requesting new
    // credentials. A token/session id may legitimately be reused only by bugs;
    // Discord issues a fresh token for every voice server update.
    drain_broadcast(transport_rx);
    signaling.clear_voice_server().await;

    let guild_id = config.guild_id.map(|id| id.to_string());
    let channel_id = config.channel_id.to_string();
    signaling
        .send_voice_state_update(
            guild_id.as_deref(),
            Some(&channel_id),
            self_mute,
            self_deaf,
        )
        .await
        .context("Falha ao enviar Opcode 4 (Voice State Update)")?;

    let mut captured_session_id: Option<String> = None;
    let mut captured_token: Option<String> = None;
    let mut captured_endpoint: Option<String> = None;

    tokio::time::timeout(VOICE_CREDENTIALS_TIMEOUT, async {
        while captured_session_id.is_none()
            || captured_token.is_none()
            || captured_endpoint.is_none()
        {
            match transport_rx.recv().await {
                Ok(TransportEvent::VoiceStateUpdate(VoiceState {
                    user_id,
                    channel_id: event_channel_id,
                    session_id,
                    ..
                })) if user_id == config.user_id.to_string()
                    && event_channel_id.as_deref() == Some(channel_id.as_str())
                    && !session_id.is_empty() =>
                {
                    if previous.is_none_or(|old| old.session_id != session_id) {
                        captured_session_id = Some(session_id);
                    }
                }
                Ok(TransportEvent::VoiceServerUpdate {
                    token,
                    guild_id: event_guild_id,
                    channel_id: event_channel_id,
                    endpoint,
                }) => {
                    let matches_guild = match (&config.guild_id, &event_guild_id) {
                        (Some(expected), Some(actual)) => expected.to_string() == *actual,
                        (None, None) => true,
                        _ => false,
                    };
                    let matches_channel = event_channel_id
                        .as_deref()
                        .is_none_or(|actual| actual == channel_id);

                    if matches_guild
                        && matches_channel
                        && !token.is_empty()
                        && previous.is_none_or(|old| old.token != token)
                        && let Some(endpoint) = endpoint.filter(|value| !value.trim().is_empty())
                    {
                        let clean = endpoint
                            .trim()
                            .trim_start_matches("wss://")
                            .trim_start_matches("ws://");
                        let port = clean
                            .rsplit_once(':')
                            .and_then(|(_, port)| port.split('/').next()?.parse::<u16>().ok());
                        log::info!(
                            "[VoiceDiag] VOICE_SERVER_UPDATE recebido: endpoint_presente=true, porta_presente={}, porta={:?}",
                            port.is_some(),
                            port
                        );
                        captured_token = Some(token);
                        captured_endpoint = Some(endpoint.trim().to_string());
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => {
                    bail!("Canal de eventos do transporte principal fechou prematuramente");
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| anyhow!("Timeout aguardando session_id, token e endpoint do Discord"))??;

    Ok(VoiceCredentials {
        session_id: captured_session_id
            .ok_or_else(|| anyhow!("session_id ausente após captura"))?,
        token: captured_token.ok_or_else(|| anyhow!("token de voz ausente após captura"))?,
        endpoint: captured_endpoint
            .ok_or_else(|| anyhow!("endpoint de voz ausente após captura"))?,
    })
}

const MAX_REASONABLE_SEQUENCE_GAP: u16 = 1000;

fn rtp_sequence_gap(previous: u16, current: u16) -> u32 {
    let gap = current.wrapping_sub(previous);
    if gap > 1 && gap <= MAX_REASONABLE_SEQUENCE_GAP {
        u32::from(gap - 1)
    } else {
        0
    }
}

fn spawn_udp_receiver(
    receiver: VoiceUdpReceiver,
    ssrc_to_user: Arc<RwLock<HashMap<u32, u64>>>,
    audio_bridge: Arc<Mutex<AudioBridge>>,
    frame_cipher: Arc<Mutex<Box<dyn FrameCipher>>>,
    recv_packets: Arc<AtomicU64>,
    lost_packets: Arc<AtomicU64>,
) -> (AbortOnDrop<()>, mpsc::Receiver<String>) {
    let (error_tx, error_rx) = mpsc::channel(1);
    let task = tokio::spawn(async move {
        let mut last_sequence_by_ssrc: HashMap<u32, u16> = HashMap::new();
        loop {
            match receiver.recv_packet().await {
                Ok(VoicePacket {
                    ssrc,
                    sequence,
                    timestamp,
                    payload,
                }) => {
                    recv_packets.fetch_add(1, Ordering::Relaxed);
                    if let Some(previous) = last_sequence_by_ssrc.insert(ssrc, sequence) {
                        let lost = rtp_sequence_gap(previous, sequence);
                        if lost > 0 {
                            lost_packets.fetch_add(u64::from(lost), Ordering::Relaxed);
                        }
                    }

                    // The SFU's canonical silence packet is the one DAVE
                    // exception that does not require a sender mapping.
                    if payload == SILENCE_FRAME {
                        let bridge = audio_bridge.lock().await;
                        bridge
                            .receive_opus_packet(ssrc, sequence, timestamp, payload)
                            .await;
                        continue;
                    }

                    let sender_user_id = ssrc_to_user.read().await.get(&ssrc).copied();
                    let Some(sender_user_id) = sender_user_id else {
                        log::trace!(
                            "[VoiceSession] Pacote de SSRC {ssrc} sem User ID descartado"
                        );
                        continue;
                    };

                    let decrypted_opus = {
                        let mut cipher = frame_cipher.lock().await;
                        match cipher.decrypt_opus_frame(sender_user_id, &payload) {
                            Ok(frame) => frame,
                            Err(err) => {
                                log::trace!(
                                    "[VoiceSession] Falha ao decifrar quadro DAVE: {err}"
                                );
                                continue;
                            }
                        }
                    };

                    let bridge = audio_bridge.lock().await;
                    bridge
                        .receive_opus_packet(ssrc, sequence, timestamp, decrypted_opus)
                        .await;
                }
                Err(err) => {
                    let reason = format!("Erro no loop de recepção UDP: {err}");
                    log::debug!("[VoiceSession] {reason}");
                    let _ = error_tx.send(reason).await;
                    break;
                }
            }
        }
    });
    (AbortOnDrop(task), error_rx)
}

#[allow(clippy::too_many_arguments)]
async fn establish_media_generation(
    config: &VoiceSessionConfig,
    credentials: &VoiceCredentials,
    gateway_factory: &VoiceGatewayFactory,
    dave_session: &Arc<std::sync::Mutex<VoiceDaveSession>>,
    events_tx: &broadcast::Sender<VoiceSessionEvent>,
    ssrc_to_user: &Arc<RwLock<HashMap<u32, u64>>>,
    audio_bridge: &Arc<Mutex<AudioBridge>>,
    frame_cipher: &Arc<Mutex<Box<dyn FrameCipher>>>,
    recv_packets: &Arc<AtomicU64>,
    lost_packets: &Arc<AtomicU64>,
    generation_id: u64,
) -> Result<(
    MediaGeneration,
    broadcast::Receiver<VoiceGatewayEvent>,
    mpsc::Receiver<String>,
)> {
    let gateway_config = VoiceGatewayConfig {
        endpoint: credentials.endpoint.clone(),
        server_id: config
            .guild_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| config.channel_id.to_string()),
        user_id: config.user_id.to_string(),
        session_id: credentials.session_id.clone(),
        token: credentials.token.clone(),
        max_dave_protocol_version: Some(DEFAULT_MAX_DAVE_PROTOCOL_VERSION),
    };
    let gateway = (gateway_factory)(gateway_config);
    // Subscribe before connect so DAVE messages emitted immediately after the
    // Session Description cannot be lost.
    let events_rx = gateway.subscribe_events();

    let result: Result<(
        MediaGeneration,
        broadcast::Receiver<VoiceGatewayEvent>,
        mpsc::Receiver<String>,
    )> = async {
        gateway
            .connect()
            .await
            .context("Falha ao conectar Voice Gateway")?;
        let ready = tokio::time::timeout(MEDIA_READY_TIMEOUT, gateway.wait_for_ready(MEDIA_READY_TIMEOUT))
            .await
            .map_err(|_| anyhow!("Timeout aguardando Opcode 2 Ready do Voice Gateway"))?
            .context("Falha ao aguardar Opcode 2 Ready do Voice Gateway")?;

        log::info!(
            "[VoiceSession] Geração {generation_id}: Ready SSRC={}, porta={}, modos={:?}",
            ready.ssrc,
            ready.port,
            ready.modes
        );
        let _ = events_tx.send(VoiceSessionEvent::ReadyReceived {
            ssrc: ready.ssrc,
            ip: ready.ip.clone(),
            port: ready.port,
        });

        let voice_socket = VoiceUdpSocket::connect(&ready.ip, ready.port, ready.ssrc)
            .await
            .context("Falha ao conectar VoiceUdpSocket")?;
        let (discovered_ip, discovered_port) = voice_socket
            .discover_ip()
            .await
            .context("Falha durante IP Discovery sobre o socket UDP")?;
        let _ = events_tx.send(VoiceSessionEvent::IpDiscovered {
            ip: discovered_ip.clone(),
            port: discovered_port,
        });

        let selected_mode = ready.select_best_mode();
        let session_desc = gateway
            .select_protocol(&discovered_ip, discovered_port, selected_mode)
            .await
            .context("Falha ao selecionar protocolo e aguardar Session Description")?;
        let _ = events_tx.send(VoiceSessionEvent::SessionDescriptionReceived {
            mode: session_desc.mode.clone(),
            dave_protocol_version: session_desc.dave_protocol_version,
        });

        let dave_version = match session_desc.dave_protocol_version {
            Some(version) => u16::try_from(version)
                .map_err(|_| anyhow!("dave_protocol_version excede o limite u16"))?,
            None => 0,
        };
        let initial_key_package = {
            let mut dave = dave_session
                .lock()
                .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?;
            dave.init_session(dave_version).with_context(|| {
                format!("Falha ao inicializar DAVE versão {dave_version}")
            })?;
            (dave_version > 0)
                .then(|| dave.create_key_package())
                .transpose()?
        };
        if let Some(key_package) = initial_key_package {
            gateway
                .send_binary(key_package)
                .await
                .context("Falha ao enviar Opcode 26 Key Package inicial")?;
        }

        let key_32: [u8; 32] = session_desc.secret_key.try_into().map_err(|v: Vec<u8>| {
            anyhow!(
                "Chave secreta de voz inválida: esperados 32 bytes, obtidos {}",
                v.len()
            )
        })?;
        let crypto = RtpCrypto::new(&session_desc.mode, &key_32)
            .context("Falha ao inicializar cifrador RTP com a chave recebida")?;
        let (udp_sender, udp_receiver) = voice_socket.into_connection(crypto).split();
        let keepalive_task = AbortOnDrop(udp_sender.spawn_keepalive_task(Duration::from_secs(5)));
        let (recv_task, media_error_rx) = spawn_udp_receiver(
            udp_receiver,
            ssrc_to_user.clone(),
            audio_bridge.clone(),
            frame_cipher.clone(),
            recv_packets.clone(),
            lost_packets.clone(),
        );

        ssrc_to_user
            .write()
            .await
            .insert(ready.ssrc, config.user_id);
        dave_session
            .lock()
            .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?
            .register_ssrc_user_mapping(ready.ssrc, config.user_id);

        Ok((
            MediaGeneration {
                id: generation_id,
                gateway: gateway.clone(),
                udp_sender,
                local_ssrc: ready.ssrc,
                dave_version,
                _keepalive_task: keepalive_task,
                _recv_task: recv_task,
            },
            events_rx,
            media_error_rx,
        ))
    }
    .await;

    if result.is_err() {
        let _ = gateway.disconnect().await;
    }
    result
}

/// Runs a handshake while continuously discarding captured audio. VAD events
/// still update `vad_active`, allowing SpeakingStarted to be re-announced with
/// the new SSRC once the replacement generation becomes active.
async fn await_handshake_discarding_audio<T>(
    handshake: impl std::future::Future<Output = Result<T>>,
    outgoing_rx: &mut mpsc::Receiver<Vec<u8>>,
    speaking_rx: &mut mpsc::Receiver<SpeakingEvent>,
    vad_active: &mut bool,
) -> Result<T> {
    tokio::pin!(handshake);
    loop {
        tokio::select! {
            result = &mut handshake => return result,
            Some(_) = outgoing_rx.recv() => {}
            Some(event) = speaking_rx.recv() => match event {
                SpeakingEvent::SpeakingStarted => *vad_active = true,
                SpeakingEvent::SpeakingStopped => *vad_active = false,
            },
        }
    }
}

fn dave_handshake_deadline(now: Instant) -> Instant {
    now + DAVE_HANDSHAKE_TIMEOUT
}

/// Derives the observable state from the current transport and DAVE readiness.
/// The deadline is created only when readiness is lost and is never extended by
/// heartbeats, audio frames, or unrelated gateway events.
fn refresh_connection_state(
    state_tx: &watch::Sender<VoiceSessionState>,
    gateway_resumed: bool,
    dave_version: u16,
    dave_ready: bool,
    dave_deadline: &mut Option<Instant>,
) -> bool {
    let can_stream = gateway_resumed && (dave_version == 0 || dave_ready);
    if can_stream {
        *dave_deadline = None;
        if !matches!(*state_tx.borrow(), VoiceSessionState::Connected) {
            let _ = state_tx.send(VoiceSessionState::Connected);
        }
        return true;
    }

    if gateway_resumed
        && dave_version > 0
        && !dave_ready
        && dave_deadline.is_none()
    {
        *dave_deadline = Some(dave_handshake_deadline(Instant::now()));
    }
    if !matches!(*state_tx.borrow(), VoiceSessionState::Disconnected { .. }) {
        let _ = state_tx.send(VoiceSessionState::Reconnecting);
    }
    false
}

async fn send_dave_transition_ready(
    gateway: &dyn VoiceGatewayTransport,
    transition_id: u16,
) -> Result<()> {
    let payload = serde_json::json!({
        "op": DaveOpcode::TransitionReady as u8,
        "d": { "transition_id": transition_id },
    });
    gateway.send_text(payload.to_string()).await
}

fn lock_dave_session<'a>(
    dave_session: &'a Arc<std::sync::Mutex<VoiceDaveSession>>,
) -> Result<std::sync::MutexGuard<'a, VoiceDaveSession>> {
    dave_session
        .lock()
        .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))
}

async fn send_dave_invalid_recovery(
    gateway: &dyn VoiceGatewayTransport,
    dave_session: &Arc<std::sync::Mutex<VoiceDaveSession>>,
    invalid: DaveInvalidCommitWelcomePayload,
) -> Result<()> {
    let payload = serde_json::json!({
        "op": DaveOpcode::MlsInvalidCommitWelcome as u8,
        "d": invalid,
    });
    gateway.send_text(payload.to_string()).await?;

    // O reset já foi feito por handle_announce_commit/handle_welcome. Um novo
    // Key Package é obrigatório no mesmo fluxo de recuperação.
    let key_package = lock_dave_session(dave_session)?.create_key_package()?;
    gateway
        .send_binary(format_client_binary_message(
            DaveOpcode::MlsKeyPackage,
            &key_package,
        ))
        .await
}

async fn process_dave_binary_message(
    gateway: &dyn VoiceGatewayTransport,
    dave_session: &Arc<std::sync::Mutex<VoiceDaveSession>>,
    data: &[u8],
) -> Result<()> {
    if data.len() > MAX_DAVE_BINARY_PAYLOAD {
        bail!(
            "Mensagem binária DAVE excede o limite de {} bytes",
            MAX_DAVE_BINARY_PAYLOAD
        );
    }
    let (_seq, opcode, payload) = parse_server_binary_message(data)?;
    match opcode {
        DaveOpcode::MlsExternalSender => {
            let response = lock_dave_session(dave_session)?.handle_external_sender(payload)?;
            gateway.send_binary(response).await?;
        }
        DaveOpcode::MlsProposals => {
            if payload.is_empty() {
                bail!("Opcode 27 Proposals sem operation_type");
            }
            let response =
                lock_dave_session(dave_session)?.handle_proposals(payload[0], &payload[1..], None)?;
            if let Some(response) = response {
                gateway.send_binary(response).await?;
            }
        }
        DaveOpcode::MlsAnnounceCommitTransition => {
            if payload.len() < 2 {
                bail!("Opcode 29 Announce Commit sem transition_id");
            }
            let transition_id = u16::from_be_bytes([payload[0], payload[1]]);
            let invalid = lock_dave_session(dave_session)?
                .handle_announce_commit(transition_id, &payload[2..])?;
            if let Some(invalid) = invalid {
                send_dave_invalid_recovery(gateway, dave_session, invalid).await?;
            } else {
                send_dave_transition_ready(gateway, transition_id).await?;
            }
        }
        DaveOpcode::MlsWelcome => {
            if payload.len() < 2 {
                bail!("Opcode 30 Welcome sem transition_id");
            }
            let transition_id = u16::from_be_bytes([payload[0], payload[1]]);
            let invalid =
                lock_dave_session(dave_session)?.handle_welcome(transition_id, &payload[2..])?;
            if let Some(invalid) = invalid {
                send_dave_invalid_recovery(gateway, dave_session, invalid).await?;
            } else {
                send_dave_transition_ready(gateway, transition_id).await?;
            }
        }
        _ => log::debug!("Opcode binário DAVE não esperado na sessão: {opcode:?}"),
    }
    Ok(())
}

async fn process_dave_gateway_event(
    gateway: &dyn VoiceGatewayTransport,
    dave_session: &Arc<std::sync::Mutex<VoiceDaveSession>>,
    event: VoiceGatewayEvent,
) -> Result<Option<u16>> {
    match event {
        VoiceGatewayEvent::DaveBinaryMessage(data) => {
            process_dave_binary_message(gateway, dave_session, &data).await?;
        }
        VoiceGatewayEvent::DavePrepareTransition(prep) => {
            {
                let mut dave = dave_session
                    .lock()
                    .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?;
                dave.handle_prepare_transition(prep.transition_id, prep.protocol_version);
            }
            if prep.transition_id == 0 {
                dave_session
                    .lock()
                    .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?
                    .handle_execute_transition(0)?;
                return Ok(Some(0));
            }
            send_dave_transition_ready(gateway, prep.transition_id).await?;
        }
        VoiceGatewayEvent::DaveExecuteTransition(exec) => {
            dave_session
                .lock()
                .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?
                .handle_execute_transition(exec.transition_id)?;
            return Ok(Some(exec.transition_id));
        }
        VoiceGatewayEvent::DavePrepareEpoch(epoch) => {
            let key_package = dave_session
                .lock()
                .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?
                .handle_prepare_epoch(epoch.epoch, epoch.protocol_version)?;
            if let Some(key_package) = key_package {
                gateway.send_binary(key_package).await?;
            }
        }
        _ => {}
    }
    Ok(None)
}

// ============================================================================
// 7. Loop de Execução e Handshake de Ponta a Ponta
// ============================================================================

async fn run_voice_session(
    config: VoiceSessionConfig,
    signaling: Arc<dyn MainSignalingChannel>,
    audio_bridge: Arc<Mutex<AudioBridge>>,
    frame_cipher: Arc<Mutex<Box<dyn FrameCipher>>>,
    dave_session: Arc<std::sync::Mutex<VoiceDaveSession>>,
    events_tx: broadcast::Sender<VoiceSessionEvent>,
    sent_packets: Arc<AtomicU64>,
    recv_packets: Arc<AtomicU64>,
    lost_packets: Arc<AtomicU64>,
    gateway_factory: VoiceGatewayFactory,
    state_tx: watch::Sender<VoiceSessionState>,
    active_gateway: Arc<Mutex<Option<Arc<dyn VoiceGatewayTransport>>>>,
    self_mute: Arc<AtomicBool>,
    self_deaf: Arc<AtomicBool>,
    ssrc_to_user: Arc<RwLock<HashMap<u32, u64>>>,
    ping_ms: Arc<AtomicU32>,
    entry_confirmed_tx: watch::Sender<bool>,
) -> Result<()> {
    ssrc_to_user.write().await.clear();
    let mut transport_rx = signaling.subscribe_events();
    let guild_str = config.guild_id.map(|id| id.to_string());
    let channel_str = config.channel_id.to_string();

    // CALL_CREATE/CALL_UPDATE confirmation remains active across reconnects.
    let mut call_events_rx = signaling.subscribe_events();
    let target_channel_for_call = channel_str.clone();
    let confirmed_tx_for_call = entry_confirmed_tx.clone();
    let call_listener_task = tokio::spawn(async move {
        loop {
            match call_events_rx.recv().await {
                Ok(TransportEvent::CallCreate {
                    channel_id,
                    ringing,
                    ..
                })
                | Ok(TransportEvent::CallUpdate {
                    channel_id,
                    ringing,
                    ..
                }) if channel_id == target_channel_for_call => {
                    log::info!(
                        "[VoiceDiag] CALL_CREATE/CALL_UPDATE recebido: sim, ringing_count={}",
                        ringing.len()
                    );
                    let _ = confirmed_tx_for_call.send(true);
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    let _call_listener_guard = AbortOnDrop(call_listener_task);

    let mut credentials = request_voice_credentials(
        &signaling,
        &mut transport_rx,
        &config,
        None,
        self_mute.load(Ordering::Relaxed),
        self_deaf.load(Ordering::Relaxed),
    )
    .await?;
    let _ = events_tx.send(VoiceSessionEvent::VoiceStateSent);
    let _ = events_tx.send(VoiceSessionEvent::VoiceServerReceived {
        endpoint: credentials.endpoint.clone(),
    });
    log::info!(
        "[VoiceDiag] Opcode 4 enviado: guild_presente={}",
        guild_str.is_some()
    );
    if !*entry_confirmed_tx.borrow() && config.guild_id.is_none() {
        log::info!("[VoiceDiag] CALL_CREATE recebido: não, ringing_count=0");
    }

    // Start capture once. Its receivers survive every media generation; frames
    // captured while negotiating are explicitly discarded rather than replayed.
    let (mut outgoing_rx, mut speaking_rx) = {
        let mut bridge = audio_bridge.lock().await;
        bridge
            .start_session()
            .await
            .context("Falha ao inicializar sessões de hardware de áudio")?;
        let outgoing = bridge
            .outgoing_rx
            .take()
            .ok_or_else(|| anyhow!("Receptor de áudio de saída já consumido"))?;
        let speaking = bridge
            .speaking_rx
            .take()
            .ok_or_else(|| anyhow!("Receptor de eventos de fala já consumido"))?;
        (outgoing, speaking)
    };

    let mut vad_active = false;
    let mut generation_id = 1;
    let initial_handshake = establish_media_generation(
        &config,
        &credentials,
        &gateway_factory,
        &dave_session,
        &events_tx,
        &ssrc_to_user,
        &audio_bridge,
        &frame_cipher,
        &recv_packets,
        &lost_packets,
        generation_id,
    );
    let (initial_gen, mut gateway_events_rx, mut media_error_rx) =
        match await_handshake_discarding_audio(
            initial_handshake,
            &mut outgoing_rx,
            &mut speaking_rx,
            &mut vad_active,
        )
        .await
    {
        Ok(generation) => generation,
        Err(err) => {
            audio_bridge.lock().await.stop_session().await;
            let _ = state_tx.send(VoiceSessionState::Disconnected {
                reason: format!("{err:#}"),
            });
            return Err(err);
        }
    };
    let mut active = ActiveGeneration::new(initial_gen);
    if let Some(generation) = active.current() {
        *active_gateway.lock().await = Some(generation.gateway.clone());
    }
    log::info!(
        "[VoiceSession] Geração {generation_id}: transporte UDP pronto; aguardando readiness de mídia."
    );

    let mut gateway_resumed = true;
    let mut speaking_active = false;
    let mut needs_speaking_stop = false;
    let mut dave_deadline: Option<Instant> = None;
    let mut close_reason: Option<String> = None;

    loop {
        let (current_id, current_gateway, current_ssrc, current_dave_version) =
            match active.current() {
                Some(generation) => (
                    generation.id,
                    generation.gateway.clone(),
                    generation.local_ssrc,
                    generation.dave_version,
                ),
                None => {
                    close_reason = Some("Geração de mídia ativa ausente".into());
                    break;
                }
            };

        let dave_ready = match dave_session.lock() {
            Ok(dave) => dave.is_ready(),
            Err(_) => {
                close_reason = Some("Lock envenenado em VoiceDaveSession".into());
                break;
            }
        };
        let can_stream = refresh_connection_state(
            &state_tx,
            gateway_resumed,
            current_dave_version,
            dave_ready,
            &mut dave_deadline,
        );

        // Re-announce VAD state with the current generation's SSRC before any
        // audio is released after Resume or NewSession.
        if can_stream && !speaking_active {
            if vad_active {
                if let Err(err) = current_gateway
                    .set_speaking(true, 0, current_ssrc)
                    .await
                {
                    close_reason = Some(format!("Falha ao reenviar SpeakingStarted: {err:#}"));
                    break;
                }
                speaking_active = true;
                let _ = events_tx.send(VoiceSessionEvent::Speaking {
                    ssrc: current_ssrc,
                    user_id: Some(config.user_id),
                    is_speaking: true,
                });
            } else if needs_speaking_stop {
                if let Err(err) = current_gateway
                    .set_speaking(false, 0, current_ssrc)
                    .await
                {
                    close_reason = Some(format!("Falha ao encerrar SpeakingStarted: {err:#}"));
                    break;
                }
                if let Some(generation) = active.current_mut() {
                    if let Err(err) = generation.udp_sender.send_silence_frames().await {
                        log::warn!("[VoiceSession] Falha ao enviar silêncio Opus: {err}");
                    }
                } else {
                    close_reason = Some("Geração de mídia ativa ausente".into());
                    break;
                }
                speaking_active = false;
                needs_speaking_stop = false;
                let _ = events_tx.send(VoiceSessionEvent::Speaking {
                    ssrc: current_ssrc,
                    user_id: Some(config.user_id),
                    is_speaking: false,
                });
            }
        }

        tokio::select! {
            Some(opus_frame) = outgoing_rx.recv() => {
                if !can_stream {
                    continue;
                }
                vad_active = true;
                if !speaking_active {
                    if let Err(err) = current_gateway
                        .set_speaking(true, 0, current_ssrc)
                        .await
                    {
                        close_reason = Some(format!("Falha ao enviar Opcode 5 inicial: {err:#}"));
                        break;
                    }
                    speaking_active = true;
                }

                let ciphered_frame = {
                    let mut cipher = frame_cipher.lock().await;
                    match cipher.encrypt_opus_frame(&opus_frame) {
                        Ok(frame) => frame,
                        Err(err) => {
                            log::debug!("[VoiceSession] Frame Opus descartado: {err}");
                            continue;
                        }
                    }
                };
                match active.current_mut() {
                    Some(generation) => {
                        if let Err(err) = generation.udp_sender.send_opus_frame(&ciphered_frame).await {
                            log::warn!("[VoiceSession] Erro ao transmitir quadro RTP: {err}");
                        } else {
                            sent_packets.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    None => {
                        close_reason = Some("Geração de mídia ativa ausente".into());
                        break;
                    }
                }
            }

            Some(event) = speaking_rx.recv() => {
                match event {
                    SpeakingEvent::SpeakingStarted => vad_active = true,
                    SpeakingEvent::SpeakingStopped => vad_active = false,
                }
                if !can_stream {
                    continue;
                }
                match event {
                    SpeakingEvent::SpeakingStarted => {
                        if let Err(err) = current_gateway
                            .set_speaking(true, 0, current_ssrc)
                            .await
                        {
                            close_reason = Some(format!("Falha ao anunciar SpeakingStarted: {err:#}"));
                            break;
                        }
                        speaking_active = true;
                        needs_speaking_stop = false;
                        let _ = events_tx.send(VoiceSessionEvent::Speaking {
                            ssrc: current_ssrc,
                            user_id: Some(config.user_id),
                            is_speaking: true,
                        });
                    }
                    SpeakingEvent::SpeakingStopped => {
                        if let Err(err) = current_gateway
                            .set_speaking(false, 0, current_ssrc)
                            .await
                        {
                            close_reason = Some(format!("Falha ao anunciar SpeakingStopped: {err:#}"));
                            break;
                        }
                        speaking_active = false;
                        needs_speaking_stop = false;
                        if let Some(generation) = active.current_mut() {
                            if let Err(err) = generation.udp_sender.send_silence_frames().await {
                                log::warn!("[VoiceSession] Falha ao enviar silêncio Opus: {err}");
                            }
                        } else {
                            close_reason = Some("Geração de mídia ativa ausente".into());
                            break;
                        }
                        let _ = events_tx.send(VoiceSessionEvent::Speaking {
                            ssrc: current_ssrc,
                            user_id: Some(config.user_id),
                            is_speaking: false,
                        });
                    }
                }
            }

            Ok(gw_event) = gateway_events_rx.recv() => {
                match gw_event {
                    VoiceGatewayEvent::Speaking(payload) => {
                        let ssrc = payload.ssrc;
                        let is_speaking = payload.speaking != 0;
                        let mut resolved_uid = None;
                        if let Some(user_id) = payload.user_id.as_deref().and_then(|id| id.parse().ok()) {
                            ssrc_to_user.write().await.insert(ssrc, user_id);
                            if let Ok(mut dave) = dave_session.lock() {
                                dave.register_ssrc_user_mapping(ssrc, user_id);
                            }
                            resolved_uid = Some(user_id);
                        }
                        if resolved_uid.is_none() {
                            resolved_uid = ssrc_to_user.read().await.get(&ssrc).copied();
                        }
                        let _ = events_tx.send(VoiceSessionEvent::Speaking {
                            ssrc,
                            user_id: resolved_uid,
                            is_speaking,
                        });
                    }
                    VoiceGatewayEvent::ClientsConnect(clients) => {
                        let result: Result<()> = async {
                            let mut ids = Vec::with_capacity(clients.user_ids.len());
                            for id in &clients.user_ids {
                                ids.push(id.parse::<u64>().with_context(|| {
                                    format!("User ID inválido em Clients Connect: {id}")
                                })?);
                            }
                            let mut dave = dave_session
                                .lock()
                                .map_err(|_| anyhow!("Lock envenenado em VoiceDaveSession"))?;
                            for id in ids {
                                dave.register_user_connected(id);
                            }
                            Ok(())
                        }
                        .await;
                        if let Err(err) = result {
                            close_reason = Some(format!("Falha DAVE Clients Connect: {err:#}"));
                            break;
                        }
                    }
                    VoiceGatewayEvent::ClientDisconnect(client) => {
                        let user_id = match client.user_id.parse::<u64>() {
                            Ok(user_id) => user_id,
                            Err(err) => {
                                close_reason = Some(format!("User ID inválido em Client Disconnect: {err}"));
                                break;
                            }
                        };
                        if let Ok(mut dave) = dave_session.lock() {
                            dave.register_user_disconnected(user_id);
                        }
                        ssrc_to_user.write().await.retain(|_, uid| *uid != user_id);
                    }
                    VoiceGatewayEvent::DaveBinaryMessage(_)
                    | VoiceGatewayEvent::DavePrepareTransition(_)
                    | VoiceGatewayEvent::DaveExecuteTransition(_)
                    | VoiceGatewayEvent::DavePrepareEpoch(_) => {
                        let result = process_dave_gateway_event(
                            current_gateway.as_ref(),
                            &dave_session,
                            gw_event,
                        )
                        .await;
                        match result {
                            Ok(Some(transition_id)) => {
                                let _ = events_tx.send(VoiceSessionEvent::DaveTransitionExecuted {
                                    transition_id,
                                });
                            }
                            Ok(None) => {}
                            Err(err) => {
                                let msg = format!("{err:#}");
                                if msg.contains("Voice gateway is not connected") {
                                    log::warn!(
                                        "[VoiceSession] Ignorando resposta DAVE enquanto Voice Gateway reconecta: {msg}"
                                    );
                                    continue;
                                }
                                close_reason = Some(format!("Falha no handshake DAVE: {msg}"));
                                break;
                            }
                        }
                    }
                    VoiceGatewayEvent::Disconnected { code, reason, decision } => {
                        log::info!(
                            "[VoiceSession] Geração {} desconectada: {reason} (decisão: {decision:?})",
                            current_id
                        );
                        let _ = events_tx.send(VoiceSessionEvent::Disconnected {
                            code,
                            reason: reason.clone(),
                        });
                        match decision {
                            VoiceCloseDecision::Resume => {
                                needs_speaking_stop = speaking_active && !vad_active;
                                gateway_resumed = false;
                                speaking_active = false;
                                let _ = state_tx.send(VoiceSessionState::Reconnecting);
                            }
                            VoiceCloseDecision::NewSession => {
                                gateway_resumed = false;
                                speaking_active = false;
                                needs_speaking_stop = false;
                                let _ = state_tx.send(VoiceSessionState::Reconnecting);

                                // Drop the old event receiver first so delayed
                                // Ready/DAVE/Disconnected events cannot affect
                                // the replacement generation.
                                drop(gateway_events_rx);
                                let old = match active.take() {
                                    Some(old) => old,
                                    None => {
                                        close_reason =
                                            Some("Geração de mídia ativa ausente".into());
                                        break;
                                    }
                                };
                                let old_gateway = old.gateway.clone();
                                drop(old);
                                let _ = old_gateway.disconnect().await;
                                ssrc_to_user.write().await.clear();

                                let credentials_future = request_voice_credentials(
                                    &signaling,
                                    &mut transport_rx,
                                    &config,
                                    Some(&credentials),
                                    self_mute.load(Ordering::Relaxed),
                                    self_deaf.load(Ordering::Relaxed),
                                );
                                credentials = match await_handshake_discarding_audio(
                                    credentials_future,
                                    &mut outgoing_rx,
                                    &mut speaking_rx,
                                    &mut vad_active,
                                )
                                .await
                                {
                                    Ok(next) => next,
                                    Err(err) => {
                                        close_reason = Some(format!(
                                            "Falha ao obter credenciais da nova sessão: {err:#}"
                                        ));
                                        break;
                                    }
                                };
                                let _ = events_tx.send(VoiceSessionEvent::VoiceStateSent);
                                let _ = events_tx.send(VoiceSessionEvent::VoiceServerReceived {
                                    endpoint: credentials.endpoint.clone(),
                                });

                                generation_id += 1;
                                let handshake = establish_media_generation(
                                    &config,
                                    &credentials,
                                    &gateway_factory,
                                    &dave_session,
                                    &events_tx,
                                    &ssrc_to_user,
                                    &audio_bridge,
                                    &frame_cipher,
                                    &recv_packets,
                                    &lost_packets,
                                    generation_id,
                                );
                                let (generation, events, errors) = match await_handshake_discarding_audio(
                                    handshake,
                                    &mut outgoing_rx,
                                    &mut speaking_rx,
                                    &mut vad_active,
                                )
                                .await
                                {
                                    Ok(next) => next,
                                    Err(err) => {
                                        close_reason = Some(format!(
                                            "Falha no handshake da nova geração: {err:#}"
                                        ));
                                        break;
                                    }
                                };
                                *active_gateway.lock().await = Some(generation.gateway.clone());
                                active = ActiveGeneration::new(generation);
                                gateway_events_rx = events;
                                media_error_rx = errors;
                                gateway_resumed = true;
                                dave_deadline = None;
                                log::info!(
                                    "[VoiceSession] Geração {generation_id}: nova mídia negociada."
                                );
                            }
                            VoiceCloseDecision::Fatal => {
                                close_reason = Some(reason);
                                break;
                            }
                        }
                    }
                    VoiceGatewayEvent::HeartbeatAck { latency_ms } => {
                        if let Some(latency) = latency_ms {
                            let latency = u32::try_from(latency).unwrap_or(u32::MAX);
                            ping_ms.store(latency, Ordering::Relaxed);
                            let _ = events_tx.send(VoiceSessionEvent::HeartbeatAck {
                                ping_ms: latency,
                            });
                        }
                    }
                    VoiceGatewayEvent::Resumed => {
                        gateway_resumed = true;
                        log::info!("[VoiceSession] Voice Gateway retomou a sessão de mídia.");
                    }
                    // WebSocket Connected and duplicate Ready/SessionDescription
                    // events are not proof that this generation can stream.
                    VoiceGatewayEvent::Connected
                    | VoiceGatewayEvent::Ready(_)
                    | VoiceGatewayEvent::SessionDescription(_) => {}
                }
            }

            Some(reason) = media_error_rx.recv() => {
                close_reason = Some(reason);
                break;
            }

            _ = async {
                if let Some(deadline) = dave_deadline {
                    tokio::time::sleep_until(deadline.into()).await;
                } else {
                    std::future::pending::<()>().await;
                }
            }, if dave_deadline.is_some() => {
                let reason = "Timeout aguardando o handshake MLS do DAVE".to_string();
                log::error!("[VoiceSession] {reason}");
                let _ = events_tx.send(VoiceSessionEvent::Disconnected {
                    code: None,
                    reason: reason.clone(),
                });
                close_reason = Some(reason);
                break;
            }

            else => break,
        }
    }

    if let Some(generation) = active.take() {
        let gateway = generation.gateway.clone();
        drop(generation);
        let _ = gateway.disconnect().await;
    }
    *active_gateway.lock().await = None;
    audio_bridge.lock().await.stop_session().await;

    let final_reason = close_reason
        .clone()
        .unwrap_or_else(|| "Sessão finalizada".to_string());
    let _ = state_tx.send(VoiceSessionState::Disconnected {
        reason: final_reason.clone(),
    });
    if close_reason.is_some() {
        Err(anyhow!(final_reason))
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingGateway {
        texts: Arc<std::sync::Mutex<Vec<String>>>,
        binaries: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    }

    impl RecordingGateway {
        fn new() -> Self {
            Self {
                texts: Arc::new(std::sync::Mutex::new(Vec::new())),
                binaries: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl VoiceGatewayTransport for RecordingGateway {
        async fn connect(&self) -> Result<()> {
            Ok(())
        }

        async fn wait_for_ready(&self, _timeout: Duration) -> Result<VoiceReady> {
            bail!("not used")
        }

        async fn select_protocol(
            &self,
            _external_ip: &str,
            _external_port: u16,
            _mode: &str,
        ) -> Result<SessionDescription> {
            bail!("not used")
        }

        async fn set_speaking(&self, _speaking: bool, _delay: u32, _ssrc: u32) -> Result<()> {
            Ok(())
        }

        fn subscribe_events(&self) -> broadcast::Receiver<VoiceGatewayEvent> {
            let (tx, rx) = broadcast::channel(1);
            drop(tx);
            rx
        }

        async fn send_text(&self, text: String) -> Result<()> {
            self.texts.lock().unwrap().push(text);
            Ok(())
        }

        async fn send_binary(&self, data: Vec<u8>) -> Result<()> {
            self.binaries.lock().unwrap().push(data);
            Ok(())
        }

        async fn disconnect(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_external_invalid_commit_sends_31_and_fresh_26() {
        let dave = Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(100, 200)));
        dave.lock().unwrap().init_session(1).unwrap();
        let gateway = RecordingGateway::new();

        process_dave_binary_message(&gateway, &dave, &[0x00, 0x01, 29, 0x00, 0x07, 0xAA])
            .await
            .unwrap();

        let texts = gateway.texts.lock().unwrap();
        assert_eq!(texts.len(), 1);
        let value: serde_json::Value = serde_json::from_str(&texts[0]).unwrap();
        assert_eq!(value["op"], 31);
        assert_eq!(value["d"]["transition_id"], 7);
        let binaries = gateway.binaries.lock().unwrap();
        assert_eq!(binaries.len(), 1);
        assert_eq!(binaries[0][0], 26);
        assert!(binaries[0].len() > 1);
    }

    #[tokio::test]
    async fn test_external_malformed_external_sender_is_observable_error() {
        let dave = Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(100, 200)));
        dave.lock().unwrap().init_session(1).unwrap();
        let gateway = RecordingGateway::new();

        assert!(
            process_dave_binary_message(&gateway, &dave, &[0x00, 0x01, 25, 0xAA])
                .await
                .is_err()
        );
        assert!(gateway.texts.lock().unwrap().is_empty());
        assert!(gateway.binaries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_transition_ready_uses_v8_json_shape() {
        let gateway = RecordingGateway::new();
        send_dave_transition_ready(&gateway, 42).await.unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&gateway.texts.lock().unwrap()[0]).unwrap();
        assert_eq!(value["op"], 23);
        assert_eq!(value["d"]["transition_id"], 42);
    }

    #[tokio::test]
    async fn test_truncated_and_unknown_dave_binary_payloads_are_errors() {
        let dave = Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(100, 200)));
        dave.lock().unwrap().init_session(1).unwrap();
        let gateway = RecordingGateway::new();

        let fixtures: &[&[u8]] = &[
            &[],
            &[0x00],
            &[0x00, 0x01],
            &[0x00, 0x01, 29],
            &[0x00, 0x01, 29, 0x00],
            &[0x00, 0x01, 30],
            &[0x00, 0x01, 27],
            &[0x00, 0x01, 0xFF],
        ];
        for fixture in fixtures {
            assert!(
                process_dave_binary_message(&gateway, &dave, fixture)
                    .await
                    .is_err(),
                "fixture {fixture:?} deveria falhar com erro de domínio"
            );
        }
        assert!(gateway.texts.lock().unwrap().is_empty());
        assert!(gateway.binaries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_oversized_dave_binary_payload_is_rejected_before_parse() {
        let dave = Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(100, 200)));
        dave.lock().unwrap().init_session(1).unwrap();
        let gateway = RecordingGateway::new();

        let oversized = vec![0u8; MAX_DAVE_BINARY_PAYLOAD + 1];
        let err = process_dave_binary_message(&gateway, &dave, &oversized)
            .await
            .expect_err("payload acima do limite deve ser recusado");
        assert!(err.to_string().contains("limite"));
        assert!(gateway.texts.lock().unwrap().is_empty());
        assert!(gateway.binaries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_poisoned_dave_mutex_is_domain_error_not_panic() {
        let dave = Arc::new(std::sync::Mutex::new(VoiceDaveSession::new(100, 200)));
        let dave_poison = dave.clone();
        let _ = std::thread::spawn(move || {
            let _guard = dave_poison.lock().expect("lock recém-criado");
            panic!("poison intencional para o teste");
        })
        .join();
        let gateway = RecordingGateway::new();

        let result = process_dave_binary_message(&gateway, &dave, &[0x00, 0x01, 25]).await;
        assert!(result.is_err(), "mutex envenenado deve virar erro de domínio");
        assert!(gateway.texts.lock().unwrap().is_empty());
        assert!(gateway.binaries.lock().unwrap().is_empty());
    }
}
