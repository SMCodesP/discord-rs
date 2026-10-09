use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of screen / route in the Discord interface
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenType {
    GuildChannel { guild_id: String, channel_id: String },
    DirectMessage { channel_id: String },
    FriendsHome,
    ExploreServers,
    UserSettings { tab: String },
    Login,
    VoiceStage { channel_id: String },
    Custom(String),
}

/// Voice interactions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceActionType {
    Connect { channel_id: String },
    Disconnect { channel_id: String },
    MuteToggle { is_muted: bool },
    DeafenToggle { is_deafened: bool },
    ScreenshareToggle { is_sharing: bool },
    PttKeyDown,
    PttKeyUp,
}

/// Primary interaction events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteractionType {
    Click {
        target: String,
        component_type: String,
        detail: Option<String>,
    },
    ChannelSelect {
        guild_id: String,
        channel_id: String,
    },
    GuildSelect {
        guild_id: String,
    },
    MessageSend {
        channel_id: String,
        content_length: usize,
        has_attachment: bool,
    },
    ReactionClick {
        message_id: String,
        emoji: String,
    },
    SearchExecute {
        query: String,
        scope: String,
    },
    Voice(VoiceActionType),
    SettingsTabChange {
        tab: String,
    },
    ModalToggle {
        modal: String,
        opened: bool,
    },
    Scroll {
        view: String,
        offset_y: f32,
    },
}

/// Top-level telemetry event envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryEvent {
    ScreenChange {
        from: Option<ScreenType>,
        to: ScreenType,
        timestamp: DateTime<Utc>,
    },
    Interaction {
        interaction: InteractionType,
        timestamp: DateTime<Utc>,
    },
    ComponentMount {
        component_name: String,
        timestamp: DateTime<Utc>,
    },
    ComponentRender {
        component_name: String,
        render_duration_us: u64,
        timestamp: DateTime<Utc>,
    },
    PerformanceSample {
        metric_name: String,
        value_us: u64,
        is_bottleneck: bool,
        context: Option<String>,
        timestamp: DateTime<Utc>,
    },
    NetworkAudit {
        endpoint: String,
        method: String,
        status: u16,
        duration_ms: u64,
        is_cached_304: bool,
        rate_limit_remaining: Option<u32>,
        timestamp: DateTime<Utc>,
    },
    GatewayDispatch {
        event_name: String,
        sequence: i64,
        timestamp: DateTime<Utc>,
    },
}
