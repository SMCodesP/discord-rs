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
use gpui::{App, Global, Rgba, rgb, rgba};

use crate::domain::settings::ThemeChoice;

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
    discord_theme_json_for_palette(&colors::DiscordPalette::dark(), ThemeChoice::Dark)
}

/// Formats an [`Rgba`] as the `#rrggbb` (opaque) or `#rrggbbaa` (alpha) string
/// the kit theme loader expects.
fn rgba_hex(color: Rgba) -> String {
    let r = (color.r.clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (color.g.clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (color.b.clamp(0.0, 1.0) * 255.0).round() as u32;
    let a = (color.a.clamp(0.0, 1.0) * 255.0).round() as u32;
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// Builds the `ThemeSet` JSON for a concrete palette and theme choice.
///
/// Every surface/text color comes from the palette, so switching Dark, Light
/// or Midnight re-registers the same `"Discord"` theme with the new values.
pub fn discord_theme_json_for_palette(
    palette: &colors::DiscordPalette,
    choice: ThemeChoice,
) -> String {
    let color = |c: Rgba| rgba_hex(c);
    let colors = serde_json::json!({
        // --- surfaces / text ---
        "background": color(palette.bg_chat),
        "foreground": color(palette.text_primary),
        "accent.background": color(palette.channel_hover),
        "accent.foreground": color(palette.white),
        "accordion.background": color(palette.bg_channels),
        "border": color(palette.borders),
        // --- default button (secondary style) ---
        "button.background": color(palette.channel_hover),
        "button.hover.background": color(palette.channel_active),
        "button.foreground": color(palette.white),
        // --- primary / brand button ---
        "button.primary.background": color(palette.blurple),
        "button.primary.active.background": color(palette.blurple_active),
        "button.primary.foreground": color(palette.white),
        "button.primary.hover.background": color(palette.blurple_hover),
        // --- secondary button ---
        "button.secondary.background": color(palette.channel_hover),
        "button.secondary.foreground": color(palette.white),
        "button.secondary.hover.background": color(palette.channel_active),
        // --- success / warning / danger / info buttons ---
        "button.success.background": color(palette.status_online),
        "button.success.active.background": CSS_SUCCESS_ACTIVE,
        "button.success.foreground": color(palette.white),
        "button.success.hover.background": CSS_SUCCESS_HOVER,
        "button.warning.background": color(palette.status_ausente),
        "button.warning.active.background": CSS_WARNING_ACTIVE,
        "button.warning.foreground": color(palette.white),
        "button.warning.hover.background": CSS_WARNING_HOVER,
        "button.danger.background": color(palette.status_dnd),
        "button.danger.active.background": CSS_DANGER_ACTIVE,
        "button.danger.foreground": color(palette.white),
        "button.danger.hover.background": CSS_DANGER_HOVER,
        "button.info.background": CSS_INFO,
        "button.info.active.background": CSS_INFO_ACTIVE,
        "button.info.foreground": color(palette.white),
        "button.info.hover.background": CSS_INFO_HOVER,
        // --- status palettes ---
        "danger.background": color(palette.status_dnd),
        "danger.active.background": CSS_DANGER_ACTIVE,
        "danger.foreground": color(palette.white),
        "danger.hover.background": CSS_DANGER_HOVER,
        "success.background": color(palette.status_online),
        "success.active.background": CSS_SUCCESS_ACTIVE,
        "success.foreground": color(palette.white),
        "success.hover.background": CSS_SUCCESS_HOVER,
        "warning.background": color(palette.status_ausente),
        "warning.active.background": CSS_WARNING_ACTIVE,
        "warning.foreground": color(palette.white),
        "warning.hover.background": CSS_WARNING_HOVER,
        "info.background": CSS_INFO,
        "info.active.background": CSS_INFO_ACTIVE,
        "info.foreground": color(palette.white),
        "info.hover.background": CSS_INFO_HOVER,
        // --- brand scales ---
        "primary.background": color(palette.blurple),
        "primary.active.background": color(palette.blurple_active),
        "primary.foreground": color(palette.white),
        "primary.hover.background": color(palette.blurple_hover),
        "secondary.background": color(palette.channel_hover),
        "secondary.foreground": color(palette.white),
        "secondary.hover.background": color(palette.channel_active),
        "muted.background": color(palette.channel_hover),
        "muted.foreground": color(palette.text_muted),
        // --- overlays / popovers ---
        "popover.background": color(palette.bg_floating),
        "popover.foreground": color(palette.text_primary),
        "overlay": color(palette.modal_backdrop),
        "drop_target.background": color(palette.mention_bg),
        "drag.border": color(palette.blurple),
        // --- lists (channel / member rows) ---
        "list.background": color(palette.bg_channels),
        "list.active.background": color(palette.channel_active),
        "list.even.background": color(palette.bg_channels),
        "list.head.background": color(palette.bg_channels),
        "list.hover.background": color(palette.channel_hover),
        // --- inputs / links / selection affordances ---
        "input.border": color(palette.borders),
        "link": CSS_LINK,
        "link.active": CSS_LINK,
        "link.hover": CSS_LINK,
        "progress.bar.background": color(palette.blurple),
        "ring": color(palette.blurple),
        // --- scrollbar ---
        "scrollbar.thumb.background": color(palette.scrollbar_thumb),
        // --- sidebars / bars ---
        "sidebar.background": color(palette.bg_channels),
        "sidebar.accent.background": color(palette.channel_hover),
        "sidebar.accent.foreground": color(palette.text_primary),
        "sidebar.foreground": color(palette.text_secondary),
        "sidebar.primary.background": color(palette.white),
        "sidebar.primary.foreground": color(palette.bg_server),
        "status_bar.background": color(palette.bg_user_bar),
        "title_bar.background": color(palette.bg_server),
        // --- tabs (friends filter bar) ---
        "tab.foreground": color(palette.text_secondary),
        "tab.active.foreground": color(palette.white),
        "tab_bar.background": color(palette.bg_chat),
        // --- base hues for badges / dots ---
        "base.red": color(palette.status_dnd),
        "base.red.light": CSS_RED_LIGHT,
        "base.green": color(palette.status_online),
        "base.green.light": CSS_GREEN_LIGHT,
        "base.blue": CSS_LINK,
        "base.blue.light": CSS_BLUE_LIGHT,
        "base.yellow": color(palette.status_ausente),
        "base.yellow.light": CSS_WARNING_HOVER,
        // --- switches / sliders ---
        "switch.background": CSS_SWITCH_BG,
        "switch.thumb.background": color(palette.white),
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

    let mode = if choice == ThemeChoice::Light { "light" } else { "dark" };

    serde_json::json!({
        "name": "Discord",
        "author": "discord-rs",
        "themes": [
            {
                "name": DISCORD_THEME_NAME,
                "mode": mode,
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

/// Builds the palette for a user-facing theme choice.
///
/// Dark is the audited reference palette in [`colors::DiscordPalette::dark`].
/// Light and Midnight use the base backgrounds documented in
/// `ThemeChoice::bg_hex` / `docs/SCOPING_USER_SETTINGS_ADVANCED.md` (Light
/// `#ffffff`, Midnight OLED `#000000`) and the corresponding Discord light
/// surface/text stops. `SyncWithSystem` has no DBus probe in a headless
/// process, so it resolves to Dark instead of fabricating a selection.
pub fn palette_for_theme(choice: ThemeChoice) -> colors::DiscordPalette {
    let mut palette = colors::DiscordPalette::dark();
    match choice {
        ThemeChoice::Dark | ThemeChoice::SyncWithSystem => {}
        ThemeChoice::Light => {
            // Discord light theme surfaces/text stops.
            palette.bg_server = rgb(0xe3e5e8);
            palette.bg_channels = rgb(0xf2f3f5);
            palette.bg_chat = rgb(0xffffff);
            palette.bg_chat_hover = rgb(0xf2f3f5);
            palette.bg_user_bar = rgb(0xf2f3f5);
            palette.bg_input = rgb(0xebedef);
            palette.bg_floating = rgb(0xffffff);
            palette.text_primary = rgb(0x313338);
            palette.text_secondary = rgb(0x5c5e66);
            palette.text_titles = rgb(0x060607);
            palette.text_muted = rgb(0x80848e);
            palette.borders = rgb(0xe3e5e8);
            palette.border_subtle = rgba(0x0000001f);
            palette.channel_hover = rgb(0xe9eaec);
            palette.channel_active = rgb(0xe3e5e8);
            palette.channels_default = rgb(0x5c5e66);
            palette.interactive_normal = rgb(0x313338);
            palette.scrollbar_thumb = rgb(0xb5bac1);
            palette.neutral_67 = rgb(0xdcdee1);
            palette.mention_foreground = rgb(0x4752c4);
            palette.reaction_text_default = rgb(0x313338);
            palette.reaction_text_reacted_default = rgb(0x4752c4);
            palette.legacy_949ba4 = rgb(0x5c5e66);
            palette.legacy_282a2e = rgb(0xe9eaec);
            palette.legacy_3f4147 = rgb(0xe3e5e8);
        }
        ThemeChoice::Midnight => {
            // OLED surfaces: pure black base, raised stops keep Discord's
            // dark text palette.
            palette.bg_server = rgb(0x000000);
            palette.bg_channels = rgb(0x000000);
            palette.bg_chat = rgb(0x000000);
            palette.bg_chat_hover = rgb(0x1a1b1e);
            palette.bg_user_bar = rgb(0x000000);
            palette.bg_input = rgb(0x111214);
            palette.bg_floating = rgb(0x111214);
            palette.borders = rgb(0x1f2023);
            palette.border_subtle = rgba(0xffffff14);
            palette.channel_hover = rgb(0x1a1b1e);
            palette.channel_active = rgb(0x2b2d31);
            palette.scrollbar_thumb = rgb(0x1a1b1e);
            palette.neutral_67 = rgb(0x1f2023);
        }
    }
    palette
}

// ============================================================================
// install()
// ============================================================================

/// Registers the `"Discord"` theme with the given palette.
///
/// Re-registering the same name replaces the previous theme data, so this is
/// the path used by live Dark/Light/Midnight switching.
pub fn install_with_palette(cx: &mut App, palette: &colors::DiscordPalette) {
    install_with_palette_and_choice(cx, palette, ThemeChoice::Dark);
}

fn install_with_palette_and_choice(
    cx: &mut App,
    palette: &colors::DiscordPalette,
    choice: ThemeChoice,
) {
    let json = discord_theme_json_for_palette(palette, choice);
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

    cx.set_global(DiscordExtras::from_palette(palette));
}

/// Registers the theme for a concrete Dark/Light/Midnight choice.
pub fn install_for_theme(cx: &mut App, choice: ThemeChoice) {
    let palette = palette_for_theme(choice);
    install_with_palette_and_choice(cx, &palette, choice);
    cx.set_global(AppliedTheme(choice));
}

/// Last theme choice applied to the kit, so observers can avoid re-registering
/// the same theme on every notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedTheme(pub ThemeChoice);

impl Global for AppliedTheme {}

/// Applies a theme only when it differs from the last applied choice.
pub fn apply_theme_choice(cx: &mut App, choice: ThemeChoice) {
    if cx.try_global::<AppliedTheme>().copied() == Some(AppliedTheme(choice)) {
        return;
    }
    install_for_theme(cx, choice);
}

/// Registers the dark `"Discord"` theme through the kit's official path
/// (`ThemeRegistry::load_themes_from_str`) with the standard dark palette.
/// Must be called after `gpui::init(cx)`.
pub fn install(cx: &mut App) {
    let palette = colors::DiscordPalette::dark();
    install_with_palette_and_choice(cx, &palette, ThemeChoice::Dark);
    cx.set_global(AppliedTheme(ThemeChoice::Dark));
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
    /// Mention foreground (#cdd7ff).
    pub mention_foreground: Rgba,
    /// Border subtle (#97979f1f).
    pub border_subtle: Rgba,
    /// Channels default text (#999aa1).
    pub channels_default: Rgba,
    /// Interactive normal text (#c5c6ca).
    pub interactive_normal: Rgba,
    /// Reaction text default (#c5c6ca).
    pub reaction_text_default: Rgba,
    /// Reaction text reacted (#dfe4ff).
    pub reaction_text_reacted_default: Rgba,
    /// Status danger red (#da3e44).
    pub status_danger: Rgba,
    /// Status positive green (#3d9e60).
    pub status_positive: Rgba,
    /// Status warning yellow (#fdb833).
    pub status_warning: Rgba,
    /// Badge brand bubble pink stop (#eb459e).
    pub badge_brand_bubble: Rgba,
    /// Muted text alias (#80848e).
    pub text_muted: Rgba,
    /// COR ANTIGA SEM ORIGEM no Discord atual: header secondary (#949ba4).
    pub legacy_949ba4: Rgba,
    /// COR ANTIGA SEM ORIGEM no Discord atual: background modifier hover (#282a2e).
    pub legacy_282a2e: Rgba,
    /// COR ANTIGA SEM ORIGEM no Discord atual: background modifier accent (#3f4147).
    pub legacy_3f4147: Rgba,
    /// COR ANTIGA SEM ORIGEM no Discord atual: text danger (#fa777c).
    pub legacy_fa777c: Rgba,
    /// COR ANTIGA SEM ORIGEM no Discord atual: button danger background hover (#da373c).
    pub legacy_da373c: Rgba,
    /// COR ANTIGA SEM ORIGEM no Discord atual: text warning (#fee75c).
    pub legacy_fee75c: Rgba,
}

impl Global for DiscordExtras {}

impl DiscordExtras {
    /// Builds the extras from a given palette.
    pub fn from_palette(p: &colors::DiscordPalette) -> Self {
        Self {
            bg_server: p.bg_server,
            bg_channels: p.bg_channels,
            bg_chat: p.bg_chat,
            bg_chat_hover: p.bg_chat_hover,
            bg_user_bar: p.bg_user_bar,
            bg_input: p.bg_input,
            bg_floating: p.bg_floating,
            blurple: p.blurple,
            blurple_hover: p.blurple_hover,
            blurple_active: p.blurple_active,
            status_online: p.status_online,
            status_idle: p.status_ausente,
            status_dnd: p.status_dnd,
            status_offline: p.status_offline,
            text_primary: p.text_primary,
            text_secondary: p.text_secondary,
            text_titles: p.text_titles,
            borders: p.borders,
            channel_hover: p.channel_hover,
            channel_active: p.channel_active,
            mention_bg: p.mention_bg,
            mention_hover: p.mention_hover,
            modal_backdrop: p.modal_backdrop,
            scrollbar_thumb: p.scrollbar_thumb,
            white: p.white,
            neutral_67: p.neutral_67,
            transparent: p.transparent,
            opacity_white_8: p.opacity_white_8,
            control_primary_border: p.control_primary_border,
            opacity_white_20: p.opacity_white_20,
            premium_tier_2_purple: p.premium_tier_2_purple,
            bg_animated_gradient_pink_1: p.bg_animated_gradient_pink_1,
            platform_xbox: p.platform_xbox,
            platform_spotify: p.platform_spotify,
            role_purple: p.role_purple,
            platform_reddit: p.platform_reddit,
            control_secondary_bg_default: p.control_secondary_bg_default,
            control_secondary_bg_hover: p.control_secondary_bg_hover,
            control_secondary_bg_active: p.control_secondary_bg_active,
            control_critical_primary_bg_default: p.control_critical_primary_bg_default,
            control_critical_primary_bg_hover: p.control_critical_primary_bg_hover,
            control_critical_primary_bg_active: p.control_critical_primary_bg_active,
            control_critical_secondary_text_default: p.control_critical_secondary_text_default,
            control_icon_only_icon_default: p.control_icon_only_icon_default,
            mention_foreground: p.mention_foreground,
            border_subtle: p.border_subtle,
            channels_default: p.channels_default,
            interactive_normal: p.interactive_normal,
            reaction_text_default: p.reaction_text_default,
            reaction_text_reacted_default: p.reaction_text_reacted_default,
            status_danger: p.status_danger,
            status_positive: p.status_positive,
            status_warning: p.status_warning,
            badge_brand_bubble: p.badge_brand_bubble,
            legacy_949ba4: p.legacy_949ba4,
            legacy_282a2e: p.legacy_282a2e,
            legacy_3f4147: p.legacy_3f4147,
            legacy_fa777c: p.legacy_fa777c,
            legacy_da373c: p.legacy_da373c,
            legacy_fee75c: p.legacy_fee75c,
            text_muted: p.text_muted,
        }
    }

    /// Builds the extras from the active canonical token palette.
    pub fn current() -> Self {
        Self::from_palette(&colors::active_palette())
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
