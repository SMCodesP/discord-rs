//! Discord RS - Tooltip Component
//! Styled Discord tooltip with dark background, crisp typography, and optional shortcut pills.

use gpui::prelude::*;
use gpui::*;

use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Discord styled tooltip element
#[derive(IntoElement)]
pub struct Tooltip {
    text: SharedString,
    shortcut: Option<SharedString>,
}

impl Tooltip {
    /// Creates a new tooltip with given text
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            shortcut: None,
        }
    }

    /// Sets an optional keyboard shortcut to display in the tooltip
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut el = div()
            .flex()
            .items_center()
            .gap(space_2())
            .px(px(12.0))
            .py(px(6.0))
            .bg(rgb(HEX_BG_FLOATING))
            .border_1()
            .border_color(rgba(0xffffff14))
            .rounded(radius_md())
            .text_size(font_size_xs())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text_primary())
            .child(self.text);

        if let Some(shortcut) = self.shortcut {
            el = el.child(
                div()
                    .px(px(4.0))
                    .py(px(1.0))
                    .rounded_xs()
                    .bg(rgba(0xffffff1a))
                    .text_color(text_muted())
                    .text_size(font_size_xxs())
                    .font_weight(FontWeight::BOLD)
                    .child(shortcut),
            );
        }

        el
    }
}

/// A View that wraps `Tooltip` for use with GPUI's `div().tooltip(...)` API
pub struct TooltipView {
    text: SharedString,
    shortcut: Option<SharedString>,
}

impl TooltipView {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            shortcut: None,
        }
    }

    pub fn with_shortcut(
        text: impl Into<SharedString>,
        shortcut: impl Into<SharedString>,
    ) -> Self {
        Self {
            text: text.into(),
            shortcut: Some(shortcut.into()),
        }
    }
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut t = Tooltip::new(self.text.clone());
        if let Some(shortcut) = &self.shortcut {
            t = t.shortcut(shortcut.clone());
        }
        t
    }
}

/// Helper that returns a closure suitable for `div().tooltip(...)`
pub fn discord_tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_window, cx| {
        cx.new(|_cx| TooltipView {
            text: text.clone(),
            shortcut: None,
        })
        .into()
    }
}

/// Helper that returns a closure with text and shortcut for `div().tooltip(...)`
pub fn discord_tooltip_with_shortcut(
    text: impl Into<SharedString>,
    shortcut: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    let shortcut = shortcut.into();
    move |_window, cx| {
        cx.new(|_cx| TooltipView {
            text: text.clone(),
            shortcut: Some(shortcut.clone()),
        })
        .into()
    }
}
