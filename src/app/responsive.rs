//! Responsive layout breakpoints and viewport-derived shell decisions.
//!
//! Sources (task T-090 / docs/PARITY_MASTER_TASKS.md):
//! - "Active Now <1200px" (official Discord breakpoint cited in task prompt)
//! - Discord official CSS evidence recorded in T-090: inviteToolbar hidden at
//!   `max-width:940px`; members full-width at `max-width:485px`.
//! - T-090 acceptance matrix: 1017x605, 940x600, 800x600, 600x600, 485x600.

use gpui::{px, Pixels};

/// Official Discord breakpoint: hide Active Now panel below 1200px viewport width.
pub const ACTIVE_NOW_BREAKPOINT_PX: f32 = 1200.0;

/// Narrow viewport where member sidebar should auto-collapse to preserve chat space.
/// Derived from T-090 matrix: 1017 keeps shell usable; below 940 the layout must
/// avoid zero-width page content.
pub const MEMBER_SIDEBAR_AUTO_HIDE_BELOW_PX: f32 = 940.0;

/// Very narrow viewport where channel sidebar should collapse into drawer mode.
/// T-090 requires 485x600 without inaccessible controls or horizontal overflow.
pub const CHANNEL_SIDEBAR_DRAWER_BELOW_PX: f32 = 600.0;

/// Minimum supported window width for the desktop shell. Below this, layout uses
/// drawer/collapsed behavior rather than allowing zero-width content.
pub const MIN_WINDOW_WIDTH_PX: f32 = 485.0;

/// Minimum supported window height for the desktop shell.
pub const MIN_WINDOW_HEIGHT_PX: f32 = 480.0;

/// Default/clamped startup window width bounds from main.rs evidence.
pub const STARTUP_MAX_WIDTH_PX: f32 = 1280.0;
pub const STARTUP_MIN_WIDTH_PX: f32 = 940.0;

/// Fixed shell widths from current app layout.
pub const GUILD_RAIL_WIDTH_PX: f32 = 72.0;
pub const CHANNEL_SIDEBAR_WIDTH_PX: f32 = 240.0;
pub const MEMBER_SIDEBAR_WIDTH_PX: f32 = 240.0;
pub const ACTIVE_NOW_WIDTH_PX: f32 = 360.0;

/// Minimum readable main content width before sidebars must collapse.
pub const MIN_MAIN_CONTENT_WIDTH_PX: f32 = 240.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponsiveLayout {
    pub show_guild_rail: bool,
    pub show_channel_sidebar: bool,
    pub show_member_sidebar: bool,
    pub show_active_now: bool,
    pub channel_sidebar_is_drawer: bool,
    pub member_sidebar_is_drawer: bool,
}

impl ResponsiveLayout {
    /// Compute layout visibility from viewport width and user preferences.
    ///
    /// User preferences are preserved as intent; viewport may temporarily auto-hide
    /// panels. When width increases again, callers should restore using the same
    /// user preference values, not mutating them during auto-collapse.
    pub fn compute(
        viewport_width_px: f32,
        user_show_member_sidebar: bool,
        user_show_channel_sidebar: bool,
    ) -> Self {
        let width = viewport_width_px.max(0.0);
        let show_active_now = width >= ACTIVE_NOW_BREAKPOINT_PX;
        let channel_sidebar_is_drawer = width < CHANNEL_SIDEBAR_DRAWER_BELOW_PX;
        let member_sidebar_is_drawer =
            width < MEMBER_SIDEBAR_AUTO_HIDE_BELOW_PX && width >= CHANNEL_SIDEBAR_DRAWER_BELOW_PX;

        let show_channel_sidebar =
            user_show_channel_sidebar && !channel_sidebar_is_drawer;

        let available_for_members = available_main_width(
            width,
            true,
            show_channel_sidebar,
        );
        let show_member_sidebar = user_show_member_sidebar
            && width >= MEMBER_SIDEBAR_AUTO_HIDE_BELOW_PX
            && available_for_members >= MIN_MAIN_CONTENT_WIDTH_PX + MEMBER_SIDEBAR_WIDTH_PX;

        Self {
            show_guild_rail: true,
            show_channel_sidebar,
            show_member_sidebar,
            show_active_now,
            channel_sidebar_is_drawer,
            member_sidebar_is_drawer,
        }
    }
}

pub fn should_show_active_now(viewport_width_px: f32) -> bool {
    viewport_width_px.max(0.0) >= ACTIVE_NOW_BREAKPOINT_PX
}

pub fn should_auto_hide_member_sidebar(viewport_width_px: f32) -> bool {
    viewport_width_px.max(0.0) < MEMBER_SIDEBAR_AUTO_HIDE_BELOW_PX
}

pub fn should_use_channel_drawer(viewport_width_px: f32) -> bool {
    viewport_width_px.max(0.0) < CHANNEL_SIDEBAR_DRAWER_BELOW_PX
}

/// Available width for main content after fixed sidebars that are currently visible.
pub fn available_main_width(
    viewport_width_px: f32,
    show_guild_rail: bool,
    show_channel_sidebar: bool,
) -> f32 {
    let mut width = viewport_width_px.max(0.0);
    if show_guild_rail {
        width -= GUILD_RAIL_WIDTH_PX;
    }
    if show_channel_sidebar {
        width -= CHANNEL_SIDEBAR_WIDTH_PX;
    }
    width.max(0.0)
}

/// Available width for friends hub content column when Active Now may be visible.
pub fn available_friends_content_width(
    viewport_width_px: f32,
    show_guild_rail: bool,
    show_channel_sidebar: bool,
    show_active_now: bool,
) -> f32 {
    let mut width = available_main_width(viewport_width_px, show_guild_rail, show_channel_sidebar);
    if show_active_now {
        width -= ACTIVE_NOW_WIDTH_PX;
    }
    width.max(0.0)
}

/// Clamp a modal/dialog width to the viewport, preserving a minimum margin.
pub fn clamp_modal_width(viewport_width_px: f32, desired_width_px: f32, margin_px: f32) -> f32 {
    let viewport = viewport_width_px.max(0.0);
    let desired = desired_width_px.max(0.0);
    let margin = margin_px.max(0.0);
    let max_width = (viewport - margin * 2.0).max(0.0);
    if max_width == 0.0 {
        return 0.0;
    }
    desired.min(max_width)
}

/// Clamp a modal/dialog height to the viewport, preserving a minimum margin.
pub fn clamp_modal_height(viewport_height_px: f32, desired_height_px: f32, margin_px: f32) -> f32 {
    let viewport = viewport_height_px.max(0.0);
    let desired = desired_height_px.max(0.0);
    let margin = margin_px.max(0.0);
    let max_height = (viewport - margin * 2.0).max(0.0);
    if max_height == 0.0 {
        return 0.0;
    }
    desired.min(max_height)
}

pub fn px_f32(value: f32) -> Pixels {
    px(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_now_breakpoint_is_1200() {
        assert!(!should_show_active_now(1199.9));
        assert!(should_show_active_now(1200.0));
        assert!(should_show_active_now(1600.0));
    }

    #[test]
    fn member_sidebar_auto_hides_below_940() {
        assert!(should_auto_hide_member_sidebar(939.0));
        assert!(!should_auto_hide_member_sidebar(940.0));
    }

    #[test]
    fn channel_drawer_below_600() {
        assert!(should_use_channel_drawer(485.0));
        assert!(should_use_channel_drawer(599.0));
        assert!(!should_use_channel_drawer(600.0));
    }

    #[test]
    fn compute_layout_matrix_cases() {
        let m1017 = ResponsiveLayout::compute(1017.0, true, true);
        assert!(m1017.show_guild_rail);
        assert!(m1017.show_channel_sidebar);
        assert!(m1017.show_member_sidebar);
        assert!(!m1017.show_active_now);

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
        assert!(!m485.show_channel_sidebar);
        assert!(!m485.show_member_sidebar);
        assert!(m485.channel_sidebar_is_drawer);
        assert!(!m485.show_active_now);
    }

    #[test]
    fn user_preference_is_preserved_as_intent() {
        let hidden_by_user = ResponsiveLayout::compute(1280.0, false, true);
        assert!(!hidden_by_user.show_member_sidebar);
        let restored_when_wide = ResponsiveLayout::compute(1280.0, true, true);
        assert!(restored_when_wide.show_member_sidebar);
        let auto_hidden_narrow = ResponsiveLayout::compute(800.0, true, true);
        assert!(!auto_hidden_narrow.show_member_sidebar);
    }

    #[test]
    fn clamp_modal_width_respects_viewport() {
        assert_eq!(clamp_modal_width(485.0, 440.0, 16.0), 440.0);
        assert_eq!(clamp_modal_width(485.0, 600.0, 16.0), 453.0);
        assert_eq!(clamp_modal_width(200.0, 600.0, 16.0), 168.0);
    }

    #[test]
    fn available_main_width_never_negative() {
        assert_eq!(available_main_width(100.0, true, true), 0.0);
        assert_eq!(available_main_width(1017.0, true, true), 705.0);
    }
}
