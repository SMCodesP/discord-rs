use discord_rs::app::responsive::{
    available_friends_content_width, available_main_width, clamp_modal_height, clamp_modal_width,
    should_auto_hide_member_sidebar, should_show_active_now, should_use_channel_drawer,
    ResponsiveLayout, ACTIVE_NOW_BREAKPOINT_PX, CHANNEL_SIDEBAR_DRAWER_BELOW_PX,
    MEMBER_SIDEBAR_AUTO_HIDE_BELOW_PX, MIN_MAIN_CONTENT_WIDTH_PX,
};

#[test]
fn active_now_breakpoint_matches_discord_1200px() {
    // Source: T-090 task prompt - "Active Now <1200px" official Discord breakpoint.
    assert_eq!(ACTIVE_NOW_BREAKPOINT_PX, 1200.0);
    assert!(!should_show_active_now(1199.0));
    assert!(!should_show_active_now(1017.0));
    assert!(should_show_active_now(1200.0));
    assert!(should_show_active_now(1280.0));
}

#[test]
fn member_sidebar_collapses_below_940px() {
    // Source: T-090 evidence cites Discord CSS max-width:940px behavior and matrix.
    assert_eq!(MEMBER_SIDEBAR_AUTO_HIDE_BELOW_PX, 940.0);
    assert!(should_auto_hide_member_sidebar(939.9));
    assert!(!should_auto_hide_member_sidebar(940.0));
}

#[test]
fn channel_sidebar_uses_drawer_below_600px() {
    // Source: T-090 matrix requires 600x600 and 485x600 without zero-width page/overflow.
    assert_eq!(CHANNEL_SIDEBAR_DRAWER_BELOW_PX, 600.0);
    assert!(should_use_channel_drawer(485.0));
    assert!(should_use_channel_drawer(599.0));
    assert!(!should_use_channel_drawer(600.0));
}

#[test]
fn responsive_matrix_1017_940_800_600_485() {
    // Source: T-090 acceptance matrix viewports.
    let m1017 = ResponsiveLayout::compute(1017.0, true, true);
    assert!(m1017.show_guild_rail);
    assert!(m1017.show_channel_sidebar);
    assert!(m1017.show_member_sidebar);
    assert!(!m1017.show_active_now);
    assert!(!m1017.channel_sidebar_is_drawer);

    let m940 = ResponsiveLayout::compute(940.0, true, true);
    assert!(m940.show_channel_sidebar);
    assert!(m940.show_member_sidebar);
    assert!(!m940.show_active_now);

    let m800 = ResponsiveLayout::compute(800.0, true, true);
    assert!(m800.show_channel_sidebar);
    assert!(!m800.show_member_sidebar);
    assert!(!m800.show_active_now);

    let m600 = ResponsiveLayout::compute(600.0, true, true);
    assert!(m600.show_channel_sidebar);
    assert!(!m600.show_member_sidebar);
    assert!(!m600.channel_sidebar_is_drawer);

    let m485 = ResponsiveLayout::compute(485.0, true, true);
    assert!(m485.show_guild_rail);
    assert!(!m485.show_channel_sidebar);
    assert!(m485.channel_sidebar_is_drawer);
    assert!(!m485.show_member_sidebar);
    assert!(!m485.show_active_now);
}

#[test]
fn user_sidebar_preferences_are_preserved_as_intent() {
    // T-090: toggles restore sidebars without losing selection; preference intent remains.
    let wide_hidden = ResponsiveLayout::compute(1280.0, false, true);
    assert!(!wide_hidden.show_member_sidebar);
    let wide_shown = ResponsiveLayout::compute(1280.0, true, true);
    assert!(wide_shown.show_member_sidebar);

    let narrow_auto_hidden = ResponsiveLayout::compute(800.0, true, true);
    assert!(!narrow_auto_hidden.show_member_sidebar);
    let wide_restored = ResponsiveLayout::compute(1280.0, true, true);
    assert!(wide_restored.show_member_sidebar);
}

#[test]
fn main_content_width_never_negative_and_has_minimum_budget() {
    assert_eq!(available_main_width(100.0, true, true), 0.0);
    assert_eq!(available_main_width(485.0, true, false), 413.0);
    assert!(available_main_width(485.0, true, false) >= MIN_MAIN_CONTENT_WIDTH_PX);
}

#[test]
fn active_now_reduces_friends_content_width_only_when_visible() {
    let wide = available_friends_content_width(1280.0, true, true, true);
    let narrow_hidden = available_friends_content_width(1017.0, true, true, false);
    assert!(wide < 1280.0 - 72.0 - 240.0);
    assert_eq!(narrow_hidden, 705.0);
}

#[test]
fn modal_clamp_prevents_overflow_on_small_viewports() {
    // T-090: clamp de modais and no horizontal overflow at small widths.
    assert_eq!(clamp_modal_width(485.0, 440.0, 16.0), 440.0);
    assert_eq!(clamp_modal_width(485.0, 700.0, 16.0), 453.0);
    assert_eq!(clamp_modal_height(600.0, 900.0, 24.0), 552.0);
    assert_eq!(clamp_modal_height(100.0, 900.0, 24.0), 52.0);
}
