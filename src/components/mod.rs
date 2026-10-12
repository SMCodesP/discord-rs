//! Discord RS - Reusable GPUI Components
//! Native GPUI components styled according to the canonical Discord Dark Theme design tokens.

pub mod avatar;
pub mod badge;
pub mod banner;
pub mod button;
pub mod guild_item;
pub mod member_item;
pub mod profile_effect;
pub mod text_input;

pub use avatar::{Avatar, AvatarSize};
pub use badge::MentionBadge;
pub use banner::{Banner, BannerVariant};
pub use button::{Button, ButtonSize, ButtonVariant, IconButton};
pub use guild_item::GuildItem;
pub use member_item::MemberItem;
pub use profile_effect::{
    render_profile_effect, render_profile_effect_for_user, ProfileEffectCatalog,
    ProfileEffectConfig, ProfileEffectLayer, ProfileEffectLayerSource, UserProfileEffect,
};
pub use text_input::{
    TextInput, TextInputActionCallback, TextInputChangeCallback, TextInputMaskCallback,
};
