//! Discord RS - Asynchronous Protocol Bridge
//!
//! Bridges real-time Discord API & Gateway v10 WebSocket traffic with the GPUI application state.
//! Manages credentials persistence, WebSocket event forwarding, message dispatching,
//! and REST historical queries.

use crate::domain::{Channel, Guild, Message};
use crate::protocol::auth::{AuthToken, TokenStorage, TokenType};
use crate::protocol::discord_api::DiscordApiClient;
use crate::protocol::transport::{DiscordTransport, TransportEvent};
use anyhow::{bail, Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

/// Default capacity for broadcast channels
const BROADCAST_CAPACITY: usize = 512;

/// Asynchronous Protocol Bridge connecting Discord REST & Gateway WebSocket with GPUI.
#[derive(Clone)]
pub struct ProtocolBridge {
    client: Arc<RwLock<Option<DiscordApiClient>>>,
    token_storage: Arc<TokenStorage>,
    event_broadcast: broadcast::Sender<TransportEvent>,
    unbounded_sender: mpsc::UnboundedSender<TransportEvent>,
    initial_unbounded_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<TransportEvent>>>>,
    is_connected: Arc<AtomicBool>,
    forwarding_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl ProtocolBridge {
    /// Creates a new protocol bridge using the default user config path (`~/.config/discord-rs/auth.json`).
    pub fn new() -> Result<Self> {
        let storage = TokenStorage::new().context("Failed to initialize default TokenStorage")?;
        Ok(Self::with_storage(storage))
    }

    /// Creates a new protocol bridge with a custom TokenStorage (useful for tests or alternate paths).
    pub fn with_storage(storage: TokenStorage) -> Self {
        let (event_broadcast, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (unbounded_sender, unbounded_rx) = mpsc::unbounded_channel();

        Self {
            client: Arc::new(RwLock::new(None)),
            token_storage: Arc::new(storage),
            event_broadcast,
            unbounded_sender,
            initial_unbounded_rx: Arc::new(Mutex::new(Some(unbounded_rx))),
            is_connected: Arc::new(AtomicBool::new(false)),
            forwarding_task: Arc::new(Mutex::new(None)),
        }
    }

    // ========================================================================
    // Credential Management (TokenStorage)
    // ========================================================================

    /// Loads saved token from `auth.json`, falling back to `DISCORD_TOKEN` environment variable.
    pub fn load_saved_token(&self) -> Result<Option<AuthToken>> {
        if let Some(token) = self.token_storage.load()? {
            return Ok(Some(token));
        }
        Ok(TokenStorage::load_from_env())
    }

    /// Persists credentials to `auth.json` with secure file permissions.
    pub fn save_token(&self, token: &AuthToken) -> Result<()> {
        self.token_storage.save(token)
    }

    /// Clears and removes stored credentials file.
    pub fn clear_token(&self) -> Result<()> {
        self.token_storage.clear()
    }

    /// Returns the filesystem path where credentials are stored.
    pub fn token_storage_path(&self) -> &std::path::Path {
        self.token_storage.path()
    }

    // ========================================================================
    // Connection Lifecycle
    // ========================================================================

    /// Connects using an existing or user-provided `AuthToken`.
    ///
    /// If `persist` is true, the token is saved to `auth.json`.
    /// Starts the Gateway WebSocket (v10) event loop in the background via `tokio::spawn`.
    pub async fn connect_with_token(&self, token: AuthToken, persist: bool) -> Result<()> {
        if !token.is_valid_format() {
            bail!("Invalid Discord authentication token format");
        }

        if persist {
            if let Err(e) = self.save_token(&token) {
                log::warn!("Failed to persist token to storage: {:#}", e);
            }
        }

        // Disconnect any previous active connection
        self.disconnect().await?;

        let client = DiscordApiClient::new(token.clone());
        let mut client_events = client.subscribe_events();

        // Connect Gateway WebSocket v10
        client
            .connect()
            .await
            .context("Failed to connect DiscordApiClient gateway")?;

        self.is_connected.store(true, Ordering::SeqCst);

        // Spawn background task forwarding events from client to bridge channels
        let b_tx = self.event_broadcast.clone();
        let u_tx = self.unbounded_sender.clone();
        let is_connected_flag = self.is_connected.clone();

        let forward_handle = tokio::spawn(async move {
            while let Ok(event) = client_events.recv().await {
                // Broadcast to multiple UI listeners
                let _ = b_tx.send(event.clone());
                // Stream to unbounded channel
                let _ = u_tx.send(event);
            }
            is_connected_flag.store(false, Ordering::SeqCst);
        });

        // Store active client and task handle
        {
            let mut client_guard = self.client.write().await;
            *client_guard = Some(client.clone());
        }
        {
            let mut task_guard = self.forwarding_task.lock().await;
            *task_guard = Some(forward_handle);
        }

        // Asynchronously fetch real initial data (user @me, guilds, channels, messages)
        let client_clone = client.clone();
        let b_tx_clone = self.event_broadcast.clone();
        let u_tx_clone = self.unbounded_sender.clone();

        tokio::spawn(async move {
            log::info!("ProtocolBridge: buscando dados iniciais da API do Discord (@me, guilds, canais, mensagens)...");

            // 1. Buscar usuário autenticado via API (@me)
            let user = match client_clone.get_current_user().await {
                Ok(u) => {
                    log::info!("ProtocolBridge: Usuário autenticado obtido com sucesso: @{} ({})", u.username, u.id);
                    u
                }
                Err(e) => {
                    let err_msg = "Falha na autenticação: Token inválido ou não autorizado (401)".to_string();
                    log::warn!("ProtocolBridge: {}: {:?}", err_msg, e);
                    let disconnect_event = TransportEvent::Disconnected(err_msg);
                    let _ = b_tx_clone.send(disconnect_event.clone());
                    let _ = u_tx_clone.send(disconnect_event);
                    return;
                }
            };

            // 2. Buscar servidores reais via API (/users/@me/guilds)
            let mut guilds = match client_clone.list_guilds().await {
                Ok(g) => {
                    log::info!("ProtocolBridge: Obtidos {} servidores do Discord.", g.len());
                    g
                }
                Err(e) => {
                    log::warn!("ProtocolBridge: Falha ao listar servidores via REST: {:?}", e);
                    Vec::new()
                }
            };

            // 3. Buscar canais reais das guilds
            let mut first_text_channel_id: Option<String> = None;
            for guild in &mut guilds {
                match client_clone.list_channels(&guild.id).await {
                    Ok(channels) => {
                        log::info!("ProtocolBridge: Servidor '{}' possui {} canais.", guild.name, channels.len());
                        if first_text_channel_id.is_none() {
                            if let Some(ch) = channels.iter().find(|c| c.channel_type == crate::domain::ChannelType::Text) {
                                first_text_channel_id = Some(ch.id.clone());
                            } else if let Some(ch) = channels.first() {
                                first_text_channel_id = Some(ch.id.clone());
                            }
                        }
                        guild.channels = channels;
                    }
                    Err(e) => {
                        log::warn!("ProtocolBridge: Falha ao buscar canais para guild {}: {:?}", guild.id, e);
                    }
                }
            }

            // 4. Buscar mensagens reais do canal (/channels/{id}/messages)
            let mut initial_messages = Vec::new();
            if let Some(ref cid) = first_text_channel_id {
                match client_clone.get_messages(cid, 50).await {
                    Ok(msgs) => {
                        log::info!("ProtocolBridge: Obtidas {} mensagens reais para o canal #{}.", msgs.len(), cid);
                        initial_messages = msgs;
                    }
                    Err(e) => {
                        log::warn!("ProtocolBridge: Falha ao obter mensagens para canal {}: {:?}", cid, e);
                    }
                }
            }

            // 5. Despachar os dados reais via TransportEvent::Ready para sincronizar o AppState
            let ready_event = TransportEvent::Ready {
                user: user.clone(),
                guilds,
                guild_folders: Vec::new(),
            };
            let _ = b_tx_clone.send(ready_event.clone());
            let _ = u_tx_clone.send(ready_event);

            // Despachar as mensagens reais obtidas
            for msg in initial_messages {
                let msg_event = TransportEvent::MessageCreate(msg);
                let _ = b_tx_clone.send(msg_event.clone());
                let _ = u_tx_clone.send(msg_event);
            }
        });

        Ok(())
    }

    /// Connects using credentials from `auth.json` or `DISCORD_TOKEN` env var if available.
    ///
    /// Returns `Ok(true)` if a token was found and connection initiated, `Ok(false)` if no token was found.
    pub async fn connect_saved(&self) -> Result<bool> {
        match self.load_saved_token()? {
            Some(token) => {
                self.connect_with_token(token, false).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Connects using raw string token and specified token type (useful for login input forms).
    pub async fn connect_raw(&self, raw_token: &str, token_type: TokenType, persist: bool) -> Result<()> {
        let token = match token_type {
            TokenType::Bot => AuthToken::bot(raw_token),
            TokenType::User => AuthToken::user(raw_token),
        };
        self.connect_with_token(token, persist).await
    }

    /// Gracefully disconnects the gateway loop and REST client.
    pub async fn disconnect(&self) -> Result<()> {
        self.is_connected.store(false, Ordering::SeqCst);

        // Abort forwarding task
        {
            let mut task_guard = self.forwarding_task.lock().await;
            if let Some(handle) = task_guard.take() {
                handle.abort();
            }
        }

        // Disconnect and clear client
        let client_opt = {
            let mut client_guard = self.client.write().await;
            client_guard.take()
        };

        if let Some(client) = client_opt {
            let _ = client.disconnect().await;
        }

        // Notify subscribers of disconnection
        let disconnect_event = TransportEvent::Disconnected("ProtocolBridge disconnected".to_string());
        let _ = self.event_broadcast.send(disconnect_event.clone());
        let _ = self.unbounded_sender.send(disconnect_event);

        Ok(())
    }

    /// Checks whether the bridge is actively connected to the Discord Gateway.
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    /// Retrieves the current authenticated token, if any.
    pub async fn active_token(&self) -> Option<AuthToken> {
        let client_guard = self.client.read().await;
        client_guard.as_ref().map(|c| c.token().clone())
    }

    // ========================================================================
    // Event Channels (Unbounded & Broadcast)
    // ========================================================================

    /// Takes the primary unbounded receiver created on initialization.
    /// Can only be claimed once by the primary GPUI event loop.
    pub async fn take_unbounded_receiver(&self) -> Option<mpsc::UnboundedReceiver<TransportEvent>> {
        let mut guard = self.initial_unbounded_rx.lock().await;
        guard.take()
    }

    /// Subscribes to the broadcast channel, receiving all subsequent `TransportEvent`s.
    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.event_broadcast.subscribe()
    }

    /// Subscribes to real-time events via a new dedicated `mpsc::UnboundedReceiver`.
    ///
    /// Spawns a lightweight forwarding task that streams all broadcast events into the receiver.
    pub fn subscribe_unbounded(&self) -> mpsc::UnboundedReceiver<TransportEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut b_rx = self.event_broadcast.subscribe();

        tokio::spawn(async move {
            while let Ok(event) = b_rx.recv().await {
                if tx.send(event).is_err() {
                    break;
                }
            }
        });

        rx
    }

    /// Emits an event directly into the bridge channels.
    ///
    /// Useful for synthetic local events, testing, or optimistic UI updates.
    pub fn emit_event(&self, event: TransportEvent) {
        let _ = self.event_broadcast.send(event.clone());
        let _ = self.unbounded_sender.send(event);
    }

    // ========================================================================
    // REST API Actions (Messaging & Channels)
    // ========================================================================

    /// Posts a new chat message to a Discord text channel.
    pub async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot send message: ProtocolBridge is not connected to Discord")?
        };

        client
            .send_message(channel_id, content)
            .await
            .context("Failed to send message via DiscordApiClient")
    }

    /// Fetches recent message history for a channel.
    pub async fn get_messages(&self, channel_id: &str, limit: usize) -> Result<Vec<Message>> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot fetch messages: ProtocolBridge is not connected to Discord")?
        };

        client
            .get_messages(channel_id, limit)
            .await
            .context("Failed to get messages via DiscordApiClient")
    }

    /// Fetches all guilds/servers the authenticated account is a member of.
    pub async fn list_guilds(&self) -> Result<Vec<Guild>> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot list guilds: ProtocolBridge is not connected to Discord")?
        };

        client.list_guilds().await
    }

    /// Retrieves detailed guild information by guild ID.
    pub async fn get_guild(&self, guild_id: &str) -> Result<Option<Guild>> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot get guild: ProtocolBridge is not connected to Discord")?
        };

        client.get_guild(guild_id).await
    }

    /// Lists all channels within a guild.
    pub async fn list_channels(&self, guild_id: &str) -> Result<Vec<Channel>> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot list channels: ProtocolBridge is not connected to Discord")?
        };

        client.list_channels(guild_id).await
    }

    /// Lists all direct message (DM) channels for the authenticated user.
    pub async fn list_dm_channels(&self) -> Result<Vec<Channel>> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot list DM channels: ProtocolBridge is not connected to Discord")?
        };

        client.list_dm_channels().await
    }

    /// Creates or opens a direct message channel with a user.
    pub async fn create_or_get_dm_channel(&self, recipient_id: &str) -> Result<Channel> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot create DM channel: ProtocolBridge is not connected to Discord")?
        };

        client.create_or_get_dm_channel(recipient_id).await
    }

    /// Adds an emoji reaction to a message.
    pub async fn add_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot add reaction: ProtocolBridge is not connected to Discord")?
        };

        client.add_reaction(channel_id, message_id, emoji).await
    }

    /// Removes an emoji reaction from a message.
    pub async fn remove_reaction(&self, channel_id: &str, message_id: &str, emoji: &str) -> Result<()> {
        let client = {
            let guard = self.client.read().await;
            guard
                .as_ref()
                .cloned()
                .context("Cannot remove reaction: ProtocolBridge is not connected to Discord")?
        };

        client.remove_reaction(channel_id, message_id, emoji).await
    }
}
