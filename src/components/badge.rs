//! Discord RS - Badge Component
//! Thin wrapper over gpui-kit Badge (`gpui::component::badge::Badge`) with Discord styling.

use gpui::component::badge::Badge as KitBadge;
use gpui::prelude::*;
use gpui::*;

use crate::tokens::colors::{bg_server, status_dnd};
use crate::tokens::spacing::radius_full;

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
