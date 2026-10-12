use crate::domain::{Channel, Guild, Message, User};
use crate::protocol::media_cache::{DiscordEmoji, DiscordSticker};
use crate::protocol::transport::{DiscordTransport, Relationship, TransportEvent};
use async_trait::async_trait;
use tokio::sync::broadcast;

/// Idle transport for production when disconnected or unauthenticated.
/// Strictly zero mocks: returns empty collections or "not connected" errors on all operations.
#[derive(Debug)]
pub struct IdleTransport {
    event_tx: broadcast::Sender<TransportEvent>,
}

impl IdleTransport {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(16);
        Self { event_tx }
    }
}

impl Default for IdleTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DiscordTransport for IdleTransport {
    async fn connect(&self) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn disconnect(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_current_user(&self) -> anyhow::Result<User> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn list_guilds(&self) -> anyhow::Result<Vec<Guild>> {
        Ok(Vec::new())
    }

    async fn get_guild(&self, _guild_id: &str) -> anyhow::Result<Option<Guild>> {
        Ok(None)
    }

    async fn get_guild_emojis(&self, _guild_id: &str) -> anyhow::Result<Vec<DiscordEmoji>> {
        Ok(Vec::new())
    }

    async fn get_guild_stickers(&self, _guild_id: &str) -> anyhow::Result<Vec<DiscordSticker>> {
        Ok(Vec::new())
    }

    async fn list_channels(&self, _guild_id: &str) -> anyhow::Result<Vec<Channel>> {
        Ok(Vec::new())
    }

    async fn list_dm_channels(&self) -> anyhow::Result<Vec<Channel>> {
        Ok(Vec::new())
    }

    async fn create_or_get_dm_channel(&self, _recipient_id: &str) -> anyhow::Result<Channel> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn list_relationships(&self) -> anyhow::Result<Vec<Relationship>> {
        Ok(Vec::new())
    }

    async fn send_friend_request(
        &self,
        _username: &str,
        _discriminator: Option<&str>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn accept_friend_request(&self, _user_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn remove_relationship(&self, _user_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn get_messages(&self, _channel_id: &str, _limit: usize) -> anyhow::Result<Vec<Message>> {
        Ok(Vec::new())
    }

    async fn send_message(&self, _channel_id: &str, _content: &str) -> anyhow::Result<Message> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn send_sticker(&self, _channel_id: &str, _sticker_id: &str) -> anyhow::Result<Message> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn add_reaction(
        &self,
        _channel_id: &str,
        _message_id: &str,
        _emoji: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn remove_reaction(
        &self,
        _channel_id: &str,
        _message_id: &str,
        _emoji: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn send_voice_state_update(
        &self,
        _guild_id: Option<&str>,
        _channel_id: Option<&str>,
        _self_mute: bool,
        _self_deaf: bool,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    async fn ring_call(
        &self,
        _channel_id: &str,
        _recipients: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("Não conectado ao Discord")
    }

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }
}
