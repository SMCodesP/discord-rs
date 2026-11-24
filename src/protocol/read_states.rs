//! Discord read-state model, unread/mention tracking, and mark-as-read ACK.
//!
//! Data source / format notes:
//! - READY `read_state` payload shape and fields (`id`, `last_message_id`,
//!   `mention_count`, `flags`, `last_pin_timestamp`) follow Discord user gateway
//!   behavior documented by the community reverse-engineering docs:
//!   https://docs.discord.sex/resources/read-state and observed READY payloads.
//! - Snowflake ordering uses Discord snowflake chronology (higher 42 bits are
//!   timestamp), implemented via `crate::domain::Snowflake`/u64 comparison.
//! - ACK endpoint is `POST /channels/{channel_id}/messages/{message_id}/ack`;
//!   payload fields modeled here (`manual`, `mention_count`) match commonly
//!   observed Discord client payloads. This module only models/builds payloads
//!   and debounce decisions; it does not perform network I/O.

use crate::domain::Snowflake;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A single channel read-state entry from READY `read_state.entries`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReadStateEntry {
    /// Channel ID this read state applies to.
    #[serde(default)]
    pub id: String,
    /// Last message ID the user has read in this channel.
    #[serde(default)]
    pub last_message_id: Option<String>,
    /// Number of mentions the user has not read.
    #[serde(default)]
    pub mention_count: u32,
    /// Read-state flags from Discord; preserved for round-trip/future use.
    #[serde(default)]
    pub flags: u32,
    /// Last pin timestamp; preserved for round-trip/future use.
    #[serde(default)]
    pub last_pin_timestamp: Option<String>,
}

/// READY `read_state` container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReadState {
    /// Monotonic read-state version from Discord.
    #[serde(default)]
    pub version: i64,
    /// Whether this is a partial read-state payload.
    #[serde(default)]
    pub partial: bool,
    /// Per-channel read-state entries.
    #[serde(default)]
    pub entries: Vec<ReadStateEntry>,
}

/// Local tracked read state for a channel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChannelReadState {
    pub channel_id: String,
    pub last_read_message_id: Option<String>,
    pub last_message_id: Option<String>,
    pub mention_count: u32,
    pub read_state_version: i64,
}

impl ChannelReadState {
    pub fn new(channel_id: impl Into<String>) -> Self {
        Self {
            channel_id: channel_id.into(),
            last_read_message_id: None,
            last_message_id: None,
            mention_count: 0,
            read_state_version: 0,
        }
    }

    pub fn from_ready_entry(entry: &ReadStateEntry, read_state_version: i64) -> Self {
        Self {
            channel_id: entry.id.clone(),
            last_read_message_id: normalize_snowflake_opt(entry.last_message_id.as_deref()),
            last_message_id: None,
            mention_count: entry.mention_count,
            read_state_version,
        }
    }

    /// Returns true when the channel has unread messages based on snowflake order.
    pub fn has_unread(&self) -> bool {
        is_message_unread(
            self.last_message_id.as_deref(),
            self.last_read_message_id.as_deref(),
        )
    }

    /// Updates last_message_id from an incoming message. Returns true if this
    /// made the channel unread for the current user.
    pub fn observe_message(
        &mut self,
        message_id: &str,
        from_current_user: bool,
        mention_increments: u32,
    ) -> bool {
        let Some(message_id) = normalize_snowflake(message_id) else {
            return false;
        };

        if is_newer_snowflake(Some(&message_id), self.last_message_id.as_deref()) {
            self.last_message_id = Some(message_id.clone());
        }

        if from_current_user {
            // Own messages are treated as read up to that point.
            if is_newer_snowflake(Some(&message_id), self.last_read_message_id.as_deref()) {
                self.last_read_message_id = Some(message_id);
                self.mention_count = 0;
            }
            return false;
        }

        if mention_increments > 0 {
            self.mention_count = self.mention_count.saturating_add(mention_increments);
        }

        self.has_unread()
    }

    /// Marks the channel read up to `message_id`. Returns true if state changed.
    pub fn mark_read_up_to(&mut self, message_id: &str) -> bool {
        let Some(message_id) = normalize_snowflake(message_id) else {
            return false;
        };

        if !is_newer_snowflake(Some(&message_id), self.last_read_message_id.as_deref()) {
            // Already read at or beyond this message; still clear mentions if fully caught up.
            let caught_up = !is_newer_snowflake(
                self.last_message_id.as_deref(),
                self.last_read_message_id.as_deref(),
            );
            if caught_up && self.mention_count != 0 {
                self.mention_count = 0;
                return true;
            }
            return false;
        }

        self.last_read_message_id = Some(message_id);
        let caught_up = !is_newer_snowflake(
            self.last_message_id.as_deref(),
            self.last_read_message_id.as_deref(),
        );
        if caught_up {
            self.mention_count = 0;
        }
        true
    }
}

/// Normalizes a snowflake string: trims, rejects empty/invalid numeric values.
pub fn normalize_snowflake(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u64>().ok()?;
    Some(trimmed.to_string())
}

pub fn normalize_snowflake_opt(value: Option<&str>) -> Option<String> {
    value.and_then(normalize_snowflake)
}

/// Parses a snowflake string into `Snowflake` for canonical ordering.
pub fn parse_snowflake(value: &str) -> Option<Snowflake> {
    normalize_snowflake(value)
        .and_then(|s| s.parse::<u64>().ok())
        .map(Snowflake::new)
}

/// Compares two snowflake IDs chronologically.
///
/// Invalid/missing IDs are ordered before valid IDs. Invalid-vs-invalid compares
/// as equal for unread decisions, but callers should prefer `normalize_snowflake`.
pub fn compare_snowflakes(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (parse_snowflake_opt(a), parse_snowflake_opt(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn parse_snowflake_opt(value: Option<&str>) -> Option<Snowflake> {
    value.and_then(parse_snowflake)
}

/// Returns true when `candidate` is strictly newer than `current`.
pub fn is_newer_snowflake(candidate: Option<&str>, current: Option<&str>) -> bool {
    compare_snowflakes(candidate, current) == Ordering::Greater
}

/// Returns true when `last_message_id` is newer than `last_read_message_id`.
pub fn is_message_unread(last_message_id: Option<&str>, last_read_message_id: Option<&str>) -> bool {
    is_newer_snowflake(last_message_id, last_read_message_id)
}

/// Payload for Discord message ACK endpoint.
///
/// Modeled from observed Discord client behavior:
/// `POST /channels/{channel_id}/messages/{message_id}/ack`
/// body commonly includes `manual` and `mention_count`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AckPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention_count: Option<u32>,
    /// Optional read-state token observed in some clients; omitted when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl AckPayload {
    pub fn manual(mention_count: u32) -> Self {
        Self {
            manual: Some(true),
            mention_count: Some(mention_count),
            token: None,
        }
    }
}

/// A debounced/coalesced mark-as-read ACK decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAck {
    pub channel_id: String,
    pub message_id: String,
    pub payload: AckPayload,
}

/// Tracks pending read ACKs and coalesces them by highest message snowflake.
#[derive(Debug, Clone)]
pub struct ReadAckDebouncer {
    debounce: Duration,
    pending: HashMap<String, PendingAckState>,
}

#[derive(Debug, Clone)]
struct PendingAckState {
    message_id: String,
    mention_count: u32,
    manual: bool,
    deadline: Instant,
}

impl Default for ReadAckDebouncer {
    fn default() -> Self {
        Self::new(Duration::from_secs(2))
    }
}

impl ReadAckDebouncer {
    pub fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            pending: HashMap::new(),
        }
    }

    pub fn debounce(&self) -> Duration {
        self.debounce
    }

    /// Schedules/updates a pending ACK for a channel. Returns false if message
    /// ID is invalid. If a pending ACK exists, keeps the newest message ID and
    /// latest mention_count/manual flag.
    pub fn schedule(
        &mut self,
        channel_id: impl Into<String>,
        message_id: &str,
        mention_count: u32,
        manual: bool,
        now: Instant,
    ) -> bool {
        let Some(message_id) = normalize_snowflake(message_id) else {
            return false;
        };
        let channel_id = channel_id.into();
        let deadline = now + self.debounce;

        match self.pending.get_mut(&channel_id) {
            Some(state) => {
                if is_newer_snowflake(Some(&message_id), Some(&state.message_id)) {
                    state.message_id = message_id;
                }
                state.mention_count = mention_count;
                state.manual = manual;
                state.deadline = deadline;
            }
            None => {
                self.pending.insert(
                    channel_id,
                    PendingAckState {
                        message_id,
                        mention_count,
                        manual,
                        deadline,
                    },
                );
            }
        }
        true
    }

    /// Returns ACKs whose debounce deadline has elapsed, removing them.
    pub fn drain_due(&mut self, now: Instant) -> Vec<PendingAck> {
        let mut due_channels = Vec::new();
        for (channel_id, state) in &self.pending {
            if now >= state.deadline {
                due_channels.push(channel_id.clone());
            }
        }
        due_channels.sort();

        let mut due = Vec::with_capacity(due_channels.len());
        for channel_id in due_channels {
            if let Some(state) = self.pending.remove(&channel_id) {
                due.push(PendingAck {
                    channel_id,
                    message_id: state.message_id,
                    payload: AckPayload {
                        manual: Some(state.manual),
                        mention_count: Some(state.mention_count),
                        token: None,
                    },
                });
            }
        }
        due
    }

    /// Forces all pending ACKs out immediately.
    pub fn drain_all(&mut self) -> Vec<PendingAck> {
        let mut channel_ids: Vec<String> = self.pending.keys().cloned().collect();
        channel_ids.sort();
        let mut out = Vec::with_capacity(channel_ids.len());
        for channel_id in channel_ids {
            if let Some(state) = self.pending.remove(&channel_id) {
                out.push(PendingAck {
                    channel_id,
                    message_id: state.message_id,
                    payload: AckPayload {
                        manual: Some(state.manual),
                        mention_count: Some(state.mention_count),
                        token: None,
                    },
                });
            }
        }
        out
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn has_pending(&self, channel_id: &str) -> bool {
        self.pending.contains_key(channel_id)
    }
}

/// In-memory store of channel read states, keyed by channel ID.
#[derive(Debug, Clone, Default)]
pub struct ReadStateStore {
    states: HashMap<String, ChannelReadState>,
}

impl ReadStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_ready(read_state: &ReadState) -> Self {
        let mut store = Self::new();
        store.apply_ready(read_state);
        store
    }

    pub fn apply_ready(&mut self, read_state: &ReadState) {
        for entry in &read_state.entries {
            let channel_id = entry.id.trim();
            if channel_id.is_empty() {
                continue;
            }
            let next = ChannelReadState::from_ready_entry(entry, read_state.version);
            match self.states.get_mut(channel_id) {
                Some(existing) => {
                    // Apply newer read-state version; if same version, keep newest read marker.
                    if read_state.version >= existing.read_state_version {
                        existing.read_state_version = read_state.version;
                        if is_newer_snowflake(
                            next.last_read_message_id.as_deref(),
                            existing.last_read_message_id.as_deref(),
                        ) {
                            existing.last_read_message_id = next.last_read_message_id;
                        }
                        existing.mention_count = next.mention_count;
                    }
                }
                None => {
                    self.states.insert(channel_id.to_string(), next);
                }
            }
        }
    }

    pub fn get(&self, channel_id: &str) -> Option<&ChannelReadState> {
        self.states.get(channel_id)
    }

    pub fn get_mut(&mut self, channel_id: &str) -> Option<&mut ChannelReadState> {
        self.states.get_mut(channel_id)
    }

    pub fn ensure_channel(&mut self, channel_id: impl Into<String>) -> &mut ChannelReadState {
        let channel_id = channel_id.into();
        self.states
            .entry(channel_id.clone())
            .or_insert_with(|| ChannelReadState::new(channel_id))
    }

    pub fn set_last_message_id(&mut self, channel_id: &str, last_message_id: Option<&str>) {
        let Some(state) = self.states.get_mut(channel_id) else {
            if let Some(last_message_id) = normalize_snowflake_opt(last_message_id) {
                let mut created = ChannelReadState::new(channel_id);
                created.last_message_id = Some(last_message_id);
                self.states.insert(channel_id.to_string(), created);
            }
            return;
        };
        if let Some(last_message_id) = normalize_snowflake_opt(last_message_id) {
            if is_newer_snowflake(Some(&last_message_id), state.last_message_id.as_deref()) {
                state.last_message_id = Some(last_message_id);
            }
        }
    }

    pub fn unread_count(&self, channel_id: &str) -> bool {
        self.states.get(channel_id).is_some_and(ChannelReadState::has_unread)
    }

    pub fn mention_count(&self, channel_id: &str) -> u32 {
        self.states
            .get(channel_id)
            .map(|s| s.mention_count)
            .unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}
