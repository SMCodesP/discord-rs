//! Status picker model: user-selectable presence, custom status draft,
//! expiration presets, and gateway/settings payload builders.
//!
//! Sources:
//! - Gateway Identify presence shape: `src/protocol/discord_api.rs` identify payload
//!   (`presence.status`, `presence.activities`, `presence.since`, `presence.afk`).
//! - Presence/activity parsing and domain models: `src/domain/user.rs`,
//!   `src/protocol/discord_api.rs::parse_presence_from_json`.
//! - Custom status activity wire shape (type 4, name "Custom Status", state, emoji):
//!   `src/protocol/discord_api.rs` presence parsing and existing tests.
//! - Expiration presets requested by task: Today, 4h, 1h, 30m, Never.

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde_json::{json, Value};

use crate::domain::user::{CustomStatus, CustomStatusEmoji, PresenceStatus};

/// User-selectable Discord status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStatusSelection {
    Online,
    Idle,
    Dnd,
    Invisible,
}

impl UserStatusSelection {
    pub const ALL: [Self; 4] = [Self::Online, Self::Idle, Self::Dnd, Self::Invisible];

    /// Discord gateway wire value for presence status.
    pub fn as_gateway_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Idle => "idle",
            Self::Dnd => "dnd",
            Self::Invisible => "invisible",
        }
    }

    /// Human label for UI/status pickers.
    pub fn label(self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Idle => "Idle",
            Self::Dnd => "Do Not Disturb",
            Self::Invisible => "Invisible",
        }
    }

    /// Whether Discord treats this status as AFK.
    pub fn is_afk(self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Presence status used by domain/UI presence models.
    /// Discord represents invisible as offline to others; domain currently models
    /// this as `PresenceStatus::Offline`.
    pub fn to_presence_status(self) -> PresenceStatus {
        match self {
            Self::Online => PresenceStatus::Online,
            Self::Idle => PresenceStatus::Idle,
            Self::Dnd => PresenceStatus::DoNotDisturb,
            Self::Invisible => PresenceStatus::Offline,
        }
    }

    pub fn from_presence_status(status: PresenceStatus) -> Self {
        match status {
            PresenceStatus::Online => Self::Online,
            PresenceStatus::Idle => Self::Idle,
            PresenceStatus::DoNotDisturb => Self::Dnd,
            PresenceStatus::Offline => Self::Invisible,
        }
    }

    pub fn from_gateway_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "online" => Some(Self::Online),
            "idle" => Some(Self::Idle),
            "dnd" => Some(Self::Dnd),
            "invisible" => Some(Self::Invisible),
            "offline" => Some(Self::Invisible),
            _ => None,
        }
    }
}

/// Custom status expiration presets shown by Discord status picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CustomStatusExpiration {
    Today,
    FourHours,
    OneHour,
    ThirtyMinutes,
    #[default]
    Never,
}

impl CustomStatusExpiration {
    pub const ALL: [Self; 5] = [
        Self::Today,
        Self::FourHours,
        Self::OneHour,
        Self::ThirtyMinutes,
        Self::Never,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::FourHours => "4 hours",
            Self::OneHour => "1 hour",
            Self::ThirtyMinutes => "30 minutes",
            Self::Never => "Never",
        }
    }

    /// Computes expiration timestamp from a reference time.
    ///
    /// "Today" expires at the next local-day boundary in UTC (00:00:00 UTC of
    /// the following day) when using UTC reference times. Callers that need
    /// user-local semantics should pass a reference converted to the intended
    /// timezone boundary.
    pub fn expires_at(self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Never => None,
            Self::ThirtyMinutes => Some(now + Duration::minutes(30)),
            Self::OneHour => Some(now + Duration::hours(1)),
            Self::FourHours => Some(now + Duration::hours(4)),
            Self::Today => end_of_utc_day(now),
        }
    }
}

fn end_of_utc_day(now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let date = now.date_naive().succ_opt()?;
    let next_midnight = date.and_hms_opt(0, 0, 0)?;
    Some(Utc.from_utc_datetime(&next_midnight))
}

/// Editable custom status draft.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CustomStatusDraft {
    pub text: Option<String>,
    pub emoji: Option<CustomStatusEmoji>,
    pub expiration: CustomStatusExpiration,
}

impl CustomStatusDraft {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds an editable draft from a confirmed custom status payload.
    /// Discord does not include the expiration in PRESENCE_UPDATE, so an
    /// echoed/loaded status always starts with the `Never` preset.
    pub fn from_custom_status(custom_status: &CustomStatus) -> Self {
        Self {
            text: normalize_optional_text(custom_status.text.clone()),
            emoji: custom_status.emoji.clone(),
            expiration: CustomStatusExpiration::Never,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = normalize_optional_text(Some(text.into()));
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = normalize_optional_text(Some(text.into()));
    }

    pub fn with_emoji(mut self, emoji: CustomStatusEmoji) -> Self {
        self.emoji = Some(emoji);
        self
    }

    pub fn set_emoji(&mut self, emoji: Option<CustomStatusEmoji>) {
        self.emoji = emoji;
    }

    pub fn clear_emoji(mut self) -> Self {
        self.emoji = None;
        self
    }

    pub fn with_expiration(mut self, expiration: CustomStatusExpiration) -> Self {
        self.expiration = expiration;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.text.as_deref().unwrap_or("").trim().is_empty() && self.emoji.is_none()
    }

    pub fn expires_at(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if self.is_empty() {
            None
        } else {
            self.expiration.expires_at(now)
        }
    }

    pub fn to_custom_status(&self) -> Option<CustomStatus> {
        if self.is_empty() {
            return None;
        }
        Some(CustomStatus {
            text: self.text.clone(),
            emoji: self.emoji.clone(),
        })
    }

    /// Gateway activity payload for custom status (type 4).
    /// Returns None when the draft is empty.
    pub fn to_gateway_activity(&self, now: DateTime<Utc>) -> Option<Value> {
        let custom = self.to_custom_status()?;
        let mut activity = json!({
            "type": 4,
            "name": "Custom Status",
            "state": custom.text,
        });

        if let Some(emoji) = custom.emoji {
            let mut emoji_json = json!({
                "name": emoji.name,
            });
            if let Some(id) = emoji.id {
                emoji_json["id"] = Value::String(id);
            }
            if emoji.animated {
                emoji_json["animated"] = Value::Bool(true);
            }
            activity["emoji"] = emoji_json;
        }

        if let Some(expires_at) = self.expires_at(now) {
            activity["timestamps"] = json!({
                "end": expires_at.timestamp_millis(),
            });
        }

        Some(activity)
    }

    /// Settings payload for custom status updates (PATCH /users/@me/settings shape).
    /// `null` clears custom status.
    pub fn to_settings_custom_status(&self, now: DateTime<Utc>) -> Value {
        let Some(custom) = self.to_custom_status() else {
            return Value::Null;
        };

        let mut payload = json!({
            "text": custom.text,
        });

        match custom.emoji {
            Some(emoji) => {
                payload["emoji_name"] = Value::String(emoji.name);
                payload["emoji_id"] = emoji.id.map(Value::String).unwrap_or(Value::Null);
                if emoji.animated {
                    payload["animated"] = Value::Bool(true);
                }
            }
            None => {
                payload["emoji_name"] = Value::Null;
                payload["emoji_id"] = Value::Null;
            }
        }

        payload["expires_at"] = self
            .expires_at(now)
            .map(|ts| Value::String(ts.to_rfc3339()))
            .unwrap_or(Value::Null);

        payload
    }
}

fn normalize_optional_text(text: Option<String>) -> Option<String> {
    text.and_then(|t| {
        let trimmed = t.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

/// Status picker state combining selected status and custom status draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusPickerState {
    pub status: UserStatusSelection,
    pub custom_status: CustomStatusDraft,
}

impl Default for StatusPickerState {
    fn default() -> Self {
        Self {
            status: UserStatusSelection::Online,
            custom_status: CustomStatusDraft::default(),
        }
    }
}

impl StatusPickerState {
    pub fn new(status: UserStatusSelection) -> Self {
        Self {
            status,
            custom_status: CustomStatusDraft::default(),
        }
    }

    pub fn with_custom_status(mut self, custom_status: CustomStatusDraft) -> Self {
        self.custom_status = custom_status;
        self
    }

    pub fn set_status(&mut self, status: UserStatusSelection) {
        self.status = status;
    }

    pub fn set_custom_status(&mut self, custom_status: CustomStatusDraft) {
        self.custom_status = custom_status;
    }

    pub fn clear_custom_status(&mut self) {
        self.custom_status = CustomStatusDraft::default();
    }

    /// Builds gateway op 3 presence update payload.
    pub fn gateway_presence_update(&self, now: DateTime<Utc>) -> Value {
        build_gateway_presence_update_payload(self.status, &self.custom_status, now)
    }

    /// Builds settings payload for PATCH /users/@me/settings custom status.
    pub fn settings_custom_status_payload(&self, now: DateTime<Utc>) -> Value {
        build_settings_custom_status_payload(&self.custom_status, now)
    }
}

/// Builds a Discord gateway op 3 (Presence Update) payload.
///
/// Shape follows the gateway identify presence fields already used by
/// `src/protocol/discord_api.rs`: `status`, `since`, `activities`, `afk`.
pub fn build_gateway_presence_update_payload(
    status: UserStatusSelection,
    custom_status: &CustomStatusDraft,
    now: DateTime<Utc>,
) -> Value {
    let mut activities = Vec::new();
    if let Some(activity) = custom_status.to_gateway_activity(now) {
        activities.push(activity);
    }

    let since = if status.is_afk() {
        Value::from(now.timestamp_millis())
    } else {
        Value::from(0)
    };

    json!({
        "op": 3,
        "d": {
            "status": status.as_gateway_str(),
            "since": since,
            "activities": activities,
            "afk": status.is_afk(),
        }
    })
}

/// Builds settings payload for custom status updates.
///
/// Returns object with `custom_status` field; value is null when clearing.
pub fn build_settings_custom_status_payload(custom_status: &CustomStatusDraft, now: DateTime<Utc>) -> Value {
    json!({
        "custom_status": custom_status.to_settings_custom_status(now),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn status_selection_wire_and_presence_mapping() {
        assert_eq!(UserStatusSelection::Online.as_gateway_str(), "online");
        assert_eq!(UserStatusSelection::Idle.as_gateway_str(), "idle");
        assert_eq!(UserStatusSelection::Dnd.as_gateway_str(), "dnd");
        assert_eq!(UserStatusSelection::Invisible.as_gateway_str(), "invisible");

        assert_eq!(UserStatusSelection::Online.to_presence_status(), PresenceStatus::Online);
        assert_eq!(UserStatusSelection::Idle.to_presence_status(), PresenceStatus::Idle);
        assert_eq!(UserStatusSelection::Dnd.to_presence_status(), PresenceStatus::DoNotDisturb);
        assert_eq!(UserStatusSelection::Invisible.to_presence_status(), PresenceStatus::Offline);

        assert_eq!(UserStatusSelection::from_gateway_str("offline"), Some(UserStatusSelection::Invisible));
    }

    #[test]
    fn expiration_presets_compute_expected_timestamps() {
        let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
        assert_eq!(CustomStatusExpiration::ThirtyMinutes.expires_at(now), Some(now + Duration::minutes(30)));
        assert_eq!(CustomStatusExpiration::OneHour.expires_at(now), Some(now + Duration::hours(1)));
        assert_eq!(CustomStatusExpiration::FourHours.expires_at(now), Some(now + Duration::hours(4)));
        assert_eq!(CustomStatusExpiration::Never.expires_at(now), None);
        assert_eq!(
            CustomStatusExpiration::Today.expires_at(now),
            Some(Utc.with_ymd_and_hms(2026, 11, 25, 0, 0, 0).unwrap())
        );
    }

    #[test]
    fn gateway_payload_includes_custom_status_activity() {
        let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
        let draft = CustomStatusDraft::new()
            .with_text("Focus")
            .with_emoji(CustomStatusEmoji::unicode("🎯"))
            .with_expiration(CustomStatusExpiration::OneHour);
        let payload = build_gateway_presence_update_payload(UserStatusSelection::Dnd, &draft, now);

        assert_eq!(payload["op"], 3);
        assert_eq!(payload["d"]["status"], "dnd");
        assert_eq!(payload["d"]["afk"], false);
        assert_eq!(payload["d"]["activities"][0]["type"], 4);
        assert_eq!(payload["d"]["activities"][0]["name"], "Custom Status");
        assert_eq!(payload["d"]["activities"][0]["state"], "Focus");
        assert_eq!(payload["d"]["activities"][0]["emoji"]["name"], "🎯");
        assert_eq!(payload["d"]["activities"][0]["timestamps"]["end"], (now + Duration::hours(1)).timestamp_millis());
    }

    #[test]
    fn settings_payload_clears_when_empty() {
        let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
        let payload = build_settings_custom_status_payload(&CustomStatusDraft::default(), now);
        assert!(payload["custom_status"].is_null());
    }
}
