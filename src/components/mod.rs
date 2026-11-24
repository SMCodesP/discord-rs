//! Discord RS - Reusable GPUI Components
//! Native GPUI components styled according to the canonical Discord Dark Theme design tokens.

pub mod avatar;
pub mod badge;
pub mod banner;
pub mod button;
pub mod guild_item;
pub mod profile_effect;
pub mod text_input;

pub use avatar::{Avatar, AvatarSize};
pub use badge::MentionBadge;
pub use banner::{Banner, BannerVariant};
pub use button::{Button, ButtonSize, ButtonVariant, IconButton};
pub use guild_item::GuildItem;
pub use profile_effect::{
    render_profile_effect, render_profile_effect_for_user, ProfileEffectCatalog,
    ProfileEffectConfig, ProfileEffectLayer, ProfileEffectLayerSource, UserProfileEffect,
};
pub use text_input::{
    TextInput, TextInputActionCallback, TextInputChangeCallback, TextInputMaskCallback,
};

pub mod media_viewer;

pub use media_viewer::{
    browser_open_command, collect_media_items, render_media_viewer, MediaDownloadTarget,
    MediaViewerCallbacks, MediaViewerItem, MediaViewerKind, MediaViewerState,
    MEDIA_VIEWER_MAX_ZOOM, MEDIA_VIEWER_MIN_ZOOM, MEDIA_VIEWER_ZOOM_STEP,
};

pub mod context_menus;
pub use context_menus::{
    apply_entries_to_popup_menu, build_menu_entries, build_popup_menu, ContextMenuContext,
    MenuAction, MenuCallbacks, MenuEntry, MenuPermissions, MenuTargetKind,
};

pub mod status_picker;
pub use status_picker::{
    build_gateway_presence_update_payload, build_settings_custom_status_payload, CustomStatusDraft,
    CustomStatusExpiration, StatusPickerState, UserStatusSelection,
};
