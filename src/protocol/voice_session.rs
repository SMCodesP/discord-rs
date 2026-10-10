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
//! 6. **Ponto de Extensão DAVE (E2EE com MLS)**: Trait [`FrameCipher`] entre a codificação Opus
//!    e a cifragem de transporte RTP. Fornece [`PassthroughFrameCipher`] para testes locais e desenvolvimento.
//!
//! # Aviso Arquitetural Mandatório sobre o Protocolo DAVE
//!
//! **IMPORTANTE**: Desde 1º de março de 2026, a infraestrutura oficial do Discord só aceita
//! chamadas de voz com DAVE (Discord Audio Video End-to-end Encryption baseado em MLS / RFC 9420).
//! Portanto, o [`PassthroughFrameCipher`] fornecido por padrão repassa os quadros Opus sem a camada
//! criptográfica MLS do DAVE. Ao conectar em salas de produção do Discord onde o DAVE é obrigatório,
//! o servidor recusará a conexão com o código de fechamento WebSocket **4017** (`E2EE/DAVE protocol required`).
//! O [`PassthroughFrameCipher`] existe estritamente para testes locais offline, desenvolvimento e como
//! contrato de integração com a implementação completa do agente VoiceGW.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use tokio::sync::{broadcast, mpsc, oneshot, watch, Mutex, RwLock};

use crate::domain::VoiceState;
use crate::media::capture::SpeakingEvent;
use crate::media::{AudioEngine, AudioError};
use crate::protocol::discord_api::DiscordApiClient;
use crate::protocol::transport::{DiscordTransport, TransportEvent};
use crate::protocol::voice_crypto::RtpCrypto;
use crate::protocol::voice_gateway::{
    SessionDescription, VoiceCloseDecision, VoiceGateway, VoiceGatewayConfig, VoiceGatewayEvent,
    VoiceReady, VoiceServerData,
};
use crate::protocol::voice_udp::{VoicePacket, VoiceUdpSocket};

// ============================================================================
// 1. Ponto de Extensão DAVE (FrameCipher)
// ============================================================================

/// Contrato para cifradores de quadros Opus entre a camada de áudio e o transporte RTP.
///
/// O agente VoiceGW fornecerá a implementação de produção com MLS / DAVE.
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
        <Self as DiscordTransport>::send_voice_state_update(self, guild_id, channel_id, self_mute, self_deaf)
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
}

impl AudioBridge {
    /// Constrói a ponte executando o [`AudioEngine`] em uma thread dedicada do SO.
    pub fn from_engine_factory<F>(factory: F) -> Self
    where
        F: FnOnce() -> AudioEngine + Send + 'static,
    {
        let (init_tx, init_rx) = std::sync::mpsc::channel();
        let (command_tx, mut command_rx) = mpsc::channel::<AudioBridgeCommand>(256);

        std::thread::Builder::new()
            .name("discord-audio-engine".into())
            .spawn(move || {
                let mut engine = factory();
                let outgoing_rx = engine.take_outgoing_receiver();
                let speaking_rx = engine.take_speaking_receiver();
                let _ = init_tx.send((outgoing_rx, speaking_rx));

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
                        AudioBridgeCommand::ReceiveOpusPacket { ssrc, opus_data, .. } => {
                            let _ = engine.receive_opus_packet(ssrc, &opus_data);
                        }
                        AudioBridgeCommand::Shutdown => {
                            engine.stop_session();
                            break;
                        }
                    }
                }
            })
            .expect("Falha ao iniciar thread dedicada do AudioEngine");

        let (outgoing_rx, speaking_rx) = init_rx
            .recv()
            .unwrap_or((None, None));

        Self {
            outgoing_rx,
            speaking_rx,
            command_tx,
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
        }
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
        let _ = self.command_tx.send(AudioBridgeCommand::SetMuted(muted)).await;
    }

    /// Atualiza ensurdecimento local.
    pub async fn set_deafened(&self, deafened: bool) {
        let _ = self.command_tx.send(AudioBridgeCommand::SetDeafened(deafened)).await;
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

    /// Encerra a thread de áudio.
    pub async fn shutdown(&self) {
        let _ = self.command_tx.send(AudioBridgeCommand::Shutdown).await;
    }
}

// ============================================================================
// 6. Configuração e Estrutura Principal da Sessão de Voz (VoiceSession)
// ============================================================================

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
    gateway_factory: VoiceGatewayFactory,
    session_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    active_gateway: Arc<Mutex<Option<Arc<dyn VoiceGatewayTransport>>>>,
    self_mute: Arc<AtomicBool>,
    self_deaf: Arc<AtomicBool>,
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

        let self_mute = Arc::new(AtomicBool::new(config.self_mute));
        let self_deaf = Arc::new(AtomicBool::new(config.self_deaf));

        Self {
            config,
            state_tx,
            state_rx,
            signaling,
            audio_bridge: Arc::new(Mutex::new(audio_bridge)),
            frame_cipher: Arc::new(Mutex::new(Box::new(PassthroughFrameCipher))),
            gateway_factory: Arc::new(|cfg| Arc::new(VoiceGateway::new(cfg))),
            session_task: Arc::new(Mutex::new(None)),
            active_gateway: Arc::new(Mutex::new(None)),
            self_mute,
            self_deaf,
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

    /// Configura um cifrador de quadros customizado (ex.: DAVE MLS do agente VoiceGW).
    pub fn with_cipher<C: FrameCipher + 'static>(self, cipher: C) -> Self {
        *self.frame_cipher.try_lock().expect("cipher mutex uncontended at build time") = Box::new(cipher);
        self
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

    /// Altera o estado de microfone local (mute).
    pub async fn set_muted(&self, muted: bool) -> Result<()> {
        self.self_mute.store(muted, Ordering::Relaxed);
        self.audio_bridge.lock().await.set_muted(muted).await;

        let guild_id_str = self.config.guild_id.map(|g| g.to_string());
        let channel_id_str = self.config.channel_id.to_string();
        let deaf = self.self_deaf.load(Ordering::Relaxed);

        self.signaling
            .send_voice_state_update(guild_id_str.as_deref(), Some(&channel_id_str), muted, deaf)
            .await
    }

    /// Altera o estado de ensurdecimento local (deafen).
    pub async fn set_deafened(&self, deafened: bool) -> Result<()> {
        self.self_deaf.store(deafened, Ordering::Relaxed);
        self.audio_bridge.lock().await.set_deafened(deafened).await;

        let guild_id_str = self.config.guild_id.map(|g| g.to_string());
        let channel_id_str = self.config.channel_id.to_string();
        let mute = self.self_mute.load(Ordering::Relaxed);

        self.signaling
            .send_voice_state_update(guild_id_str.as_deref(), Some(&channel_id_str), mute, deafened)
            .await
    }

    /// Inicia o processo de conexão em segundo plano.
    pub async fn start(&self) -> Result<()> {
        let _ = self.state_tx.send(VoiceSessionState::Connecting);

        let config = self.config.clone();
        let signaling = self.signaling.clone();
        let audio_bridge = self.audio_bridge.clone();
        let frame_cipher = self.frame_cipher.clone();
        let gateway_factory = self.gateway_factory.clone();
        let state_tx = self.state_tx.clone();
        let active_gateway = self.active_gateway.clone();
        let self_mute = self.self_mute.clone();
        let self_deaf = self.self_deaf.clone();

        let handle = tokio::spawn(async move {
            if let Err(err) = run_voice_session(
                config,
                signaling,
                audio_bridge,
                frame_cipher,
                gateway_factory,
                state_tx.clone(),
                active_gateway,
                self_mute,
                self_deaf,
            )
            .await
            {
                log::error!("[VoiceSession] Sessão encerrou com erro: {err:?}");
                let _ = state_tx.send(VoiceSessionState::Disconnected {
                    reason: format!("{err:#}"),
                });
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
        log::info!("[VoiceSession] Desconectando sessão de voz...");

        // 1. Envia Opcode 4 com channel_id nulo no canal de sinalização principal
        let guild_id_str = self.config.guild_id.map(|g| g.to_string());
        let _ = self
            .signaling
            .send_voice_state_update(guild_id_str.as_deref(), None, false, false)
            .await;

        // 2. Desconecta gateway de voz ativo
        let mut gw_lock = self.active_gateway.lock().await;
        if let Some(gw) = gw_lock.take() {
            let _ = gw.disconnect().await;
        }

        // 3. Cancela a tarefa de sessão
        let mut task_lock = self.session_task.lock().await;
        if let Some(task) = task_lock.take() {
            task.abort();
        }

        // 4. Para a sessão no motor de áudio
        self.audio_bridge.lock().await.stop_session().await;

        // 5. Transita o estado
        let _ = self.state_tx.send(VoiceSessionState::Disconnected {
            reason: "Desconectado pelo usuário".into(),
        });

        Ok(())
    }
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        if let Ok(mut lock) = self.session_task.try_lock() {
            if let Some(task) = lock.take() {
                task.abort();
            }
        }
    }
}

// ============================================================================
// 7. Loop de Execução e Handshake de Ponta a Ponta
// ============================================================================

async fn run_voice_session(
    config: VoiceSessionConfig,
    signaling: Arc<dyn MainSignalingChannel>,
    audio_bridge: Arc<Mutex<AudioBridge>>,
    frame_cipher: Arc<Mutex<Box<dyn FrameCipher>>>,
    gateway_factory: VoiceGatewayFactory,
    state_tx: watch::Sender<VoiceSessionState>,
    active_gateway: Arc<Mutex<Option<Arc<dyn VoiceGatewayTransport>>>>,
    self_mute: Arc<AtomicBool>,
    self_deaf: Arc<AtomicBool>,
) -> Result<()> {
    // ------------------------------------------------------------------------
    // Passo 1: Subscrever eventos do transporte principal e disparar Opcode 4
    // ------------------------------------------------------------------------
    let mut transport_rx = signaling.subscribe_events();

    let guild_str = config.guild_id.map(|g| g.to_string());
    let channel_str = config.channel_id.to_string();
    let initial_mute = self_mute.load(Ordering::Relaxed);
    let initial_deaf = self_deaf.load(Ordering::Relaxed);

    signaling
        .send_voice_state_update(guild_str.as_deref(), Some(&channel_str), initial_mute, initial_deaf)
        .await
        .context("Falha ao enviar Opcode 4 (Voice State Update)")?;

    // ------------------------------------------------------------------------
    // Passo 2: Aguardar session_id + token + endpoint
    // ------------------------------------------------------------------------
    let mut captured_session_id = signaling.current_voice_session_id().await;
    let mut captured_token: Option<String> = None;
    let mut captured_endpoint: Option<String> = None;

    if let Some(server) = signaling.current_voice_server().await {
        captured_token = Some(server.token);
        captured_endpoint = server.endpoint;
    }

    let discovery_timeout = Duration::from_secs(15);
    tokio::time::timeout(discovery_timeout, async {
        while captured_session_id.is_none() || captured_token.is_none() || captured_endpoint.is_none() {
            match transport_rx.recv().await {
                Ok(TransportEvent::VoiceStateUpdate(VoiceState {
                    user_id,
                    session_id,
                    ..
                })) if user_id == config.user_id.to_string() && !session_id.is_empty() => {
                    captured_session_id = Some(session_id);
                }
                Ok(TransportEvent::VoiceServerUpdate {
                    token,
                    endpoint,
                    ..
                }) => {
                    captured_token = Some(token);
                    captured_endpoint = endpoint;
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => {
                    bail!("Canal de eventos do transporte principal fechado prematuramente");
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| anyhow!("Timeout aguardando session_id, token e endpoint do Discord"))??;

    let session_id = captured_session_id.unwrap();
    let token = captured_token.unwrap();
    let raw_endpoint = captured_endpoint.ok_or_else(|| anyhow!("Servidor não forneceu endpoint de voz"))?;

    // Remove eventuais sufixos de porta (:80 ou :443) do endpoint
    let endpoint = raw_endpoint
        .split(':')
        .next()
        .unwrap_or(&raw_endpoint)
        .to_string();

    log::info!(
        "[VoiceSession] Handshake inicial concluído. Endpoint: {endpoint}, SessionID: {session_id}"
    );

    // ------------------------------------------------------------------------
    // Passo 3: Conectar Voice Gateway v8 e aguardar Opcode 2 Ready
    // ------------------------------------------------------------------------
    let gw_config = VoiceGatewayConfig {
        endpoint,
        server_id: guild_str.unwrap_or_else(|| channel_str.clone()),
        user_id: config.user_id.to_string(),
        session_id,
        token,
        max_dave_protocol_version: Some(0),
    };

    let gateway = (gateway_factory)(gw_config);
    *active_gateway.lock().await = Some(gateway.clone());

    gateway.connect().await.context("Falha ao conectar Voice Gateway")?;

    let ready = gateway
        .wait_for_ready(Duration::from_secs(10))
        .await
        .context("Falha ao aguardar Opcode 2 Ready do Voice Gateway")?;

    log::info!(
        "[VoiceSession] Voice Gateway Ready recebido: SSRC={}, IP={}, Porta={}, Modos={:?}",
        ready.ssrc,
        ready.ip,
        ready.port,
        ready.modes
    );

    // ------------------------------------------------------------------------
    // Passo 4: Conectar socket UDP e executar IP Discovery no MESMO socket
    // ------------------------------------------------------------------------
    let voice_socket = VoiceUdpSocket::connect(&ready.ip, ready.port, ready.ssrc)
        .await
        .context("Falha ao conectar VoiceUdpSocket")?;

    let (discovered_ip, discovered_port) = voice_socket
        .discover_ip()
        .await
        .context("Falha durante IP Discovery sobre o socket UDP")?;

    log::info!(
        "[VoiceSession] IP Discovery concluído com sucesso: {}:{}",
        discovered_ip,
        discovered_port
    );

    // ------------------------------------------------------------------------
    // Passo 5: Opcode 1 Select Protocol e Opcode 4 Session Description
    // ------------------------------------------------------------------------
    let selected_mode = ready.select_best_mode();
    let session_desc = gateway
        .select_protocol(&discovered_ip, discovered_port, selected_mode)
        .await
        .context("Falha ao selecionar protocolo e aguardar Session Description")?;

    log::info!(
        "[VoiceSession] Session Description recebida com sucesso: modo={}",
        session_desc.mode
    );

    let key_32: [u8; 32] = session_desc
        .secret_key
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("Chave secreta de voz inválida: esperados 32 bytes, obtidos {}", v.len()))?;

    let crypto = RtpCrypto::new(&session_desc.mode, &key_32)
        .context("Falha ao inicializar cifrador RTP com a chave recebida")?;

    // ------------------------------------------------------------------------
    // Passo 6: Transição para VoiceUdpConnection e inicialização do Keepalive
    // ------------------------------------------------------------------------
    let connection = voice_socket.into_connection(crypto);
    let (mut udp_sender, udp_receiver) = connection.split();

    // Tarefa periódica de keepalive UDP a cada 5 segundos
    let keepalive_task = udp_sender.spawn_keepalive_task(Duration::from_secs(5));

    // ------------------------------------------------------------------------
    // Passo 7: Iniciar motor de áudio e configurar streaming de mídia
    // ------------------------------------------------------------------------
    let (mut outgoing_rx, mut speaking_rx) = {
        let mut bridge = audio_bridge.lock().await;
        bridge
            .start_session()
            .await
            .context("Falha ao inicializar sessões de hardware de áudio")?;

        let out = bridge
            .outgoing_rx
            .take()
            .ok_or_else(|| anyhow!("Receptor de áudio de saída já consumido"))?;
        let spk = bridge
            .speaking_rx
            .take()
            .ok_or_else(|| anyhow!("Receptor de eventos de fala já consumido"))?;
        (out, spk)
    };

    let mut gateway_events_rx = gateway.subscribe_events();
    let local_ssrc = ready.ssrc;

    // Mapa SSRC -> User ID alimentado pelos eventos de Speaking
    let ssrc_to_user: Arc<RwLock<HashMap<u32, u64>>> = Arc::new(RwLock::new(HashMap::new()));

    // Sessão agora está completamente conectada!
    let _ = state_tx.send(VoiceSessionState::Connected);
    log::info!("[VoiceSession] Sessão de voz completamente conectada e pronta para áudio.");

    // ------------------------------------------------------------------------
    // Passo 8: Loops de Mídia Concorrentes
    // ------------------------------------------------------------------------
    let ssrc_map_clone = ssrc_to_user.clone();
    let audio_bridge_clone = audio_bridge.clone();
    let frame_cipher_clone = frame_cipher.clone();

    // Loop de Recepção UDP (áudio remoto)
    let recv_handle = tokio::spawn(async move {
        loop {
            match udp_receiver.recv_packet().await {
                Ok(VoicePacket {
                    ssrc,
                    sequence,
                    timestamp,
                    payload,
                }) => {
                    let sender_user_id = {
                        let map = ssrc_map_clone.read().await;
                        map.get(&ssrc).copied().unwrap_or(ssrc as u64)
                    };

                    let decrypted_opus = {
                        let mut cipher = frame_cipher_clone.lock().await;
                        match cipher.decrypt_opus_frame(sender_user_id, &payload) {
                            Ok(frame) => frame,
                            Err(e) => {
                                log::trace!("[VoiceSession] Falha ao decifrar quadro DAVE: {e}");
                                continue;
                            }
                        }
                    };

                    let bridge = audio_bridge_clone.lock().await;
                    bridge
                        .receive_opus_packet(ssrc, sequence, timestamp, decrypted_opus)
                        .await;
                }
                Err(err) => {
                    log::debug!("[VoiceSession] Erro no loop de recepção UDP: {err}");
                    break;
                }
            }
        }
    });

    // Loop de Transmissão UDP e Eventos de Fala (áudio local)
    let mut close_reason: Option<String> = None;

    loop {
        tokio::select! {
            // 1. Quadro Opus capturado pelo microfone -> FrameCipher -> Envio RTP UDP
            Some(opus_frame) = outgoing_rx.recv() => {
                let ciphered_frame = {
                    let mut cipher = frame_cipher.lock().await;
                    cipher.encrypt_opus_frame(&opus_frame)?
                };

                if let Err(err) = udp_sender.send_opus_frame(&ciphered_frame).await {
                    log::warn!("[VoiceSession] Erro ao transmitir quadro RTP: {err}");
                }
            }

            // 2. Transições de fala (VAD) -> Opcode 5 Speaking + 5 quadros de silêncio ao parar
            Some(event) = speaking_rx.recv() => {
                match event {
                    SpeakingEvent::SpeakingStarted => {
                        let _ = gateway.set_speaking(true, 0, local_ssrc).await;
                    }
                    SpeakingEvent::SpeakingStopped => {
                        let _ = gateway.set_speaking(false, 0, local_ssrc).await;
                        let _ = udp_sender.send_silence_frames().await;
                    }
                }
            }

            // 3. Eventos do Voice Gateway (alimenta SSRC map e monitora fechamento)
            Ok(gw_event) = gateway_events_rx.recv() => {
                match gw_event {
                    VoiceGatewayEvent::Speaking(speaking_payload) => {
                        if let Some(ref uid_str) = speaking_payload.user_id {
                            if let Ok(uid) = uid_str.parse::<u64>() {
                                ssrc_to_user.write().await.insert(speaking_payload.ssrc, uid);
                            }
                        }
                    }
                    VoiceGatewayEvent::Disconnected { code: _, reason, decision } => {
                        log::info!("[VoiceSession] Voice Gateway desconectado: {reason} (decisão: {decision:?})");
                        match decision {
                            VoiceCloseDecision::Resume | VoiceCloseDecision::NewSession => {
                                let _ = state_tx.send(VoiceSessionState::Reconnecting);
                            }
                            VoiceCloseDecision::Fatal => {
                                close_reason = Some(reason);
                                break;
                            }
                        }
                    }
                    VoiceGatewayEvent::Connected | VoiceGatewayEvent::Resumed => {
                        let _ = state_tx.send(VoiceSessionState::Connected);
                    }
                    _ => {}
                }
            }

            else => {
                break;
            }
        }
    }

    // ------------------------------------------------------------------------
    // Passo 9: Limpeza dos Recursos
    // ------------------------------------------------------------------------
    keepalive_task.abort();
    recv_handle.abort();

    audio_bridge.lock().await.stop_session().await;

    let final_reason = close_reason.unwrap_or_else(|| "Sessão finalizada".to_string());
    let _ = state_tx.send(VoiceSessionState::Disconnected { reason: final_reason });

    Ok(())
}
