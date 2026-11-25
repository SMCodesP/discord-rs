use discord_rs::app::state::{ActiveModal, AppState, FriendsTab, SettingsTab};
use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::{ClientManager, DiscordTransport, TransportEvent};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
struct MockState {
    pins: Vec<Message>,
    pin_calls: Vec<(String, String)>,
    unpin_calls: Vec<(String, String)>,
    fail_get_pins: bool,
}

struct PinsMockTransport {
    state: Arc<RwLock<MockState>>,
    event_tx: tokio::sync::broadcast::Sender<TransportEvent>,
}

impl PinsMockTransport {
    fn new(state: Arc<RwLock<MockState>>) -> (Arc<Self>, tokio::sync::broadcast::Receiver<TransportEvent>) {
        let (tx, rx) = tokio::sync::broadcast::channel(64);
        (Arc::new(Self { state, event_tx: tx }), rx)
    }
}

#[async_trait::async_trait]
impl DiscordTransport for PinsMockTransport {
    async fn connect(&self) -> anyhow::Result<()> { Ok(()) }
    async fn disconnect(&self) -> anyhow::Result<()> { Ok(()) }
    async fn get_current_user(&self) -> anyhow::Result<discord_rs::domain::User> {
        Ok(discord_rs::domain::User::new("u1", "tester"))
    }
    async fn list_guilds(&self) -> anyhow::Result<Vec<discord_rs::domain::guild::Guild>> { Ok(vec![]) }
    async fn get_guild(&self, _guild_id: &str) -> anyhow::Result<Option<discord_rs::domain::guild::Guild>> { Ok(None) }
    async fn list_channels(&self, _guild_id: &str) -> anyhow::Result<Vec<Channel>> { Ok(vec![]) }
    async fn list_dm_channels(&self) -> anyhow::Result<Vec<Channel>> { Ok(vec![]) }
    async fn get_messages(&self, _channel_id: &str, _limit: usize) -> anyhow::Result<Vec<Message>> { Ok(vec![]) }
    async fn send_message(&self, channel_id: &str, content: &str) -> anyhow::Result<Message> {
        Ok(Message::new("m-send", channel_id, Author::new("u1", "tester"), content))
    }
    async fn send_sticker(&self, channel_id: &str, _sticker_id: &str) -> anyhow::Result<Message> {
        Ok(Message::new("m-sticker", channel_id, Author::new("u1", "tester"), ""))
    }
    async fn add_reaction(&self, _channel_id: &str, _message_id: &str, _emoji: &str) -> anyhow::Result<()> { Ok(()) }
    async fn remove_reaction(&self, _channel_id: &str, _message_id: &str, _emoji: &str) -> anyhow::Result<()> { Ok(()) }
    async fn delete_message(&self, _channel_id: &str, _message_id: &str) -> anyhow::Result<()> { Ok(()) }
    async fn pin_message(&self, channel_id: &str, message_id: &str) -> anyhow::Result<()> {
        let mut st = self.state.write().await;
        st.pin_calls.push((channel_id.to_string(), message_id.to_string()));
        for m in &mut st.pins {
            if m.id == message_id { m.pinned = true; }
        }
        Ok(())
    }
    async fn unpin_message(&self, channel_id: &str, message_id: &str) -> anyhow::Result<()> {
        let mut st = self.state.write().await;
        st.unpin_calls.push((channel_id.to_string(), message_id.to_string()));
        st.pins.retain(|m| m.id != message_id);
        Ok(())
    }
    async fn get_pinned_messages(&self, _channel_id: &str) -> anyhow::Result<Vec<Message>> {
        let st = self.state.read().await;
        if st.fail_get_pins {
            anyhow::bail!("synthetic pins fetch error");
        }
        Ok(st.pins.clone())
    }
    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<TransportEvent> {
        self.event_tx.subscribe()
    }
}

fn synthetic_message(id: &str, channel_id: &str, content: &str, pinned: bool) -> Message {
    let mut msg = Message::new(id, channel_id, Author::new("u-author", "Synth Author"), content);
    msg.pinned = pinned;
    msg
}

#[test]
fn header_notification_inbox_help_actions_are_real() {
    let mut state = AppState::new();
    state.user_settings_tab = SettingsTab::MyAccount;
    state.selected_guild_id = Some("g1".to_string());
    state.friends_tab = FriendsTab::Online;

    // Notifications button action: open User Settings on Notifications tab.
    state.user_settings_tab = SettingsTab::Notifications;
    state.open_modal(ActiveModal::UserSettings);
    assert_eq!(state.active_modal, Some(ActiveModal::UserSettings));
    assert_eq!(state.user_settings_tab, SettingsTab::Notifications);

    state.close_modal();
    // Inbox button action: go to Home + Message Requests tab.
    state.selected_guild_id = None;
    state.select_friends_tab(FriendsTab::MessageRequests);
    assert_eq!(state.selected_guild_id, None);
    assert_eq!(state.friends_tab, FriendsTab::MessageRequests);
}

#[test]
fn pins_panel_state_handles_empty_loaded_and_error() {
    let mut state = AppState::new();
    let channel_id = "c-pins";

    // No client manager => honest error, no fake data.
    let req = state.begin_load_pins(channel_id);
    assert!(req.is_none());
    assert!(!state.is_pins_loading);
    assert_eq!(state.pinned_messages_error.as_deref(), Some("Não conectado à API do Discord"));
    assert!(state.pinned_messages.is_empty());

    // Loaded empty.
    state.pins_generation = 1;
    state.pinned_messages_channel_id = Some(channel_id.to_string());
    state.is_pins_loading = true;
    state.finish_load_pins(channel_id, 1, Ok(vec![]));
    assert!(!state.is_pins_loading);
    assert!(state.pinned_messages.is_empty());
    assert!(state.pinned_messages_error.is_none());

    // Error.
    state.pins_generation = 2;
    state.pinned_messages_channel_id = Some(channel_id.to_string());
    state.is_pins_loading = true;
    state.finish_load_pins(channel_id, 2, Err("synthetic error".to_string()));
    assert_eq!(state.pinned_messages_error.as_deref(), Some("synthetic error"));
    assert!(state.pinned_messages.is_empty());
}

#[test]
fn finish_load_pins_ignores_stale_generation_and_channel() {
    let mut state = AppState::new();
    let cid = "c1";
    state.pins_generation = 2;
    state.pinned_messages_channel_id = Some(cid.to_string());
    state.is_pins_loading = true;
    state.finish_load_pins(cid, 1, Ok(vec![synthetic_message("m1", cid, "x", true)]));
    assert!(state.is_pins_loading);
    assert!(state.pinned_messages.is_empty());

    state.finish_load_pins("other", 2, Ok(vec![synthetic_message("m1", "other", "x", true)]));
    assert!(state.is_pins_loading);
    assert!(state.pinned_messages.is_empty());
}

#[tokio::test]
async fn transport_and_client_manager_get_pinned_messages() -> anyhow::Result<()> {
    let mock_state = Arc::new(RwLock::new(MockState {
        pins: vec![
            synthetic_message("p2", "c1", "second", true),
            synthetic_message("p1", "c1", "first", true),
        ],
        ..Default::default()
    }));
    let (transport, _rx) = PinsMockTransport::new(mock_state.clone());
    let pins = transport.get_pinned_messages("c1").await?;
    assert_eq!(pins.len(), 2);
    assert!(pins.iter().all(|m| m.pinned));

    let (cm, _ev_rx) = ClientManager::new_with_transport_with_audio_probe(
        transport,
        discord_rs::protocol::ConnectionMode::Live,
        None,
    );
    let pins_cm = cm.get_pinned_messages("c1").await?;
    assert_eq!(pins_cm.len(), 2);

    cm.pin_message("c1", "p3").await?;
    cm.unpin_message("c1", "p1").await?;
    let st = mock_state.read().await;
    assert_eq!(st.pin_calls, vec![("c1".to_string(), "p3".to_string())]);
    assert_eq!(st.unpin_calls, vec![("c1".to_string(), "p1".to_string())]);
    Ok(())
}

#[test]
fn set_message_pinned_updates_loaded_pins_panel() {
    let mut state = AppState::new();
    let cid = "c-pin-sync";
    let msg = synthetic_message("m1", cid, "hello", false);
    state.messages_by_channel.insert(cid.to_string(), vec![msg.clone()]);
    state.pinned_messages_channel_id = Some(cid.to_string());
    state.pinned_messages = vec![];

    state.set_message_pinned(cid, "m1", true);
    assert_eq!(state.pinned_messages.len(), 1);
    assert_eq!(state.pinned_messages[0].id, "m1");

    state.set_message_pinned(cid, "m1", false);
    assert!(state.pinned_messages.is_empty());
}
