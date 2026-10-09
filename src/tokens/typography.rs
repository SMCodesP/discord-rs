//! Discord RS - Typography Tokens
//! Font families, sizes, weights, and line heights aligned with Discord UI specifications.

use gpui::{px, FontWeight, Pixels};

// ============================================================================
// Font Family Stacks
// ============================================================================

/// Primary font stack used by Discord ("gg sans", with system fallbacks)
pub const FONT_FAMILY_PRIMARY: &str = "gg sans, Noto Sans, Helvetica Neue, Helvetica, Arial, sans-serif";

/// Monospace font stack used for code blocks and inline code
pub const FONT_FAMILY_CODE: &str = "Consolas, Andale Mono WT, Andale Mono, Lucida Console, Lucida Sans Typewriter, DejaVu Sans Mono, Bitstream Vera Sans Mono, Liberation Mono, Monaco, Courier New, Courier, monospace";

// ============================================================================
// Raw Numerical Sizes (px)
// ============================================================================

pub const SIZE_XXS: f32 = 10.0;
pub const SIZE_XS: f32 = 12.0;
pub const SIZE_SM: f32 = 14.0;
pub const SIZE_BASE: f32 = 16.0;
pub const SIZE_MD: f32 = 16.0;
pub const SIZE_LG: f32 = 18.0;
pub const SIZE_XL: f32 = 20.0;
pub const SIZE_2XL: f32 = 24.0;
pub const SIZE_3XL: f32 = 32.0;

// ============================================================================
// Font Sizes (gpui::Pixels)
// ============================================================================

/// Micro badges, indicators (10px)
#[inline]
pub fn font_size_xxs() -> Pixels {
    px(SIZE_XXS)
}

/// Category headers, timestamps, tooltips, subtext (12px)
#[inline]
pub fn font_size_xs() -> Pixels {
    px(SIZE_XS)
}

/// Channel names, member list secondary, small buttons (14px)
#[inline]
pub fn font_size_sm() -> Pixels {
    px(SIZE_SM)
}

/// Main chat message body, inputs, default text (16px)
#[inline]
pub fn font_size_base() -> Pixels {
    px(SIZE_BASE)
}

/// Alias for base font size (16px)
#[inline]
pub fn font_size_md() -> Pixels {
    px(SIZE_MD)
}

/// Section headers, topic lines, popout titles (18px)
#[inline]
pub fn font_size_lg() -> Pixels {
    px(SIZE_LG)
}

/// Modal headers, server name title (20px)
#[inline]
pub fn font_size_xl() -> Pixels {
    px(SIZE_XL)
}

/// Large headers, welcome banners (24px)
#[inline]
pub fn font_size_2xl() -> Pixels {
    px(SIZE_2XL)
}

/// Hero headlines (32px)
#[inline]
pub fn font_size_3xl() -> Pixels {
    px(SIZE_3XL)
}

// ============================================================================
// Font Weights (gpui::FontWeight)
// ============================================================================

/// Light weight (300)
pub const WEIGHT_LIGHT: FontWeight = FontWeight::LIGHT;

/// Normal / Regular weight (400)
pub const WEIGHT_NORMAL: FontWeight = FontWeight::NORMAL;

/// Medium weight (500)
pub const WEIGHT_MEDIUM: FontWeight = FontWeight::MEDIUM;

/// Semibold weight (600)
pub const WEIGHT_SEMIBOLD: FontWeight = FontWeight::SEMIBOLD;

/// Bold weight (700)
pub const WEIGHT_BOLD: FontWeight = FontWeight::BOLD;

/// Extra Bold weight (800)
pub const WEIGHT_EXTRA_BOLD: FontWeight = FontWeight::EXTRA_BOLD;

// ============================================================================
// Raw Numerical Line Heights (px)
// ============================================================================

pub const LH_TIGHT: f32 = 14.0;
pub const LH_XS: f32 = 16.0;
pub const LH_SM: f32 = 18.0;
pub const LH_BASE: f32 = 22.0; // Discord standard body line-height (1.375rem / 22px)
pub const LH_MD: f32 = 22.0;
pub const LH_LG: f32 = 24.0;
pub const LH_XL: f32 = 26.0;
pub const LH_2XL: f32 = 30.0;
pub const LH_3XL: f32 = 38.0;

// ============================================================================
// Line Heights (gpui::Pixels)
// ============================================================================

/// Compact / tight lines (14px)
#[inline]
pub fn line_height_tight() -> Pixels {
    px(LH_TIGHT)
}

/// Extra small text line height (16px)
#[inline]
pub fn line_height_xs() -> Pixels {
    px(LH_XS)
}

/// Small text line height (18px)
#[inline]
pub fn line_height_sm() -> Pixels {
    px(LH_SM)
}

/// Discord message body line height (22px)
#[inline]
pub fn line_height_base() -> Pixels {
    px(LH_BASE)
}

/// Alias for base line height (22px)
#[inline]
pub fn line_height_md() -> Pixels {
    px(LH_MD)
}

/// Large line height (24px)
#[inline]
pub fn line_height_lg() -> Pixels {
    px(LH_LG)
}

/// Extra large line height (26px)
#[inline]
pub fn line_height_xl() -> Pixels {
    px(LH_XL)
}

/// Title / modal line height (30px)
#[inline]
pub fn line_height_2xl() -> Pixels {
    px(LH_2XL)
}

/// Hero banner line height (38px)
#[inline]
pub fn line_height_3xl() -> Pixels {
    px(LH_3XL)
}

// ============================================================================
// Semantic Presets
// ============================================================================

/// Typography specification for a UI element
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyPreset {
    pub size: Pixels,
    pub weight: FontWeight,
    pub line_height: Pixels,
}

impl TypographyPreset {
    pub const fn new(size: Pixels, weight: FontWeight, line_height: Pixels) -> Self {
        Self {
            size,
            weight,
            line_height,
        }
    }
}

/// Message body text (16px / 400 / 22px)
#[inline]
pub fn typo_message_body() -> TypographyPreset {
    TypographyPreset::new(font_size_base(), WEIGHT_NORMAL, line_height_base())
}

/// Message author username (16px / 500 / 22px)
#[inline]
pub fn typo_username() -> TypographyPreset {
    TypographyPreset::new(font_size_base(), WEIGHT_MEDIUM, line_height_base())
}

/// Channel item in channel list (16px / 500 / 20px)
#[inline]
pub fn typo_channel_item() -> TypographyPreset {
    TypographyPreset::new(font_size_base(), WEIGHT_MEDIUM, px(20.0))
}

/// Category header in channel list (12px / 700 / 16px uppercase)
#[inline]
pub fn typo_category_header() -> TypographyPreset {
    TypographyPreset::new(font_size_xs(), WEIGHT_BOLD, line_height_xs())
}

/// Timestamp in chat message (12px / 500 / 16px)
#[inline]
pub fn typo_timestamp() -> TypographyPreset {
    TypographyPreset::new(font_size_xs(), WEIGHT_MEDIUM, line_height_xs())
}

/// Server header title (16px / 700 / 20px)
#[inline]
pub fn typo_server_header() -> TypographyPreset {
    TypographyPreset::new(font_size_base(), WEIGHT_BOLD, px(20.0))
}

/// Modal / Dialog title (20px / 600 / 26px)
#[inline]
pub fn typo_modal_title() -> TypographyPreset {
    TypographyPreset::new(font_size_xl(), WEIGHT_SEMIBOLD, line_height_xl())
}

/// User bar name (14px / 600 / 18px)
#[inline]
pub fn typo_user_bar_name() -> TypographyPreset {
    TypographyPreset::new(font_size_sm(), WEIGHT_SEMIBOLD, line_height_sm())
}

/// User bar discriminator / status (12px / 400 / 14px)
#[inline]
pub fn typo_user_bar_subtext() -> TypographyPreset {
    TypographyPreset::new(font_size_xs(), WEIGHT_NORMAL, line_height_tight())
}
