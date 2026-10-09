//! Discord RS - App Shell, Layout and Navigation
//!
//! Provides the primary desktop application structure:
//! - `AppState`: Centralized interactive state (guilds, channels, voice, members, modals)
//! - `GuildRail`: 72px left server navigation rail
//! - `ChannelSidebar`: 240px channel hierarchy and bottom user controls
//! - `HeaderBar`: 48px channel top bar with topic, search, and action buttons
//! - `MemberSidebar`: 240px role-grouped member list
//! - `VoiceCallPanel`: Voice room stage with RTC ping, participant grid, and controls
//! - `UserSettingsModal`: Full-screen modal with user profile and app preferences
//! - `RootView`: Top-level multi-column GPUI layout container

pub mod channel_sidebar;
pub mod friends_hub;
pub mod guild_rail;
pub mod header_bar;
pub mod member_sidebar;
pub mod root_view;
pub mod state;
pub mod user_settings;
pub mod voice_call_panel;

pub use channel_sidebar::ChannelSidebar;
pub use friends_hub::render_friends_hub;
pub use guild_rail::GuildRail;
pub use header_bar::HeaderBar;
pub use member_sidebar::MemberSidebar;
pub use root_view::RootView;
pub use state::*;
pub use user_settings::UserSettingsModal;
pub use voice_call_panel::VoiceCallPanel;
