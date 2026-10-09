//! Discord RS - Spacing & Layout Tokens
//! Padding, margins, layout dimensions (rail 72px, channels 240px, members 240px), and border radiuses.

use gpui::{px, Pixels};

// ============================================================================
// Layout Dimensions (Exact Discord Desktop Layout)
// ============================================================================

/// Rail: Server navigation sidebar width (72px)
pub const RAW_RAIL_WIDTH: f32 = 72.0;

/// Channels: Channel list sidebar width (240px)
pub const RAW_CHANNELS_WIDTH: f32 = 240.0;

/// Members: Server member list sidebar width (240px)
pub const RAW_MEMBERS_WIDTH: f32 = 240.0;

/// User bottom profile bar height (52px)
pub const RAW_USER_BAR_HEIGHT: f32 = 52.0;

/// Top channel header bar height (48px)
pub const RAW_HEADER_HEIGHT: f32 = 48.0;

/// Server icon standard dimension (48x48px)
pub const RAW_SERVER_ICON_SIZE: f32 = 48.0;

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

/// Channel list sidebar width (240px)
#[inline]
pub fn channels_width() -> Pixels {
    px(RAW_CHANNELS_WIDTH)
}

/// Server member list sidebar width (240px)
#[inline]
pub fn members_width() -> Pixels {
    px(RAW_MEMBERS_WIDTH)
}

/// Bottom user profile panel height (52px)
#[inline]
pub fn user_bar_height() -> Pixels {
    px(RAW_USER_BAR_HEIGHT)
}

/// Header bar height (48px)
#[inline]
pub fn header_height() -> Pixels {
    px(RAW_HEADER_HEIGHT)
}

/// Server icon standard dimension (48px)
#[inline]
pub fn server_icon_size() -> Pixels {
    px(RAW_SERVER_ICON_SIZE)
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

/// Server icon squircle radius on hover/active (16px)
#[inline]
pub fn radius_server_hover() -> Pixels {
    px(RAW_RADIUS_SERVER_HOVER)
}

/// Server icon default circle radius (24px for 48px circle)
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
