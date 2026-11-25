//! Discord RS - Client Manager & Real-Time Bridge
//! Coordinates the active transport (live Gateway/REST Discord API or deterministic mock),
//! processes incoming WebSocket Gateway events, and provides thread-safe messaging.

use anyhow::Result;
use log::{error, info, warn};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, broadcast};

use crate::domain::channel::{Channel, ChannelType};
use crate::domain::guild::Guild;
use crate::domain::message::Message;
use crate::protocol::auth::AuthToken;
use crate::protocol::discord_api::DiscordApiClient;
use crate::protocol::idle_transport::IdleTransport;
use crate::protocol::mock_provider::MockDiscordProvider;
use crate::protocol::transport::{DiscordTransport, Relationship, TransportEvent};

/// Global transport connection mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionMode {
    Mock,
    Live,
    LiveDiscord(String), // username/tag
    Connecting,
    Reconnecting { attempt: u32, delay_ms: u64, reason: String },
    Disconnected(String),
}

/// Orçamento padrão de encerramento do ClientManager (voz + gateway).
pub const CLIENT_MANAGER_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(150);

/// Thread-safe client manager coordinating REST and Gateway events
#[derive(Clone)]
pub struct ClientManager {
    transport: Arc<RwLock<Arc<dyn DiscordTransport>>>,
    mode: Arc<RwLock<ConnectionMode>>,
    audio_hardware_probe: Option<bool>,
    event_tx: broadcast::Sender<TransportEvent>,
    real_client: Arc<RwLock<Option<Arc<DiscordApiClient>>>>,
    active_voice: Arc<RwLock<Option<Arc<crate::protocol::voice_session::VoiceSession>>>>,
    voice_state_tx: broadcast::Sender<(
        crate::protocol::voice_session::VoiceSessionState,
        Option<u32>,
    )>,
    voice_event_tx: broadcast::Sender<crate::protocol::voice_session::VoiceSessionEvent>,
    voice_gateway_factory: Arc<RwLock<Option<crate::protocol::voice_session::VoiceGatewayFactory>>>,
    active_dm_calls: Arc<RwLock<std::collections::HashSet<String>>>,
    shutdown_started: Arc<AtomicBool>,
    background_tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

fn remaining_budget(deadline: tokio::time::Instant) -> Duration {
    deadline
        .saturating_duration_since(tokio::time::Instant::now())
        .max(Duration::from_millis(1))
}

impl std::fmt::Debug for ClientManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientManager")
            .field("mode", &"<ConnectionMode>")
            .finish()
    }
}

impl ClientManager {
    /// Creates a new production ClientManager initialized with an idle transport.
    /// Mode starts as Disconnected("Não conectado").
    pub fn new() -> (Self, broadcast::Receiver<TransportEvent>) {
        let idle = Arc::new(IdleTransport::new());
        let (event_tx, event_rx) = broadcast::channel(256);
        let (voice_state_tx, _) = broadcast::channel(64);
        let (voice_event_tx, _) = broadcast::channel(64);

        let manager = Self {
            transport: Arc::new(RwLock::new(idle)),
            mode: Arc::new(RwLock::new(ConnectionMode::Disconnected(
                "Não conectado".to_string(),
            ))),
            audio_hardware_probe: None,
            event_tx,
            real_client: Arc::new(RwLock::new(None)),
            active_voice: Arc::new(RwLock::new(None)),
            voice_state_tx,
            voice_event_tx,
            voice_gateway_factory: Arc::new(RwLock::new(None)),
            active_dm_calls: Arc::new(RwLock::new(std::collections::HashSet::new())),
            shutdown_started: Arc::new(AtomicBool::new(false)),
            background_tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        (manager, event_rx)
    }

    /// Somente testes: cria um ClientManager inicializado com o provedor Mock determinístico.
    #[doc(hidden)]
    pub fn new_mock() -> (Self, broadcast::Receiver<TransportEvent>) {
        let mock = Arc::new(MockDiscordProvider::new());
        let (event_tx, event_rx) = broadcast::channel(256);
        let (voice_state_tx, _) = broadcast::channel(64);
        let (voice_event_tx, _) = broadcast::channel(64);

        let manager = Self {
            transport: Arc::new(RwLock::new(mock)),
            mode: Arc::new(RwLock::new(ConnectionMode::Mock)),
            audio_hardware_probe: None,
            event_tx,
            real_client: Arc::new(RwLock::new(None)),
            active_voice: Arc::new(RwLock::new(None)),
            voice_state_tx,
            voice_event_tx,
            voice_gateway_factory: Arc::new(RwLock::new(None)),
            active_dm_calls: Arc::new(RwLock::new(std::collections::HashSet::new())),
            shutdown_started: Arc::new(AtomicBool::new(false)),
            background_tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        (manager, event_rx)
    }

    /// Somente testes: cria um ClientManager com o provedor de transporte fornecido.
    #[doc(hidden)]
    pub fn new_with_transport(
        transport: Arc<dyn DiscordTransport>,
    ) -> (Self, broadcast::Receiver<TransportEvent>) {
        Self::new_with_transport_with_audio_probe(transport, ConnectionMode::Mock, None)
    }

    /// Somente testes: permite exercitar Live offline e fixar a disponibilidade
    /// de áudio sem depender do hardware da máquina.
    #[doc(hidden)]
    pub fn new_with_transport_with_audio_probe(
        transport: Arc<dyn DiscordTransport>,
        mode: ConnectionMode,
        has_hardware_audio: Option<bool>,
    ) -> (Self, broadcast::Receiver<TransportEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let (voice_state_tx, _) = broadcast::channel(64);
        let (voice_event_tx, _) = broadcast::channel(64);
        let active_dm_calls = Arc::new(RwLock::new(std::collections::HashSet::new()));

        let active_calls_ref = active_dm_calls.clone();
        let mut transport_events = transport.subscribe_events();

        let manager = Self {
            transport: Arc::new(RwLock::new(transport)),
            mode: Arc::new(RwLock::new(mode)),
            audio_hardware_probe: has_hardware_audio,
            event_tx,
            real_client: Arc::new(RwLock::new(None)),
            active_voice: Arc::new(RwLock::new(None)),
            voice_state_tx,
            voice_event_tx,
            voice_gateway_factory: Arc::new(RwLock::new(None)),
            active_dm_calls,
            shutdown_started: Arc::new(AtomicBool::new(false)),
            background_tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
        };

        manager.spawn_tracked(async move {
            while let Ok(event) = transport_events.recv().await {
                match &event {
                    TransportEvent::CallCreate { channel_id, .. } => {
                        let mut calls = active_calls_ref.write().await;
                        calls.insert(channel_id.clone());
                    }
                    TransportEvent::CallDelete { channel_id } => {
                        let mut calls = active_calls_ref.write().await;
                        calls.remove(channel_id);
                    }
                    _ => {}
                }
            }
        });

        (manager, event_rx)
    }

    /// Spawns a background task that is tracked so shutdown can abort and join
    /// it instead of leaving network tasks alive after the window closes.
    fn spawn_tracked<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if self.shutdown_started.load(Ordering::SeqCst) {
            return;
        }
        let handle = tokio::spawn(future);
        match self.background_tasks.lock() {
            Ok(mut tasks) => {
                tasks.retain(|task| !task.is_finished());
                tasks.push(handle);
            }
            Err(_) => handle.abort(),
        }
    }

    /// Connects to real Discord API using an AuthToken (Bot or User token).
    ///
    /// A autenticação REST acontece antes de iniciar o Gateway. Nenhum estado
    /// conectado é publicado sem uma resposta real de `/users/@me`.
    pub async fn connect_with_token(&self, token: AuthToken) -> Result<()> {
        info!("Iniciando autenticação real no Discord API e WebSocket Gateway...");
        if !token.is_valid_format() {
            let reason = "Falha na autenticação: token inválido ou vazio.".to_string();
            {
                let mut mode = self.mode.write().await;
                *mode = ConnectionMode::Disconnected(reason.clone());
            }
            let _ = self.event_tx.send(TransportEvent::Disconnected(reason));
            anyhow::bail!("Token do Discord inválido ou vazio");
        }

        {
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Connecting;
        }

        let real_client = Arc::new(DiscordApiClient::new(token));

        // Confirma as credenciais antes de abrir o WebSocket. Erros 401/403/2FA
        // são traduzidos pela camada REST e não geram READY ou transporte ativo.
        let user = match real_client.verify_current_user().await {
            Ok(user) => user,
            Err(error) => {
                let reason = format!("Falha na autenticação: {error:#}");
                warn!("{}", reason);
                let idle = Arc::new(IdleTransport::new());
                {
                    let mut current = self.transport.write().await;
                    *current = idle;
                    let mut client = self.real_client.write().await;
                    *client = None;
                }
                {
                    let mut mode = self.mode.write().await;
                    *mode = ConnectionMode::Disconnected(reason.clone());
                }
                let _ = self.event_tx.send(TransportEvent::Disconnected(reason));
                return Err(error);
            }
        };
        info!("Credenciais confirmadas pela API do Discord.");

        // Inscreve antes do handshake para não perder o READY inicial.
        let mut gateway_rx = real_client.subscribe_events();

        // connect() aguarda READY/RESUMED, portanto falhas fatais do Gateway
        // também retornam aqui em vez de deixar uma task oculta reconectando.
        if let Err(error) = real_client.connect().await {
            let reason = format!("Falha ao conectar no Gateway: {error:#}");
            error!("{}", reason);
            let idle = Arc::new(IdleTransport::new());
            {
                let mut current = self.transport.write().await;
                *current = idle;
                let mut client = self.real_client.write().await;
                *client = None;
            }
            {
                let mut mode = self.mode.write().await;
                *mode = ConnectionMode::Disconnected(reason.clone());
            }
            let _ = self.event_tx.send(TransportEvent::Disconnected(reason));
            return Err(error);
        }

        // Bridge gateway events to global event_tx.
        let event_tx = self.event_tx.clone();
        let mode_ref = self.mode.clone();
        let active_calls_ref = self.active_dm_calls.clone();

        self.spawn_tracked(async move {
            let mut authenticated = false;
            while let Ok(event) = gateway_rx.recv().await {
                match &event {
                    TransportEvent::Ready { user, .. } => {
                        authenticated = true;
                        let mut mode = mode_ref.write().await;
                        *mode = ConnectionMode::LiveDiscord(user.username.clone());
                        info!("Discord Gateway READY recebido.");
                    }
                    TransportEvent::Connected => {
                        info!("Discord Gateway WebSocket conectado.");
                        if !authenticated {
                            // Socket aberto não é login concluído. Só READY (ou
                            // reconexão de uma sessão já autenticada) chega à UI.
                            continue;
                        }
                    }
                    TransportEvent::Reconnecting { attempt, delay_ms, reason } => {
                        let mut mode = mode_ref.write().await;
                        *mode = ConnectionMode::Reconnecting { attempt: *attempt, delay_ms: *delay_ms, reason: reason.clone() };
                        warn!("Discord Gateway reconectando (tentativa {attempt}): {reason}");
                    }
                    TransportEvent::Disconnected(reason) => {
                        let mut mode = mode_ref.write().await;
                        *mode = ConnectionMode::Disconnected(reason.clone());
                        warn!("Discord Gateway desconectado: {}", reason);
                    }
                    TransportEvent::CallCreate { channel_id, .. } => {
                        let mut calls = active_calls_ref.write().await;
                        calls.insert(channel_id.clone());
                    }
                    TransportEvent::CallDelete { channel_id } => {
                        let mut calls = active_calls_ref.write().await;
                        calls.remove(channel_id);
                    }
                    _ => {}
                }
                let _ = event_tx.send(event);
            }
        });

        // Swap active transport only after real authentication and Gateway READY.
        {
            let mut current = self.transport.write().await;
            *current = real_client.clone();
            let mut client = self.real_client.write().await;
            *client = Some(real_client.clone());
        }

        info!("Cliente autenticado configurado como transporte ativo.");

        // Para contas de usuário (TokenType::User), o próprio Gateway READY já entrega
        // guilds (com canais/cargos/emojis), private_channels (DMs), relationships e
        // user_settings_proto. Disparar rajada de 7 chamadas REST logo após o login não
        // existe no Discord Desktop oficial e dispara anti-abuso em tokens recém-gerados.
        if real_client.token().token_type == crate::protocol::auth::TokenType::User {
            info!(
                "Conta de usuário autenticada: usando snapshot do Gateway READY sem rajada REST redundante no boot."
            );
            return Ok(());
        }

        // Busca dados iniciais reais para Bot Token (que não recebe DMs/settings no READY).
        let client_clone = real_client.clone();
        let event_tx_clone = self.event_tx.clone();

        self.spawn_tracked(async move {
            info!("Buscando dados iniciais da API do Discord (bot: servidores e canais)...");

            let (mut guilds, guilds_loaded) = match client_clone.list_guilds().await {
                Ok(guilds) => {
                    info!("Obtidos {} servidores do Discord.", guilds.len());
                    (guilds, true)
                }
                Err(error) => {
                    warn!("Falha ao listar servidores via REST: {:#}", error);
                    (Vec::new(), false)
                }
            };

            let mut first_text_channel_id: Option<String> = None;
            if let Some(first_guild) = guilds.first_mut() {
                match client_clone.list_channels(&first_guild.id).await {
                    Ok(channels) => {
                        info!("Servidor inicial possui {} canais.", channels.len());
                        if let Some(channel) = channels
                            .iter()
                            .find(|channel| channel.channel_type == ChannelType::Text)
                            .or_else(|| channels.first())
                        {
                            first_text_channel_id = Some(channel.id.clone());
                        }
                        first_guild.channels = channels;
                    }
                    Err(error) => {
                        warn!("Falha ao buscar canais do servidor inicial: {:#}", error);
                    }
                }
            }

            let mut initial_messages = Vec::new();
            if let Some(channel_id) = first_text_channel_id.as_deref() {
                match client_clone.get_messages(channel_id, 50).await {
                    Ok(messages) => {
                        info!(
                            "Obtidas {} mensagens reais do canal inicial.",
                            messages.len()
                        );
                        initial_messages = messages;
                    }
                    Err(error) => {
                        warn!("Falha ao obter mensagens do canal inicial: {:#}", error);
                    }
                }
            }

            let guild_folders = client_clone
                .get_user_guild_settings()
                .await
                .unwrap_or_default();

            // Um READY vazio só é publicado quando a lista de servidores foi
            // confirmada. Falha transitória não apaga o snapshot real do Gateway.
            if guilds_loaded {
                let _ = event_tx_clone.send(TransportEvent::Ready {
                    user: user.clone(),
                    guilds,
                    guild_folders,
                    read_state: None,
                });
            }

            if let Some(channel_id) = first_text_channel_id {
                if !initial_messages.is_empty() {
                    let mut messages = initial_messages;
                    messages.sort_by_key(|message| message.timestamp);
                    let _ = event_tx_clone.send(TransportEvent::ChannelMessagesLoaded {
                        channel_id,
                        messages,
                    });
                }
            }

            if let Ok(dms) = client_clone.list_dm_channels().await {
                let _ = event_tx_clone.send(TransportEvent::DmChannelsUpdated(dms));
            } else {
                warn!("Falha ao listar canais de DM.");
            }
            if let Ok(relationships) = client_clone.list_relationships().await {
                let _ = event_tx_clone.send(TransportEvent::RelationshipsUpdated(relationships));
            } else {
                warn!("Falha ao listar relacionamentos.");
            }

            // settings-proto complementa o READY sem substituir um tema já
            // recebido. O decoder é somente leitura e nunca registra o payload.
            if let Ok(settings) = client_clone.get_user_settings_proto().await {
                if let Some(theme) = settings.theme() {
                    let _ = event_tx_clone.send(TransportEvent::AccountThemeUpdated {
                        theme,
                        client_theme_settings: settings
                            .appearance
                            .as_ref()
                            .and_then(|appearance| appearance.client_theme_settings.clone()),
                    });
                }
            }
        });

        Ok(())
    }

    /// Disconnects the active client and switches to the idle transport.
    pub async fn disconnect(&self) -> Result<()> {
        let current_transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        let _ = current_transport.disconnect().await;
        crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared()
            .clear()
            .await;

        let idle = Arc::new(IdleTransport::new());
        {
            let mut current = self.transport.write().await;
            *current = idle;
            let mut rc = self.real_client.write().await;
            *rc = None;
        }
        {
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Disconnected("Desconectado".to_string());
        }
        let _ = self
            .event_tx
            .send(TransportEvent::Disconnected("Desconectado".to_string()));
        Ok(())
    }

    /// Stops voice/audio, aborts tracked background tasks and disconnects the
    /// main Gateway. Idempotent and bounded by a default shutdown budget.
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_with_timeout(CLIENT_MANAGER_SHUTDOWN_TIMEOUT)
            .await
    }

    /// Ordered shutdown with a caller-provided budget shared across all steps.
    pub async fn shutdown_with_timeout(&self, timeout: Duration) -> Result<()> {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let deadline = tokio::time::Instant::now() + timeout;

        // 1. Voice + AudioEngine (including the session task and gateway).
        let _ = tokio::time::timeout(
            remaining_budget(deadline),
            self.disconnect_voice_session(),
        )
        .await;

        // 2. Network/UI bridge tasks spawned by this manager.
        let tasks = match self.background_tasks.lock() {
            Ok(mut tasks) => std::mem::take(&mut *tasks),
            Err(_) => Vec::new(),
        };
        for task in &tasks {
            task.abort();
        }
        let _ = tokio::time::timeout(
            remaining_budget(deadline),
            futures::future::join_all(tasks),
        )
        .await;

        // 3. Main Discord Gateway / transport.
        let _ = tokio::time::timeout(remaining_budget(deadline), self.disconnect()).await;

        Ok(())
    }

    /// Somente testes: alterna para o provedor mock determinístico.
    #[doc(hidden)]
    pub async fn connect_mock(&self) -> Result<()> {
        info!("Alternando para provedor local determinístico...");
        let mock = Arc::new(MockDiscordProvider::new());
        {
            let mut current = self.transport.write().await;
            *current = mock;
            let mut rc = self.real_client.write().await;
            *rc = None;
        }
        {
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Mock;
        }
        let _ = self.event_tx.send(TransportEvent::Connected);
        Ok(())
    }

    /// Fetches a user profile from the active Discord API client (/users/{user_id}/profile)
    pub async fn get_user_profile(
        &self,
        user_id: &str,
    ) -> Result<crate::domain::profile::UserProfileData> {
        self.get_user_profile_with_guild(user_id, None).await
    }

    /// Fetches a user profile within an optional guild context (/users/{user_id}/profile)
    pub async fn get_user_profile_with_guild(
        &self,
        user_id: &str,
        guild_id: Option<&str>,
    ) -> Result<crate::domain::profile::UserProfileData> {
        let client_opt = {
            let lock = self.real_client.read().await;
            lock.clone()
        };
        if let Some(client) = client_opt {
            client.get_user_profile_with_guild(user_id, guild_id).await
        } else {
            anyhow::bail!("Cliente Discord não conectado para busca de perfil ao vivo")
        }
    }

    /// Creates or opens an existing direct message (DM) channel with a user
    pub async fn create_or_get_dm_channel(&self, recipient_id: &str) -> Result<Channel> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.create_or_get_dm_channel(recipient_id).await
    }

    /// Sends a chat message to a text channel using the active transport
    pub async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        self.send_message_with_reference(channel_id, content, None, true)
            .await
    }

    /// Sends a chat message with optional reply reference and mention control.
    pub async fn send_message_with_reference(
        &self,
        channel_id: &str,
        content: &str,
        message_reference: Option<&crate::domain::message::MessageReference>,
        mention_replied_user: bool,
    ) -> Result<Message> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => {
                        self.emit_event(TransportEvent::DmChannelResolved {
                            temp_id: channel_id.to_string(),
                            channel: ch.clone(),
                        });
                        ch.id
                    }
                    Err(e) => {
                        log::warn!("Falha ao resolver DM para envio: {:?}", e);
                        channel_id.to_string()
                    }
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        let msg = transport
            .send_message_with_reference(
                &real_channel_id,
                content,
                message_reference,
                mention_replied_user,
            )
            .await?;
        let _ = self
            .event_tx
            .send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }

    /// Sends a sticker message to a text channel using the active transport
    pub async fn send_sticker(&self, channel_id: &str, sticker_id: &str) -> Result<Message> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => {
                        self.emit_event(TransportEvent::DmChannelResolved {
                            temp_id: channel_id.to_string(),
                            channel: ch.clone(),
                        });
                        ch.id
                    }
                    Err(e) => {
                        log::warn!("Falha ao resolver DM para envio de figurinha: {:?}", e);
                        channel_id.to_string()
                    }
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        let msg = transport.send_sticker(&real_channel_id, sticker_id).await?;
        // Broadcast local message creation event
        let _ = self
            .event_tx
            .send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }

    /// Adds a reaction emoji to a message using the active transport
    pub async fn add_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => {
                        self.emit_event(TransportEvent::DmChannelResolved {
                            temp_id: channel_id.to_string(),
                            channel: ch.clone(),
                        });
                        ch.id
                    }
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .add_reaction(&real_channel_id, message_id, emoji)
            .await?;
        let (emoji_name, emoji_id) = crate::domain::message::parse_reaction_key(emoji);
        let _ = self.event_tx.send(TransportEvent::ReactionAdd {
            channel_id: real_channel_id,
            message_id: message_id.to_string(),
            user_id: "me".to_string(),
            emoji_name,
            emoji_id,
            emoji_animated: false,
        });
        Ok(())
    }

    /// Removes a reaction emoji from a message using the active transport
    pub async fn remove_reaction(
        &self,
        channel_id: &str,
        message_id: &str,
        emoji: &str,
    ) -> Result<()> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => {
                        self.emit_event(TransportEvent::DmChannelResolved {
                            temp_id: channel_id.to_string(),
                            channel: ch.clone(),
                        });
                        ch.id
                    }
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .remove_reaction(&real_channel_id, message_id, emoji)
            .await?;
        let (emoji_name, emoji_id) = crate::domain::message::parse_reaction_key(emoji);
        let _ = self.event_tx.send(TransportEvent::ReactionRemove {
            channel_id: real_channel_id,
            message_id: message_id.to_string(),
            user_id: "me".to_string(),
            emoji_name,
            emoji_id,
            emoji_animated: false,
        });
        Ok(())
    }

    /// Fetches a single message by ID from a channel.
    pub async fn get_message(
        &self,
        channel_id: &str,
        message_id: &str,
    ) -> Result<Option<Message>> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_message(&real_channel_id, message_id).await
    }

    /// Subscribes to events from the active transport
    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }

    /// Emits a transport event into the event stream (useful for synthetic or UI events)
    pub fn emit_event(&self, event: TransportEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Resolves a temporary `dm-{user_id}` channel to its real channel ID,
    /// emitting `DmChannelResolved` so the UI can migrate pending rows.
    async fn resolve_message_channel(&self, channel_id: &str) -> String {
        if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
            let recipient_id = channel_id.trim_start_matches("dm-");
            match self.create_or_get_dm_channel(recipient_id).await {
                Ok(ch) => {
                    self.emit_event(TransportEvent::DmChannelResolved {
                        temp_id: channel_id.to_string(),
                        channel: ch.clone(),
                    });
                    ch.id
                }
                Err(e) => {
                    log::warn!("Falha ao resolver DM em get_messages: {:?}", e);
                    channel_id.to_string()
                }
            }
        } else {
            channel_id.to_string()
        }
    }

    /// Fetches message history for a channel using the active transport
    pub async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<Message>> {
        let real_channel_id = self.resolve_message_channel(channel_id).await;

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_messages(&real_channel_id, limit).await
    }

    /// Fetches the page of messages immediately older than `before_id`.
    pub async fn get_messages_before(
        &self,
        channel_id: &str,
        before_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let real_channel_id = self.resolve_message_channel(channel_id).await;

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .get_messages_before(&real_channel_id, before_id, limit)
            .await
    }

    /// Fetches messages centered around `around_id` for search/reply jumps.
    pub async fn get_messages_around(
        &self,
        channel_id: &str,
        around_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>> {
        let real_channel_id = self.resolve_message_channel(channel_id).await;

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .get_messages_around(&real_channel_id, around_id, limit)
            .await
    }

    /// Lists all available guilds using the active transport
    pub async fn list_guilds(&self) -> Result<Vec<Guild>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.list_guilds().await
    }

    /// Lists channels for a guild using the active transport
    pub async fn list_channels(&self, guild_id: &str) -> Result<Vec<Channel>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.list_channels(guild_id).await
    }

    /// Lists channels for a guild using the active transport (for lazy loading)
    pub async fn list_channels_for_guild(&self, guild_id: &str) -> Result<Vec<Channel>> {
        self.list_channels(guild_id).await
    }

    /// Lists direct message (DM) channels using the active transport (/users/@me/channels)
    pub async fn list_dm_channels(&self) -> Result<Vec<Channel>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.list_dm_channels().await
    }

    /// Lists user relationships (friends, requests, blocked) using the active transport (/users/@me/relationships)
    pub async fn list_relationships(&self) -> Result<Vec<Relationship>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.list_relationships().await
    }

    /// Current connection mode
    pub async fn current_mode(&self) -> ConnectionMode {
        let lock = self.mode.read().await;
        lock.clone()
    }

    /// Fetches all custom emojis for a guild using the active transport
    pub async fn get_guild_emojis(
        &self,
        guild_id: &str,
    ) -> Result<Vec<crate::protocol::media_cache::DiscordEmoji>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_guild_emojis(guild_id).await
    }

    /// Fetches all custom stickers for a guild using the active transport
    pub async fn get_guild_stickers(
        &self,
        guild_id: &str,
    ) -> Result<Vec<crate::protocol::media_cache::DiscordSticker>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_guild_stickers(guild_id).await
    }

    /// Fetches the official Discord sticker packs using the active transport
    pub async fn get_sticker_packs(
        &self,
    ) -> Result<Vec<crate::protocol::media_cache::StickerPack>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_sticker_packs().await
    }

    /// Sends a friend request to a user (/users/@me/relationships)
    pub async fn send_friend_request(
        &self,
        username: &str,
        discriminator: Option<&str>,
    ) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.send_friend_request(username, discriminator).await
    }

    /// Accepts an incoming friend request (/users/@me/relationships/{user_id})
    pub async fn accept_friend_request(&self, user_id: &str) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.accept_friend_request(user_id).await
    }

    /// Removes, declines, cancels or unblocks a relationship (/users/@me/relationships/{user_id})
    pub async fn remove_relationship(&self, user_id: &str) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.remove_relationship(user_id).await
    }

    /// ACKs a message as read through the active transport.
    pub async fn ack_message(
        &self,
        channel_id: &str,
        message_id: &str,
        token: Option<&str>,
    ) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.ack_message(channel_id, message_id, token).await
    }

    /// Updates the current user's guild nickname (PATCH /guilds/{guild_id}/members/@me).
    pub async fn update_my_guild_nickname(
        &self,
        guild_id: &str,
        nick: Option<&str>,
    ) -> Result<Option<String>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.update_my_guild_nickname(guild_id, nick).await
    }

    /// Leaves a guild (DELETE /users/@me/guilds/{guild_id}).
    pub async fn leave_guild(&self, guild_id: &str) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.leave_guild(guild_id).await
    }

    /// Applies a presence/custom-status update through the active transport.
    /// Both payloads are built from the status picker model; the transport only
    /// delivers them (Gateway Opcode 3 + PATCH /users/@me/settings).
    pub async fn update_user_status(
        &self,
        gateway_payload: &serde_json::Value,
        settings_payload: &serde_json::Value,
    ) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .update_user_status(gateway_payload, settings_payload)
            .await
    }

    /// Ring participants in a DM / private channel via POST /channels/{id}/call/ring
    pub async fn ring_call(&self, channel_id: &str, recipients: Option<Vec<String>>) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.ring_call(channel_id, recipients).await
    }

    /// Sets an optional custom voice gateway factory (e.g. for deterministic unit / offline tests).
    pub async fn set_voice_gateway_factory(
        &self,
        factory: Option<crate::protocol::voice_session::VoiceGatewayFactory>,
    ) {
        let mut gw = self.voice_gateway_factory.write().await;
        *gw = factory;
    }

    /// Returns the active VoiceSession if one is currently running.
    pub async fn active_voice_session(
        &self,
    ) -> Option<Arc<crate::protocol::voice_session::VoiceSession>> {
        let active = self.active_voice.read().await;
        active.clone()
    }

    /// Subscribes to real-time voice state transitions and measured latency.
    pub fn subscribe_voice_state(
        &self,
    ) -> broadcast::Receiver<(
        crate::protocol::voice_session::VoiceSessionState,
        Option<u32>,
    )> {
        self.voice_state_tx.subscribe()
    }

    /// Subscribes to real-time voice session events (Speaking indicators, ping, etc).
    pub fn subscribe_voice_events(
        &self,
    ) -> broadcast::Receiver<crate::protocol::voice_session::VoiceSessionEvent> {
        self.voice_event_tx.subscribe()
    }

    /// Disconnects the active voice session gracefully.
    pub async fn disconnect_voice_session(&self) -> Result<()> {
        let session_opt = {
            let mut active = self.active_voice.write().await;
            active.take()
        };
        if let Some(session) = session_opt {
            session.disconnect().await?;
        }
        let rc = self.real_client.read().await;
        if let Some(client) = rc.as_ref() {
            client.clear_voice_session().await;
        }
        let _ = self.voice_state_tx.send((
            crate::protocol::voice_session::VoiceSessionState::Disconnected {
                reason: "Desconectado pelo usuário".into(),
            },
            None,
        ));
        Ok(())
    }

    /// Bridge determinístico exclusivo do transporte Mock/testes offline.
    fn no_op_audio_bridge() -> crate::protocol::voice_session::AudioBridge {
        let (_audio_out_tx, audio_out_rx) = tokio::sync::mpsc::channel(64);
        let (_audio_spk_tx, audio_spk_rx) = tokio::sync::mpsc::channel(16);
        let (audio_cmd_tx, mut audio_cmd_rx) = tokio::sync::mpsc::channel(64);

        tokio::spawn(async move {
            while let Some(command) = audio_cmd_rx.recv().await {
                match command {
                    crate::protocol::voice_session::AudioBridgeCommand::StartSession {
                        responder,
                    } => {
                        let _ = responder.send(Ok(()));
                    }
                    crate::protocol::voice_session::AudioBridgeCommand::ReceiveOpusPacket {
                        ..
                    } => {}
                    crate::protocol::voice_session::AudioBridgeCommand::StopSession => {}
                    crate::protocol::voice_session::AudioBridgeCommand::SetMuted(_) => {}
                    crate::protocol::voice_session::AudioBridgeCommand::SetDeafened(_) => {}
                    crate::protocol::voice_session::AudioBridgeCommand::SwitchInputDevice {
                        responder,
                        ..
                    } => {
                        let _ = responder.send(Ok(()));
                    }
                    crate::protocol::voice_session::AudioBridgeCommand::SwitchOutputDevice {
                        responder,
                        ..
                    } => {
                        let _ = responder.send(Ok(()));
                    }
                    crate::protocol::voice_session::AudioBridgeCommand::Shutdown => break,
                }
            }
        });

        crate::protocol::voice_session::AudioBridge::from_channels(
            audio_out_rx,
            audio_spk_rx,
            audio_cmd_tx,
        )
    }

    /// Starts a real VoiceSession using client_manager transport or active live client.
    pub async fn start_voice_session(
        &self,
        guild_id: Option<u64>,
        channel_id: u64,
        user_id: u64,
        self_mute: bool,
        self_deaf: bool,
        ring_recipients: Option<Vec<String>>,
    ) -> Result<Arc<crate::protocol::voice_session::VoiceSession>> {
        let mode = self.mode.read().await.clone();
        let is_mock = matches!(mode, ConnectionMode::Mock);
        let has_hardware_audio = if is_mock {
            false
        } else {
            self.audio_hardware_probe.unwrap_or_else(|| {
                let device_manager = crate::media::devices::MediaDeviceManager::new();
                device_manager
                    .list_input_devices()
                    .map(|devices| !devices.is_empty())
                    .unwrap_or(false)
                    && device_manager
                        .list_output_devices()
                        .map(|devices| !devices.is_empty())
                        .unwrap_or(false)
            })
        };

        if !is_mock && !has_hardware_audio {
            let reason =
                "Áudio indisponível: é necessário um dispositivo de entrada e um de saída.";
            warn!("Sessão de voz não iniciada: {}", reason);
            let _ = self.voice_state_tx.send((
                crate::protocol::voice_session::VoiceSessionState::Disconnected {
                    reason: reason.to_string(),
                },
                None,
            ));
            return Err(anyhow::anyhow!(reason.to_string()));
        }

        // O bridge no-op é exclusivo do transporte Mock/teste offline. Live só
        // continua quando o bridge real confirmar o início da sessão.
        let audio_bridge = if is_mock {
            Self::no_op_audio_bridge()
        } else {
            crate::protocol::voice_session::AudioBridge::new_thread()
        };

        let session_config = crate::protocol::voice_session::VoiceSessionConfig {
            guild_id,
            channel_id,
            user_id,
            self_mute,
            self_deaf,
        };

        let signaling: Arc<dyn crate::protocol::voice_session::MainSignalingChannel> = {
            let rc = self.real_client.read().await;
            if let Some(client) = rc.as_ref() {
                client.clone()
            } else {
                let tr = self.transport.read().await;
                Arc::new(tr.clone())
            }
        };

        let mut session = crate::protocol::voice_session::VoiceSession::new(
            session_config,
            signaling,
            audio_bridge,
        );

        let gw_factory = {
            let lock = self.voice_gateway_factory.read().await;
            lock.clone()
        };
        if let Some(factory) = gw_factory {
            session = session.with_gateway_factory(move |cfg| factory(cfg));
        }

        let session = Arc::new(session);

        {
            let mut active = self.active_voice.write().await;
            if let Some(prev) = active.take() {
                let _ = prev.disconnect().await;
            }
            *active = Some(session.clone());
        }

        // Bridge session state changes to voice_state_tx
        let mut state_rx = session.subscribe_state();
        let voice_tx = self.voice_state_tx.clone();
        let session_ref = session.clone();
        self.spawn_tracked(async move {
            let initial = state_rx.borrow().clone();
            let initial_ping = session_ref.ping_ms();
            let _ = voice_tx.send((initial, initial_ping));

            while state_rx.changed().await.is_ok() {
                let current_st = state_rx.borrow().clone();
                let is_term = matches!(
                    current_st,
                    crate::protocol::voice_session::VoiceSessionState::Disconnected { .. }
                );
                let current_ping = session_ref.ping_ms();
                let _ = voice_tx.send((current_st, current_ping));
                if is_term {
                    break;
                }
            }
        });

        // Subscribe to session events (speaking / SSRC mapping / latency).
        // The task holds only a weak session reference so it cannot keep the
        // session (and its broadcast sender) alive after shutdown.
        let mut events_rx = session.subscribe_events();
        let mut events_state_rx = session.subscribe_state();
        let voice_evt_tx = self.voice_event_tx.clone();
        let voice_tx_ack = self.voice_state_tx.clone();
        let session_weak = Arc::downgrade(&session);
        self.spawn_tracked(async move {
            loop {
                tokio::select! {
                    event = events_rx.recv() => {
                        let Ok(event) = event else { break };
                        if let crate::protocol::voice_session::VoiceSessionEvent::HeartbeatAck { ping_ms } = event {
                            if let Some(session) = session_weak.upgrade() {
                                let current_st = session.state();
                                let _ = voice_tx_ack.send((current_st, Some(ping_ms)));
                            }
                        }
                        let _ = voice_evt_tx.send(event);
                    }
                    changed = events_state_rx.changed() => {
                        if changed.is_err() { break; }
                        if matches!(
                            *events_state_rx.borrow(),
                            crate::protocol::voice_session::VoiceSessionState::Disconnected { .. }
                        ) {
                            break;
                        }
                    }
                }
            }
        });

        // Se destinatários de toque foram fornecidos (chamada DM):
        // 1. Verifica se a chamada já existia (outro lado ligou primeiro): se sim, NÃO toca.
        // 2. Se for nova chamada, aguarda CALL_CREATE/CALL_UPDATE do canal antes de disparar o ring_call.
        if let Some(recips) = ring_recipients {
            let ch_str = channel_id.to_string();
            let already_existed = {
                let calls = self.active_dm_calls.read().await;
                calls.contains(&ch_str)
            };

            if already_existed {
                log::info!(
                    "[VoiceDiag] Chamada já existia no canal; outro lado ligou primeiro (não tocando), chamada_ja_existia=true"
                );
            } else {
                let mut confirm_rx = session.subscribe_entry_confirmed();
                let mut state_rx = session.subscribe_state();
                let cm_ring = self.clone();
                let session_weak = Arc::downgrade(&session);
                self.spawn_tracked(async move {
                    let trigger_ring = || {
                        let cm = cm_ring.clone();
                        let ch = ch_str.clone();
                        let r = recips.clone();
                        let weak = session_weak.clone();
                        async move {
                            log::info!(
                                "[ClientManager] CALL_CREATE confirmado na chamada DM; disparando ring_call"
                            );
                            if let Err(e) = cm.ring_call(&ch, Some(r)).await {
                                let short_msg = "Não foi possível tocar a chamada".to_string();
                                log::warn!(
                                    "[VoiceRing] Falha ao tocar chamada (ring_call recusado): {}",
                                    e
                                );
                                if let Some(sess) = weak.upgrade() {
                                    sess.set_call_warning(Some(short_msg));
                                }
                            } else {
                                log::info!(
                                    "[VoiceRing] ring_call enviado com sucesso para a chamada DM"
                                );
                            }
                        }
                    };

                    if *confirm_rx.borrow() {
                        trigger_ring().await;
                        return;
                    }

                    let is_connected = matches!(
                        *state_rx.borrow(),
                        crate::protocol::voice_session::VoiceSessionState::Connected
                    );

                    while !is_connected {
                        tokio::select! {
                            res = confirm_rx.changed() => {
                                if res.is_ok() && *confirm_rx.borrow() {
                                    trigger_ring().await;
                                    return;
                                } else if res.is_err() {
                                    return;
                                }
                            }
                            res = state_rx.changed() => {
                                if res.is_err() {
                                    return;
                                }
                                match *state_rx.borrow() {
                                    crate::protocol::voice_session::VoiceSessionState::Connected => {
                                        break;
                                    }
                                    crate::protocol::voice_session::VoiceSessionState::Disconnected { .. } => {
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    if *confirm_rx.borrow() {
                        trigger_ring().await;
                        return;
                    }

                    let timeout_duration = Duration::from_secs(10);
                    let timeout_fut = tokio::time::sleep(timeout_duration);
                    tokio::pin!(timeout_fut);

                    loop {
                        tokio::select! {
                            _ = &mut timeout_fut => {
                                log::warn!("[VoiceDiag] CALL_CREATE não chegou em 10s; toque não enviado");
                                let short_msg = "Não foi possível tocar a chamada".to_string();
                                if let Some(sess) = session_weak.upgrade() {
                                    sess.set_call_warning(Some(short_msg));
                                }
                                return;
                            }
                            res = confirm_rx.changed() => {
                                if res.is_ok() && *confirm_rx.borrow() {
                                    trigger_ring().await;
                                    return;
                                } else if res.is_err() {
                                    return;
                                }
                            }
                            res = state_rx.changed() => {
                                if res.is_err() {
                                    return;
                                }
                                if matches!(*state_rx.borrow(), crate::protocol::voice_session::VoiceSessionState::Disconnected { .. }) {
                                    return;
                                }
                            }
                        }
                    }
                });
            }
        }

        if let Err(e) = session.start().await {
            let reason = format!("Falha ao iniciar sessão de voz: {}", e);
            let _ = self.voice_state_tx.send((
                crate::protocol::voice_session::VoiceSessionState::Disconnected { reason },
                None,
            ));
            return Err(e);
        }
        Ok(session)
    }
}

impl ClientManager {
    /// Deletes a message from a channel using the active transport
    pub async fn delete_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.delete_message(&real_channel_id, message_id).await
    }

    /// Subscribes to lazy guild members / viewport for a guild and channel
    pub async fn subscribe_guild_members(&self, guild_id: &str, channel_id: &str) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .subscribe_guild_members(guild_id, channel_id)
            .await
    }

    /// Edits an existing message (/channels/{channel_id}/messages/{message_id})
    pub async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        content: &str,
    ) -> Result<Message> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .edit_message(&real_channel_id, message_id, content)
            .await
    }

    /// Pins a message in a channel (/channels/{channel_id}/pins/{message_id})
    pub async fn pin_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.pin_message(&real_channel_id, message_id).await
    }

    /// Unpins a message in a channel (/channels/{channel_id}/pins/{message_id})
    pub async fn unpin_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.unpin_message(&real_channel_id, message_id).await
    }

    /// Fetches pinned messages in a channel (GET /channels/{channel_id}/pins).
    pub async fn get_pinned_messages(&self, channel_id: &str) -> Result<Vec<Message>> {
        let real_channel_id =
            if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
                let recipient_id = channel_id.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => channel_id.to_string(),
                }
            } else {
                channel_id.to_string()
            };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_pinned_messages(&real_channel_id).await
    }

    /// Requests guild members for message authors missing from cache (Gateway Opcode 8)
    pub async fn request_guild_members(&self, guild_id: &str, user_ids: Vec<String>) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.request_guild_members(guild_id, user_ids).await
    }

    /// Updates guild member viewport ranges (Gateway Opcode 14)
    pub async fn update_guild_member_viewport(
        &self,
        guild_id: &str,
        channel_id: &str,
        ranges: Vec<[usize; 2]>,
    ) -> Result<()> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport
            .update_guild_member_viewport(guild_id, channel_id, ranges)
            .await
    }
}

impl ClientManager {
    /// Searches messages across channel or guild using the active DiscordApiClient
    pub async fn search_messages(
        &self,
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        query: &crate::chat::search::SearchQuery,
        offset: u32,
        sort_order: Option<&str>,
    ) -> Result<crate::chat::search::SearchApiResponse> {
        let client_opt = {
            let rc = self.real_client.read().await;
            rc.clone()
        };
        let client = match client_opt {
            Some(c) => c,
            None => {
                anyhow::bail!("Não conectado à API do Discord");
            }
        };

        if let Some(gid) = guild_id {
            client
                .search_guild_messages(gid, query, offset, sort_order)
                .await
        } else if let Some(cid) = channel_id {
            let real_cid = if cid.starts_with("dm-") && !cid.starts_with("dm-group-") {
                let recipient_id = cid.trim_start_matches("dm-");
                match self.create_or_get_dm_channel(recipient_id).await {
                    Ok(ch) => ch.id,
                    Err(_) => cid.to_string(),
                }
            } else {
                cid.to_string()
            };
            client
                .search_channel_messages(&real_cid, query, offset, sort_order)
                .await
        } else {
            anyhow::bail!("Nenhum canal ou servidor selecionado para busca");
        }
    }
}
