//! Discord RS - Button Component
//! Buttons: Blurple, Gray, Danger, Ghost, and Icon buttons with hover/active states.

use gpui::prelude::*;
use gpui::*;

use crate::tokens::colors::{white, *};
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Visual variants for Discord buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Discord Brand Blurple (#5865F2)
    #[default]
    Blurple,
    /// Secondary Gray (#4E5058)
    Gray,
    /// Destructive Danger Red (#DA373C)
    Danger,
    /// Ghost transparent button
    Ghost,
}

/// Sizing options for Discord buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// Compact button (height 24px, 12px text)
    Sm,
    /// Standard Discord button (height 38px, 14px text)
    #[default]
    Md,
    /// Large button (height 44px, 16px text)
    Lg,
}

/// Standard Discord push button with interactive states
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    variant: ButtonVariant,
    size: ButtonSize,
    label: Option<SharedString>,
    icon: Option<SharedString>,
    trailing_icon: Option<SharedString>,
    disabled: bool,
    full_width: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl Button {
    /// Creates a new button with a text label
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            variant: ButtonVariant::Blurple,
            size: ButtonSize::Md,
            label: Some(label.into()),
            icon: None,
            trailing_icon: None,
            disabled: false,
            full_width: false,
            on_click: None,
        }
    }

    /// Blurple brand action button
    pub fn blurple(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Blurple)
    }

    /// Secondary gray button
    pub fn gray(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Gray)
    }

    /// Destructive danger button
    pub fn danger(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Danger)
    }

    /// Transparent ghost button
    pub fn ghost(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::new(id, label).variant(ButtonVariant::Ghost)
    }

    /// Sets button variant
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets button size
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Sets a leading icon (text/symbol)
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets a trailing icon
    pub fn trailing_icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.trailing_icon = Some(icon.into());
        self
    }

    /// Sets disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Expands button to 100% width
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Attaches a click event handler
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (height, text_size, px_pad, py_pad, radius) = match self.size {
            ButtonSize::Sm => (px(24.0), font_size_xs(), px(10.0), px(2.0), radius_sm()),
            ButtonSize::Md => (px(38.0), font_size_sm(), px(16.0), px(6.0), radius_sm()),
            ButtonSize::Lg => (px(44.0), font_size_base(), px(24.0), px(8.0), radius_md()),
        };

        let (bg_normal, bg_hover, bg_active, text_color_normal) = match self.variant {
            ButtonVariant::Blurple => (
                blurple(),
                blurple_hover(),
                blurple_active(),
                white(),
            ),
            ButtonVariant::Gray => (
                rgb(0x4e5058),
                rgb(0x6d6f78),
                rgb(0x80848e),
                white(),
            ),
            ButtonVariant::Danger => (
                rgb(0xda373c),
                rgb(0xa12828),
                rgb(0x8f2022),
                white(),
            ),
            ButtonVariant::Ghost => (
                rgba(0x00000000),
                rgba(0xffffff14),
                rgba(0xffffff24),
                text_normal(),
            ),
        };

        let mut el = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .h(height)
            .px(px_pad)
            .py(py_pad)
            .rounded(radius)
            .gap(space_2())
            .text_size(text_size)
            .font_weight(FontWeight::SEMIBOLD)
            .flex_shrink_0();

        if self.full_width {
            el = el.w_full();
        }

        if self.disabled {
            el = el
                .bg(set_alpha(bg_normal, 0.5))
                .text_color(set_alpha(text_color_normal, 0.5))
                .cursor_not_allowed();
        } else {
            el = el
                .bg(bg_normal)
                .text_color(text_color_normal)
                .cursor_pointer()
                .hover(move |s| s.bg(bg_hover).text_color(white()))
                .active(move |s| s.bg(bg_active).text_color(white()));

            if let Some(on_click) = self.on_click {
                el = el.on_click(move |ev, window, cx| {
                    (on_click)(ev, window, cx);
                });
            }
        }

        // Add leading icon if present
        if let Some(icon) = self.icon {
            el = el.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon),
            );
        }

        // Add label if present
        if let Some(label) = self.label {
            el = el.child(
                div()
                    .flex()
                    .items_center()
                    .child(label),
            );
        }

        // Add trailing icon if present
        if let Some(trailing) = self.trailing_icon {
            el = el.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(trailing),
            );
        }

        el
    }
}

/// Compact icon button with Discord hover & active styling
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: SharedString,
    size: Pixels,
    icon_size: Pixels,
    active: bool,
    danger: bool,
    disabled: bool,
    tooltip_text: Option<SharedString>,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl IconButton {
    /// Creates a new icon button
    pub fn new(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            size: px(32.0),
            icon_size: px(18.0),
            active: false,
            danger: false,
            disabled: false,
            tooltip_text: None,
            on_click: None,
        }
    }

    /// Sets explicit square dimensions
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Sets icon font size
    pub fn icon_size(mut self, icon_size: Pixels) -> Self {
        self.icon_size = icon_size;
        self
    }

    /// Sets active/toggled state
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Sets danger styling (e.g. disconnect or mute)
    pub fn danger(mut self, danger: bool) -> Self {
        self.danger = danger;
        self
    }

    /// Sets disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets tooltip text
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip_text = Some(text.into());
        self
    }

    /// Sets click handler
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (normal_color, hover_color, active_color, normal_bg, hover_bg) = if self.danger {
            (
                rgb(0xfa777c),
                white(),
                white(),
                rgba(0xda373c22),
                rgb(0xda373c),
            )
        } else if self.active {
            (
                white(),
                white(),
                white(),
                rgba(0xffffff24),
                rgba(0xffffff33),
            )
        } else {
            (
                text_muted(),
                text_normal(),
                white(),
                rgba(0x00000000),
                rgba(0xffffff14),
            )
        };

        let mut el = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .w(self.size)
            .h(self.size)
            .rounded(radius_sm())
            .text_size(self.icon_size)
            .flex_shrink_0();

        if self.disabled {
            el = el
                .bg(normal_bg)
                .text_color(set_alpha(normal_color, 0.4))
                .cursor_not_allowed();
        } else {
            el = el
                .bg(normal_bg)
                .text_color(normal_color)
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg).text_color(hover_color))
                .active(move |s| s.bg(hover_bg).text_color(active_color));

            if let Some(on_click) = self.on_click {
                el = el.on_click(move |ev, window, cx| {
                    (on_click)(ev, window, cx);
                });
            }
        }

        el.child(self.icon)
    }
}
