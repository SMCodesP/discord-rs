//! Discord RS - Spacing & Layout Tokens
//! Padding, margins, layout dimensions (rail 72px, measured channel sidebar
//! 272px, members 240px), and border radiuses.

use gpui::{Pixels, px};

// ============================================================================
// Layout Dimensions (Exact Discord Desktop Layout)
// ============================================================================

/// Rail: Server navigation sidebar width (72px)
pub const RAW_RAIL_WIDTH: f32 = 72.0;

/// Channels: legacy documented channel list width (240px).
/// Kept for compatibility with the original design-token contract; the shell
/// uses [`RAW_CHANNEL_SIDEBAR_WIDTH`] from the measured Discord Desktop client.
pub const RAW_CHANNELS_WIDTH: f32 = 240.0;

/// Measured Discord Desktop channel sidebar width (272px).
pub const RAW_CHANNEL_SIDEBAR_WIDTH: f32 = 272.0;

/// Members: Server member list sidebar width (240px)
pub const RAW_MEMBERS_WIDTH: f32 = 240.0;

/// Measured Discord Desktop floating user panel height (58px)
pub const RAW_USER_BAR_HEIGHT: f32 = 58.0;

/// Measured Discord Desktop channel header height (49px)
pub const RAW_HEADER_HEIGHT: f32 = 49.0;

/// Measured Discord Desktop server icon dimension (40x40px)
pub const RAW_SERVER_ICON_SIZE: f32 = 40.0;

/// Guild rail icon/item height (40px); the rail adds an 8px step gap.
pub const RAW_GUILD_ITEM_HEIGHT: f32 = 40.0;

/// Collapsed and expanded guild folder button size (48px)
pub const RAW_GUILD_FOLDER_SIZE: f32 = 48.0;

/// Inner folder preview container size (40px)
pub const RAW_GUILD_FOLDER_PREVIEW_CONTAINER_SIZE: f32 = 40.0;

/// Four-up guild folder preview thumbnail size (19px)
pub const RAW_GUILD_FOLDER_PREVIEW_SIZE: f32 = 19.0;

/// Padding inside the four-up guild folder preview container (4px)
pub const RAW_GUILD_FOLDER_PREVIEW_PADDING: f32 = 4.0;

/// Gap inside the four-up guild folder preview grid (2px)
pub const RAW_GUILD_FOLDER_PREVIEW_GAP: f32 = 2.0;

/// Measured guild selection pill width (8px)
pub const RAW_GUILD_PILL_WIDTH: f32 = 8.0;

/// Unread guild selection pill height (8px)
pub const RAW_GUILD_PILL_UNREAD_HEIGHT: f32 = 8.0;

/// Hovered guild selection pill height (20px)
pub const RAW_GUILD_PILL_HOVER_HEIGHT: f32 = 20.0;

/// Selected guild selection pill height (40px)
pub const RAW_GUILD_PILL_SELECTED_HEIGHT: f32 = 40.0;

/// Guild rail separator width (32px)
pub const RAW_GUILD_SEPARATOR_WIDTH: f32 = 32.0;

/// Guild rail separator height (1px)
pub const RAW_GUILD_SEPARATOR_HEIGHT: f32 = 1.0;

/// Measured Discord Desktop channel row height (32px)
pub const RAW_CHANNEL_ITEM_HEIGHT: f32 = 32.0;

/// Measured unread channel indicator width (4px)
pub const RAW_CHANNEL_UNREAD_WIDTH: f32 = 4.0;

/// Voice participant row height (28px)
pub const RAW_VOICE_USER_HEIGHT: f32 = 28.0;

/// User panel audio/settings control size (32px)
pub const RAW_USER_CONTROL_SIZE: f32 = 32.0;

/// Measured floating user panel width (328px)
pub const RAW_USER_PANEL_WIDTH: f32 = 328.0;

/// Floating user panel inset from the combined rail/sidebar edges (8px)
pub const RAW_USER_PANEL_MARGIN: f32 = 8.0;

/// Guild-rail clearance for the 58px user panel plus its 8px inset (66px).
pub const RAW_USER_PANEL_RESERVE: f32 = 66.0;

/// Measured bottom clearance of the channel navigation area (50px). The
/// floating panel is allowed to overlap this area, matching Discord Desktop.
pub const RAW_CHANNEL_NAVIGATION_RESERVE: f32 = 50.0;

/// Measured header search width (244px)
pub const RAW_HEADER_SEARCH_WIDTH: f32 = 244.0;

/// Measured header search height (32px)
pub const RAW_HEADER_SEARCH_HEIGHT: f32 = 32.0;

/// Measured header toolbar icon size (20px)
pub const RAW_HEADER_ICON_SIZE: f32 = 20.0;

/// Chat input textarea minimum height (44px)
pub const RAW_CHAT_INPUT_HEIGHT: f32 = 44.0;

/// Small avatar size (32px)
pub const RAW_AVATAR_SM: f32 = 32.0;

/// Medium / standard chat avatar size (40px)
pub const RAW_AVATAR_MD: f32 = 40.0;

/// Large profile avatar size (80px)
pub const RAW_AVATAR_LG: f32 = 80.0;

// ============================================================================
// Layout Dimension Accessors (gpui::Pixels)
// ============================================================================

/// Server navigation sidebar width (72px)
#[inline]
pub fn rail_width() -> Pixels {
    px(RAW_RAIL_WIDTH)
}

/// Legacy channel list sidebar width (240px)
#[inline]
pub fn channels_width() -> Pixels {
    px(RAW_CHANNELS_WIDTH)
}

/// Measured Discord Desktop channel sidebar width (272px)
#[inline]
pub fn channel_sidebar_width() -> Pixels {
    px(RAW_CHANNEL_SIDEBAR_WIDTH)
}

/// Server member list sidebar width (240px)
#[inline]
pub fn members_width() -> Pixels {
    px(RAW_MEMBERS_WIDTH)
}

/// Bottom user profile panel height (58px)
#[inline]
pub fn user_bar_height() -> Pixels {
    px(RAW_USER_BAR_HEIGHT)
}

/// Header bar height (49px)
#[inline]
pub fn header_height() -> Pixels {
    px(RAW_HEADER_HEIGHT)
}

/// Server icon standard dimension (40px)
#[inline]
pub fn server_icon_size() -> Pixels {
    px(RAW_SERVER_ICON_SIZE)
}

/// Guild rail item height (40px)
#[inline]
pub fn guild_item_height() -> Pixels {
    px(RAW_GUILD_ITEM_HEIGHT)
}

/// Guild folder icon size (48px)
#[inline]
pub fn guild_folder_size() -> Pixels {
    px(RAW_GUILD_FOLDER_SIZE)
}

/// Inner guild folder preview container size (40px)
#[inline]
pub fn guild_folder_preview_container_size() -> Pixels {
    px(RAW_GUILD_FOLDER_PREVIEW_CONTAINER_SIZE)
}

/// Guild folder preview thumbnail size (19px)
#[inline]
pub fn guild_folder_preview_size() -> Pixels {
    px(RAW_GUILD_FOLDER_PREVIEW_SIZE)
}

/// Guild folder preview grid padding (4px)
#[inline]
pub fn guild_folder_preview_padding() -> Pixels {
    px(RAW_GUILD_FOLDER_PREVIEW_PADDING)
}

/// Guild folder preview grid gap (2px)
#[inline]
pub fn guild_folder_preview_gap() -> Pixels {
    px(RAW_GUILD_FOLDER_PREVIEW_GAP)
}

/// Guild selection pill width (8px)
#[inline]
pub fn guild_pill_width() -> Pixels {
    px(RAW_GUILD_PILL_WIDTH)
}

/// Unread guild pill height (8px)
#[inline]
pub fn guild_pill_unread_height() -> Pixels {
    px(RAW_GUILD_PILL_UNREAD_HEIGHT)
}

/// Hovered guild pill height (20px)
#[inline]
pub fn guild_pill_hover_height() -> Pixels {
    px(RAW_GUILD_PILL_HOVER_HEIGHT)
}

/// Selected guild pill height (40px)
#[inline]
pub fn guild_pill_selected_height() -> Pixels {
    px(RAW_GUILD_PILL_SELECTED_HEIGHT)
}

/// Guild separator width (32px)
#[inline]
pub fn guild_separator_width() -> Pixels {
    px(RAW_GUILD_SEPARATOR_WIDTH)
}

/// Guild separator height (1px)
#[inline]
pub fn guild_separator_height() -> Pixels {
    px(RAW_GUILD_SEPARATOR_HEIGHT)
}

/// Channel row height (32px)
#[inline]
pub fn channel_item_height() -> Pixels {
    px(RAW_CHANNEL_ITEM_HEIGHT)
}

/// Unread channel indicator width (4px)
#[inline]
pub fn channel_unread_width() -> Pixels {
    px(RAW_CHANNEL_UNREAD_WIDTH)
}

/// Voice participant row height (28px)
#[inline]
pub fn voice_user_height() -> Pixels {
    px(RAW_VOICE_USER_HEIGHT)
}

/// User panel control size (32px)
#[inline]
pub fn user_control_size() -> Pixels {
    px(RAW_USER_CONTROL_SIZE)
}

/// Floating user panel width (328px)
#[inline]
pub fn user_panel_width() -> Pixels {
    px(RAW_USER_PANEL_WIDTH)
}

/// Floating user panel edge margin (8px)
#[inline]
pub fn user_panel_margin() -> Pixels {
    px(RAW_USER_PANEL_MARGIN)
}

/// Guild-rail clearance for the floating user panel (66px)
#[inline]
pub fn user_panel_reserve() -> Pixels {
    px(RAW_USER_PANEL_RESERVE)
}

/// Measured channel navigation bottom clearance (50px)
#[inline]
pub fn channel_navigation_reserve() -> Pixels {
    px(RAW_CHANNEL_NAVIGATION_RESERVE)
}

/// Header search width (244px)
#[inline]
pub fn header_search_width() -> Pixels {
    px(RAW_HEADER_SEARCH_WIDTH)
}

/// Header search height (32px)
#[inline]
pub fn header_search_height() -> Pixels {
    px(RAW_HEADER_SEARCH_HEIGHT)
}

/// Header toolbar icon size (20px)
#[inline]
pub fn header_icon_size() -> Pixels {
    px(RAW_HEADER_ICON_SIZE)
}

/// Chat input standard minimum height (44px)
#[inline]
pub fn chat_input_height() -> Pixels {
    px(RAW_CHAT_INPUT_HEIGHT)
}

/// Small avatar size (32px)
#[inline]
pub fn avatar_sm() -> Pixels {
    px(RAW_AVATAR_SM)
}

/// Standard chat avatar size (40px)
#[inline]
pub fn avatar_md() -> Pixels {
    px(RAW_AVATAR_MD)
}

/// Large avatar size (80px)
#[inline]
pub fn avatar_lg() -> Pixels {
    px(RAW_AVATAR_LG)
}

// ============================================================================
// Spacing Scale (Padding / Margins / Gaps)
// ============================================================================

pub const RAW_SPACE_0: f32 = 0.0;
pub const RAW_SPACE_1: f32 = 2.0;
pub const RAW_SPACE_2: f32 = 4.0;
pub const RAW_SPACE_3: f32 = 6.0;
pub const RAW_SPACE_4: f32 = 8.0;
pub const RAW_SPACE_5: f32 = 10.0;
pub const RAW_SPACE_6: f32 = 12.0;
pub const RAW_SPACE_8: f32 = 16.0;
pub const RAW_SPACE_10: f32 = 20.0;
pub const RAW_SPACE_12: f32 = 24.0;
pub const RAW_SPACE_16: f32 = 32.0;

/// 0px
#[inline]
pub fn space_0() -> Pixels {
    px(RAW_SPACE_0)
}

/// 2px (micro space, indicator margin)
#[inline]
pub fn space_1() -> Pixels {
    px(RAW_SPACE_1)
}

/// 4px (extra tight gap, badge padding)
#[inline]
pub fn space_2() -> Pixels {
    px(RAW_SPACE_2)
}

/// 6px (compact button padding)
#[inline]
pub fn space_3() -> Pixels {
    px(RAW_SPACE_3)
}

/// 8px (standard padding, item gaps)
#[inline]
pub fn space_4() -> Pixels {
    px(RAW_SPACE_4)
}

/// 10px (channel item vertical padding)
#[inline]
pub fn space_5() -> Pixels {
    px(RAW_SPACE_5)
}

/// 12px (rail server icon gap, container padding)
#[inline]
pub fn space_6() -> Pixels {
    px(RAW_SPACE_6)
}

/// 16px (standard section padding, chat item horizontal padding)
#[inline]
pub fn space_8() -> Pixels {
    px(RAW_SPACE_8)
}

/// 20px (chat gutter margin)
#[inline]
pub fn space_10() -> Pixels {
    px(RAW_SPACE_10)
}

/// 24px (dialog padding)
#[inline]
pub fn space_12() -> Pixels {
    px(RAW_SPACE_12)
}

/// 32px (large component margin)
#[inline]
pub fn space_16() -> Pixels {
    px(RAW_SPACE_16)
}

// ============================================================================
// Border Radiuses
// ============================================================================

pub const RAW_RADIUS_NONE: f32 = 0.0;
pub const RAW_RADIUS_XS: f32 = 2.0;
pub const RAW_RADIUS_SM: f32 = 4.0;
pub const RAW_RADIUS_MD: f32 = 8.0;
pub const RAW_RADIUS_LG: f32 = 12.0;
pub const RAW_RADIUS_SERVER_HOVER: f32 = 16.0;
pub const RAW_RADIUS_SERVER_DEFAULT: f32 = 24.0;
pub const RAW_RADIUS_PILL: f32 = 20.0;
pub const RAW_RADIUS_FULL: f32 = 9999.0;

/// No border radius (0px)
#[inline]
pub fn radius_none() -> Pixels {
    px(RAW_RADIUS_NONE)
}

/// Extra small radius (2px)
#[inline]
pub fn radius_xs() -> Pixels {
    px(RAW_RADIUS_XS)
}

/// Small radius (4px) - Badges, notification tags
#[inline]
pub fn radius_sm() -> Pixels {
    px(RAW_RADIUS_SM)
}

/// Medium radius (8px) - Discord buttons, inputs, channel items, chat textarea
#[inline]
pub fn radius_md() -> Pixels {
    px(RAW_RADIUS_MD)
}

/// Large radius (12px) - Modals, context menus, large popouts
#[inline]
pub fn radius_lg() -> Pixels {
    px(RAW_RADIUS_LG)
}

/// Measured 40px server-icon squircle approximation (16px). The official
/// client uses an SVG mask rather than a single CSS border radius.
#[inline]
pub fn radius_server_hover() -> Pixels {
    px(RAW_RADIUS_SERVER_HOVER)
}

/// Legacy 48px-circle radius (24px), retained for the original design-token
/// contract. The measured Desktop shell does not use this value.
#[inline]
pub fn radius_server_default() -> Pixels {
    px(RAW_RADIUS_SERVER_DEFAULT)
}

/// Pill shaped button or badge radius (20px)
#[inline]
pub fn radius_pill() -> Pixels {
    px(RAW_RADIUS_PILL)
}

/// Full circle radius (9999px) - Avatars, status dots
#[inline]
pub fn radius_full() -> Pixels {
    px(RAW_RADIUS_FULL)
}
