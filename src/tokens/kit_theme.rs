//! Discord RS - Kit Theme ("Discord")
//!
//! Registers a dark `"Discord"` theme in the gpui-kit theme system
//! (`gpui::component::{ThemeRegistry, Theme}`), derived from `src/tokens/`.
//!
//! Call [`install`] once, after `gpui::init(cx)` (i.e. `gpui_kit::init`), which
//! is what sets up the `Theme` / `ThemeRegistry` globals this builds on.
//!
//! ```text
//! gpui_kit::init(cx);
//! discord_rs::tokens::kit_theme::install(cx);
//! ```
//!
//! Every kit color field with a Discord equivalent is fed from the existing
//! token constants/functions in [`crate::tokens`] (no repeated hex literals).
//! Fields with no token equivalent use the live Discord dark-theme CSS value
//! (see the `CSS_*` constants below, each annotated with its source variable)
//! or are omitted so the kit keeps its own dark default (listed as "incerto"
//! in the task report).
//!
//! Kit colors that have no Discord surface at all (server rail, channel list,
//! chat, user bar, statuses, mentions, ...) are exposed via [`DiscordExtras`],
//! a [`gpui::Global`] installed by [`install`] and readable through the
//! [`DiscordTheme`] extension trait, so screens read everything through the
//! theme instead of `rgb(0x...)`.

use std::rc::Rc;

use gpui::component::{Theme, ThemeConfig, ThemeRegistry};
use gpui::{App, Global, Rgba};

use super::colors;
use super::spacing::{RAW_RADIUS_LG, RAW_RADIUS_MD};
use super::typography::{FONT_FAMILY_CODE, FONT_FAMILY_PRIMARY, SIZE_BASE};

/// Name under which the theme is registered in the kit [`ThemeRegistry`].
pub const DISCORD_THEME_NAME: &str = "Discord";

// ============================================================================
// Discord CSS-sourced constants (dark theme, read-only inspection of the
// live Discord web client via getComputedStyle; custom-theme mixes ignored,
// only unmixed primitive stops used).
// ============================================================================

/// Link blue. Source: computed `--text-link` first stop
/// `hsl(212.795 calc(1*82.564%) 61.765%/1)`.
pub const CSS_LINK: &str = "#4d96ee";

/// Info blue. Source: `--blue-360` = `hsl(212.791 calc(1*81.905%) 58.824%/1)`.
pub const CSS_INFO: &str = "#408eec";

/// Info hover blue (darker step). Source: `--blue-400`.
pub const CSS_INFO_HOVER: &str = "#217fe7";

/// Info active blue (darkest step). Source: `--blue-430`.
pub const CSS_INFO_ACTIVE: &str = "#006fd9";

/// Light blue (badges/accents). Source: `--blue-330`.
pub const CSS_BLUE_LIGHT: &str = "#6aa8f4";

/// Danger hover red (darker step). Source: `--red-430`.
pub const CSS_DANGER_HOVER: &str = "#d6363f";

/// Danger active red (darkest step). Source: `--red-500`.
pub const CSS_DANGER_ACTIVE: &str = "#a2212b";

/// Light red. Source: `--red-330`.
pub const CSS_RED_LIGHT: &str = "#ff9691";

/// Warning hover amber (darker step). Source: `--yellow-330`.
pub const CSS_WARNING_HOVER: &str = "#fcb323";

/// Warning active amber (darkest step). Source: `--yellow-360`.
pub const CSS_WARNING_ACTIVE: &str = "#c97e00";

/// Success hover green (darker step). Source: `--green-430`.
pub const CSS_SUCCESS_HOVER: &str = "#008043";

/// Success active green (darkest step). Source: `--green-530`.
pub const CSS_SUCCESS_ACTIVE: &str = "#00562a";

/// Light green. Source: `--green-330`.
pub const CSS_GREEN_LIGHT: &str = "#65ba7f";

/// Switch background default. Source: `--switch-background-default`
/// `hsl(0 calc(1*0%) 0%/0.12156862745098039)`.
pub const CSS_SWITCH_BG: &str = "#0000001f";

/// Switch thumb background default. Source: `--switch-thumb-background-default`
/// `hsl(0 calc(1*0%) 100%/1)`.
pub const CSS_SWITCH_THUMB: &str = "#ffffff";

/// Slider track background. Source: `--slider-track-background`
/// `color-mix(in oklab,hsl(234 calc(1*6.579%) 29.804%/1) 100%,rgb(8,0,26) 50%)`.
pub const CSS_SLIDER_TRACK: &str = "#302d3f";

/// Chart categorical 1. Source: `--chart-categorical-1`.
pub const CSS_CHART_1: &str = "#3c3da2";

/// Chart categorical 2. Source: `--chart-categorical-2`.
pub const CSS_CHART_2: &str = "#a05c40";

/// Chart categorical 3. Source: `--chart-categorical-3`.
pub const CSS_CHART_3: &str = "#6733a2";

/// Chart categorical 4. Source: `--chart-categorical-4`.
pub const CSS_CHART_4: &str = "#346f82";

/// Chart categorical 5. Source: `--chart-categorical-5`.
pub const CSS_CHART_5: &str = "#42735d";

/// Chart bullish / positive. Source: `--chart-positive`.
pub const CSS_CHART_BULLISH: &str = "#42735d";

/// Chart bearish / negative. Source: `--chart-negative`.
pub const CSS_CHART_BEARISH: &str = "#862035";

// ============================================================================
// JSON builders (pure: the exact payload `install` feeds the registry)
// ============================================================================

/// `#rrggbb` for an opaque `0xRRGGBB` token constant.
#[inline]
fn hex6(hex: u32) -> String {
    format!("#{hex:06x}")
}

/// `#rrggbbaa` for an `0xRRGGBBAA` token constant.
#[inline]
fn hex8(hex: u32) -> String {
    format!("#{hex:08x}")
}

/// Builds the `ThemeSet` JSON registering the dark `"Discord"` theme.
///
/// Same string [`install`] passes to `ThemeRegistry::load_themes_from_str`,
/// exposed separately so tests can exercise the mapping without an `App`.
pub fn discord_theme_json() -> String {
    let colors = serde_json::json!({
        // --- surfaces / text ---
        "background": hex6(colors::HEX_BG_CHAT),
        "foreground": hex6(colors::HEX_TEXT_PRIMARY),
        "accent.background": hex6(colors::HEX_CHANNEL_HOVER),
        "accent.foreground": hex6(colors::HEX_WHITE),
        "accordion.background": hex6(colors::HEX_BG_CHANNELS),
        "border": hex6(colors::HEX_BORDERS),
        // --- default button (secondary style) ---
        "button.background": hex6(colors::HEX_BTN_IGNORE),
        "button.hover.background": hex6(colors::HEX_CHANNEL_ACTIVE),
        "button.foreground": hex6(colors::HEX_WHITE),
        // --- primary / brand button ---
        "button.primary.background": hex6(colors::HEX_BLURPLE),
        "button.primary.active.background": hex6(colors::HEX_BLURPLE_ACTIVE),
        "button.primary.foreground": hex6(colors::HEX_WHITE),
        "button.primary.hover.background": hex6(colors::HEX_BLURPLE_HOVER),
        // --- secondary button ---
        "button.secondary.background": hex6(colors::HEX_CHANNEL_HOVER),
        "button.secondary.foreground": hex6(colors::HEX_WHITE),
        "button.secondary.hover.background": hex6(colors::HEX_CHANNEL_ACTIVE),
        // --- success / warning / danger / info buttons ---
        "button.success.background": hex6(colors::HEX_STATUS_ONLINE),
        "button.success.active.background": CSS_SUCCESS_ACTIVE,
        "button.success.foreground": hex6(colors::HEX_WHITE),
        "button.success.hover.background": CSS_SUCCESS_HOVER,
        "button.warning.background": hex6(colors::HEX_STATUS_IDLE),
        "button.warning.active.background": CSS_WARNING_ACTIVE,
        "button.warning.foreground": hex6(colors::HEX_WHITE),
        "button.warning.hover.background": CSS_WARNING_HOVER,
        "button.danger.background": hex6(colors::HEX_STATUS_DND),
        "button.danger.active.background": CSS_DANGER_ACTIVE,
        "button.danger.foreground": hex6(colors::HEX_WHITE),
        "button.danger.hover.background": CSS_DANGER_HOVER,
        "button.info.background": CSS_INFO,
        "button.info.active.background": CSS_INFO_ACTIVE,
        "button.info.foreground": hex6(colors::HEX_WHITE),
        "button.info.hover.background": CSS_INFO_HOVER,
        // --- status palettes ---
        "danger.background": hex6(colors::HEX_STATUS_DND),
        "danger.active.background": CSS_DANGER_ACTIVE,
        "danger.foreground": hex6(colors::HEX_WHITE),
        "danger.hover.background": CSS_DANGER_HOVER,
        "success.background": hex6(colors::HEX_STATUS_ONLINE),
        "success.active.background": CSS_SUCCESS_ACTIVE,
        "success.foreground": hex6(colors::HEX_WHITE),
        "success.hover.background": CSS_SUCCESS_HOVER,
        "warning.background": hex6(colors::HEX_STATUS_IDLE),
        "warning.active.background": CSS_WARNING_ACTIVE,
        "warning.foreground": hex6(colors::HEX_WHITE),
        "warning.hover.background": CSS_WARNING_HOVER,
        "info.background": CSS_INFO,
        "info.active.background": CSS_INFO_ACTIVE,
        "info.foreground": hex6(colors::HEX_WHITE),
        "info.hover.background": CSS_INFO_HOVER,
        // --- brand scales ---
        "primary.background": hex6(colors::HEX_BLURPLE),
        "primary.active.background": hex6(colors::HEX_BLURPLE_ACTIVE),
        "primary.foreground": hex6(colors::HEX_WHITE),
        "primary.hover.background": hex6(colors::HEX_BLURPLE_HOVER),
        "secondary.background": hex6(colors::HEX_CHANNEL_HOVER),
        "secondary.foreground": hex6(colors::HEX_WHITE),
        "secondary.hover.background": hex6(colors::HEX_CHANNEL_ACTIVE),
        "muted.background": hex6(colors::HEX_CHANNEL_HOVER),
        "muted.foreground": hex6(colors::HEX_TEXT_MUTED),
        // --- overlays / popovers ---
        "popover.background": hex6(colors::HEX_BG_FLOATING),
        "popover.foreground": hex6(colors::HEX_TEXT_PRIMARY),
        "overlay": hex8(colors::HEX_MODAL_BACKDROP),
        "drop_target.background": hex8(colors::HEX_MENTION_BG),
        "drag.border": hex6(colors::HEX_BLURPLE),
        // --- lists (channel / member rows) ---
        "list.background": hex6(colors::HEX_BG_CHANNELS),
        "list.active.background": hex6(colors::HEX_CHANNEL_ACTIVE),
        "list.even.background": hex6(colors::HEX_BG_CHANNELS),
        "list.head.background": hex6(colors::HEX_BG_CHANNELS),
        "list.hover.background": hex6(colors::HEX_CHANNEL_HOVER),
        // --- inputs / links / selection affordances ---
        "input.border": hex6(colors::HEX_BORDERS),
        "link": CSS_LINK,
        "link.active": CSS_LINK,
        "link.hover": CSS_LINK,
        "progress.bar.background": hex6(colors::HEX_BLURPLE),
        "ring": hex6(colors::HEX_BLURPLE),
        // --- scrollbar ---
        "scrollbar.thumb.background": hex6(colors::HEX_SCROLLBAR_THUMB),
        // --- sidebars / bars ---
        "sidebar.background": hex6(colors::HEX_BG_CHANNELS),
        "sidebar.accent.background": hex6(colors::HEX_CHANNEL_HOVER),
        "sidebar.accent.foreground": hex6(colors::HEX_TEXT_PRIMARY),
        "sidebar.foreground": hex6(colors::HEX_TEXT_SECONDARY),
        "sidebar.primary.background": hex6(colors::HEX_WHITE),
        "sidebar.primary.foreground": hex6(colors::HEX_BG_SERVER),
        "status_bar.background": hex6(colors::HEX_BG_USER_BAR),
        "title_bar.background": hex6(colors::HEX_BG_SERVER),
        // --- tabs (friends filter bar) ---
        "tab.foreground": hex6(colors::HEX_TEXT_SECONDARY),
        "tab.active.foreground": hex6(colors::HEX_WHITE),
        "tab_bar.background": hex6(colors::HEX_BG_CHAT),
        // --- base hues for badges / dots ---
        "base.red": hex6(colors::HEX_STATUS_DND),
        "base.red.light": CSS_RED_LIGHT,
        "base.green": hex6(colors::HEX_STATUS_ONLINE),
        "base.green.light": CSS_GREEN_LIGHT,
        "base.blue": CSS_LINK,
        "base.blue.light": CSS_BLUE_LIGHT,
        "base.yellow": hex6(colors::HEX_STATUS_IDLE),
        "base.yellow.light": CSS_WARNING_HOVER,
        // --- switches / sliders ---
        "switch.background": CSS_SWITCH_BG,
        "switch.thumb.background": CSS_SWITCH_THUMB,
        "slider.background": CSS_SLIDER_TRACK,
        // --- charts ---
        "chart.1": CSS_CHART_1,
        "chart.2": CSS_CHART_2,
        "chart.3": CSS_CHART_3,
        "chart.4": CSS_CHART_4,
        "chart.5": CSS_CHART_5,
        "chart.bullish": CSS_CHART_BULLISH,
        "chart.bearish": CSS_CHART_BEARISH,
    });

    serde_json::json!({
        "name": "Discord",
        "author": "discord-rs",
        "themes": [
            {
                "name": DISCORD_THEME_NAME,
                "mode": "dark",
                "is_default": false,
                "font.size": SIZE_BASE,
                "font.family": FONT_FAMILY_PRIMARY,
                "mono_font.family": FONT_FAMILY_CODE,
                "radius": RAW_RADIUS_MD as usize,
                "radius.lg": RAW_RADIUS_LG as usize,
                "colors": colors,
            }
        ]
    })
    .to_string()
}

// ============================================================================
// install()
// ============================================================================

/// Registers the dark `"Discord"` theme through the kit's official path
/// (`ThemeRegistry::load_themes_from_str`) and activates it
/// (`Theme::apply_config`, which also switches the mode to dark), then
/// installs the [`DiscordExtras`] global.
///
/// Must be called after `gpui::init(cx)`.
pub fn install(cx: &mut App) {
    let json = discord_theme_json();
    ThemeRegistry::global_mut(cx)
        .load_themes_from_str(&json)
        .expect("kit_theme: failed to register the Discord theme");

    let config: Rc<ThemeConfig> = ThemeRegistry::global(cx)
        .themes()
        .get(DISCORD_THEME_NAME)
        .cloned()
        .expect("kit_theme: Discord theme missing after registration");

    Theme::update(cx, |theme| {
        theme.apply_config(&config);
    });

    cx.set_global(DiscordExtras::current());
}

// ============================================================================
// DiscordExtras: Discord surfaces with no kit equivalent
// ============================================================================

/// Discord-specific colors with no equivalent kit theme field.
///
/// Installed as a [`gpui::Global`] by [`install`]; read it via
/// [`DiscordTheme::discord`] (`cx.discord().bg_server`, ...) so screens never
/// hard-code `rgb(0x...)`. Every field is built from the existing
/// [`crate::tokens`] accessor functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscordExtras {
    /// Server rail background.
    pub bg_server: Rgba,
    /// Channel sidebar background.
    pub bg_channels: Rgba,
    /// Main chat area background.
    pub bg_chat: Rgba,
    /// Chat message hover background.
    pub bg_chat_hover: Rgba,
    /// Bottom user panel background.
    pub bg_user_bar: Rgba,
    /// Chat input background.
    pub bg_input: Rgba,
    /// Popout / context menu background.
    pub bg_floating: Rgba,
    /// Brand blurple and its states.
    pub blurple: Rgba,
    pub blurple_hover: Rgba,
    pub blurple_active: Rgba,
    /// Presence statuses.
    pub status_online: Rgba,
    pub status_idle: Rgba,
    pub status_dnd: Rgba,
    pub status_offline: Rgba,
    /// Text tiers.
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_titles: Rgba,
    /// Borders / dividers.
    pub borders: Rgba,
    /// Channel row states.
    pub channel_hover: Rgba,
    pub channel_active: Rgba,
    /// Mention highlight washes.
    pub mention_bg: Rgba,
    pub mention_hover: Rgba,
    /// Modal backdrop scrim.
    pub modal_backdrop: Rgba,
    /// Scrollbar thumb.
    pub scrollbar_thumb: Rgba,
    /// Pure white (badges, pills).
    pub white: Rgba,
    /// Neutral 67 surface / border (#35363c).
    pub neutral_67: Rgba,
    /// Transparent color (#00000000).
    pub transparent: Rgba,
    /// White at ~8% opacity (#ffffff14).
    pub opacity_white_8: Rgba,
    /// Control primary border alias (#ffffff14).
    pub control_primary_border: Rgba,
    /// White at 20% opacity (#ffffff33).
    pub opacity_white_20: Rgba,
    /// Nitro Tier 2 Purple for gradients (#8547c6).
    pub premium_tier_2_purple: Rgba,
    /// Animated gradient background pink stop (#eb459e).
    pub bg_animated_gradient_pink_1: Rgba,
    /// Platform Xbox brand green (#107c10).
    pub platform_xbox: Rgba,
    /// Platform Spotify brand green (#1db954).
    pub platform_spotify: Rgba,
    /// Role color purple (#9b59b6).
    pub role_purple: Rgba,
    /// Platform Reddit brand red/orange (#ff4500).
    pub platform_reddit: Rgba,
    /// Secondary control background default (#97979f1f).
    pub control_secondary_bg_default: Rgba,
    /// Secondary control background hover (#97979f29).
    pub control_secondary_bg_hover: Rgba,
    /// Secondary control background active (#97979f33).
    pub control_secondary_bg_active: Rgba,
    /// Control critical primary background default (#d22d39).
    pub control_critical_primary_bg_default: Rgba,
    /// Control critical primary background hover (#a9232e).
    pub control_critical_primary_bg_hover: Rgba,
    /// Control critical primary background active (#971d28).
    pub control_critical_primary_bg_active: Rgba,
    /// Control critical secondary text default (#f87e7a).
    pub control_critical_secondary_text_default: Rgba,
    /// Control icon-only icon default (#abacb2).
    pub control_icon_only_icon_default: Rgba,
}

impl Global for DiscordExtras {}

impl DiscordExtras {
    /// Builds the extras from the canonical token accessors.
    pub fn current() -> Self {
        Self {
            bg_server: colors::bg_server(),
            bg_channels: colors::bg_channels(),
            bg_chat: colors::bg_chat(),
            bg_chat_hover: colors::bg_chat_hover(),
            bg_user_bar: colors::bg_user_bar(),
            bg_input: colors::bg_input(),
            bg_floating: colors::bg_floating(),
            blurple: colors::blurple(),
            blurple_hover: colors::blurple_hover(),
            blurple_active: colors::blurple_active(),
            status_online: colors::status_online(),
            status_idle: colors::status_idle(),
            status_dnd: colors::status_dnd(),
            status_offline: colors::status_offline(),
            text_primary: colors::text_primary(),
            text_secondary: colors::text_secondary(),
            text_titles: colors::text_titles(),
            borders: colors::borders(),
            channel_hover: colors::channel_hover(),
            channel_active: colors::channel_active(),
            mention_bg: colors::mention_bg(),
            mention_hover: colors::mention_hover(),
            modal_backdrop: colors::modal_backdrop(),
            scrollbar_thumb: colors::scrollbar_thumb(),
            white: colors::white(),
            neutral_67: colors::neutral_67(),
            transparent: colors::transparent(),
            opacity_white_8: colors::opacity_white_8(),
            control_primary_border: colors::control_primary_border(),
            opacity_white_20: colors::opacity_white_20(),
            premium_tier_2_purple: colors::premium_tier_2_purple(),
            bg_animated_gradient_pink_1: colors::bg_animated_gradient_pink_1(),
            platform_xbox: colors::platform_xbox(),
            platform_spotify: colors::platform_spotify(),
            role_purple: colors::role_purple(),
            platform_reddit: colors::platform_reddit(),
            control_secondary_bg_default: colors::control_secondary_bg_default(),
            control_secondary_bg_hover: colors::control_secondary_bg_hover(),
            control_secondary_bg_active: colors::control_secondary_bg_active(),
            control_critical_primary_bg_default: colors::control_critical_primary_bg_default(),
            control_critical_primary_bg_hover: colors::control_critical_primary_bg_hover(),
            control_critical_primary_bg_active: colors::control_critical_primary_bg_active(),
            control_critical_secondary_text_default: colors::control_critical_secondary_text_default(),
            control_icon_only_icon_default: colors::control_icon_only_icon_default(),
        }
    }

    /// Reads the extras installed by [`install`].
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }
}

/// Extension trait over [`App`] so screens read Discord colors via the theme.
pub trait DiscordTheme {
    /// Returns the [`DiscordExtras`] installed by [`install`].
    fn discord(&self) -> &DiscordExtras;
}

impl DiscordTheme for App {
    fn discord(&self) -> &DiscordExtras {
        DiscordExtras::global(self)
    }
}
