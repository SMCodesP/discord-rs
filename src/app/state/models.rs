use serde::{Deserialize, Serialize};

use crate::domain::user::AvatarDecorationData;

/// Status presence for users
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PresenceStatus {
    #[default]
    Online,
    Idle,
    DoNotDisturb,
    Offline,
}

impl PresenceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Idle => "Ausente",
            Self::DoNotDisturb => "Não Perturbar",
            Self::Offline => "Invisível",
        }
    }

    pub fn color_rgb(&self) -> u32 {
        match self {
            Self::Online => 0x23a55a,
            Self::Idle => 0xf0b232,
            Self::DoNotDisturb => 0xf23f43,
            Self::Offline => 0x80848e,
        }
    }
}

impl From<crate::domain::user::PresenceStatus> for PresenceStatus {
    fn from(s: crate::domain::user::PresenceStatus) -> Self {
        match s {
            crate::domain::user::PresenceStatus::Online => Self::Online,
            crate::domain::user::PresenceStatus::Idle => Self::Idle,
            crate::domain::user::PresenceStatus::DoNotDisturb => Self::DoNotDisturb,
            crate::domain::user::PresenceStatus::Offline => Self::Offline,
        }
    }
}

impl From<PresenceStatus> for crate::domain::user::PresenceStatus {
    fn from(s: PresenceStatus) -> Self {
        match s {
            PresenceStatus::Online => Self::Online,
            PresenceStatus::Idle => Self::Idle,
            PresenceStatus::DoNotDisturb => Self::DoNotDisturb,
            PresenceStatus::Offline => Self::Offline,
        }
    }
}

/// Active modal dialogs in the application
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    UserSettings,
    AddServer,
    ExploreServers,
    Login,
    LoginDiscord,
    AddFriendToGroup(String),
    CreateGroupDm,
}

/// User settings navigation tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileModalTab {
    #[default]
    AboutMe,
    MutualGuilds,
    MutualFriends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    MyAccount,
    Profiles,
    Privacy,
    AuthorizedApps,
    Connections,
    Nitro,
    ServerBoost,
    FamilyCenter,
    Appearance,
    Accessibility,
    VoiceVideo,
    Notifications,
    Keybinds,
    Language,
}

impl SettingsTab {
    pub fn title(&self) -> &'static str {
        match self {
            Self::MyAccount => "Minha Conta",
            Self::Profiles => "Perfis",
            Self::Privacy => "Privacidade & Segurança",
            Self::AuthorizedApps => "Aplicativos Autorizados",
            Self::Connections => "Conexões",
            Self::Nitro => "Nitro",
            Self::ServerBoost => "Impulso de Servidor",
            Self::FamilyCenter => "Centro da Família",
            Self::Appearance => "Aparência",
            Self::Accessibility => "Acessibilidade",
            Self::VoiceVideo => "Voz & Vídeo",
            Self::Notifications => "Notificações",
            Self::Keybinds => "Atalhos de Teclado",
            Self::Language => "Idioma",
        }
    }
}

/// Channel types in sidebar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    Text,
    Voice,
    Announcement,
}

impl ChannelKind {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Text => "#",
            Self::Voice => "",
            Self::Announcement => "",
        }
    }

    /// Discord vector icon token for the channel kind.
    pub fn discord_icon(&self) -> crate::tokens::icons::DiscordIcon {
        match self {
            Self::Text => crate::tokens::icons::DiscordIcon::Hash,
            Self::Voice => crate::tokens::icons::DiscordIcon::Speaker,
            Self::Announcement => crate::tokens::icons::DiscordIcon::Megaphone,
        }
    }
}

/// Channel representation for sidebar display
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelItemState {
    pub id: String,
    pub name: String,
    pub kind: ChannelKind,
    pub topic: Option<String>,
    pub unread: bool,
    pub mention_count: u32,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub is_group_dm: bool,
    #[serde(default)]
    pub recipient_ids: Vec<String>,
    #[serde(default)]
    pub recipient_names: Vec<String>,
    #[serde(default)]
    pub user_limit: Option<u32>,
    /// Avatar decoration of the DM recipient (first recipient for group DMs).
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

/// Category grouping in channel sidebar
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryItemState {
    pub id: String,
    pub name: String,
    pub channels: Vec<ChannelItemState>,
}

/// Guild item for guild rail
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuildItemState {
    pub id: String,
    pub name: String,
    pub acronym: String,
    pub icon_url: Option<String>,
    pub icon_hash: Option<String>,
    pub unread: bool,
    pub mention_count: u32,
    pub categories: Vec<CategoryItemState>,
    #[serde(default)]
    pub emojis: Vec<crate::protocol::media_cache::DiscordEmoji>,
    #[serde(default)]
    pub stickers: Vec<crate::protocol::media_cache::DiscordSticker>,
}

impl GuildItemState {
    /// Retorna a URL do ícone da guilda (resolvendo icon_url direta ou via CDN usando icon_hash)
    pub fn icon_url(&self) -> Option<String> {
        if let Some(ref url) = self.icon_url {
            if !url.is_empty() {
                return Some(url.clone());
            }
        }
        if let Some(ref hash) = self.icon_hash {
            if !hash.is_empty() {
                return Some(format!("https://cdn.discordapp.com/icons/{}/{}.png", self.id, hash));
            }
        }
        None
    }
}

/// Friend / Relationship representation in direct messages and friends list
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendItemState {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub custom_status: Option<String>,
    pub relationship_type: u32,
    /// Avatar decoration equipped by the friend, when the relationship payload carries it.
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

/// Navigation tabs in Friends Hub
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FriendsTab {
    #[default]
    Online,
    All,
    Pending,
    Blocked,
    AddFriend,
    MessageRequests,
}

/// Subtabs within Message Requests view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MessageRequestsSubTab {
    #[default]
    YouMayKnow,
    Spam,
}

/// Direct message request entry from a non-friend user
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRequestItem {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub mutual_guilds: usize,
    pub mutual_guild_name: Option<String>,
    pub preview_message: String,
    pub timestamp: String,
    pub is_spam: bool,
}

/// Activity category for Rich Presence & Active Now panel
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveNowKind {
    Game,
    RichPresence,
    Spotify,
    Quest,
}

/// Active Now Rich Presence card representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveNowItem {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub kind: ActiveNowKind,
    pub app_name: String,
    pub header_subtitle: String,
    pub app_icon_symbol: String,
    pub details_title: String,
    pub details_state: Option<String>,
    pub elapsed_time_text: Option<String>,
    pub spotify_track: Option<String>,
    pub spotify_artist: Option<String>,
    pub spotify_album: Option<String>,
    pub spotify_progress_text: Option<(String, String)>,
    pub spotify_progress_pct: Option<f32>,
    pub button_text: Option<String>,
    /// Large asset image URL (application asset, Spotify cover, or media proxy).
    #[serde(default)]
    pub large_image_url: Option<String>,
    /// Large asset hover text.
    #[serde(default)]
    pub large_image_text: Option<String>,
    /// Small asset image URL (e.g. status or platform icon overlay).
    #[serde(default)]
    pub small_image_url: Option<String>,
    /// Small asset hover text.
    #[serde(default)]
    pub small_image_text: Option<String>,
    /// Application snowflake ID.
    #[serde(default)]
    pub application_id: Option<String>,
    /// Activity start timestamp (milliseconds).
    #[serde(default)]
    pub timestamps_start: Option<i64>,
    /// Activity end timestamp (milliseconds).
    #[serde(default)]
    pub timestamps_end: Option<i64>,
    /// Avatar decoration equipped by the user, copied from the friend entry.
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}


/// Voice participant in an active call
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceParticipantState {
    pub id: String,
    pub name: String,
    pub avatar_text: String,
    pub avatar_url: Option<String>,
    pub is_speaking: bool,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_camera_on: bool,
    pub is_screen_sharing: bool,
    /// Avatar decoration equipped by the participant, when known.
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

/// Active voice call session
#[derive(Clone)]
pub struct ActiveVoiceCall {
    pub channel_id: String,
    pub channel_name: String,
    pub guild_name: String,
    pub ping_ms: Option<u32>,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_screen_sharing: bool,
    pub participants: Vec<VoiceParticipantState>,
    pub connection_state: crate::protocol::voice_session::VoiceSessionState,
    pub fatal_error: Option<String>,
    pub is_camera_on: bool,
    pub focus_user_id: Option<String>,
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub selected_camera_device: Option<String>,
    pub voice_session: Option<std::sync::Arc<crate::protocol::voice_session::VoiceSession>>,
}

impl std::fmt::Debug for ActiveVoiceCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveVoiceCall")
            .field("channel_id", &self.channel_id)
            .field("channel_name", &self.channel_name)
            .field("guild_name", &self.guild_name)
            .field("ping_ms", &self.ping_ms)
            .field("is_muted", &self.is_muted)
            .field("is_deafened", &self.is_deafened)
            .field("is_screen_sharing", &self.is_screen_sharing)
            .field("participants", &self.participants)
            .field("connection_state", &self.connection_state)
            .field("fatal_error", &self.fatal_error)
            .field("is_camera_on", &self.is_camera_on)
            .field("focus_user_id", &self.focus_user_id)
            .field("selected_input_device", &self.selected_input_device)
            .field("selected_output_device", &self.selected_output_device)
            .field("selected_camera_device", &self.selected_camera_device)
            .field("has_voice_session", &self.voice_session.is_some())
            .finish()
    }
}

impl PartialEq for ActiveVoiceCall {
    fn eq(&self, other: &Self) -> bool {
        self.channel_id == other.channel_id
            && self.channel_name == other.channel_name
            && self.guild_name == other.guild_name
            && self.ping_ms == other.ping_ms
            && self.is_muted == other.is_muted
            && self.is_deafened == other.is_deafened
            && self.is_screen_sharing == other.is_screen_sharing
            && self.participants == other.participants
            && self.connection_state == other.connection_state
            && self.fatal_error == other.fatal_error
            && self.is_camera_on == other.is_camera_on
            && self.focus_user_id == other.focus_user_id
            && self.selected_input_device == other.selected_input_device
            && self.selected_output_device == other.selected_output_device
            && self.selected_camera_device == other.selected_camera_device
    }
}

impl Eq for ActiveVoiceCall {}

/// Current logged in user profile
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUserProfile {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub discriminator: Option<String>,
    pub avatar_text: String,
    pub avatar_url: Option<String>,
    pub custom_status: Option<String>,
    pub presence: PresenceStatus,
    pub is_muted: bool,
    pub is_deafened: bool,
    /// Avatar decoration equipped by the current user (from READY / /users/@me).
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

/// Member displayed in the member sidebar
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberItemState {
    pub id: String,
    pub name: String,
    pub avatar_text: String,
    pub presence: PresenceStatus,
    pub role_group: String,
    pub role_color: u32,
    pub is_bot: bool,
    pub activity: Option<String>,
    /// Avatar decoration equipped by the member, when the member payload carries it.
    pub avatar_decoration_data: Option<AvatarDecorationData>,
}

impl MemberItemState {
    /// Creates an unloaded slot representing a sparse position in the member list
    pub fn unloaded(index: usize) -> Self {
        Self {
            id: format!("unloaded:{}", index),
            name: String::new(),
            avatar_text: String::new(),
            presence: PresenceStatus::Offline,
            role_group: String::new(),
            role_color: 0,
            is_bot: false,
            activity: None,
            avatar_decoration_data: None,
        }
    }

    /// Returns true if this slot is an unloaded gap placeholder
    pub fn is_unloaded(&self) -> bool {
        self.id.starts_with("unloaded:")
    }
}
