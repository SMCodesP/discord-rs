use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::client_manager::ClientManager;
use discord_rs::protocol::transport::TransportEvent;
use discord_rs::app::state::AppState;

#[test]
fn test_channel_recipients_serde() {
    let json_data = serde_json::json!({
        "id": "112233445566778899",
        "name": "Ferris",
        "type": 1,
        "recipient_ids": ["998877665544332211"],
        "recipient_names": ["Ferris"]
    });

    let ch: Channel = serde_json::from_value(json_data).expect("Failed to deserialize channel");
    assert_eq!(ch.id, "112233445566778899");
    assert_eq!(ch.channel_type, ChannelType::Dm);
    assert_eq!(ch.recipient_ids, vec!["998877665544332211"]);
    assert_eq!(ch.recipient_names, vec!["Ferris"]);

    // Test backward compatibility when recipient fields are missing
    let json_legacy = serde_json::json!({
        "id": "223344556677889900",
        "name": "general",
        "type": 0
    });
    let ch_legacy: Channel = serde_json::from_value(json_legacy).expect("Failed legacy deserialize");
    assert_eq!(ch_legacy.recipient_ids, Vec::<String>::new());
    assert_eq!(ch_legacy.recipient_names, Vec::<String>::new());
}

#[tokio::test]
async fn test_client_manager_create_and_send_dm() -> anyhow::Result<()> {
    let (cm, mut rx) = ClientManager::new_mock();

    // 1. Create or get DM channel with user-ferris
    let ch = cm.create_or_get_dm_channel("user-ferris").await?;
    assert_eq!(ch.recipient_ids, vec!["user-ferris"]);
    assert_eq!(ch.name, "Ferris");

    // 2. Send message using synthetic dm-user-ferris
    let msg = cm.send_message("dm-user-ferris", "Olá no PV!").await?;
    assert_eq!(msg.content, "Olá no PV!");
    assert_eq!(msg.channel_id, "dm-user-ferris");

    // 3. Verify that DmChannelResolved and MessageCreate events were emitted
    let mut saw_resolved = false;
    let mut saw_msg = false;

    while let Ok(ev) = rx.try_recv() {
        match ev {
            TransportEvent::DmChannelResolved { temp_id, channel } => {
                assert_eq!(temp_id, "dm-user-ferris");
                assert_eq!(channel.recipient_ids, vec!["user-ferris"]);
                saw_resolved = true;
            }
            TransportEvent::MessageCreate(m) => {
                if m.content == "Olá no PV!" {
                    saw_msg = true;
                }
            }
            _ => {}
        }
    }

    assert!(saw_resolved, "Expected DmChannelResolved event to be emitted");
    assert!(saw_msg, "Expected MessageCreate event to be emitted");

    Ok(())
}

#[test]
fn test_app_state_dm_flow_and_migration() {
    let mut state = AppState::new();

    // 1. Open DM with new user
    let temp_cid = state.open_dm_with_user("user-alice", "Alice");
    assert_eq!(temp_cid, "dm-user-alice");
    assert_eq!(state.selected_channel_id.as_deref(), Some("dm-user-alice"));
    assert!(state.dm_channels.iter().any(|d| d.id == "dm-user-alice"));

    // 2. Send an optimistic message in this temporary DM
    state.send_message("dm-user-alice", "E aí Alice, tudo bem?");
    let temp_msgs = state.messages_by_channel.get("dm-user-alice").expect("Messages should exist");
    assert_eq!(temp_msgs.len(), 1);
    assert_eq!(temp_msgs[0].content, "E aí Alice, tudo bem?");
    assert!(temp_msgs[0].is_pending);

    // 3. Simulate Discord resolving the DM channel to a real Snowflake ID
    let real_dm_channel = Channel::new("123456789012345678", "Alice", ChannelType::Dm)
        .with_recipient_ids(vec!["user-alice".to_string()]);

    state.handle_transport_event(TransportEvent::DmChannelResolved {
        temp_id: "dm-user-alice".to_string(),
        channel: real_dm_channel.clone(),
    });

    // Verify channel ID was migrated in selected_channel_id
    assert_eq!(state.selected_channel_id.as_deref(), Some("123456789012345678"));

    // Verify dm_channels was updated
    let dm_item = state.dm_channels.iter().find(|d| d.id == "123456789012345678");
    assert!(dm_item.is_some(), "Real DM channel must exist in dm_channels");
    assert_eq!(dm_item.unwrap().recipient_ids, vec!["user-alice"]);

    // Verify pending messages were migrated from temp_id to real channel ID
    assert_eq!(state.messages_by_channel.get("dm-user-alice"), None);
    let real_msgs = state.messages_by_channel.get("123456789012345678").expect("Migrated messages must exist");
    assert_eq!(real_msgs.len(), 1);
    assert_eq!(real_msgs[0].content, "E aí Alice, tudo bem?");
    assert_eq!(real_msgs[0].channel_id, "123456789012345678");

    // 4. Simulate Discord Gateway confirming the message via MessageCreate
    let mut confirmed_msg = Message::new(
        "msg-999888777",
        "123456789012345678",
        Author {
            id: state.current_user.id.clone(),
            username: state.current_user.username.clone(),
            global_name: Some(state.current_user.display_name.clone()),
            avatar: None,
            bot: false,
        },
        "E aí Alice, tudo bem?",
    );
    confirmed_msg.is_pending = false;

    state.handle_transport_event(TransportEvent::MessageCreate(confirmed_msg));

    let final_msgs = state.messages_by_channel.get("123456789012345678").unwrap();
    assert_eq!(final_msgs.len(), 1);
    assert_eq!(final_msgs[0].id, "msg-999888777");
    assert!(!final_msgs[0].is_pending, "Message should no longer be pending");

    // 5. Calling open_dm_with_user again should reuse the existing resolved channel
    let reopened = state.open_dm_with_user("user-alice", "Alice");
    assert_eq!(reopened, "123456789012345678");
}
