//! Discord RS - Reusable GPUI Components
//! Native GPUI components styled according to the canonical Discord Dark Theme design tokens.

pub mod avatar;
pub mod badge;
pub mod banner;
pub mod button;
pub mod channel_item;
pub mod guild_item;
pub mod member_item;
pub mod modal;
pub mod profile_effect;
pub mod text_input;
pub mod tooltip;

pub use avatar::{Avatar, AvatarSize};
pub use badge::{MentionBadge, UnreadMarker, UnreadMarkerState};
pub use banner::{Banner, BannerVariant};
pub use button::{Button, ButtonSize, ButtonVariant, IconButton};
pub use channel_item::ChannelItem;
pub use guild_item::GuildItem;
pub use member_item::MemberItem;
pub use modal::Modal;
pub use profile_effect::{render_profile_effect, ProfileEffect, ProfileEffectData, ProfileEffectTheme};
pub use text_input::{
    TextInput, TextInputActionCallback, TextInputChangeCallback, TextInputMaskCallback,
};
pub use tooltip::{
    discord_tooltip, discord_tooltip_with_shortcut, Tooltip, TooltipView,
};
