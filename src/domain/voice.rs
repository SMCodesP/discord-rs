use serde::{Deserialize, Serialize};

/// Represents Discord voice state data received from gateway events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceState {
    /// Guild Snowflake ID (None for DM calls).
    pub guild_id: Option<String>,
    /// Channel Snowflake ID the user is connected to (None if disconnected).
    pub channel_id: Option<String>,
    /// Snowflake ID of the user.
    pub user_id: String,
    /// Discord voice session ID.
    #[serde(default)]
    pub session_id: String,
    /// Whether the user is server-deafened.
    #[serde(default)]
    pub deaf: bool,
    /// Whether the user is server-muted.
    #[serde(default)]
    pub mute: bool,
    /// Whether the user is locally self-deafened.
    #[serde(default)]
    pub self_deaf: bool,
    /// Whether the user is locally self-muted.
    #[serde(default)]
    pub self_mute: bool,
    /// Whether the user is broadcasting camera video.
    #[serde(default)]
    pub self_video: bool,
    /// Whether the user is suppressed (e.g. stage channel audience).
    #[serde(default)]
    pub suppress: bool,
}

impl VoiceState {
    pub fn new(user_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            guild_id: None,
            channel_id: None,
            user_id: user_id.into(),
            session_id: session_id.into(),
            deaf: false,
            mute: false,
            self_deaf: false,
            self_mute: false,
            self_video: false,
            suppress: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.channel_id.is_some()
    }
}

/// Represents an active participant inside a voice channel UI view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceParticipant {
    /// Snowflake ID of the participant.
    pub user_id: String,
    /// Display username.
    pub username: String,
    /// Avatar hash or URL.
    pub avatar: Option<String>,
    /// Whether participant's microphone is muted.
    #[serde(default)]
    pub mute: bool,
    /// Whether participant is deafened.
    #[serde(default)]
    pub deaf: bool,
    /// Real-time speaking indicator flag (from Voice UDP/RTP or Voice Gateway Op 5).
    #[serde(default)]
    pub talking: bool,
    /// Whether webcam video is turned on.
    #[serde(default)]
    pub camera_on: bool,
    /// Whether screen sharing stream is active.
    #[serde(default)]
    pub screen_sharing: bool,
}

impl VoiceParticipant {
    pub fn new(user_id: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            username: username.into(),
            avatar: None,
            mute: false,
            deaf: false,
            talking: false,
            camera_on: false,
            screen_sharing: false,
        }
    }

    pub fn avatar_url(&self) -> Option<String> {
        self.avatar.as_ref().map(|hash| {
            if hash.starts_with("http://") || hash.starts_with("https://") {
                hash.clone()
            } else {
                format!("https://cdn.discordapp.com/avatars/{}/{}.png", self.user_id, hash)
            }
        })
    }
}
