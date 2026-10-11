use super::*;
use std::collections::{HashMap, HashSet};
use crate::domain::voice::VoiceState;
use crate::domain::message::Message;
use crate::domain::settings::AccessibilitySettings;
use crate::domain::settings::AccountConnection;
use crate::domain::settings::AuthorizedApp;
use crate::domain::settings::FamilyCenterLink;
use crate::domain::settings::MessageDisplayMode;
use crate::domain::settings::NitroTier;
use crate::domain::settings::ServerBoost;
use crate::domain::settings::Subscription;
use crate::domain::settings::ThemeChoice;
use crate::protocol::ClientManager;
use crate::protocol::ConnectionMode;

/// Main Application State
#[derive(Debug, Clone)]
pub struct AppState {
    /// Selected guild Snowflake ID (`None` = Home / DMs)
    pub selected_guild_id: Option<String>,
    /// Selected channel Snowflake ID
    pub selected_channel_id: Option<String>,
    /// Connected voice call session
    pub active_voice_call: Option<ActiveVoiceCall>,
    /// Whether the member sidebar (right column) is visible
    pub show_members: bool,
    /// Currently opened modal dialog
    pub active_modal: Option<ActiveModal>,
    /// Active tab in User Settings modal
    pub user_settings_tab: SettingsTab,
    /// Set of collapsed category IDs
    pub collapsed_categories: HashSet<String>,
    /// Logged in user profile
    pub current_user: CurrentUserProfile,
    /// List of user's guilds
    pub guilds: Vec<GuildItemState>,
    /// List of members in currently selected guild
    pub members: Vec<MemberItemState>,
    /// Global or channel search query
    pub search_query: String,
    /// Whether the search flyout sidebar is open
    pub is_search_open: bool,
    /// Grouped search results
    pub search_results: Vec<crate::chat::search::SearchResultGroup>,
    /// Search executing flag
    pub is_searching: bool,
    /// Target message Snowflake ID to focus/scroll to in chat
    pub target_message_id: Option<String>,
    /// Real-time coalesced typing tracker: channel_id -> user_id -> (username, timestamp)
    pub typing_tracker: HashMap<String, HashMap<String, (String, std::time::Instant)>>,

    // Real-Time Discord Gateway & Chat State
    pub is_connected_to_gateway: bool,
    pub auth_token: Option<String>,
    pub connection_status: String,
    pub connection_mode: ConnectionMode,
    pub client_manager: Option<ClientManager>,
    pub messages_by_channel: HashMap<String, Vec<Message>>,
    pub active_draft: String,
    pub replying_to_message: Option<Message>,
    pub typing_users: Vec<String>,
    pub token_input: String,
    pub is_user_token: bool,
    pub save_token_to_disk: bool,
    pub auth_error: Option<String>,
    pub is_connecting: bool,
    /// Direct Message (DM) channels
    pub dm_channels: Vec<ChannelItemState>,
    /// Pinned direct message channel IDs (ordered strictly as in user settings)
    pub pinned_dms: Vec<String>,
    /// Optional path to pinned DMs cache on disk (None disables disk persistence, e.g. in tests)
    pub pinned_dms_path: Option<std::path::PathBuf>,
    /// User's guild folders grouping
    pub guild_folders: Vec<crate::protocol::GuildFolder>,
    /// Set of expanded folder IDs in guild rail
    pub expanded_folders: std::collections::HashSet<String>,
    /// Friends / user relationships list
    pub friends: Vec<FriendItemState>,
    /// Active tab in Friends Hub
    pub friends_tab: FriendsTab,
    /// Username input for Add Friend tab
    pub add_friend_input: String,
    /// Feedback status message for Add Friend (is_success, text)
    pub add_friend_feedback: Option<(bool, String)>,
    /// Whether a friend request submission is in progress
    pub is_sending_friend_request: bool,
    /// Set of user IDs with in-flight relationship actions (accept, decline, remove)
    pub pending_relationship_actions: HashSet<String>,
    /// Active subtab in Message Requests
    pub message_requests_subtab: MessageRequestsSubTab,
    /// Direct message requests (Pending & Spam)
    pub message_requests: Vec<MessageRequestItem>,
    /// Rich Presence activities in Active Now panel
    pub active_now_items: Vec<ActiveNowItem>,
    /// Real-time user presences indexed by user Snowflake ID
    pub presences_by_user: HashMap<String, crate::domain::user::Presence>,
    /// Telemetry & Parity monitoring handle
    pub telemetry: Option<crate::telemetry::TelemetryHandle>,

    // Advanced User Settings & Account Subsystems
    pub nitro_tier: NitroTier,
    pub active_subscription: Option<Subscription>,
    pub server_boosts: Vec<ServerBoost>,
    pub connections: Vec<AccountConnection>,
    pub authorized_apps: Vec<AuthorizedApp>,
    pub family_center_links: Vec<FamilyCenterLink>,
    pub theme_choice: ThemeChoice,
    pub message_display_mode: MessageDisplayMode,
    pub accessibility_settings: AccessibilitySettings,
    pub media_cache: crate::protocol::media_cache::MediaCacheManager,
    /// Currently opened user profile popout
    pub profile_popout: Option<crate::domain::profile::ProfilePopoutState>,
    /// Screen anchor position for the user profile popout
    pub profile_anchor: Option<gpui::Point<gpui::Pixels>>,
    /// Whether the full user profile modal dialog is open
    pub profile_modal_open: bool,
    /// Active tab in the full user profile modal
    pub profile_modal_tab: ProfileModalTab,
    /// Collectibles profile effects catalog
    pub profile_effects_catalog: crate::components::ProfileEffectCatalog,
    /// Active voice states by voice channel ID
    pub voice_states_by_channel: HashMap<String, Vec<VoiceState>>,
}

pub struct AppStateGlobal(pub gpui::Entity<AppState>);
impl gpui::Global for AppStateGlobal {}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
