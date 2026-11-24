use discord_rs::domain::snowflake::{Snowflake, DISCORD_EPOCH_MS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[test]
fn snowflake_is_copy_and_preserves_raw_value() {
    let a = Snowflake(175928847299117063);
    let b = a; // Copy
    assert_eq!(a, b);
    assert_eq!(a.as_u64(), 175928847299117063);
    assert_eq!(u64::from(a), 175928847299117063);
    assert_eq!(Snowflake::new(42).as_u64(), 42);
}

#[test]
fn snowflake_timestamp_uses_discord_epoch() {
    // Discord epoch itself
    let epoch = Snowflake(0);
    assert_eq!(DISCORD_EPOCH_MS, 1_420_070_400_000);
    assert_eq!(epoch.timestamp_millis(), DISCORD_EPOCH_MS);
    let dt = epoch.created_at();
    assert_eq!(dt.to_rfc3339(), "2015-01-01T00:00:00+00:00");

    // Known snowflake from Discord docs/examples: 175928847299117063
    // (175928847299117063 >> 22) + 1420070400000 = 1462015105796 ms
    let known = Snowflake(175928847299117063);
    let expected_ms = (175928847299117063u64 >> 22) + DISCORD_EPOCH_MS;
    assert_eq!(known.timestamp_millis(), expected_ms);
    assert_eq!(known.created_at().timestamp_millis(), expected_ms as i64);
}

#[test]
fn snowflake_chronological_comparison_is_ordered_by_timestamp() {
    let older = Snowflake(100);
    let newer = Snowflake(200);
    let same_as_older = Snowflake(100);

    assert!(older < newer);
    assert!(newer > older);
    assert!(older <= same_as_older);
    assert_eq!(older, same_as_older);

    // Snowflakes created later in time have larger numeric values in the high bits,
    // so direct u64 ordering gives chronological ordering.
    let t1 = Snowflake(1_759_288_472_991_170_63u64);
    let t2 = Snowflake(1_759_288_472_991_170_64u64);
    assert!(t1 < t2);

    let mut set = BTreeSet::new();
    set.insert(newer);
    set.insert(older);
    set.insert(same_as_older);
    let ordered: Vec<u64> = set.into_iter().map(|s| s.as_u64()).collect();
    assert_eq!(ordered, vec![100, 200]);
}

#[test]
fn snowflake_serde_json_string_roundtrip() {
    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Payload {
        id: Snowflake,
        channel_id: Snowflake,
    }

    let payload = Payload {
        id: Snowflake(175928847299117063),
        channel_id: Snowflake(86913608335773696),
    };

    let json = serde_json::to_string(&payload).expect("serialize payload");
    assert_eq!(
        json,
        r#"{"id":"175928847299117063","channel_id":"86913608335773696"}"#
    );

    let decoded: Payload = serde_json::from_str(&json).expect("deserialize payload");
    assert_eq!(decoded, payload);
}

#[test]
fn snowflake_deserializes_from_json_string() {
    let s: Snowflake = serde_json::from_str(r#""175928847299117063""#).expect("deserialize string");
    assert_eq!(s, Snowflake(175928847299117063));
}

#[test]
fn snowflake_deserializes_from_json_number() {
    let s: Snowflake = serde_json::from_str("12345").expect("deserialize number");
    assert_eq!(s, Snowflake(12345));
}

#[test]
fn snowflake_display_and_parse() {
    let s = Snowflake(175928847299117063);
    assert_eq!(s.to_string(), "175928847299117063");
    let parsed: Snowflake = "175928847299117063".parse().expect("parse snowflake");
    assert_eq!(parsed, s);
}
