//! Integration tests for jump-to-message: around fetch, scroll/highlight, honest errors.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use discord_rs::app::state::{CategoryItemState, ChannelItemState, GuildItemState};
use discord_rs::app::{AppState, ChannelKind};
use discord_rs::domain::{Author, Message};
use discord_rs::protocol::transport::{DiscordTransport, TransportEvent};
use discord_rs::protocol::ClientManager;
use tokio::sync::broadcast;

fn msg(id: &str, channel_id: &str, content: &str) -> Message {
    Message::new(id, channel_id, Author::new("u1", "alice"), content)
}

fn state_with_channel(channel_id: &str) -> AppState {
    let mut state = AppState::new();
    state.guilds = vec![GuildItemState {
        id: "g1".to_string(),
        name: "Guild".to_string(),
        acronym: "G".to_string(),
        icon_url: None,
        icon_hash: None,
        unread: false,
        mention_count: 0,
        categories: vec![CategoryItemState {
            id: "cat1".to_string(),
            name: "Text".to_string(),
            channels: vec![ChannelItemState {
                id: channel_id.to_string(),
                name: "general".to_string(),
                kind: ChannelKind::Text,
                topic: None,
                unread: false,
                mention_count: 0,
                icon_url: None,
                is_pinned: false,
                last_message_id: None,
                last_read_message_id: None,
                is_group_dm: false,
                recipient_ids: vec![],
                recipient_names: vec![],
                user_limit: None,
                thread_metadata: None,
                default_tag_ids: Vec::new(),
                available_tags: Vec::new(),
                flags: 0,
                rate_limit_per_user: 0,
                avatar_decoration_data: None,
            }],
        }],
        emojis: vec![],
        stickers: vec![],
    }];
    state.selected_guild_id = Some("g1".to_string());
    state.selected_channel_id = Some(channel_id.to_string());
    state
}

#[derive(Clone, Default)]
struct AroundTransport {
    around_calls: Arc<Mutex<Vec<(String, String, usize)>>>,
    around_messages: Arc<Mutex<Vec<Message>>>,
    around_error: Arc<Mutex<Option<String>>>,
}

impl AroundTransport {
    fn with_messages(messages: Vec<Message>) -> Self {
        Self {
            around_calls: Arc::new(Mutex::new(Vec::new())),
            around_messages: Arc::new(Mutex::new(messages)),
            around_error: Arc::new(Mutex::new(None)),
        }
    }

    fn with_error(err: &str) -> Self {
        Self {
            around_calls: Arc::new(Mutex::new(Vec::new())),
            around_messages: Arc::new(Mutex::new(Vec::new())),
            around_error: Arc::new(Mutex::new(Some(err.to_string()))),
        }
    }

    fn calls(&self) -> Vec<(String, String, usize)> {
        self.around_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl DiscordTransport for AroundTransport {
    async fn connect(&self) -> anyhow::Result<()> { Ok(()) }
    async fn disconnect(&self) -> anyhow::Result<()> { Ok(()) }
    fn subscribe_events(&self) -> broadcast::Receiver<TransportEvent> { broadcast::channel(8).1 }
    async fn get_current_user(&self) -> anyhow::Result<discord_rs::domain::User> {
        Ok(discord_rs::domain::User::new("u1", "tester"))
    }
    async fn get_guild(&self, _guild_id: &str) -> anyhow::Result<Option<discord_rs::domain::Guild>> {
        Ok(None)
    }
    async fn list_guilds(&self) -> anyhow::Result<Vec<discord_rs::domain::Guild>> { Ok(vec![]) }
    async fn list_channels(&self, _guild_id: &str) -> anyhow::Result<Vec<discord_rs::domain::Channel>> { Ok(vec![]) }
    async fn list_dm_channels(&self) -> anyhow::Result<Vec<discord_rs::domain::Channel>> { Ok(vec![]) }
    async fn get_messages(&self, _channel_id: &str, _limit: usize) -> anyhow::Result<Vec<Message>> { Ok(vec![]) }
    async fn get_messages_around(
        &self,
        channel_id: &str,
        message_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<Message>> {
        self.around_calls.lock().unwrap().push((channel_id.to_string(), message_id.to_string(), limit));
        if let Some(err) = self.around_error.lock().unwrap().clone() {
            anyhow::bail!(err);
        }
        Ok(self.around_messages.lock().unwrap().clone())
    }
    async fn send_message(&self, _channel_id: &str, _content: &str) -> anyhow::Result<Message> { anyhow::bail!("not used") }
    async fn send_sticker(&self, _channel_id: &str, _sticker_id: &str) -> anyhow::Result<Message> { anyhow::bail!("not used") }
    async fn add_reaction(&self, _channel_id: &str, _message_id: &str, _emoji: &str) -> anyhow::Result<()> { Ok(()) }
    async fn remove_reaction(&self, _channel_id: &str, _message_id: &str, _emoji: &str) -> anyhow::Result<()> { Ok(()) }
    async fn delete_message(&self, _channel_id: &str, _message_id: &str) -> anyhow::Result<()> { Ok(()) }
}

#[tokio::test]
async fn jump_fetches_around_when_message_not_cached() {
    let channel_id = "c1";
    let target = "m-target";
    let transport = Arc::new(AroundTransport::with_messages(vec![
        msg("m1", channel_id, "before"),
        msg(target, channel_id, "target"),
        msg("m3", channel_id, "after"),
    ]));
    let (cm, _rx) = ClientManager::new_with_transport(transport.clone());
    let mut state = state_with_channel(channel_id);
    state.client_manager = Some(cm);

    state.jump_to_message(channel_id, target);
    assert_eq!(state.target_message_id.as_deref(), Some(target));
    assert_eq!(state.jump_target_channel_id.as_deref(), Some(channel_id));

    // Simulate ChatView resolve: fetch around through ClientManager when missing.
    let messages = state
        .client_manager
        .as_ref()
        .unwrap()
        .get_messages_around(channel_id, target, 50)
        .await
        .expect("around fetch");
    state.set_channel_messages(channel_id, messages);

    let loaded = state.get_active_messages();
    assert!(loaded.iter().any(|m| m.id == target));
    assert_eq!(transport.calls(), vec![(channel_id.to_string(), target.to_string(), 50)]);
}

#[tokio::test]
async fn jump_reports_honest_error_when_message_missing() {
    let channel_id = "c1";
    let target = "m-missing";
    let transport = Arc::new(AroundTransport::with_messages(vec![
        msg("m1", channel_id, "other"),
    ]));
    let (cm, _rx) = ClientManager::new_with_transport(transport);
    let mut state = state_with_channel(channel_id);
    state.client_manager = Some(cm);
    state.jump_to_message(channel_id, target);

    let messages = state
        .client_manager
        .as_ref()
        .unwrap()
        .get_messages_around(channel_id, target, 50)
        .await
        .expect("around fetch");
    state.set_channel_messages(channel_id, messages);
    let found = state.get_active_messages().iter().any(|m| m.id == target);
    assert!(!found);
    state.target_message_id = None;
    state.jump_target_channel_id = None;
    state.highlighted_message_id = None;
    state.set_jump_error(Some("Mensagem não encontrada ou sem permissão para acessá-la.".to_string()));

    assert_eq!(state.target_message_id, None);
    assert_eq!(state.highlighted_message_id, None);
    assert!(state.jump_error.as_deref().unwrap_or("").contains("não encontrada"));
}

#[tokio::test]
async fn jump_reports_honest_error_on_transport_failure() {
    let channel_id = "c1";
    let target = "m-forbidden";
    let transport = Arc::new(AroundTransport::with_error("HTTP 403 Forbidden"));
    let (cm, _rx) = ClientManager::new_with_transport(transport);
    let mut state = state_with_channel(channel_id);
    state.client_manager = Some(cm);
    state.jump_to_message(channel_id, target);

    let err = state
        .client_manager
        .as_ref()
        .unwrap()
        .get_messages_around(channel_id, target, 50)
        .await
        .expect_err("must fail");
    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("403") || msg.contains("forbidden"));
}
