//! Discord RS - Badge Component
//! Thin wrapper over gpui-kit Badge (`gpui::component::badge::Badge`) with Discord styling,
//! plus unread pill marker.

use gpui::component::badge::Badge as KitBadge;
use gpui::prelude::*;
use gpui::*;

use crate::tokens::colors::{bg_server, status_dnd, white};
use crate::tokens::spacing::{radius_full, radius_sm};

/// Visual states for the Discord unread indicator pill
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnreadMarkerState {
    /// No marker visible
    #[default]
    Hidden,
    /// Unread messages in channel / server (8px height)
    Unread,
    /// Mouse hovering over item (20px height)
    Hover,
    /// Item is currently active / selected (40px height)
    Selected,
}

impl UnreadMarkerState {
    /// Height of the white indicator pill
    pub fn height(&self) -> Pixels {
        match self {
            Self::Hidden => px(0.0),
            Self::Unread => px(8.0),
            Self::Hover => px(20.0),
            Self::Selected => px(40.0),
        }
    }

    /// Whether the pill is visible
    pub fn is_visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }
}

/// White indicator pill on the left edge of server icons and channel rows
#[derive(IntoElement)]
pub struct UnreadMarker {
    state: UnreadMarkerState,
    width: Pixels,
}

impl UnreadMarker {
    /// Creates a new unread marker with given state
    pub fn new(state: UnreadMarkerState) -> Self {
        Self {
            state,
            width: px(4.0),
        }
    }

    /// Convenience for unread state
    pub fn unread() -> Self {
        Self::new(UnreadMarkerState::Unread)
    }

    /// Convenience for hover state
    pub fn hover() -> Self {
        Self::new(UnreadMarkerState::Hover)
    }

    /// Convenience for selected state
    pub fn selected() -> Self {
        Self::new(UnreadMarkerState::Selected)
    }

    /// Sets custom pill width
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }
}

impl RenderOnce for UnreadMarker {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if !self.state.is_visible() {
            return div().w(self.width).h(px(0.0));
        }

        div()
            .w(self.width)
            .h(self.state.height())
            .bg(white())
            .rounded_r(radius_sm())
            .flex_shrink_0()
    }
}

/// Red notification badge for unread mentions (thin wrapper around kit Badge)
#[derive(IntoElement)]
pub struct MentionBadge {
    count: u32,
    dot_only: bool,
    cutout: bool,
    cutout_color: Option<Rgba>,
}

impl MentionBadge {
    /// Creates a new mention badge with a numeric count
    pub fn new(count: u32) -> Self {
        Self {
            count,
            dot_only: false,
            cutout: false,
            cutout_color: None,
        }
    }

    /// Creates a small red dot badge without text
    pub fn dot() -> Self {
        Self {
            count: 1,
            dot_only: true,
            cutout: false,
            cutout_color: None,
        }
    }

    /// Adds a dark cutout border ring (for floating over icons/avatars)
    pub fn with_cutout(mut self, cutout_color: impl Into<Option<Rgba>>) -> Self {
        self.cutout = true;
        self.cutout_color = cutout_color.into();
        self
    }

    /// Sets mention count
    pub fn count(mut self, count: u32) -> Self {
        self.count = count;
        self
    }
}

impl RenderOnce for MentionBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if self.count == 0 && !self.dot_only {
            return div().into_any_element();
        }

        let mut badge = KitBadge::new().color(status_dnd());
        if self.dot_only {
            badge = badge.dot();
        } else {
            badge = badge.count(self.count as usize).max(99);
        }

        if self.cutout {
            let border_bg = self.cutout_color.unwrap_or_else(bg_server);
            div()
                .flex()
                .items_center()
                .justify_center()
                .p(px(2.0))
                .rounded(radius_full())
                .bg(border_bg)
                .child(badge)
                .into_any_element()
        } else {
            badge.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_unread_marker_states() {
        assert_eq!(UnreadMarkerState::Hidden.height(), px(0.0));
        assert_eq!(UnreadMarkerState::Unread.height(), px(8.0));
        assert_eq!(UnreadMarkerState::Hover.height(), px(20.0));
        assert_eq!(UnreadMarkerState::Selected.height(), px(40.0));
        assert!(!UnreadMarkerState::Hidden.is_visible());
        assert!(UnreadMarkerState::Unread.is_visible());
    }

    #[::core::prelude::v1::test]
    fn test_mention_badge_builder() {
        let b = MentionBadge::new(5);
        assert_eq!(b.count, 5);
        assert!(!b.dot_only);
        assert!(!b.cutout);

        let dot = MentionBadge::dot().with_cutout(bg_server());
        assert!(dot.dot_only);
        assert!(dot.cutout);
    }
}
