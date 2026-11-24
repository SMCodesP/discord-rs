//! Fast typed deserialization for Discord Gateway envelopes.
//!
//! Uses `serde_json::value::RawValue` to avoid building intermediate
//! `serde_json::Value` trees when dispatching gateway payloads.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

/// Main Discord Gateway envelope (v10) with deferred `d` payload.
///
/// `d` is kept as raw JSON and deserialized directly into the target type,
/// avoiding the `serde_json::Value` intermediate allocation/tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayEnvelope<'a> {
    pub op: u8,
    #[serde(borrow, default)]
    pub d: Option<&'a RawValue>,
    #[serde(default)]
    pub s: Option<i64>,
    #[serde(borrow, default)]
    pub t: Option<&'a str>,
}

/// Voice Gateway envelope (v8) with deferred `d` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceGatewayEnvelope<'a> {
    pub op: u8,
    #[serde(borrow, default)]
    pub d: Option<&'a RawValue>,
    #[serde(default, alias = "s")]
    pub seq: Option<i64>,
}

impl<'a> GatewayEnvelope<'a> {
    /// Parses a gateway envelope from JSON text without materializing `d` as `Value`.
    pub fn parse(text: &'a str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Deserializes the `d` payload directly into `T`.
    pub fn deserialize_d<T: serde::de::DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        match self.d {
            Some(raw) => Ok(Some(serde_json::from_str(raw.get())?)),
            None => Ok(None),
        }
    }

    /// Returns the dispatch event name, if present.
    pub fn event_name(&self) -> Option<&str> {
        self.t
    }

    /// Returns the raw `d` JSON string, if present.
    pub fn d_raw(&self) -> Option<&str> {
        self.d.map(RawValue::get)
    }
}

impl<'a> VoiceGatewayEnvelope<'a> {
    /// Parses a voice gateway envelope from JSON text without materializing `d` as `Value`.
    pub fn parse(text: &'a str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Deserializes the `d` payload directly into `T`.
    pub fn deserialize_d<T: serde::de::DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        match self.d {
            Some(raw) => Ok(Some(serde_json::from_str(raw.get())?)),
            None => Ok(None),
        }
    }

    /// Returns the raw `d` JSON string, if present.
    pub fn d_raw(&self) -> Option<&str> {
        self.d.map(RawValue::get)
    }
}

/// Parses a main gateway envelope and returns (op, seq, event_name, raw_d).
///
/// This is a convenience for hot paths that need envelope metadata before
/// deciding how to deserialize `d`.
pub fn parse_gateway_envelope(text: &str) -> Result<(u8, Option<i64>, Option<&str>, Option<&RawValue>), serde_json::Error> {
    let env: GatewayEnvelope<'_> = serde_json::from_str(text)?;
    Ok((env.op, env.s, env.t, env.d))
}

/// Parses a voice gateway envelope and returns (op, seq, raw_d).
pub fn parse_voice_gateway_envelope(text: &str) -> Result<(u8, Option<i64>, Option<&RawValue>), serde_json::Error> {
    let env: VoiceGatewayEnvelope<'_> = serde_json::from_str(text)?;
    Ok((env.op, env.seq, env.d))
}

/// Deserializes a raw JSON payload directly into `T`.
pub fn deserialize_raw<T: serde::de::DeserializeOwned>(raw: &RawValue) -> Result<T, serde_json::Error> {
    serde_json::from_str(raw.get())
}
