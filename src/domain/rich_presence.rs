//! Rich Presence presentation derived exclusively from real Discord
//! [`Activity`] payloads.
//!
//! Discord activity types and fields:
//! - https://discord.com/developers/docs/topics/gateway-events#activity-object
//! - https://discord.com/developers/docs/topics/gateway-events#activity-object-activity-types
//! - https://discord.com/developers/docs/topics/gateway-events#activity-object-activity-assets
//!
//! There are no synthetic fallbacks here: missing names, details, states,
//! timestamps, emojis, or assets stay `None` instead of becoming placeholder
//! data or invented CDN URLs.

use crate::domain::user::{
    Activity, ActivityType, CustomStatusEmoji, resolve_activity_asset_url,
};

/// Discord sends activity timestamps in milliseconds. Older/synthetic
/// payloads may still carry seconds, so values below this threshold (roughly
/// 1973-03-03 in milliseconds) are normalised to milliseconds.
const MILLISECOND_THRESHOLD: i64 = 100_000_000_000;

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalized_millis(timestamp: i64) -> Option<i64> {
    if timestamp < 0 {
        return None;
    }
    if timestamp < MILLISECOND_THRESHOLD {
        timestamp.checked_mul(1000)
    } else {
        Some(timestamp)
    }
}

/// Returns the display header for an activity, matching Discord's verbs.
///
/// - Playing / Streaming / Watching: `"<verb> <name>"`
/// - Listening: `"Listening to <name>"`
/// - Competing: `"Competing in <name>"`
/// - Custom: the real custom-status text (never the literal "Custom Status")
///
/// Returns `None` when the payload has no usable text for the header.
pub fn activity_headline(activity: &Activity) -> Option<String> {
    let name = activity.name.trim();
    match activity.activity_type {
        ActivityType::Custom => non_empty(activity.state.as_deref()).map(str::to_owned),
        _ if name.is_empty() => None,
        _ => Some(format!("{} {}", activity.activity_type.label(), name)),
    }
}

/// Trimmed activity details, or `None` when absent/blank.
pub fn activity_details(activity: &Activity) -> Option<&str> {
    non_empty(activity.details.as_deref())
}

/// Trimmed activity state, or `None` when absent/blank.
pub fn activity_state(activity: &Activity) -> Option<&str> {
    non_empty(activity.state.as_deref())
}

/// Whether a Discord activity actually represents Spotify.
///
/// Discord's Spotify integration is a `Listening` activity identified by the
/// `Spotify` name, a `sync_id` (track ID), or a `spotify:` asset key. Any other
/// listening activity (for example a radio app) is not treated as Spotify.
pub fn is_spotify_activity(activity: &Activity) -> bool {
    if activity.activity_type != ActivityType::Listening {
        return false;
    }
    if activity.name.trim().eq_ignore_ascii_case("Spotify") {
        return true;
    }
    if non_empty(activity.sync_id.as_deref()).is_some() {
        return true;
    }
    activity
        .assets
        .as_ref()
        .and_then(|assets| non_empty(assets.large_image.as_deref()))
        .is_some_and(|image| image.starts_with("spotify:"))
}

/// Resolves a raw Discord activity asset key into its real CDN/proxy URL.
///
/// Delegates to [`resolve_activity_asset_url`], which implements Discord's
/// documented rules for `mp:external/...`, `spotify:...`, direct URLs, and
/// application assets. Returns `None` when the key is blank or an application
/// asset cannot be resolved without an application ID.
pub fn resolve_asset_url(asset: &str, application_id: Option<&str>) -> Option<String> {
    resolve_activity_asset_url(asset, application_id)
}

/// Real large image URL for an activity, or `None` when there is no asset.
pub fn activity_large_image_url(activity: &Activity) -> Option<String> {
    activity.large_image_url()
}

/// Real small image URL for an activity, or `None` when there is no asset.
pub fn activity_small_image_url(activity: &Activity) -> Option<String> {
    activity.small_image_url()
}

/// Resolved image URLs and hover texts carried by the activity assets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityImageUrls {
    pub large_url: Option<String>,
    pub large_text: Option<String>,
    pub small_url: Option<String>,
    pub small_text: Option<String>,
}

/// Resolves all real activity asset URLs/texts. Missing data stays `None`.
pub fn activity_image_urls(activity: &Activity) -> ActivityImageUrls {
    let large_text = activity
        .assets
        .as_ref()
        .and_then(|assets| non_empty(assets.large_text.as_deref()))
        .map(str::to_owned);
    let small_text = activity
        .assets
        .as_ref()
        .and_then(|assets| non_empty(assets.small_text.as_deref()))
        .map(str::to_owned);

    ActivityImageUrls {
        large_url: activity_large_image_url(activity),
        large_text,
        small_url: activity_small_image_url(activity),
        small_text,
    }
}

fn format_clock(total_seconds: i64) -> String {
    let total_seconds = total_seconds.max(0);
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

/// Progress of a Spotify (or any timestamped) activity at a concrete instant.
#[derive(Debug, Clone, PartialEq)]
pub struct SpotifyProgress {
    /// Normalised start timestamp in milliseconds.
    pub start_ms: i64,
    /// Normalised end timestamp in milliseconds.
    pub end_ms: i64,
    /// Evaluation instant in milliseconds.
    pub now_ms: i64,
    /// Elapsed milliseconds, clamped to `[0, total_ms]`.
    pub elapsed_ms: i64,
    /// Total track duration in milliseconds.
    pub total_ms: i64,
    /// Remaining milliseconds, clamped to `[0, total_ms]`.
    pub remaining_ms: i64,
    /// Elapsed / total ratio in `[0.0, 1.0]`.
    pub ratio: f32,
    /// Elapsed time rendered as `m:ss` or `h:mm:ss`.
    pub elapsed_text: String,
    /// Total duration rendered as `m:ss` or `h:mm:ss`.
    pub total_text: String,
}

impl SpotifyProgress {
    /// Computes progress from raw Discord timestamps at `now_ms`.
    ///
    /// Returns `None` for missing/negative/overflowing or non-positive
    /// intervals. Timestamps in seconds are normalised to milliseconds.
    pub fn from_timestamps(start: i64, end: i64, now_ms: i64) -> Option<Self> {
        let start_ms = normalized_millis(start)?;
        let end_ms = normalized_millis(end)?;
        let now_ms = if now_ms < 0 {
            return None;
        } else if now_ms < MILLISECOND_THRESHOLD {
            now_ms.checked_mul(1000)?
        } else {
            now_ms
        };

        let total_ms = end_ms.checked_sub(start_ms)?;
        if total_ms <= 0 {
            return None;
        }

        let elapsed_ms = now_ms.saturating_sub(start_ms).clamp(0, total_ms);
        let remaining_ms = total_ms - elapsed_ms;
        let ratio = (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0);
        let elapsed_seconds = elapsed_ms / 1000;
        let total_seconds = total_ms / 1000;

        Some(Self {
            start_ms,
            end_ms,
            now_ms,
            elapsed_ms,
            total_ms,
            remaining_ms,
            ratio,
            elapsed_text: format_clock(elapsed_seconds),
            total_text: format_clock(total_seconds),
        })
    }

    /// Progress for a Spotify activity at `now_ms`. Non-Spotify activities and
    /// activities without both timestamps return `None`.
    pub fn from_activity(activity: &Activity, now_ms: i64) -> Option<Self> {
        if !is_spotify_activity(activity) {
            return None;
        }
        let timestamps = activity.timestamps.as_ref()?;
        Self::from_timestamps(timestamps.start?, timestamps.end?, now_ms)
    }

    /// Progress for a Spotify activity evaluated against the system clock.
    pub fn from_activity_now(activity: &Activity) -> Option<Self> {
        Self::from_activity(activity, chrono::Utc::now().timestamp_millis())
    }

    /// Ratio as a percentage in `[0.0, 100.0]`.
    pub fn ratio_percent(&self) -> f32 {
        self.ratio * 100.0
    }
}

/// Complete presentation of a real Discord activity.
///
/// Every field is derived from the payload. Fields the payload does not carry
/// remain `None`; no placeholder texts or URLs are ever fabricated.
#[derive(Debug, Clone, PartialEq)]
pub struct RichPresence {
    pub activity_type: ActivityType,
    /// Whether the source event is Discord's Spotify integration.
    pub spotify: bool,
    pub headline: Option<String>,
    pub details: Option<String>,
    pub state: Option<String>,
    pub emoji: Option<CustomStatusEmoji>,
    pub large_image_url: Option<String>,
    pub large_image_text: Option<String>,
    pub small_image_url: Option<String>,
    pub small_image_text: Option<String>,
    /// Spotify track title (`details`) when this is a Spotify activity.
    pub spotify_track: Option<String>,
    /// Spotify artist list (`state`) when this is a Spotify activity.
    pub spotify_artist: Option<String>,
    /// Spotify album (`assets.large_text`) when this is a Spotify activity.
    pub spotify_album: Option<String>,
    /// Real timestamp-derived progress; `None` without a valid interval.
    pub spotify_progress: Option<SpotifyProgress>,
}

impl RichPresence {
    /// Builds a presentation snapshot evaluated at `now_ms`.
    pub fn from_activity(activity: &Activity, now_ms: i64) -> Self {
        let spotify = is_spotify_activity(activity);
        let images = activity_image_urls(activity);
        let (spotify_track, spotify_artist, spotify_album, spotify_progress) = if spotify {
            (
                activity_details(activity).map(str::to_owned),
                activity_state(activity).map(str::to_owned),
                images.large_text.clone(),
                SpotifyProgress::from_activity(activity, now_ms),
            )
        } else {
            (None, None, None, None)
        };

        Self {
            activity_type: activity.activity_type,
            spotify,
            headline: activity_headline(activity),
            details: activity_details(activity).map(str::to_owned),
            state: activity_state(activity).map(str::to_owned),
            emoji: activity.emoji.clone(),
            large_image_url: images.large_url,
            large_image_text: images.large_text,
            small_image_url: images.small_url,
            small_image_text: images.small_text,
            spotify_track,
            spotify_artist,
            spotify_album,
            spotify_progress,
        }
    }

    /// Builds a presentation snapshot evaluated against the system clock.
    pub fn from_activity_now(activity: &Activity) -> Self {
        Self::from_activity(activity, chrono::Utc::now().timestamp_millis())
    }

    /// Whether the source activity is Spotify.
    pub fn is_spotify(&self) -> bool {
        self.spotify
    }
}

/// Convenience formatter evaluated against the system clock.
pub fn format_activity(activity: &Activity) -> RichPresence {
    RichPresence::from_activity_now(activity)
}

/// Convenience formatter evaluated at a supplied millisecond timestamp.
pub fn format_activity_at(activity: &Activity, now_ms: i64) -> RichPresence {
    RichPresence::from_activity(activity, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::user::{ActivityAssets, ActivityTimestamps};

    #[test]
    fn spotify_detection_requires_listening_and_a_real_marker() {
        let mut activity = Activity::new("Spotify", ActivityType::Listening);
        assert!(is_spotify_activity(&activity));

        activity.name = "My Radio".to_string();
        activity.sync_id = Some("track-id".to_string());
        assert!(is_spotify_activity(&activity));

        activity.sync_id = None;
        activity.assets = Some(ActivityAssets {
            large_image: Some("spotify:cover".to_string()),
            ..ActivityAssets::default()
        });
        assert!(is_spotify_activity(&activity));

        activity.assets = None;
        assert!(!is_spotify_activity(&activity));

        let playing = Activity::new("Spotify", ActivityType::Playing);
        assert!(!is_spotify_activity(&playing));
    }

    #[test]
    fn progress_uses_normalised_timestamps_and_clamps() {
        let progress = SpotifyProgress::from_timestamps(1_000, 1_300, 1_150).unwrap();
        assert_eq!(progress.start_ms, 1_000_000);
        assert_eq!(progress.end_ms, 1_300_000);
        assert_eq!(progress.now_ms, 1_150_000);
        assert_eq!(progress.total_ms, 300_000);
        assert_eq!(progress.elapsed_ms, 150_000);
        assert_eq!(progress.remaining_ms, 150_000);
        assert_eq!(progress.ratio, 0.5);
        assert_eq!(progress.ratio_percent(), 50.0);
        assert_eq!(progress.elapsed_text, "2:30");
        assert_eq!(progress.total_text, "5:00");

        let before = SpotifyProgress::from_timestamps(1_000, 1_300, 900).unwrap();
        assert_eq!(before.elapsed_ms, 0);
        assert_eq!(before.ratio, 0.0);

        let after = SpotifyProgress::from_timestamps(1_000, 1_300, 2_000).unwrap();
        assert_eq!(after.elapsed_ms, 300_000);
        assert_eq!(after.remaining_ms, 0);
        assert_eq!(after.ratio, 1.0);
    }

    #[test]
    fn invalid_timestamp_intervals_never_fabricate_progress() {
        assert_eq!(SpotifyProgress::from_timestamps(1_300, 1_000, 1_150), None);
        assert_eq!(SpotifyProgress::from_timestamps(1_000, 1_000, 1_000), None);
        assert_eq!(SpotifyProgress::from_timestamps(-1, 1_300, 1_150), None);
        assert_eq!(SpotifyProgress::from_timestamps(1_000, 1_300, -1), None);
        assert_eq!(SpotifyProgress::from_timestamps(i64::MAX, 1_300, 1_150), None);
    }

    #[test]
    fn progress_from_activity_requires_spotify_and_both_timestamps() {
        let mut activity = Activity::new("Spotify", ActivityType::Listening);
        assert_eq!(SpotifyProgress::from_activity(&activity, 1_150_000), None);

        activity.timestamps = Some(ActivityTimestamps {
            start: Some(1_000_000),
            end: Some(1_300_000),
        });
        let progress = SpotifyProgress::from_activity(&activity, 1_150_000).unwrap();
        assert_eq!(progress.ratio, 0.5);

        activity.activity_type = ActivityType::Listening;
        activity.name = "Radio".to_string();
        assert_eq!(SpotifyProgress::from_activity(&activity, 1_150_000), None);

        activity.name = "Spotify".to_string();
        activity.timestamps = Some(ActivityTimestamps {
            start: Some(1_000_000),
            end: None,
        });
        assert_eq!(SpotifyProgress::from_activity(&activity, 1_150_000), None);
    }
}
