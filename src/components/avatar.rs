//! Discord RS - Avatar Component
//! User avatar with status indicator (online, idle, dnd, offline) in 24, 32, 40, and 80px sizes.

use gpui::prelude::*;
use gpui::*;

use crate::domain::user::PresenceStatus;
use crate::tokens::colors::{white, *};

/// Standard Discord avatar sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AvatarSize {
    /// 24x24px - Used in compact lists, reactions, mentions
    Px24,
    /// 32x32px - Used in member list, user bottom bar
    Px32,
    /// 40x40px - Standard chat message avatar
    #[default]
    Px40,
    /// 80x80px - User profile modal and popouts
    Px80,
}

impl AvatarSize {
    /// Returns the pixel dimension of the avatar
    pub fn pixels(&self) -> Pixels {
        match self {
            Self::Px24 => px(24.0),
            Self::Px32 => px(32.0),
            Self::Px40 => px(40.0),
            Self::Px80 => px(80.0),
        }
    }

    /// Size of the status indicator badge
    pub fn status_size(&self) -> Pixels {
        match self {
            Self::Px24 => px(8.0),
            Self::Px32 => px(10.0),
            Self::Px40 => px(12.0),
            Self::Px80 => px(20.0),
        }
    }

    /// Cutout border thickness around the status indicator
    pub fn cutout_border(&self) -> Pixels {
        match self {
            Self::Px24 => px(2.0),
            Self::Px32 => px(2.0),
            Self::Px40 => px(3.0),
            Self::Px80 => px(4.0),
        }
    }

    /// Font size for fallback initials
    pub fn font_size(&self) -> Pixels {
        match self {
            Self::Px24 => px(11.0),
            Self::Px32 => px(13.0),
            Self::Px40 => px(16.0),
            Self::Px80 => px(32.0),
        }
    }
}

/// Discord user avatar with status indicator dot
#[derive(IntoElement)]
pub struct Avatar {
    src: Option<SharedString>,
    fallback_text: SharedString,
    size: AvatarSize,
    status: Option<PresenceStatus>,
    bg_color: Option<Rgba>,
    cutout_color: Option<Rgba>,
}

impl Avatar {
    /// Creates a new Avatar with fallback text (e.g. username or initials)
    pub fn new(fallback_text: impl Into<SharedString>) -> Self {
        Self {
            src: None,
            fallback_text: fallback_text.into(),
            size: AvatarSize::Px40,
            status: None,
            bg_color: None,
            cutout_color: None,
        }
    }

    /// Sets the avatar image URL / asset path
    pub fn src(mut self, src: impl Into<SharedString>) -> Self {
        self.src = Some(src.into());
        self
    }

    /// Sets optional avatar image URL
    pub fn opt_src(mut self, src: Option<impl Into<SharedString>>) -> Self {
        self.src = src.map(|s| s.into());
        self
    }

    /// Sets avatar size (24, 32, 40, 80px)
    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// Sets status indicator
    pub fn status(mut self, status: PresenceStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets optional status indicator
    pub fn opt_status(mut self, status: Option<PresenceStatus>) -> Self {
        self.status = status;
        self
    }

    /// Sets custom background color for fallback
    pub fn bg_color(mut self, color: Rgba) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// Sets cutout ring color (defaults to channel sidebar background)
    pub fn cutout_color(mut self, color: Rgba) -> Self {
        self.cutout_color = Some(color);
        self
    }

    /// Convenience for online status
    pub fn online(self) -> Self {
        self.status(PresenceStatus::Online)
    }

    /// Convenience for idle status
    pub fn idle(self) -> Self {
        self.status(PresenceStatus::Idle)
    }

    /// Convenience for DND status
    pub fn dnd(self) -> Self {
        self.status(PresenceStatus::DoNotDisturb)
    }

    /// Convenience for offline status
    pub fn offline(self) -> Self {
        self.status(PresenceStatus::Offline)
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let avatar_dim = self.size.pixels();
        let fallback_font = self.size.font_size();
        let cutout_bg = self.cutout_color.unwrap_or_else(bg_channels);

        // Deterministic fallback color based on text
        let fallback_bg = self.bg_color.unwrap_or_else(|| {
            let hash = self.fallback_text.as_bytes().iter().fold(0u32, |acc, &b| {
                acc.wrapping_add(b as u32).wrapping_mul(31)
            });
            match hash % 5 {
                0 => blurple(),
                1 => rgb(0x23a55a), // green
                2 => rgb(0xf0b232), // amber
                3 => rgb(0xda373c), // red
                _ => rgb(0xeb459e), // pink
            }
        });

        // Compute 1-2 letter initials
        let initials: SharedString = {
            let trimmed = self.fallback_text.trim();
            if trimmed.is_empty() {
                "?".into()
            } else {
                let mut chars = trimmed.chars();
                let first = chars.next().unwrap_or('?').to_uppercase().to_string();
                first.into()
            }
        };

        // Inner circular avatar container
        let inner_avatar = if let Some(src) = self.src {
            if src.starts_with("http://") || src.starts_with("https://") {
                div()
                    .size_full()
                    .rounded_full()
                    .overflow_hidden()
                    .child(img(src).size_full())
            } else {
                div()
                    .size_full()
                    .rounded_full()
                    .bg(fallback_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(white())
                    .font_weight(FontWeight::BOLD)
                    .text_size(fallback_font)
                    .child(initials)
            }
        } else {
            div()
                .size_full()
                .rounded_full()
                .bg(fallback_bg)
                .flex()
                .items_center()
                .justify_center()
                .text_color(white())
                .font_weight(FontWeight::BOLD)
                .text_size(fallback_font)
                .child(initials)
        };

        // Outer relative container
        let mut container = div()
            .relative()
            .w(avatar_dim)
            .h(avatar_dim)
            .flex_shrink_0()
            .child(inner_avatar);

        // Render status dot if requested
        if let Some(presence) = self.status {
            let dot_size = self.size.status_size();
            let border_width = self.size.cutout_border();

            let (dot_color, is_dnd, is_idle, is_offline) = match presence {
                PresenceStatus::Online => (status_online(), false, false, false),
                PresenceStatus::Idle => (status_idle(), false, true, false),
                PresenceStatus::DoNotDisturb => (status_dnd(), true, false, false),
                PresenceStatus::Offline => (status_offline(), false, false, true),
            };

            let mut status_indicator = div()
                .absolute()
                .bottom(px(-1.0))
                .right(px(-1.0))
                .w(dot_size + border_width * 2.0)
                .h(dot_size + border_width * 2.0)
                .rounded_full()
                .bg(cutout_bg)
                .flex()
                .items_center()
                .justify_center();

            let mut inner_dot = div()
                .w(dot_size)
                .h(dot_size)
                .rounded_full()
                .bg(dot_color)
                .flex()
                .items_center()
                .justify_center();

            if is_dnd {
                // White horizontal bar inside red circle
                let bar_width = dot_size * 0.6;
                let bar_height = px(2.0);
                inner_dot = inner_dot.child(
                    div()
                        .w(bar_width)
                        .h(bar_height)
                        .bg(white())
                        .rounded_xs(),
                );
            } else if is_offline {
                // Dark hollow circle inside grey ring
                let inner_hole = dot_size * 0.45;
                inner_dot = inner_dot.child(
                    div()
                        .w(inner_hole)
                        .h(inner_hole)
                        .bg(cutout_bg)
                        .rounded_full(),
                );
            } else if is_idle {
                // Crescent cutout for idle
                let crescent_hole = dot_size * 0.45;
                inner_dot = inner_dot.child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .w(crescent_hole)
                        .h(crescent_hole)
                        .bg(cutout_bg)
                        .rounded_full(),
                );
            }

            status_indicator = status_indicator.child(inner_dot);
            container = container.child(status_indicator);
        }

        container
    }
}
