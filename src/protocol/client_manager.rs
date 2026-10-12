//! Discord RS - Client Manager & Real-Time Bridge
//! Coordinates the active transport (live Gateway/REST Discord API or deterministic mock),
//! processes incoming WebSocket Gateway events, and provides thread-safe messaging.

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use anyhow::Result;
use log::{info, warn, error};

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
    Disconnected(String),
}

/// Thread-safe client manager coordinating REST and Gateway events
#[derive(Clone)]
pub struct ClientManager {
    transport: Arc<RwLock<Arc<dyn DiscordTransport>>>,
    mode: Arc<RwLock<ConnectionMode>>,
    event_tx: broadcast::Sender<TransportEvent>,
    real_client: Arc<RwLock<Option<Arc<DiscordApiClient>>>>,
    active_voice: Arc<RwLock<Option<Arc<crate::protocol::voice_session::VoiceSession>>>>,
    voice_state_tx: broadcast::Sender<(crate::protocol::voice_session::VoiceSessionState, Option<u32>)>,
    voice_event_tx: broadcast::Sender<crate::protocol::voice_session::VoiceSessionEvent>,
    voice_gateway_factory: Arc<RwLock<Option<crate::protocol::voice_session::VoiceGatewayFactory>>>,
    active_dm_calls: Arc<RwLock<std::collections::HashSet<String>>>,
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
            mode: Arc::new(RwLock::new(ConnectionMode::Disconnected("Não conectado".to_string()))),
            event_tx,
            real_client: Arc::new(RwLock::new(None)),
            active_voice: Arc::new(RwLock::new(None)),
            voice_state_tx,
            voice_event_tx,
            voice_gateway_factory: Arc::new(RwLock::new(None)),
            active_dm_calls: Arc::new(RwLock::new(std::collections::HashSet::new())),
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
            event_tx,
            real_client: Arc::new(RwLock::new(None)),
            active_voice: Arc::new(RwLock::new(None)),
            voice_state_tx,
            voice_event_tx,
            voice_gateway_factory: Arc::new(RwLock::new(None)),
            active_dm_calls: Arc::new(RwLock::new(std::collections::HashSet::new())),
        };

        (manager, event_rx)
    }

    /// Somente testes: cria um ClientManager com o provedor de transporte fornecido.
    #[doc(hidden)]
    pub fn new_with_transport(transport: Arc<dyn DiscordTransport>) -> (Self, broadcast::Receiver<TransportEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let (voice_state_tx, _) = broadcast::channel(64);
        let (voice_event_tx, _) = broadcast::channel(64);
        let active_dm_calls = Arc::new(RwLock::new(std::collections::HashSet::new()));

        let active_calls_ref = active_dm_calls.clone();
        let mut tr_events = transport.subscribe_events();
        tokio::spawn(async move {
            while let Ok(evt) = tr_events.recv().await {
                match &evt {
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

        let manager = Self {
            transport: Arc::new(RwLock::new(transport)),
            mode: Arc::new(RwLock::new(ConnectionMode::Mock)),
            event_tx,
            real_client: Arc::new(RwLock::new(None)),
            active_voice: Arc::new(RwLock::new(None)),
            voice_state_tx,
            voice_event_tx,
            voice_gateway_factory: Arc::new(RwLock::new(None)),
            active_dm_calls,
        };

        (manager, event_rx)
    }

    /// Connects to real Discord API using an AuthToken (Bot or User token)
    pub async fn connect_with_token(&self, token: AuthToken) -> Result<()> {
        info!("Iniciando conexão com Discord API v10 e WebSocket Gateway...");
        {
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Connecting;
        }

        let real_client = Arc::new(DiscordApiClient::new(token));
        
        // Connect gateway loop
        if let Err(e) = real_client.connect().await {
            error!("Falha ao conectar Gateway do Discord: {:?}", e);
            let idle = Arc::new(IdleTransport::new());
            {
                let mut current = self.transport.write().await;
                *current = idle;
                let mut rc = self.real_client.write().await;
                *rc = None;
            }
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Disconnected(e.to_string());
            let _ = self.event_tx.send(TransportEvent::Disconnected(e.to_string()));
            return Err(e);
        }

        // Bridge gateway events to global event_tx
        let mut gateway_rx = real_client.subscribe_events();
        let event_tx = self.event_tx.clone();
        let mode_ref = self.mode.clone();
        let active_calls_ref = self.active_dm_calls.clone();

        tokio::spawn(async move {
            while let Ok(evt) = gateway_rx.recv().await {
                match &evt {
                    TransportEvent::Ready { user, .. } => {
                        let mut m = mode_ref.write().await;
                        *m = ConnectionMode::LiveDiscord(user.username.clone());
                        info!("Discord Gateway READY! Conectado como @{}", user.username);
                    }
                    TransportEvent::Connected => {
                        info!("Discord Gateway WebSocket conectado.");
                    }
                    TransportEvent::Disconnected(reason) => {
                        let mut m = mode_ref.write().await;
                        *m = ConnectionMode::Disconnected(reason.clone());
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
                let _ = event_tx.send(evt);
            }
        });

        // Swap active transport
        {
            let mut current = self.transport.write().await;
            *current = real_client.clone();
            let mut rc = self.real_client.write().await;
            *rc = Some(real_client.clone());
        }

        info!("Cliente do Discord configurado com sucesso como transporte ativo.");

        // Asynchronously fetch real initial data (user @me, guilds, channels, messages)
        let client_clone = real_client.clone();
        let event_tx_clone = self.event_tx.clone();
        let mode_clone = self.mode.clone();
        let transport_bg_ref = self.transport.clone();
        let real_client_bg_ref = self.real_client.clone();

        tokio::spawn(async move {
            info!("Buscando dados iniciais da API do Discord (@me, guilds, canais, mensagens)...");

            // 1. Buscar usuário autenticado via API (@me)
            let user = match client_clone.get_current_user().await {
                Ok(u) => {
                    info!("Usuário autenticado obtido com sucesso: @{} ({})", u.username, u.id);
                    let mut m = mode_clone.write().await;
                    *m = ConnectionMode::LiveDiscord(u.username.clone());
                    u
                }
                Err(e) => {
                    let err_msg = "Falha na autenticação: Token inválido ou não autorizado (401)".to_string();
                    warn!("{}: {:?}", err_msg, e);
                    let idle = Arc::new(IdleTransport::new());
                    {
                        let mut current = transport_bg_ref.write().await;
                        *current = idle;
                        let mut rc = real_client_bg_ref.write().await;
                        *rc = None;
                    }
                    {
                        let mut m = mode_clone.write().await;
                        *m = ConnectionMode::Disconnected(err_msg.clone());
                    }
                    let _ = event_tx_clone.send(TransportEvent::Disconnected(err_msg));
                    return;
                }
            };

            // 2. Buscar servidores reais via API (/users/@me/guilds)
            let mut guilds = match client_clone.list_guilds().await {
                Ok(g) => {
                    info!("Obtidos {} servidores do Discord.", g.len());
                    g
                }
                Err(e) => {
                    warn!("Falha ao listar servidores via REST: {:?}", e);
                    Vec::new()
                }
            };

            // 3. Buscar canais reais apenas do primeiro servidor para evitar sobrecarregar a API com rate-limit
            let mut first_text_channel_id: Option<String> = None;
            if let Some(first_guild) = guilds.first_mut() {
                match client_clone.list_channels(&first_guild.id).await {
                    Ok(channels) => {
                        info!("Servidor inicial '{}' possui {} canais.", first_guild.name, channels.len());
                        if let Some(ch) = channels.iter().find(|c| c.channel_type == ChannelType::Text) {
                            first_text_channel_id = Some(ch.id.clone());
                        } else if let Some(ch) = channels.first() {
                            first_text_channel_id = Some(ch.id.clone());
                        }
                        first_guild.channels = channels;
                    }
                    Err(e) => {
                        warn!("Falha ao buscar canais para servidor inicial {}: {:?}", first_guild.id, e);
                    }
                }
            }

            // 4. Buscar mensagens reais do canal (/channels/{id}/messages)
            let mut initial_messages = Vec::new();
            if let Some(ref cid) = first_text_channel_id {
                match client_clone.get_messages(cid, 50).await {
                    Ok(msgs) => {
                        info!("Obtidas {} mensagens reais para o canal #{}.", msgs.len(), cid);
                        initial_messages = msgs;
                    }
                    Err(e) => {
                        warn!("Falha ao obter mensagens para canal {}: {:?}", cid, e);
                    }
                }
            }

            // 5. Buscar pastas de servidores do usuário (/users/@me/settings)
            let guild_folders = client_clone.get_user_guild_settings().await.unwrap_or_default();

            // 6. Despachar os dados reais via TransportEvent::Ready para sincronizar o AppState
            let _ = event_tx_clone.send(TransportEvent::Ready {
                user: user.clone(),
                guilds,
                guild_folders,
            });

            // Despachar as mensagens reais obtidas em lote único cronológico
            if let Some(ref cid) = first_text_channel_id {
                if !initial_messages.is_empty() {
                    let mut msgs = initial_messages;
                    msgs.sort_by_key(|m| m.timestamp);
                    let _ = event_tx_clone.send(TransportEvent::ChannelMessagesLoaded {
                        channel_id: cid.clone(),
                        messages: msgs,
                    });
                }
            }

            // 7. Buscar canais de DM e relacionamentos reais do usuário
            if let Ok(dms) = client_clone.list_dm_channels().await {
                let _ = event_tx_clone.send(TransportEvent::DmChannelsUpdated(dms));
            }
            if let Ok(rels) = client_clone.list_relationships().await {
                let _ = event_tx_clone.send(TransportEvent::RelationshipsUpdated(rels));
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
        crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared().clear().await;

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
        let _ = self.event_tx.send(TransportEvent::Disconnected("Desconectado".to_string()));
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
    pub async fn get_user_profile(&self, user_id: &str) -> Result<crate::domain::profile::UserProfileData> {
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
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        let msg = transport.send_message(&real_channel_id, content).await?;
        // Broadcast local message creation event
        let _ = self.event_tx.send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }

    /// Sends a sticker message to a text channel using the active transport
    pub async fn send_sticker(&self, channel_id: &str, sticker_id: &str) -> Result<Message> {
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        let _ = self.event_tx.send(TransportEvent::MessageCreate(msg.clone()));
        Ok(msg)
    }

    /// Adds a reaction emoji to a message using the active transport
    pub async fn add_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        transport.add_reaction(&real_channel_id, message_id, emoji).await?;
        let _ = self.event_tx.send(TransportEvent::ReactionAdd {
            channel_id: real_channel_id,
            message_id: message_id.to_string(),
            user_id: "me".to_string(),
            emoji: emoji.to_string(),
        });
        Ok(())
    }

    /// Subscribes to events from the active transport
    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }

    /// Emits a transport event into the event stream (useful for synthetic or UI events)
    pub fn emit_event(&self, event: TransportEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Fetches message history for a channel using the active transport
    pub async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<Message>> {
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        };

        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_messages(&real_channel_id, limit).await
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
    pub async fn get_guild_emojis(&self, guild_id: &str) -> Result<Vec<crate::protocol::media_cache::DiscordEmoji>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_guild_emojis(guild_id).await
    }

    /// Fetches all custom stickers for a guild using the active transport
    pub async fn get_guild_stickers(&self, guild_id: &str) -> Result<Vec<crate::protocol::media_cache::DiscordSticker>> {
        let transport = {
            let lock = self.transport.read().await;
            lock.clone()
        };
        transport.get_guild_stickers(guild_id).await
    }

    /// Sends a friend request to a user (/users/@me/relationships)
    pub async fn send_friend_request(&self, username: &str, discriminator: Option<&str>) -> Result<()> {
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
    pub async fn active_voice_session(&self) -> Option<Arc<crate::protocol::voice_session::VoiceSession>> {
        let active = self.active_voice.read().await;
        active.clone()
    }

    /// Subscribes to real-time voice state transitions and measured latency.
    pub fn subscribe_voice_state(
        &self,
    ) -> broadcast::Receiver<(crate::protocol::voice_session::VoiceSessionState, Option<u32>)> {
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
        let device_manager = crate::media::devices::MediaDeviceManager::new();
        let has_hardware_audio = device_manager
            .list_input_devices()
            .map(|d| !d.is_empty())
            .unwrap_or(false)
            && device_manager
                .list_output_devices()
                .map(|d| !d.is_empty())
                .unwrap_or(false);

        let audio_bridge = if has_hardware_audio {
            crate::protocol::voice_session::AudioBridge::new_thread()
        } else {
            let (_audio_out_tx, audio_out_rx) = tokio::sync::mpsc::channel(64);
            let (_audio_spk_tx, audio_spk_rx) = tokio::sync::mpsc::channel(16);
            let (audio_cmd_tx, mut audio_cmd_rx) = tokio::sync::mpsc::channel(64);

            tokio::spawn(async move {
                while let Some(cmd) = audio_cmd_rx.recv().await {
                    match cmd {
                        crate::protocol::voice_session::AudioBridgeCommand::StartSession { responder } => {
                            let _ = responder.send(Ok(()));
                        }
                        crate::protocol::voice_session::AudioBridgeCommand::ReceiveOpusPacket { .. } => {}
                        crate::protocol::voice_session::AudioBridgeCommand::StopSession => {}
                        crate::protocol::voice_session::AudioBridgeCommand::SetMuted(_) => {}
                        crate::protocol::voice_session::AudioBridgeCommand::SetDeafened(_) => {}
                        crate::protocol::voice_session::AudioBridgeCommand::SwitchInputDevice { responder, .. } => {
                            let _ = responder.send(Ok(()));
                        }
                        crate::protocol::voice_session::AudioBridgeCommand::SwitchOutputDevice { responder, .. } => {
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
        tokio::spawn(async move {
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

        // Subscribe to session events (speaking / SSRC mapping / latency)
        let mut events_rx = session.subscribe_events();
        let voice_evt_tx = self.voice_event_tx.clone();
        let voice_tx_ack = self.voice_state_tx.clone();
        let session_ref2 = session.clone();
        tokio::spawn(async move {
            while let Ok(evt) = events_rx.recv().await {
                log::debug!("[ClientManager] VoiceSession event: {:?}", evt);
                if let crate::protocol::voice_session::VoiceSessionEvent::HeartbeatAck { ping_ms } = evt {
                    let current_st = session_ref2.state();
                    let _ = voice_tx_ack.send((current_st, Some(ping_ms)));
                }
                let _ = voice_evt_tx.send(evt);
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
                    "[VoiceDiag] Chamada já existia no canal {}; outro lado ligou primeiro (não tocando), chamada_ja_existia=true",
                    ch_str
                );
            } else {
                let mut confirm_rx = session.subscribe_entry_confirmed();
                let mut state_rx = session.subscribe_state();
                let cm_ring = self.clone();
                let session_weak = Arc::downgrade(&session);
                tokio::spawn(async move {
                    let mut confirmed = *confirm_rx.borrow();
                    while !confirmed {
                        tokio::select! {
                            res = confirm_rx.changed() => {
                                if res.is_ok() && *confirm_rx.borrow() {
                                    confirmed = true;
                                    break;
                                } else if res.is_err() {
                                    break;
                                }
                            }
                            res = state_rx.changed() => {
                                if res.is_ok() && matches!(*state_rx.borrow(), crate::protocol::voice_session::VoiceSessionState::Disconnected { .. }) {
                                    break;
                                }
                            }
                        }
                    }
                    if confirmed {
                        log::info!("[ClientManager] CALL_CREATE confirmado na chamada DM; disparando ring_call");
                        if let Err(e) = cm_ring.ring_call(&ch_str, Some(recips)).await {
                            let short_msg = "Não foi possível tocar a chamada".to_string();
                            log::warn!("[VoiceRing] Falha ao tocar chamada (ring_call recusado): {}", e);
                            if let Some(sess) = session_weak.upgrade() {
                                sess.set_call_warning(Some(short_msg));
                            }
                        } else {
                            log::info!("[VoiceRing] ring_call enviado com sucesso para a chamada DM");
                        }
                    }
                });
            }
        }

        if let Err(e) = session.start().await {
            let reason = format!("Falha ao iniciar sessão de voz: {}", e);
            let _ = self.voice_state_tx.send((
                crate::protocol::voice_session::VoiceSessionState::Disconnected {
                    reason,
                },
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
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        transport.subscribe_guild_members(guild_id, channel_id).await
    }

    /// Edits an existing message (/channels/{channel_id}/messages/{message_id})
    pub async fn edit_message(
        &self,
        channel_id: &str,
        message_id: &str,
        content: &str,
    ) -> Result<Message> {
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        transport.edit_message(&real_channel_id, message_id, content).await
    }

    /// Pins a message in a channel (/channels/{channel_id}/pins/{message_id})
    pub async fn pin_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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
        let real_channel_id = if channel_id.starts_with("dm-") && !channel_id.starts_with("dm-group-") {
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

    /// Requests guild members for message authors missing from cache (Gateway Opcode 8)
    pub async fn request_guild_members(
        &self,
        guild_id: &str,
        user_ids: Vec<String>,
    ) -> Result<()> {
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
        transport.update_guild_member_viewport(guild_id, channel_id, ranges).await
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
            client.search_guild_messages(gid, query, offset, sort_order).await
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
            client.search_channel_messages(&real_cid, query, offset, sort_order).await
        } else {
            anyhow::bail!("Nenhum canal ou servidor selecionado para busca");
        }
    }
}

