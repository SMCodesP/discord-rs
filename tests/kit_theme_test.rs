//! KTheme tests: the Discord theme mapping exercised for real.
//!
//! NOTE: a full `install()` round-trip inside a live `gpui::App` needs the
//! kit's `test-support` feature (`TestAppContext`), which this crate does not
//! enable. The exact `Cargo.toml` addition required is reported in
//! `/tmp/discord-rs-report-KTheme.md`; `Cargo.toml` was deliberately NOT
//! touched. What IS exercised here, with no mocks:
//!
//! * `discord_theme_json()` — the exact string `install()` feeds to the
//!   official `ThemeRegistry::load_themes_from_str` — parses as a kit
//!   `ThemeSet` with one dark `"Discord"` theme;
//! * every key color in that payload equals the corresponding `src/tokens`
//!   value (re-derived independently in the test) or the documented live
//!   Discord CSS variable;
//! * `DiscordExtras::current()` (the struct `install()` installs as a global)
//!   matches the canonical token accessors.

extern crate gpui_kit as gpui;

use discord_rs::tokens::kit_theme::{
    self, CSS_BLUE_LIGHT, CSS_DANGER_ACTIVE, CSS_DANGER_HOVER, CSS_GREEN_LIGHT, CSS_INFO,
    CSS_INFO_ACTIVE, CSS_INFO_HOVER, CSS_LINK, CSS_RED_LIGHT, CSS_SUCCESS_ACTIVE, CSS_SUCCESS_HOVER,
    CSS_WARNING_ACTIVE, CSS_WARNING_HOVER, DISCORD_THEME_NAME, DiscordExtras,
};
use discord_rs::tokens::{self as tokens, FONT_FAMILY_CODE, FONT_FAMILY_PRIMARY};

fn theme_json_value() -> serde_json::Value {
    let json = kit_theme::discord_theme_json();
    serde_json::from_str(&json).expect("discord_theme_json must be valid JSON")
}

fn color_of(v: &serde_json::Value, key: &str) -> String {
    v["themes"][0]["colors"][key]
        .as_str()
        .unwrap_or_else(|| panic!("theme colors must contain {key}"))
        .to_owned()
}

#[test]
fn discord_theme_json_parses_as_kit_theme_set() {
    let json = kit_theme::discord_theme_json();
    let set: gpui::component::ThemeSet =
        serde_json::from_str(&json).expect("payload must parse as kit ThemeSet");
    assert_eq!(set.themes.len(), 1);
    let theme = &set.themes[0];
    assert_eq!(theme.name.as_ref(), DISCORD_THEME_NAME);
    assert!(theme.mode.is_dark(), "Discord theme must be dark");
    assert!(!theme.is_default, "must not override kit defaults");

    // Scalars come from tokens, not literals.
    assert_eq!(theme.font_size, Some(tokens::SIZE_BASE));
    assert_eq!(
        theme.font_family.as_deref(),
        Some(tokens::FONT_FAMILY_PRIMARY)
    );
    assert_eq!(
        theme.mono_font_family.as_deref(),
        Some(tokens::FONT_FAMILY_CODE)
    );
    assert_eq!(theme.radius, Some(tokens::RAW_RADIUS_MD as usize));
    assert_eq!(theme.radius_lg, Some(tokens::RAW_RADIUS_LG as usize));

    // Re-assert via raw JSON too (font family strings travel verbatim).
    let v = theme_json_value();
    assert_eq!(v["themes"][0]["font.family"], FONT_FAMILY_PRIMARY);
    assert_eq!(v["themes"][0]["mono_font.family"], FONT_FAMILY_CODE);
}

#[test]
fn key_theme_colors_match_tokens() {
    let v = theme_json_value();
    let hex6 = |h: u32| format!("#{h:06x}");
    let hex8 = |h: u32| format!("#{h:08x}");

    // Surfaces / text / borders.
    assert_eq!(color_of(&v, "background"), hex6(tokens::HEX_BG_CHAT));
    assert_eq!(color_of(&v, "foreground"), hex6(tokens::HEX_TEXT_PRIMARY));
    assert_eq!(
        color_of(&v, "accent.background"),
        hex6(tokens::HEX_CHANNEL_HOVER)
    );
    assert_eq!(
        color_of(&v, "accordion.background"),
        hex6(tokens::HEX_BG_CHANNELS)
    );
    assert_eq!(color_of(&v, "border"), hex6(tokens::HEX_BORDERS));
    assert_eq!(color_of(&v, "input.border"), hex6(tokens::HEX_BORDERS));

    // Brand / primary.
    assert_eq!(
        color_of(&v, "primary.background"),
        hex6(tokens::HEX_BLURPLE)
    );
    assert_eq!(
        color_of(&v, "primary.hover.background"),
        hex6(tokens::HEX_BLURPLE_HOVER)
    );
    assert_eq!(
        color_of(&v, "primary.active.background"),
        hex6(tokens::HEX_BLURPLE_ACTIVE)
    );
    assert_eq!(
        color_of(&v, "button.primary.background"),
        hex6(tokens::HEX_BLURPLE)
    );

    // Status palettes (bases are tokens; states are live Discord CSS).
    assert_eq!(
        color_of(&v, "success.background"),
        hex6(tokens::HEX_STATUS_ONLINE)
    );
    assert_eq!(color_of(&v, "success.hover.background"), CSS_SUCCESS_HOVER);
    assert_eq!(
        color_of(&v, "success.active.background"),
        CSS_SUCCESS_ACTIVE
    );
    assert_eq!(
        color_of(&v, "warning.background"),
        hex6(tokens::HEX_STATUS_IDLE)
    );
    assert_eq!(color_of(&v, "warning.hover.background"), CSS_WARNING_HOVER);
    assert_eq!(
        color_of(&v, "warning.active.background"),
        CSS_WARNING_ACTIVE
    );
    assert_eq!(
        color_of(&v, "danger.background"),
        hex6(tokens::HEX_STATUS_DND)
    );
    assert_eq!(color_of(&v, "danger.hover.background"), CSS_DANGER_HOVER);
    assert_eq!(
        color_of(&v, "danger.active.background"),
        CSS_DANGER_ACTIVE
    );
    assert_eq!(color_of(&v, "info.background"), CSS_INFO);
    assert_eq!(color_of(&v, "info.hover.background"), CSS_INFO_HOVER);
    assert_eq!(color_of(&v, "info.active.background"), CSS_INFO_ACTIVE);

    // Links (live Discord --text-link; hover/active keep the color).
    assert_eq!(color_of(&v, "link"), CSS_LINK);
    assert_eq!(color_of(&v, "link.hover"), CSS_LINK);
    assert_eq!(color_of(&v, "link.active"), CSS_LINK);

    // Lists / sidebars / bars.
    assert_eq!(
        color_of(&v, "list.background"),
        hex6(tokens::HEX_BG_CHANNELS)
    );
    assert_eq!(
        color_of(&v, "list.active.background"),
        hex6(tokens::HEX_CHANNEL_ACTIVE)
    );
    assert_eq!(
        color_of(&v, "sidebar.background"),
        hex6(tokens::HEX_BG_CHANNELS)
    );
    assert_eq!(
        color_of(&v, "status_bar.background"),
        hex6(tokens::HEX_BG_USER_BAR)
    );
    assert_eq!(
        color_of(&v, "title_bar.background"),
        hex6(tokens::HEX_BG_SERVER)
    );
    assert_eq!(
        color_of(&v, "popover.background"),
        hex6(tokens::HEX_BG_FLOATING)
    );
    assert_eq!(color_of(&v, "overlay"), hex8(tokens::HEX_MODAL_BACKDROP));
    assert_eq!(
        color_of(&v, "drop_target.background"),
        hex8(tokens::HEX_MENTION_BG)
    );
    assert_eq!(
        color_of(&v, "scrollbar.thumb.background"),
        hex6(tokens::HEX_SCROLLBAR_THUMB)
    );

    // Base hues.
    assert_eq!(color_of(&v, "base.red"), hex6(tokens::HEX_STATUS_DND));
    assert_eq!(color_of(&v, "base.red.light"), CSS_RED_LIGHT);
    assert_eq!(
        color_of(&v, "base.green"),
        hex6(tokens::HEX_STATUS_ONLINE)
    );
    assert_eq!(color_of(&v, "base.green.light"), CSS_GREEN_LIGHT);
    assert_eq!(color_of(&v, "base.blue"), CSS_LINK);
    assert_eq!(color_of(&v, "base.blue.light"), CSS_BLUE_LIGHT);
    assert_eq!(
        color_of(&v, "base.yellow"),
        hex6(tokens::HEX_STATUS_IDLE)
    );
}

#[test]
fn fields_without_discord_source_are_left_to_kit_defaults() {
    let v = theme_json_value();
    let colors = &v["themes"][0]["colors"];
    // No Discord surface exists for these; omitting keeps the kit dark default.
    for key in [
        "caret",
        "chart.1",
        "chart.2",
        "chart.3",
        "chart.4",
        "chart.5",
        "chart.bullish",
        "chart.bearish",
        "chart.grid",
        "table.background",
        "table.hover.background",
        "group_box.background",
        "description_list.label.background",
        "skeleton.background",
        "switch.background",
        "switch.thumb.background",
        "slider.background",
        "slider.thumb.background",
        "selection.background",
        "scrollbar.thumb.hover.background",
        "base.magenta",
        "base.cyan",
    ] {
        assert!(
            colors.get(key).is_none(),
            "{key} must be omitted (kit default)"
        );
    }
}

#[test]
fn discord_extras_match_token_accessors() {
    let ex = DiscordExtras::current();
    assert_eq!(ex.bg_server, tokens::bg_server());
    assert_eq!(ex.bg_channels, tokens::bg_channels());
    assert_eq!(ex.bg_chat, tokens::bg_chat());
    assert_eq!(ex.bg_chat_hover, tokens::bg_chat_hover());
    assert_eq!(ex.bg_user_bar, tokens::bg_user_bar());
    assert_eq!(ex.bg_input, tokens::bg_input());
    assert_eq!(ex.bg_floating, tokens::bg_floating());
    assert_eq!(ex.blurple, tokens::blurple());
    assert_eq!(ex.blurple_hover, tokens::blurple_hover());
    assert_eq!(ex.blurple_active, tokens::blurple_active());
    assert_eq!(ex.status_online, tokens::status_online());
    assert_eq!(ex.status_idle, tokens::status_idle());
    assert_eq!(ex.status_dnd, tokens::status_dnd());
    assert_eq!(ex.status_offline, tokens::status_offline());
    assert_eq!(ex.text_primary, tokens::text_primary());
    assert_eq!(ex.text_secondary, tokens::text_secondary());
    assert_eq!(ex.text_titles, tokens::text_titles());
    assert_eq!(ex.borders, tokens::borders());
    assert_eq!(ex.channel_hover, tokens::channel_hover());
    assert_eq!(ex.channel_active, tokens::channel_active());
    assert_eq!(ex.mention_bg, tokens::mention_bg());
    assert_eq!(ex.mention_hover, tokens::mention_hover());
    assert_eq!(ex.modal_backdrop, tokens::modal_backdrop());
    assert_eq!(ex.scrollbar_thumb, tokens::scrollbar_thumb());
    assert_eq!(ex.white, tokens::white());
}
