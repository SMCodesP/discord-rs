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
    self, CSS_BLUE_LIGHT, CSS_CHART_1, CSS_CHART_2, CSS_CHART_3, CSS_CHART_4, CSS_CHART_5,
    CSS_CHART_BEARISH, CSS_CHART_BULLISH, CSS_DANGER_ACTIVE, CSS_DANGER_HOVER, CSS_GREEN_LIGHT,
    CSS_INFO, CSS_INFO_ACTIVE, CSS_INFO_HOVER, CSS_LINK, CSS_RED_LIGHT, CSS_SLIDER_TRACK,
    CSS_SUCCESS_ACTIVE, CSS_SUCCESS_HOVER, CSS_SWITCH_BG, CSS_SWITCH_THUMB, CSS_WARNING_ACTIVE,
    CSS_WARNING_HOVER, DISCORD_THEME_NAME, DiscordExtras,
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

    // Switches / sliders.
    assert_eq!(color_of(&v, "switch.background"), CSS_SWITCH_BG);
    assert_eq!(color_of(&v, "switch.thumb.background"), CSS_SWITCH_THUMB);
    assert_eq!(color_of(&v, "slider.background"), CSS_SLIDER_TRACK);

    // Charts.
    assert_eq!(color_of(&v, "chart.1"), CSS_CHART_1);
    assert_eq!(color_of(&v, "chart.2"), CSS_CHART_2);
    assert_eq!(color_of(&v, "chart.3"), CSS_CHART_3);
    assert_eq!(color_of(&v, "chart.4"), CSS_CHART_4);
    assert_eq!(color_of(&v, "chart.5"), CSS_CHART_5);
    assert_eq!(color_of(&v, "chart.bullish"), CSS_CHART_BULLISH);
    assert_eq!(color_of(&v, "chart.bearish"), CSS_CHART_BEARISH);
}

#[test]
fn fields_without_discord_source_are_left_to_kit_defaults() {
    let v = theme_json_value();
    let colors = &v["themes"][0]["colors"];
    // No proven Discord surface exists for these; omitting keeps the kit dark default.
    for key in [
        "caret",
        "chart.grid",
        "table.background",
        "table.hover.background",
        "group_box.background",
        "description_list.label.background",
        "skeleton.background",
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
    assert_eq!(ex.neutral_67, tokens::neutral_67());
    assert_eq!(ex.transparent, tokens::transparent());
    assert_eq!(ex.opacity_white_8, tokens::opacity_white_8());
    assert_eq!(ex.control_primary_border, tokens::control_primary_border());
    assert_eq!(ex.opacity_white_20, tokens::opacity_white_20());
    assert_eq!(ex.premium_tier_2_purple, tokens::premium_tier_2_purple());
    assert_eq!(ex.bg_animated_gradient_pink_1, tokens::bg_animated_gradient_pink_1());
    assert_eq!(ex.platform_xbox, tokens::platform_xbox());
    assert_eq!(ex.platform_spotify, tokens::platform_spotify());
    assert_eq!(ex.role_purple, tokens::role_purple());
    assert_eq!(ex.platform_reddit, tokens::platform_reddit());
    assert_eq!(ex.control_secondary_bg_default, tokens::control_secondary_bg_default());
    assert_eq!(ex.control_secondary_bg_hover, tokens::control_secondary_bg_hover());
    assert_eq!(ex.control_secondary_bg_active, tokens::control_secondary_bg_active());
    assert_eq!(ex.control_critical_primary_bg_default, tokens::control_critical_primary_bg_default());
    assert_eq!(ex.control_critical_primary_bg_hover, tokens::control_critical_primary_bg_hover());
    assert_eq!(ex.control_critical_primary_bg_active, tokens::control_critical_primary_bg_active());
    assert_eq!(ex.control_critical_secondary_text_default, tokens::control_critical_secondary_text_default());
    assert_eq!(ex.control_icon_only_icon_default, tokens::control_icon_only_icon_default());
    assert_eq!(ex.mention_foreground, tokens::mention_foreground());
    assert_eq!(ex.border_subtle, tokens::border_subtle());
    assert_eq!(ex.channels_default, tokens::channels_default());
    assert_eq!(ex.interactive_normal, tokens::interactive_normal());
    assert_eq!(ex.reaction_text_default, tokens::reaction_text_default());
    assert_eq!(ex.reaction_text_reacted_default, tokens::reaction_text_reacted_default());
    assert_eq!(ex.status_danger, tokens::status_danger());
    assert_eq!(ex.status_positive, tokens::status_positive());
    assert_eq!(ex.status_warning, tokens::status_warning());
    assert_eq!(ex.badge_brand_bubble, tokens::badge_brand_bubble());
    assert_eq!(ex.legacy_949ba4, tokens::legacy_949ba4());
    assert_eq!(ex.legacy_282a2e, tokens::legacy_282a2e());
    assert_eq!(ex.legacy_3f4147, tokens::legacy_3f4147());
    assert_eq!(ex.legacy_fa777c, tokens::legacy_fa777c());
    assert_eq!(ex.legacy_da373c, tokens::legacy_da373c());
    assert_eq!(ex.legacy_fee75c, tokens::legacy_fee75c());
    assert_eq!(ex.text_muted, tokens::text_muted());
}

#[test]
fn theme_choices_produce_distinct_live_palettes() {
    use discord_rs::domain::settings::ThemeChoice;
    use discord_rs::tokens::{kit_theme, rgb};

    let dark = kit_theme::palette_for_theme(ThemeChoice::Dark);
    let darker = kit_theme::palette_for_theme(ThemeChoice::Darker);
    let light = kit_theme::palette_for_theme(ThemeChoice::Light);
    let midnight = kit_theme::palette_for_theme(ThemeChoice::Midnight);

    // Dark=theme-darker is the audited CDP reference; Darker shares it because
    // no separate measured surface set exists.
    assert_eq!(darker.bg_chat, dark.bg_chat);
    assert_eq!(dark.bg_chat, rgb(0x313338));
    assert_eq!(light.bg_chat, rgb(0xffffff));
    assert_eq!(midnight.bg_chat, rgb(0x000000));
    assert_ne!(dark.bg_chat, light.bg_chat);
    assert_ne!(dark.bg_chat, midnight.bg_chat);

    // The kit payload is rebuilt from the palette, so applying a new choice
    // repaints the same registered theme instead of requiring a restart.
    let dark_json = kit_theme::discord_theme_json_for_palette(&dark, ThemeChoice::Dark);
    let light_json = kit_theme::discord_theme_json_for_palette(&light, ThemeChoice::Light);
    assert_ne!(dark_json, light_json);
    assert!(dark_json.contains("\"background\":\"#313338\""));
    assert!(light_json.contains("\"background\":\"#ffffff\""));
}

#[test]
fn account_custom_colors_are_parsed_and_drive_accent() {
    use discord_rs::domain::settings::ThemeChoice;
    use discord_rs::tokens::kit_theme;

    assert_eq!(kit_theme::parse_custom_color_hex("#1e1f22"), Some(0x1e1f22ff));
    assert_eq!(kit_theme::parse_custom_color_hex("#abc"), Some(0xaabbccff));
    assert_eq!(kit_theme::parse_custom_color_hex("#11223344"), Some(0x11223344));
    assert_eq!(kit_theme::parse_custom_color_hex("not-a-color"), None);

    let palette = kit_theme::palette_for_theme_with_custom(
        ThemeChoice::Dark,
        &["#ff0000".to_string(), "invalid".to_string(), "#00ff00".to_string()],
    );
    assert_eq!(palette.blurple, gpui::rgb(0xff0000));
    // Hover/active scale the same audited blurple ratios (0.81/0.68).
    assert!((palette.blurple_hover.r - 0.81).abs() < 0.01);
    assert!((palette.blurple_hover.g - 0.0).abs() < 0.01);
    assert!((palette.blurple_active.r - 0.68).abs() < 0.01);
    // The gradient stops beyond the first remain available to future work but
    // do not fabricate surfaces the client cannot render.
    assert_eq!(palette.bg_chat, gpui::rgb(0x313338));

    let uncustomized = kit_theme::palette_for_theme_with_custom(ThemeChoice::Dark, &[]);
    assert_eq!(uncustomized.blurple, gpui::rgb(tokens::HEX_BLURPLE));
}
