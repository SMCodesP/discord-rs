//! Discord RS - Typography Tokens
//! Font families, sizes, weights, and line heights modeled on Discord's UI scale.
//!
//! # Font licensing and parity scope
//!
//! Discord ships the proprietary **gg sans** family. This project does not
//! bundle or redistribute that font (nor Whitney), so the UI uses an open
//! fallback with metrics measured from the font file itself:
//!
//! * Family: **DejaVu Sans** (Bitstream Vera / DejaVu license), installed as a
//!   system font on the supported Linux target.
//! * Source of every value below: `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`
//!   (`head.unitsPerEm`, `hhea` ascent/descent/lineGap, `hmtx` advances and
//!   `glyf` bounding boxes). No value is estimated.
//!
//! Because the family differs from gg sans, this module makes **no claim of
//! pixel-identical glyph metrics**. Sizes and line heights follow the measured
//! Discord UI scale, but glyph outlines/advances are those of the open fallback.

use gpui::{FontWeight, Pixels, px};

// ============================================================================
// Font Family Stacks
// ============================================================================

/// Effective primary font family: open fallback, licensed for redistribution.
///
/// GPUI resolves one family name at a time (it does not parse a CSS-style
/// comma stack), so this must be a single installed family. See the module
/// documentation for why gg sans is deliberately not used.
pub const FONT_FAMILY_PRIMARY: &str = "DejaVu Sans";

/// Proprietary Discord reference family, kept only for comparison/telemetry.
///
/// It is **never fed to the renderer** and is not distributed with the app.
pub const FONT_FAMILY_OFFICIAL_REFERENCE: &str = "gg sans";

/// Ordered open fallbacks for callers that resolve availability themselves.
/// The first entry is [`FONT_FAMILY_PRIMARY`]; the rest are permissively
/// licensed sans-serif families commonly installed on Linux/Windows/macOS.
pub const FONT_FAMILY_OPEN_FALLBACKS: &[&str] = &[
    "DejaVu Sans",
    "Noto Sans",
    "Liberation Sans",
    "Cantarell",
    "Adwaita Sans",
    "sans-serif",
];

/// Monospace family used for code blocks and inline code.
///
/// GPUI takes a single family name; the previous comma stack silently resolved
/// to the system fallback. DejaVu Sans Mono is open and installed alongside the
/// primary fallback.
pub const FONT_FAMILY_CODE: &str = "DejaVu Sans Mono";

// ============================================================================
// Open fallback metrics (measured from DejaVuSans.ttf, not estimated)
// ============================================================================

/// Units per em from `head.unitsPerEm` of DejaVuSans.ttf.
pub const OPEN_FALLBACK_UNITS_PER_EM: u32 = 2048;
/// Typographic ascent from `hhea.ascender` (font units).
pub const OPEN_FALLBACK_ASCENT: i32 = 1901;
/// Typographic descent from `hhea.descender` (font units, negative).
pub const OPEN_FALLBACK_DESCENT: i32 = -483;
/// Recommended line gap from `hhea.lineGap` (font units).
pub const OPEN_FALLBACK_LINE_GAP: i32 = 0;

/// One glyph's metrics in font units, measured from DejaVuSans.ttf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFallbackGlyph {
    pub ch: char,
    /// Advance width from `hmtx`.
    pub advance: u32,
    /// Bounding-box top from `glyf` (relative to baseline).
    pub y_max: i32,
    /// Bounding-box bottom from `glyf` (relative to baseline).
    pub y_min: i32,
}

/// Reference glyphs used by the typography regression test.
pub const OPEN_FALLBACK_GLYPHS: &[OpenFallbackGlyph] = &[
    OpenFallbackGlyph { ch: 'M', advance: 1767, y_max: 1493, y_min: 0 },
    OpenFallbackGlyph { ch: 'W', advance: 2025, y_max: 1493, y_min: 0 },
    OpenFallbackGlyph { ch: 'i', advance: 569, y_max: 1556, y_min: 0 },
    OpenFallbackGlyph { ch: 'l', advance: 569, y_max: 1556, y_min: 0 },
    OpenFallbackGlyph { ch: 'g', advance: 1300, y_max: 1147, y_min: -426 },
    OpenFallbackGlyph { ch: '0', advance: 1303, y_max: 1520, y_min: -29 },
    OpenFallbackGlyph { ch: ' ', advance: 651, y_max: 0, y_min: 0 },
];

/// Returns the measured fallback glyph record for `ch`.
#[inline]
pub fn open_fallback_glyph(ch: char) -> Option<OpenFallbackGlyph> {
    OPEN_FALLBACK_GLYPHS.iter().copied().find(|glyph| glyph.ch == ch)
}

/// Rendered advance width of `ch` at `size` px, using the measured font units.
#[inline]
pub fn glyph_advance_px(ch: char, size: f32) -> Option<f32> {
    open_fallback_glyph(ch)
        .map(|glyph| glyph.advance as f32 * size / OPEN_FALLBACK_UNITS_PER_EM as f32)
}

/// Rendered ink height (bounding-box height) of `ch` at `size` px.
#[inline]
pub fn glyph_height_px(ch: char, size: f32) -> Option<f32> {
    open_fallback_glyph(ch).map(|glyph| {
        (glyph.y_max - glyph.y_min) as f32 * size / OPEN_FALLBACK_UNITS_PER_EM as f32
    })
}

/// Natural line-box height of the open fallback at `size` px:
/// `(ascent - descent + lineGap) / unitsPerEm * size`.
#[inline]
pub fn open_fallback_line_height_px(size: f32) -> f32 {
    let em = OPEN_FALLBACK_UNITS_PER_EM as f32;
    let units = (OPEN_FALLBACK_ASCENT - OPEN_FALLBACK_DESCENT + OPEN_FALLBACK_LINE_GAP) as f32;
    units * size / em
}

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

/// Channel item in channel list (16px / 500 / 24px).
/// Source: measured Discord Desktop shell.
#[inline]
pub fn typo_channel_item() -> TypographyPreset {
    TypographyPreset::new(font_size_base(), WEIGHT_MEDIUM, px(24.0))
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

/// Server header title (16px / 600 / 20px).
/// Source: measured Discord Desktop shell.
#[inline]
pub fn typo_server_header() -> TypographyPreset {
    TypographyPreset::new(font_size_base(), WEIGHT_SEMIBOLD, px(20.0))
}

/// Modal / Dialog title (20px / 600 / 26px)
#[inline]
pub fn typo_modal_title() -> TypographyPreset {
    TypographyPreset::new(font_size_xl(), WEIGHT_SEMIBOLD, line_height_xl())
}

/// User bar name (14px / 500 / 14px).
/// Source: measured Discord Desktop shell.
#[inline]
pub fn typo_user_bar_name() -> TypographyPreset {
    TypographyPreset::new(font_size_sm(), WEIGHT_MEDIUM, px(14.0))
}

/// User bar discriminator / status (12px / 400 / 14px)
#[inline]
pub fn typo_user_bar_subtext() -> TypographyPreset {
    TypographyPreset::new(font_size_xs(), WEIGHT_NORMAL, line_height_tight())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn open_fallback_is_not_a_proprietary_discord_font() {
        assert_eq!(FONT_FAMILY_PRIMARY, "DejaVu Sans");
        assert!(
            !FONT_FAMILY_PRIMARY
                .to_ascii_lowercase()
                .contains(FONT_FAMILY_OFFICIAL_REFERENCE),
            "the renderer must not select the proprietary gg sans family"
        );
        assert_eq!(FONT_FAMILY_OPEN_FALLBACKS[0], FONT_FAMILY_PRIMARY);
    }

    #[test]
    fn measured_glyph_metrics_match_dejavu_sans_at_16px() {
        // Expected values are advances/heights measured in font units and
        // scaled by 16 / 2048; see the module documentation for the source.
        approx(glyph_advance_px('M', 16.0).unwrap(), 13.8047);
        approx(glyph_advance_px('W', 16.0).unwrap(), 15.8203);
        approx(glyph_advance_px('i', 16.0).unwrap(), 4.4453);
        approx(glyph_advance_px('g', 16.0).unwrap(), 10.1563);
        approx(glyph_advance_px(' ', 16.0).unwrap(), 5.0859);

        approx(glyph_height_px('M', 16.0).unwrap(), 11.6641);
        approx(glyph_height_px('i', 16.0).unwrap(), 12.1563);
        approx(glyph_height_px('g', 16.0).unwrap(), 12.2891);

        assert!(glyph_advance_px('☃', 16.0).is_none());
    }

    #[test]
    fn open_fallback_line_height_and_discord_line_box_are_documented() {
        // Natural DejaVu Sans line box at 16px: (1901 + 483) / 2048 * 16.
        approx(open_fallback_line_height_px(SIZE_BASE), 18.6250);
        // The Discord UI body line box remains the measured 16px/22px pair.
        approx(line_height_base().as_f32(), 22.0);
        assert!(line_height_base().as_f32() > open_fallback_line_height_px(SIZE_BASE));
        assert_eq!(typo_message_body().size, font_size_base());
        assert_eq!(typo_message_body().line_height, line_height_base());
    }
}
