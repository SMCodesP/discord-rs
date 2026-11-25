use discord_rs::app::state::AppState;
use discord_rs::domain::channel::{Channel, ChannelType};
use discord_rs::domain::permissions::{OverwriteType, PermissionOverwrite};
use discord_rs::domain::Guild;
use discord_rs::protocol::discord_api::{parse_channel_from_json, parse_channel_from_json_str};
use discord_rs::protocol::TransportEvent;
use serde_json::json;

fn category(id: &str, name: &str, position: i32) -> Channel {
    Channel::category(id, name).with_position(position)
}

fn text(id: &str, name: &str, position: i32, parent: &str) -> Channel {
    Channel::text(id, name)
        .with_position(position)
        .with_category_id(parent)
}

fn synthetic_guild() -> Guild {
    Guild::new("g1", "Synthetic Guild", "owner-1")
}

fn seed_guild(state: &mut AppState, channels: Vec<Channel>) {
    state.handle_transport_event(TransportEvent::GuildCreate(
        synthetic_guild().with_channels(channels),
    ));
}

#[test]
fn modern_channel_types_are_not_discarded() {
    for (code, expected) in [
        (10, ChannelType::AnnouncementThread),
        (11, ChannelType::PublicThread),
        (12, ChannelType::PrivateThread),
        (13, ChannelType::Stage),
        (14, ChannelType::Directory),
        (15, ChannelType::Forum),
        (16, ChannelType::Media),
    ] {
        assert_eq!(ChannelType::from_discord_code(code), Some(expected));
        assert_eq!(expected.discord_code(), code);

        let parsed = parse_channel_from_json(
            &json!({
                "id": format!("chan-{code}"),
                "name": format!("channel-{code}"),
                "type": code
            }),
            Some("g1"),
        )
        .expect("modern channel type must parse");
        assert_eq!(parsed.channel_type, expected);
    }
}

#[test]
fn channel_type_string_aliases_preserve_modern_types() {
    for (value, expected) in [
        ("public_thread", ChannelType::PublicThread),
        ("private_thread", ChannelType::PrivateThread),
        ("announcement_thread", ChannelType::AnnouncementThread),
        ("stage_voice", ChannelType::Stage),
        ("forum", ChannelType::Forum),
        ("directory", ChannelType::Directory),
    ] {
        let parsed = parse_channel_from_json_str(&json!({
            "id": "chan-alias",
            "name": "alias",
            "type": value
        })
        .to_string())
        .expect("string channel type must parse");
        assert_eq!(parsed.channel_type, expected);
    }
}

#[test]
fn serde_channel_uses_parent_id_and_round_trips_it() {
    let value = json!({
        "id": "chan-parent",
        "guild_id": "g1",
        "name": "child",
        "type": 0,
        "parent_id": "cat-100"
    });

    let channel: Channel = serde_json::from_value(value).expect("channel must deserialize");
    assert_eq!(channel.category_id.as_deref(), Some("cat-100"));
    assert_eq!(channel.parent_id(), Some("cat-100"));

    let serialized = serde_json::to_value(&channel).expect("channel must serialize");
    assert_eq!(serialized.get("parent_id").and_then(|v| v.as_str()), Some("cat-100"));

    // Older cached payloads used `category_id`; they must keep loading.
    let legacy: Channel = serde_json::from_value(json!({
        "id": "chan-legacy",
        "name": "legacy",
        "type": 0,
        "category_id": "cat-200"
    }))
    .expect("legacy category_id must deserialize");
    assert_eq!(legacy.category_id.as_deref(), Some("cat-200"));
}

#[test]
fn permission_overwrites_are_parsed_and_serialized() {
    let parsed = parse_channel_from_json_str(
        r#"{
            "id": "chan-perms",
            "guild_id": "g1",
            "name": "restricted",
            "type": 0,
            "parent_id": "cat-1",
            "permission_overwrites": [
                { "id": "role-1", "type": 0, "allow": "1024", "deny": "2048" },
                { "id": "user-1", "type": 1, "allow": 0, "deny": 1024 }
            ]
        }"#,
    )
    .expect("channel with overwrites must parse");

    assert_eq!(parsed.permission_overwrites.len(), 2);
    assert_eq!(
        parsed.permission_overwrites[0],
        PermissionOverwrite::new("role-1", OverwriteType::Role, 1024, 2048)
    );
    assert_eq!(
        parsed.permission_overwrites[1],
        PermissionOverwrite::new("user-1", OverwriteType::Member, 0, 1024)
    );

    let json = serde_json::to_value(&parsed).expect("channel must serialize");
    let overwrites = json
        .get("permission_overwrites")
        .and_then(|value| value.as_array())
        .expect("overwrites must survive serialization");
    assert_eq!(overwrites.len(), 2);
    assert_eq!(overwrites[0]["id"], "role-1");
    assert_eq!(overwrites[0]["type"], 0);
    assert_eq!(overwrites[0]["allow"], "1024");
    assert_eq!(overwrites[0]["deny"], "2048");

    let round_tripped: Channel =
        serde_json::from_value(json).expect("serialized channel must deserialize");
    assert_eq!(round_tripped.permission_overwrites, parsed.permission_overwrites);
}

#[test]
fn thread_metadata_is_preserved() {
    let parsed = parse_channel_from_json_str(
        r#"{
            "id": "thread-1",
            "guild_id": "g1",
            "parent_id": "chan-1",
            "name": "thread topic",
            "type": 11,
            "thread_metadata": {
                "archived": false,
                "auto_archive_duration": 1440,
                "locked": true,
                "create_timestamp": "2026-11-24T12:00:00+00:00"
            }
        }"#,
    )
    .expect("thread must parse");

    assert!(parsed.is_thread());
    assert_eq!(parsed.category_id.as_deref(), Some("chan-1"));
    let metadata = parsed.thread_metadata.expect("thread metadata must parse");
    assert!(!metadata.archived);
    assert_eq!(metadata.auto_archive_duration, 1440);
    assert!(metadata.locked);
    assert_eq!(
        metadata.create_timestamp.as_deref(),
        Some("2026-11-24T12:00:00+00:00")
    );
}

#[test]
fn categories_and_channels_are_deterministic_on_position_ties() {
    let mut state = AppState::default();
    seed_guild(
        &mut state,
        vec![
            category("cat-b", "B", 10),
            category("cat-a", "A", 10),
            text("900", "z", 1, "cat-a"),
            text("100", "a", 1, "cat-a"),
        ],
    );

    let guild = &state.guilds[0];
    let category_ids: Vec<&str> = guild
        .categories
        .iter()
        .map(|category| category.id.as_str())
        .collect();
    assert_eq!(category_ids, vec!["cat-a", "cat-b"]);

    let channel_names: Vec<&str> = guild.categories[0]
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    assert_eq!(channel_names, vec!["a", "z"]);
}

#[test]
fn channel_create_update_delete_reconcile_sidebar() {
    let mut state = AppState::default();
    seed_guild(
        &mut state,
        vec![category("cat-1", "Text", 0), text("chan-1", "general", 0, "cat-1")],
    );

    let created = text("chan-2", "new-channel", 1, "cat-1").with_guild_id("g1");
    state.handle_transport_event(TransportEvent::ChannelCreate(created));
    assert!(state.find_channel("chan-2").is_some());

    let mut updated = text("chan-2", "renamed", 1, "cat-1").with_guild_id("g1");
    updated.topic = Some("new topic".to_string());
    state.handle_transport_event(TransportEvent::ChannelUpdate(updated));
    let (_, channel) = state.find_channel("chan-2").expect("updated channel");
    assert_eq!(channel.name, "renamed");
    assert_eq!(channel.topic.as_deref(), Some("new topic"));

    state.handle_transport_event(TransportEvent::ChannelDelete {
        channel_id: "chan-2".to_string(),
        guild_id: Some("g1".to_string()),
    });
    assert!(state.find_channel("chan-2").is_none());
}

#[test]
fn thread_lifecycle_is_reconciled_below_parent_channel() {
    let mut state = AppState::default();
    seed_guild(
        &mut state,
        vec![category("cat-1", "Text", 0), text("chan-1", "general", 0, "cat-1")],
    );

    let thread = Channel::new("thread-1", "first topic", ChannelType::PublicThread)
        .with_guild_id("g1")
        .with_category_id("chan-1");
    state.handle_transport_event(TransportEvent::ThreadCreate(thread));
    let (_, created) = state.find_channel("thread-1").expect("created thread");
    assert_eq!(created.name, "first topic");

    let mut updated = Channel::new("thread-1", "edited topic", ChannelType::PublicThread)
        .with_guild_id("g1")
        .with_category_id("chan-1");
    updated.thread_metadata = Some(discord_rs::domain::channel::ThreadMetadata {
        archived: true,
        auto_archive_duration: 60,
        ..Default::default()
    });
    state.handle_transport_event(TransportEvent::ThreadUpdate(updated));
    let (_, updated) = state.find_channel("thread-1").expect("updated thread");
    assert_eq!(updated.name, "edited topic");

    let synced = Channel::new("thread-2", "synced topic", ChannelType::PublicThread)
        .with_guild_id("g1")
        .with_category_id("chan-1");
    state.handle_transport_event(TransportEvent::ThreadListSync {
        guild_id: "g1".to_string(),
        threads: vec![synced],
    });
    assert!(state.find_channel("thread-2").is_some());

    state.handle_transport_event(TransportEvent::ThreadDelete {
        thread_id: "thread-1".to_string(),
        guild_id: Some("g1".to_string()),
        parent_id: Some("chan-1".to_string()),
    });
    assert!(state.find_channel("thread-1").is_none());
    assert!(state.find_channel("thread-2").is_some());
}

#[test]
fn dm_ties_are_ordered_deterministically() {
    let mut state = AppState::default();
    let older = Channel::new("100", "Older", ChannelType::Dm);
    let newer = Channel::new("200", "Newer", ChannelType::Dm);

    state.handle_transport_event(TransportEvent::DmChannelsUpdated(vec![older, newer]));

    let ids: Vec<&str> = state
        .dm_channels
        .iter()
        .map(|channel| channel.id.as_str())
        .collect();
    assert_eq!(ids, vec!["200", "100"]);
}
