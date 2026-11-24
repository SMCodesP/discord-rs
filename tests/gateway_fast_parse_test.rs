use discord_rs::protocol::{
    deserialize_raw, parse_gateway_envelope, parse_voice_gateway_envelope, GatewayEnvelope,
    VoiceGatewayEnvelope,
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HelloData {
    heartbeat_interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReadyData {
    ssrc: u32,
    ip: String,
    port: u16,
    modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MessageCreate {
    id: String,
    channel_id: String,
    content: String,
}

#[test]
fn gateway_envelope_parses_dispatch_without_value_intermediate() {
    let text = r#"{"op":0,"s":42,"t":"MESSAGE_CREATE","d":{"id":"m1","channel_id":"c1","content":"hello"}}"#;
    let env = GatewayEnvelope::parse(text).expect("parse envelope");

    assert_eq!(env.op, 0);
    assert_eq!(env.s, Some(42));
    assert_eq!(env.event_name(), Some("MESSAGE_CREATE"));

    let raw = env.d.expect("d present");
    // Ensure we can inspect raw JSON directly (no Value tree required by our API).
    assert!(raw.get().contains("\"content\":\"hello\""));

    let msg: MessageCreate = env.deserialize_d().expect("deserialize d").expect("d some");
    assert_eq!(
        msg,
        MessageCreate {
            id: "m1".into(),
            channel_id: "c1".into(),
            content: "hello".into(),
        }
    );
}

#[test]
fn gateway_envelope_handles_null_and_missing_d() {
    // serde deserializes JSON null into Option::None for Option<&RawValue>,
    // so both null and missing d are treated as no payload.
    let null_d = r#"{"op":1,"d":null}"#;
    let env = GatewayEnvelope::parse(null_d).expect("parse");
    assert!(env.d.is_none());
    let parsed: Option<serde_json::Value> = env.deserialize_d().expect("deserialize");
    assert_eq!(parsed, None);

    let missing_d = r#"{"op":11}"#;
    let env = GatewayEnvelope::parse(missing_d).expect("parse");
    assert!(env.d.is_none());
    let parsed: Option<serde_json::Value> = env.deserialize_d().expect("deserialize");
    assert_eq!(parsed, None);
}

#[test]
fn gateway_envelope_borrows_event_name() {
    let text = r#"{"op":0,"t":"READY","d":{"v":10}}"#;
    let env = GatewayEnvelope::parse(text).expect("parse");
    let t = env.event_name().expect("t");
    // Borrowed from input text buffer.
    let text_ptr = text.as_ptr() as usize;
    let text_end = text_ptr + text.len();
    let t_ptr = t.as_ptr() as usize;
    assert!(t_ptr >= text_ptr && t_ptr < text_end);
}

#[test]
fn voice_gateway_envelope_parses_seq_alias_and_d() {
    let text = r#"{"op":2,"seq":7,"d":{"ssrc":1,"ip":"127.0.0.1","port":1234,"modes":["aead_aes256_gcm_rtpsize"]}}"#;
    let env = VoiceGatewayEnvelope::parse(text).expect("parse");

    assert_eq!(env.op, 2);
    assert_eq!(env.seq, Some(7));

    let ready: ReadyData = env.deserialize_d().expect("deserialize").expect("d");
    assert_eq!(ready.ssrc, 1);
    assert_eq!(ready.ip, "127.0.0.1");
    assert_eq!(ready.port, 1234);
    assert_eq!(ready.modes, vec!["aead_aes256_gcm_rtpsize"]);
}

#[test]
fn parse_helpers_return_raw_d_without_materializing_value() {
    let gateway_text = r#"{"op":10,"s":null,"t":null,"d":{"heartbeat_interval":41250}}"#;
    let (op, s, t, d) = parse_gateway_envelope(gateway_text).expect("parse");
    assert_eq!(op, 10);
    assert_eq!(s, None);
    assert_eq!(t, None);
    let raw = d.expect("d");
    let hello: HelloData = deserialize_raw(raw).expect("deserialize raw");
    assert_eq!(hello.heartbeat_interval, 41250);

    let voice_text = r#"{"op":8,"d":{"v":8,"heartbeat_interval":41250.0}}"#;
    let (op, seq, d) = parse_voice_gateway_envelope(voice_text).expect("parse");
    assert_eq!(op, 8);
    assert_eq!(seq, None);
    let raw = d.expect("d");
    // Deserialize only the needed field directly from raw JSON.
    #[derive(Deserialize)]
    struct VoiceHello {
        heartbeat_interval: f64,
    }
    let hello: VoiceHello = deserialize_raw(raw).expect("deserialize");
    assert_eq!(hello.heartbeat_interval, 41250.0);
}

#[test]
fn raw_value_can_be_used_for_deferred_typed_deserialization() {
    let text = r#"{"op":0,"t":"X","d":{"a":1,"b":"two"}}"#;
    let env = GatewayEnvelope::parse(text).expect("parse");
    let raw: &RawValue = env.d.expect("d");

    #[derive(Debug, Deserialize, PartialEq)]
    struct Partial {
        b: String,
    }
    // Typed direct parse from RawValue; no serde_json::Value constructed by this API.
    let partial: Partial = serde_json::from_str(raw.get()).expect("deserialize partial");
    assert_eq!(partial, Partial { b: "two".into() });
}

#[test]
fn gateway_envelope_round_trip_preserves_raw_payload() {
    #[derive(Serialize)]
    struct Out<'a> {
        op: u8,
        #[serde(borrow)]
        d: &'a RawValue,
        s: i64,
        t: &'a str,
    }

    let raw = RawValue::from_string(r#"{"k":"v","n":1}"#.to_string()).expect("raw");
    let out = Out {
        op: 0,
        d: &raw,
        s: 7,
        t: "EVT",
    };
    let json = serde_json::to_string(&out).expect("serialize");
    let env = GatewayEnvelope::parse(&json).expect("parse");
    assert_eq!(env.op, 0);
    assert_eq!(env.s, Some(7));
    assert_eq!(env.t, Some("EVT"));
    assert_eq!(env.d_raw(), Some(r#"{"k":"v","n":1}"#));
}
