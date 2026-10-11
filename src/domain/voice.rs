use serde::{Deserialize, Serialize};

use super::user::AvatarDecorationData;

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
    /// Whether the user is broadcasting a screen share / Go Live stream.
    #[serde(default)]
    pub self_stream: bool,
    /// Whether the user is suppressed (e.g. stage channel audience).
    #[serde(default)]
    pub suppress: bool,
    /// Optional member display name / username
    #[serde(default)]
    pub user_name: Option<String>,
    /// Optional member avatar URL / hash
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// Avatar decoration equipped by the user (guild-level `member.avatar_decoration_data`
    /// takes precedence over `member.user.avatar_decoration_data` when present).
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    // Fed by voice gateway WebRTC / RTP connection opcode 5 Speaking
    #[serde(skip)]
    pub is_speaking: bool,
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
            self_stream: false,
            suppress: false,
            user_name: None,
            avatar_url: None,
            avatar_decoration_data: None,
            is_speaking: false,
        }
    }

    pub fn with_channel_id(mut self, channel_id: impl Into<String>) -> Self {
        self.channel_id = Some(channel_id.into());
        self
    }

    pub fn with_guild_id(mut self, guild_id: impl Into<String>) -> Self {
        self.guild_id = Some(guild_id.into());
        self
    }

    pub fn with_user_name(mut self, name: impl Into<String>) -> Self {
        self.user_name = Some(name.into());
        self
    }

    pub fn with_avatar_url(mut self, url: impl Into<String>) -> Self {
        self.avatar_url = Some(url.into());
        self
    }

    pub fn with_avatar_decoration(mut self, decor: AvatarDecorationData) -> Self {
        self.avatar_decoration_data = Some(decor);
        self
    }

    pub fn with_self_stream(mut self, stream: bool) -> Self {
        self.self_stream = stream;
        self
    }

    pub fn with_speaking(mut self, speaking: bool) -> Self {
        self.is_speaking = speaking;
        self
    }

    pub fn with_mute(mut self, mute: bool) -> Self {
        self.mute = mute;
        self
    }

    pub fn with_deaf(mut self, deaf: bool) -> Self {
        self.deaf = deaf;
        self
    }

    pub fn is_muted(&self) -> bool {
        self.mute || self.self_mute
    }

    pub fn is_deafened(&self) -> bool {
        self.deaf || self.self_deaf
    }

    pub fn display_name(&self) -> &str {
        self.user_name.as_deref().unwrap_or(&self.user_id)
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
    /// Avatar decoration equipped by the participant, when known.
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
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
            avatar_decoration_data: None,
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
