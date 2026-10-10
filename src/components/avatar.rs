//! Discord RS - Avatar Component
//! User avatar with status indicator (online, idle, dnd, offline) in 24, 32, 40, and 80px sizes.

use gpui::prelude::*;
use gpui::*;

use crate::domain::user::{AvatarDecorationData, PresenceStatus};
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
    decoration: Option<AvatarDecorationData>,
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
            decoration: None,
        }
    }

    /// Sets avatar decoration data
    pub fn decoration(mut self, decoration: AvatarDecorationData) -> Self {
        self.decoration = Some(decoration);
        self
    }

    /// Sets optional avatar decoration data
    pub fn opt_decoration(mut self, decoration: Option<AvatarDecorationData>) -> Self {
        self.decoration = decoration;
        self
    }

    /// Convenience builder to set decoration by asset name
    pub fn decoration_asset(mut self, asset: impl Into<String>) -> Self {
        self.decoration = Some(AvatarDecorationData::new(asset));
        self
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
            if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") || src.starts_with("/") || src.starts_with("file://") {
                div()
                    .size_full()
                    .rounded_full()
                    .overflow_hidden()
                    .child(img(src).size_full().rounded_full())
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
            .rounded_full()
            .flex_shrink_0()
            .child(inner_avatar);

        // Render avatar decoration layer if present (120% size, -10% offset, unique id per asset)
        if let Some(ref decor) = self.decoration {
            let decor_url = SharedString::from(decor.url());
            let asset = decor.asset.clone();
            let decor_px = avatar_dim * 1.2;
            let offset_px = avatar_dim * (-0.1);

            let decor_layer = div()
                .absolute()
                .w(decor_px)
                .h(decor_px)
                .top(offset_px)
                .left(offset_px)
                .child(
                    img(decor_url)
                        .id(ElementId::from(SharedString::from(format!("avatar-decor-{}", asset))))
                        .size_full(),
                );

            container = container.child(decor_layer);
        }

        // Render status dot if requested
        if let Some(presence) = self.status {
            let dot_size = self.size.status_size();
            let border_width = self.size.cutout_border();
            container = container.child(render_status_indicator(
                presence,
                dot_size,
                border_width,
                cutout_bg,
            ));
        }

        container
    }
}

fn render_status_indicator(
    presence: PresenceStatus,
    dot_size: Pixels,
    border_width: Pixels,
    cutout_bg: Rgba,
) -> AnyElement {
    let (dot_color, is_dnd, is_idle, is_offline) = match presence {
        PresenceStatus::Online => (status_online(), false, false, false),
        PresenceStatus::Idle => (status_idle(), false, true, false),
        PresenceStatus::DoNotDisturb => (status_dnd(), true, false, false),
        PresenceStatus::Offline => (status_offline(), false, false, true),
    };

    let inner_dot: AnyElement = if is_dnd {
        let bar_width = dot_size * 0.6;
        let bar_height = px(2.0);
        div()
            .w(dot_size)
            .h(dot_size)
            .rounded_full()
            .bg(dot_color)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(bar_width)
                    .h(bar_height)
                    .bg(white())
                    .rounded_xs(),
            )
            .into_any_element()
    } else if is_offline {
        let inner_hole = dot_size * 0.5;
        div()
            .w(dot_size)
            .h(dot_size)
            .rounded_full()
            .bg(dot_color)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(inner_hole)
                    .h(inner_hole)
                    .bg(cutout_bg)
                    .rounded_full(),
            )
            .into_any_element()
    } else if is_idle {
        let crescent_hole = dot_size * 0.65;
        let offset = dot_size * 0.2;
        div()
            .w(dot_size)
            .h(dot_size)
            .rounded_full()
            .bg(dot_color)
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top(-offset)
                    .left(-offset)
                    .w(crescent_hole)
                    .h(crescent_hole)
                    .bg(cutout_bg)
                    .rounded_full(),
            )
            .into_any_element()
    } else {
        div()
            .w(dot_size)
            .h(dot_size)
            .rounded_full()
            .bg(dot_color)
            .into_any_element()
    };

    div()
        .absolute()
        .bottom(px(-2.0))
        .right(px(-2.0))
        .w(dot_size + border_width * 2.0)
        .h(dot_size + border_width * 2.0)
        .rounded_full()
        .bg(cutout_bg)
        .flex()
        .items_center()
        .justify_center()
        .child(inner_dot)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_avatar_builder_with_decoration() {
        let decor = AvatarDecorationData::new("preset_cat_ears");
        let avatar = Avatar::new("User")
            .size(AvatarSize::Px32)
            .decoration(decor.clone());

        assert_eq!(avatar.size, AvatarSize::Px32);
        assert_eq!(avatar.decoration, Some(decor));
        assert_eq!(AvatarSize::Px32.pixels(), px(32.0));
    }

    #[::core::prelude::v1::test]
    fn test_avatar_builder_opt_and_asset() {
        let avatar1 = Avatar::new("A").opt_decoration(None);
        assert_eq!(avatar1.decoration, None);

        let avatar2 = Avatar::new("B").decoration_asset("a_sparkles");
        assert!(avatar2.decoration.is_some());
        assert_eq!(avatar2.decoration.as_ref().unwrap().asset, "a_sparkles");
        assert!(avatar2.decoration.as_ref().unwrap().is_animated());
    }

    #[::core::prelude::v1::test]
    fn test_avatar_size_dimensions() {
        assert_eq!(AvatarSize::Px24.pixels(), px(24.0));
        assert_eq!(AvatarSize::Px32.pixels(), px(32.0));
        assert_eq!(AvatarSize::Px40.pixels(), px(40.0));
        assert_eq!(AvatarSize::Px80.pixels(), px(80.0));
    }
}
