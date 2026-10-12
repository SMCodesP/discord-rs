use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::message::{Author, Message};
use discord_rs::protocol::client_manager::ClientManager;
use discord_rs::protocol::transport::TransportEvent;
use discord_rs::app::state::{AppState, ChannelItemState, ChannelKind};

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
    assert_eq!(msg.channel_id, ch.id);

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
            avatar_decoration_data: None,
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

#[test]
fn test_no_hardcoded_ferris_dm_and_clean_state() {
    let mut state = AppState::new();

    // 1. Initial dm_channels must be completely empty
    assert!(
        state.dm_channels.is_empty(),
        "AppState::new() must start with clean/empty dm_channels, got: {:?}",
        state.dm_channels.iter().map(|d| &d.id).collect::<Vec<_>>()
    );

    // 2. No hardcoded messages for Ferris or fake groups in seed messages
    assert!(
        !state.messages_by_channel.contains_key("dm-ferris"),
        "Seed messages must not contain dm-ferris"
    );
    assert!(
        !state.messages_by_channel.contains_key("dm-group-gpui"),
        "Seed messages must not contain dm-group-gpui"
    );

    // 3. No Ferris friend in initial friends list
    assert!(
        !state.friends.iter().any(|f| f.id == "user-ferris" || f.display_name.to_lowercase().contains("ferris")),
        "Initial friends list must not have Ferris"
    );

    // 4. Test Gateway DmChannelsUpdated does NOT preserve old hardcoded dm-ferris,
    //    but DOES preserve legitimate pending dynamic DMs (e.g. dm-user-bob)
    state.dm_channels.push(ChannelItemState {
        id: "dm-ferris".into(),
        name: "Ferris".into(),
        kind: ChannelKind::Text,
        topic: Some("Mensagens Diretas com Ferris".into()),
        unread: false,
        mention_count: 0,
        icon_url: None,
        is_pinned: true,
        last_message_id: None,
        is_group_dm: false,
        recipient_ids: vec!["user-ferris".into()],
        recipient_names: vec!["Ferris".into()],
        user_limit: None,
        avatar_decoration_data: None,
    });

    // Legitimate temporary DM opened by user
    state.open_dm_with_user("user-bob", "Bob");
    assert!(state.dm_channels.iter().any(|d| d.id == "dm-user-bob"));

    // Real channels dispatched by Gateway
    let real_dm = Channel::new("555444333222111000", "Carlos Real", ChannelType::Dm)
        .with_recipient_ids(vec!["user-carlos-real".to_string()]);

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![real_dm]));

    // dm-ferris must be eliminated
    assert!(
        !state.dm_channels.iter().any(|d| d.id == "dm-ferris"),
        "dm-ferris must not be preserved after Gateway DmChannelsUpdated"
    );
    // Real DM from gateway must exist
    assert!(state.dm_channels.iter().any(|d| d.id == "555444333222111000"));
    // Legitimate pending temp DM must still be preserved
    assert!(state.dm_channels.iter().any(|d| d.id == "dm-user-bob"));
}

#[test]
fn test_guild_messages_do_not_leak_into_dms() {
    let mut state = AppState::seeded_for_tests();

    // 1. Setup a bot DM channel and a user DM channel
    let bot_dm_id = "dm-chan-bot-dyno-999".to_string();
    state.dm_channels.push(ChannelItemState {
        id: bot_dm_id.clone(),
        name: "Dyno Bot".into(),
        kind: ChannelKind::Text,
        topic: Some("Direct Message with Dyno".into()),
        unread: false,
        mention_count: 0,
        icon_url: None,
        is_pinned: false,
        last_message_id: Some("old-dm-msg-1".into()),
        is_group_dm: false,
        recipient_ids: vec!["user-bot-dyno".into()],
        recipient_names: vec!["Dyno".into()],
        user_limit: None,
        avatar_decoration_data: None,
    });

    let friend_dm_id = "dm-chan-alice-888".to_string();
    state.dm_channels.push(ChannelItemState {
        id: friend_dm_id.clone(),
        name: "Alice".into(),
        kind: ChannelKind::Text,
        topic: Some("Direct Message with Alice".into()),
        unread: false,
        mention_count: 0,
        icon_url: None,
        is_pinned: false,
        last_message_id: Some("old-dm-msg-2".into()),
        is_group_dm: false,
        recipient_ids: vec!["user-alice".into()],
        recipient_names: vec!["Alice".into()],
        user_limit: None,
        avatar_decoration_data: None,
    });

    // Also open a pending temp DM with Carol
    state.open_dm_with_user("user-carol", "Carol");
    assert!(state.dm_channels.iter().any(|d| d.id == "dm-user-carol"));

    // Select the bot DM channel
    state.selected_channel_id = Some(bot_dm_id.clone());

    // 2. Simulate Bot sending a message in a GUILD text channel (#general)
    let guild_chan_id = "chan-server-general";
    let bot_guild_msg = Message {
        id: "guild-msg-bot-1".into(),
        channel_id: guild_chan_id.into(),
        guild_id: Some("guild-main-1".into()),
        author: Author {
            id: "user-bot-dyno".into(),
            username: "Dyno".into(),
            global_name: Some("Dyno".into()),
            avatar: None,
            bot: true,
            avatar_decoration_data: None,
        },
        content: "Bot announcement in server #general channel!".into(),
        timestamp: chrono::Utc::now(),
        attachments: Vec::new(),
        reactions: Vec::new(),
        embeds: Vec::new(),
        sticker_items: Vec::new(),
        edited_timestamp: None,
        pinned: false,
        is_pending: false,
    };

    state.handle_transport_event(TransportEvent::MessageCreate(bot_guild_msg));

    // VERIFY CRITICAL ISOLATION:
    // A. Bot DM channel ID must NOT have been mutated to guild_chan_id!
    let bot_dm = state.dm_channels.iter().find(|d| d.recipient_ids.contains(&"user-bot-dyno".to_string()));
    assert!(bot_dm.is_some(), "Bot DM must still exist");
    assert_eq!(bot_dm.unwrap().id, bot_dm_id, "Bot DM ID must remain untouched, NOT replaced by guild channel ID!");
    assert_eq!(bot_dm.unwrap().last_message_id.as_deref(), Some("old-dm-msg-1"), "DM last_message_id must not be modified by guild message");

    // B. selected_channel_id must NOT have been hijacked
    assert_eq!(state.selected_channel_id.as_ref(), Some(&bot_dm_id), "selected_channel_id must NOT be redirected to server channel");

    // C. DM channel must NOT contain the guild message
    assert!(!state.messages_by_channel.entry(bot_dm_id.clone()).or_default().iter().any(|m| m.id == "guild-msg-bot-1"));

    // D. Guild channel must contain the message
    let guild_msgs = state.messages_by_channel.get(guild_chan_id).expect("Guild channel messages should exist");
    assert!(guild_msgs.iter().any(|m| m.id == "guild-msg-bot-1"));

    // 3. Simulate Alice sending a message in a known guild channel where guild_id is None in the event
    // Find or check a known channel in state.guilds
    let known_guild_chan_id = state
        .guilds
        .first()
        .and_then(|g| g.categories.first())
        .and_then(|c| c.channels.first())
        .map(|ch| ch.id.clone())
        .unwrap_or_else(|| "chan-general".to_string());

    let alice_guild_msg = Message {
        id: "guild-msg-alice-1".into(),
        channel_id: known_guild_chan_id.clone(),
        guild_id: None, // Missing guild_id, but channel exists in state.guilds
        author: Author {
            id: "user-alice".into(),
            username: "Alice".into(),
            global_name: Some("Alice".into()),
            avatar: None,
            bot: false,
            avatar_decoration_data: None,
        },
        content: "Hey everyone in the server!".into(),
        timestamp: chrono::Utc::now(),
        attachments: Vec::new(),
        reactions: Vec::new(),
        embeds: Vec::new(),
        sticker_items: Vec::new(),
        edited_timestamp: None,
        pinned: false,
        is_pending: false,
    };

    state.handle_transport_event(TransportEvent::MessageCreate(alice_guild_msg));

    let alice_dm = state.dm_channels.iter().find(|d| d.recipient_ids.contains(&"user-alice".to_string()));
    assert_eq!(alice_dm.unwrap().id, friend_dm_id, "Alice DM ID must remain untouched");
    assert_eq!(alice_dm.unwrap().last_message_id.as_deref(), Some("old-dm-msg-2"));

    // 4. Carol sends a message in a server channel: pending temp DM dm-user-carol must NOT be hijacked
    let carol_guild_msg = Message {
        id: "guild-msg-carol-1".into(),
        channel_id: "chan-server-memes".into(),
        guild_id: Some("guild-main-1".into()),
        author: Author {
            id: "user-carol".into(),
            username: "Carol".into(),
            global_name: Some("Carol".into()),
            avatar: None,
            bot: false,
            avatar_decoration_data: None,
        },
        content: "Check this meme in the server!".into(),
        timestamp: chrono::Utc::now(),
        attachments: Vec::new(),
        reactions: Vec::new(),
        embeds: Vec::new(),
        sticker_items: Vec::new(),
        edited_timestamp: None,
        pinned: false,
        is_pending: false,
    };

    state.handle_transport_event(TransportEvent::MessageCreate(carol_guild_msg));

    assert!(state.dm_channels.iter().any(|d| d.id == "dm-user-carol"), "Temp DM dm-user-carol must NOT be hijacked by server message");

    // 5. Finally, test a LEGITIMATE DM message from Bot:
    let bot_real_dm_msg = Message {
        id: "dm-msg-bot-real".into(),
        channel_id: bot_dm_id.clone(),
        guild_id: None,
        author: Author {
            id: "user-bot-dyno".into(),
            username: "Dyno".into(),
            global_name: Some("Dyno".into()),
            avatar: None,
            bot: true,
            avatar_decoration_data: None,
        },
        content: "Here is your private verification code: 123456".into(),
        timestamp: chrono::Utc::now(),
        attachments: Vec::new(),
        reactions: Vec::new(),
        embeds: Vec::new(),
        sticker_items: Vec::new(),
        edited_timestamp: None,
        pinned: false,
        is_pending: false,
    };

    state.handle_transport_event(TransportEvent::MessageCreate(bot_real_dm_msg));

    let bot_dm_after = state.dm_channels.iter().find(|d| d.id == bot_dm_id).unwrap();
    assert_eq!(bot_dm_after.last_message_id.as_deref(), Some("dm-msg-bot-real"), "Legitimate DM message must update DM last_message_id");
}

#[test]
fn test_dm_welcome_header_presentation() {
    let username = "Usuário B";
    let dm_topic = format!(
        "Este é o início do seu histórico de mensagens diretas com {}.",
        username
    );
    assert_eq!(
        dm_topic,
        "Este é o início do seu histórico de mensagens diretas com Usuário B."
    );

    let dm_element = discord_rs::chat::message_list::render_welcome_banner(
        username,
        &dm_topic,
        true,
        Some("https://cdn.discordapp.com/avatars/user-b/avatar.png".into()),
        Some(discord_rs::domain::user::AvatarDecorationData {
            asset: "a_synth_dm_banner_decor".to_string(),
            sku_id: None,
        }),
    );
    drop(dm_element);

    let dm_element_bare = discord_rs::chat::message_list::render_welcome_banner(
        username,
        &dm_topic,
        true,
        None,
        None,
    );
    drop(dm_element_bare);

    let guild_element = discord_rs::chat::message_list::render_welcome_banner(
        "geral",
        "Este é o início do canal #geral.",
        false,
        None,
        None,
    );
    drop(guild_element);
}
