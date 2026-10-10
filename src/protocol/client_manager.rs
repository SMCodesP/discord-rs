//! Discord RS - Client Manager & Real-Time Bridge
//! Coordinates the active transport (live Gateway/REST Discord API or deterministic mock),
//! processes incoming WebSocket Gateway events, and provides thread-safe messaging.

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use anyhow::Result;
use log::{info, warn, error};

use crate::domain::channel::{Channel, ChannelType};
use crate::domain::message::Message;
use crate::protocol::auth::AuthToken;
use crate::protocol::discord_api::DiscordApiClient;
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
}

impl std::fmt::Debug for ClientManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientManager")
            .field("mode", &"<ConnectionMode>")
            .finish()
    }
}

impl ClientManager {
    /// Creates a new ClientManager initialized with the deterministic Mock provider
    pub fn new_mock() -> (Self, broadcast::Receiver<TransportEvent>) {
        let mock = Arc::new(MockDiscordProvider::new());
        let (event_tx, event_rx) = broadcast::channel(256);

        let manager = Self {
            transport: Arc::new(RwLock::new(mock)),
            mode: Arc::new(RwLock::new(ConnectionMode::Mock)),
            event_tx,
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
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Disconnected(e.to_string());
            let _ = self.event_tx.send(TransportEvent::Disconnected(e.to_string()));
            return Err(e);
        }

        // Bridge gateway events to global event_tx
        let mut gateway_rx = real_client.subscribe_events();
        let event_tx = self.event_tx.clone();
        let mode_ref = self.mode.clone();

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
                    _ => {}
                }
                let _ = event_tx.send(evt);
            }
        });

        // Swap active transport
        {
            let mut current = self.transport.write().await;
            *current = real_client.clone();
        }

        info!("Cliente do Discord configurado com sucesso como transporte ativo.");

        // Asynchronously fetch real initial data (user @me, guilds, channels, messages)
        let client_clone = real_client.clone();
        let event_tx_clone = self.event_tx.clone();
        let mode_clone = self.mode.clone();

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

    /// Swaps back to deterministic mock provider
    pub async fn connect_mock(&self) -> Result<()> {
        info!("Alternando para provedor local determinístico...");
        let mock = Arc::new(MockDiscordProvider::new());
        {
            let mut current = self.transport.write().await;
            *current = mock;
        }
        {
            let mut mode = self.mode.write().await;
            *mode = ConnectionMode::Mock;
        }
        let _ = self.event_tx.send(TransportEvent::Connected);
        Ok(())
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
}
