use discord_rs::domain::events::{
    GuildScheduledEvent, GuildScheduledEventEntityMetadata, GuildScheduledEventEntityType,
    GuildScheduledEventPrivacyLevel, GuildScheduledEventRecurrenceRule,
    GuildScheduledEventRecurrenceRuleFrequency, GuildScheduledEventRecurrenceRuleMonth,
    GuildScheduledEventRecurrenceRuleNWeekday, GuildScheduledEventRecurrenceRuleWeekday,
    GuildScheduledEventStatus, StageInstance, StageInstancePrivacyLevel,
};

#[test]
fn stage_instance_parses_official_shape() {
    // Shape based on Discord Stage Instance object docs:
    // https://discord.com/developers/docs/resources/stage-instance#stage-instance-object
    let json = r#"{
        "id": "840647391636226060",
        "guild_id": "197038439483310086",
        "channel_id": "733488538393510049",
        "topic": "Testing Testing, 123",
        "privacy_level": 2,
        "discoverable_disabled": false,
        "guild_scheduled_event_id": "947656305244532806"
    }"#;

    let stage: StageInstance = serde_json::from_str(json).expect("stage instance should parse");
    assert_eq!(stage.id, "840647391636226060");
    assert_eq!(stage.guild_id, "197038439483310086");
    assert_eq!(stage.channel_id, "733488538393510049");
    assert_eq!(stage.topic, "Testing Testing, 123");
    assert_eq!(stage.privacy_level, StageInstancePrivacyLevel::GuildOnly);
    assert!(!stage.discoverable_disabled);
    assert_eq!(
        stage.guild_scheduled_event_id.as_deref(),
        Some("947656305244532806")
    );

    let round_trip = serde_json::to_value(&stage).expect("stage instance should serialize");
    assert_eq!(round_trip["privacy_level"], 2);
    assert_eq!(round_trip["guild_scheduled_event_id"], "947656305244532806");
}

#[test]
fn stage_instance_defaults_optional_fields() {
    let json = r#"{
        "id": "1",
        "guild_id": "2",
        "channel_id": "3",
        "topic": "Town Hall",
        "privacy_level": 1
    }"#;

    let stage: StageInstance = serde_json::from_str(json).expect("minimal stage instance should parse");
    assert_eq!(stage.privacy_level, StageInstancePrivacyLevel::Public);
    assert!(!stage.discoverable_disabled);
    assert_eq!(stage.guild_scheduled_event_id, None);
}

#[test]
fn stage_instance_rejects_unknown_privacy_level() {
    let json = r#"{
        "id": "1",
        "guild_id": "2",
        "channel_id": "3",
        "topic": "Bad level",
        "privacy_level": 99
    }"#;

    let err = serde_json::from_str::<StageInstance>(json).expect_err("unknown privacy level should fail");
    assert!(err.to_string().contains("Unknown StageInstancePrivacyLevel"));
}

#[test]
fn guild_scheduled_event_parses_official_shape() {
    // Shape based on Discord Guild Scheduled Event object docs:
    // https://discord.com/developers/docs/resources/guild-scheduled-event#guild-scheduled-event-object
    let json = r#"{
        "id": "1001",
        "guild_id": "2002",
        "channel_id": "3003",
        "creator_id": "4004",
        "name": "Community Meetup",
        "description": "Monthly sync",
        "scheduled_start_time": "2026-11-25T20:00:00+00:00",
        "scheduled_end_time": "2026-11-25T21:00:00+00:00",
        "privacy_level": 2,
        "status": 1,
        "entity_type": 2,
        "entity_id": "3003",
        "entity_metadata": null,
        "user_count": 42,
        "image": "abc123hash",
        "recurrence_rule": null
    }"#;

    let event: GuildScheduledEvent =
        serde_json::from_str(json).expect("guild scheduled event should parse");
    assert_eq!(event.id, "1001");
    assert_eq!(event.guild_id, "2002");
    assert_eq!(event.channel_id.as_deref(), Some("3003"));
    assert_eq!(event.creator_id.as_deref(), Some("4004"));
    assert_eq!(event.name, "Community Meetup");
    assert_eq!(event.description.as_deref(), Some("Monthly sync"));
    assert_eq!(event.scheduled_start_time, "2026-11-25T20:00:00+00:00");
    assert_eq!(
        event.scheduled_end_time.as_deref(),
        Some("2026-11-25T21:00:00+00:00")
    );
    assert_eq!(
        event.privacy_level,
        GuildScheduledEventPrivacyLevel::GuildOnly
    );
    assert_eq!(event.status, GuildScheduledEventStatus::Scheduled);
    assert_eq!(event.entity_type, GuildScheduledEventEntityType::Voice);
    assert_eq!(event.entity_id.as_deref(), Some("3003"));
    assert_eq!(event.entity_metadata, None);
    assert_eq!(event.user_count, Some(42));
    assert_eq!(event.image.as_deref(), Some("abc123hash"));
    assert_eq!(event.recurrence_rule, None);
}

#[test]
fn guild_scheduled_event_parses_external_metadata_and_recurrence() {
    // Entity metadata/location for EXTERNAL events and recurrence rule fields from Discord docs.
    let json = r#"{
        "id": "5005",
        "guild_id": "6006",
        "channel_id": null,
        "name": "External Launch",
        "description": null,
        "scheduled_start_time": "2026-12-01T18:00:00+00:00",
        "scheduled_end_time": "2026-12-01T19:00:00+00:00",
        "privacy_level": 2,
        "status": 2,
        "entity_type": 3,
        "entity_id": null,
        "entity_metadata": { "location": "https://example.invalid/stage" },
        "recurrence_rule": {
            "start": "2026-12-01T18:00:00+00:00",
            "end": "2027-01-01T18:00:00+00:00",
            "frequency": 2,
            "interval": 1,
            "by_weekday": [1, 3],
            "by_n_weekday": [{ "n": 1, "day": 1 }],
            "by_month": [12],
            "by_month_day": [1, 15],
            "by_year_day": [1],
            "count": 4
        }
    }"#;

    let event: GuildScheduledEvent =
        serde_json::from_str(json).expect("external scheduled event should parse");
    assert_eq!(event.channel_id, None);
    assert_eq!(event.status, GuildScheduledEventStatus::Active);
    assert_eq!(event.entity_type, GuildScheduledEventEntityType::External);
    assert_eq!(
        event.entity_metadata,
        Some(GuildScheduledEventEntityMetadata {
            location: Some("https://example.invalid/stage".to_string()),
        })
    );

    let rule: &GuildScheduledEventRecurrenceRule =
        event.recurrence_rule.as_ref().expect("recurrence rule should exist");
    assert_eq!(rule.start, "2026-12-01T18:00:00+00:00");
    assert_eq!(rule.end.as_deref(), Some("2027-01-01T18:00:00+00:00"));
    assert_eq!(
        rule.frequency,
        GuildScheduledEventRecurrenceRuleFrequency::Weekly
    );
    assert_eq!(rule.interval, 1);
    assert_eq!(
        rule.by_weekday.as_deref(),
        Some(&[
            GuildScheduledEventRecurrenceRuleWeekday::Tuesday,
            GuildScheduledEventRecurrenceRuleWeekday::Thursday
        ][..])
    );
    assert_eq!(
        rule.by_n_weekday.as_deref(),
        Some(&[GuildScheduledEventRecurrenceRuleNWeekday {
            n: 1,
            day: GuildScheduledEventRecurrenceRuleWeekday::Tuesday,
        }][..])
    );
    assert_eq!(
        rule.by_month.as_deref(),
        Some(&[GuildScheduledEventRecurrenceRuleMonth::December][..])
    );
    assert_eq!(rule.by_month_day.as_deref(), Some(&[1u8, 15u8][..]));
    assert_eq!(rule.by_year_day.as_deref(), Some(&[1u16][..]));
    assert_eq!(rule.count, Some(4));
}

#[test]
fn guild_scheduled_event_rejects_unknown_enums() {
    let bad_status = r#"{
        "id":"1","guild_id":"2","name":"n","scheduled_start_time":"2026-01-01T00:00:00Z",
        "privacy_level":2,"status":99,"entity_type":2
    }"#;
    let err = serde_json::from_str::<GuildScheduledEvent>(bad_status)
        .expect_err("unknown status should fail");
    assert!(err.to_string().contains("Unknown GuildScheduledEventStatus"));

    let bad_entity = r#"{
        "id":"1","guild_id":"2","name":"n","scheduled_start_time":"2026-01-01T00:00:00Z",
        "privacy_level":2,"status":1,"entity_type":99
    }"#;
    let err = serde_json::from_str::<GuildScheduledEvent>(bad_entity)
        .expect_err("unknown entity type should fail");
    assert!(
        err.to_string()
            .contains("Unknown GuildScheduledEventEntityType")
    );
}
