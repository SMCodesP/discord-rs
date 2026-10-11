//! Discord RS - Button Component
//! Thin Discord-styled wrapper over the gpui-kit `Button`
//! (`gpui::component::button::Button`). Our public types (`Button`,
//! `ButtonSize`, `ButtonVariant`, `IconButton`) keep their exact API; only the
//! rendering is delegated to the kit.
//!
//! Visual source (2026-10-11, read-only inspection of discord.com web client,
//! own tab, `sm`/`md`/`icon-only`/`critical-*` classes + resolved
//! `--control-*` CSS variables, screenshots in
//! `~/.cache/discord-rs-parity/discord/button-*.png`):
//! - primary: normal #5865F2 (= token), hover #4452BB (token #4752C4
//!   diverges), active #3A48A3 (token #3C45A5 diverges); text white.
//! - secondary: translucent gray rgb(151,151,159) at 12%/16%/20% alpha for
//!   normal/hover/active; text near-white.
//! - danger solid: #D22D39 / #A9232E / #971D28; text white.
//! - danger secondary: translucent gray bg (same alphas), text #F87E7A.
//! - icon-only: transparent, icon #ABACB2, hover bg 16% + white icon,
//!   active bg 20% + white icon.
//! - geometry: sm h32/px11/min-w60/text14, md h40/px15/min-w100/text16,
//!   icon-only 32x32, radius 8px, weight 500, disabled = whole-button
//!   opacity 0.5 (`:disabled { opacity: 0.5 }`).
//! - Kit note: `ButtonVariant::Custom` carries the exact hover/active colors
//!   (applied verbatim) while its normal background is faded 20% toward
//!   transparent, so the wrapper refines `.bg(exact)` on top for the normal
//!   state. No `.hover()` is set on the kit button (it would clash with the
//!   hover style the kit wires internally).

use gpui::component::button::{
    Button as KitButton, ButtonCustomVariant, ButtonVariant as KitVariant, ButtonVariants,
};
use gpui::component::{Disableable, Size, Sizable};
use gpui::prelude::*;
use gpui::*;

use crate::tokens::colors::{
    blurple, blurple_active, blurple_hover, rgba, rgb, text_normal, white,
};
use crate::tokens::spacing::radius_md;
use crate::tokens::typography::WEIGHT_MEDIUM;

/// Visual variants for Discord buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Discord Brand Blurple (#5865F2)
    #[default]
    Blurple,
    /// Secondary translucent gray button
    Gray,
    /// Destructive Danger Red (#D22D39)
    Danger,
    /// Ghost transparent button
    Ghost,
}

/// Sizing options for Discord buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// Compact button (height 32px, 14px text)
    Sm,
    /// Standard Discord button (height 40px, 16px text)
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

/// Discord secondary gray: rgb(151,151,159) at 12% alpha (measured
/// `--control-secondary-background-default`, live `rgba(151, 151, 159, 0.12)`).
#[inline]
fn secondary_bg() -> Rgba {
    rgba(0x97979f1f)
}

/// Discord secondary hover: same gray at 16% alpha (live hover match).
#[inline]
fn secondary_bg_hover() -> Rgba {
    rgba(0x97979f29)
}

/// Discord secondary active: same gray at 20% alpha
/// (`--control-secondary-background-active`).
#[inline]
fn secondary_bg_active() -> Rgba {
    rgba(0x97979f33)
}

/// Discord danger red: #D22D39 (measured live `rgb(210, 45, 57)`; the previous
/// hardcoded #DA373C diverges).
#[inline]
fn danger_bg() -> Rgba {
    rgb(0xd22d39)
}

/// Discord danger hover: #A9232E (measured live + CSS variable).
#[inline]
fn danger_bg_hover() -> Rgba {
    rgb(0xa9232e)
}

/// Discord danger active: #971D28 (`--control-critical-primary-…-active`).
#[inline]
fn danger_bg_active() -> Rgba {
    rgb(0x971d28)
}

/// Discord critical-secondary text: #F87E7A (measured live).
#[inline]
fn danger_text() -> Rgba {
    rgb(0xf87e7a)
}

/// Discord icon-only glyph color: #ABACB2 (measured live +
/// `--control-icon-only-icon-default`).
#[inline]
fn icon_muted() -> Rgba {
    rgb(0xabacb2)
}

/// Builds a kit `Custom` variant carrying the exact Discord colors for the
/// hover/active states (the kit applies them verbatim). The normal background
/// is faded 20% toward transparent by the kit, so callers additionally refine
/// `.bg(exact)` on top; per-state styles still win for hover/active. The
/// custom `foreground` is the hover/active text color; the normal text color
/// is refined on top the same way.
fn kit_custom(cx: &App, bg: Rgba, fg_hover: Rgba, hover: Rgba, active: Rgba) -> KitVariant {
    KitVariant::Custom(
        ButtonCustomVariant::new(cx)
            .color(bg.into())
            .foreground(fg_hover.into())
            .hover(hover.into())
            .active(active.into()),
    )
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        render_button(self, cx)
    }
}

/// Builds the kit button for [`Button`]. Kept as a free function returning
/// `AnyElement` so the `rustc` watcher never sees one giant expression.
///
/// NOTE: only base-style refinements (`.bg`, `.text_color`, …) are applied to
/// the kit button here. Setting `.hover()` on it would clash with the hover
/// style the kit wires internally, so hover/active colors travel through the
/// `Custom` variant instead.
fn render_button(btn: Button, cx: &mut App) -> AnyElement {
    // Measured Discord geometry: sm h32/px11/min-w60, md h40/px15/min-w100.
    // Kit text sizes line up via Size::Small (14px) and Size::Large (16px);
    // heights/padding are overridden below.
    let (height, pad_x, min_w, kit_size) = match btn.size {
        ButtonSize::Sm => (px(32.0), px(11.0), Some(px(60.0)), Size::Small),
        ButtonSize::Md => (px(40.0), px(15.0), Some(px(100.0)), Size::Large),
        // Lg has no measured Discord counterpart in the inspected surfaces;
        // values carried over from the previous implementation.
        ButtonSize::Lg => (px(44.0), px(24.0), None, Size::Large),
    };

    let (bg_normal, bg_hover, bg_active, text_color_normal) = match btn.variant {
        ButtonVariant::Blurple => (
            blurple(),
            blurple_hover(),
            blurple_active(),
            white(),
        ),
        ButtonVariant::Gray => (
            secondary_bg(),
            secondary_bg_hover(),
            secondary_bg_active(),
            white(),
        ),
        ButtonVariant::Danger => (
            danger_bg(),
            danger_bg_hover(),
            danger_bg_active(),
            white(),
        ),
        // No measured Discord sample for a ghost button; values carried over.
        ButtonVariant::Ghost => (
            rgba(0x00000000),
            rgba(0xffffff14),
            rgba(0xffffff24),
            text_normal(),
        ),
    };

    let mut el = KitButton::new(btn.id)
        .with_variant(kit_custom(
            cx,
            bg_normal,
            white(),
            bg_hover,
            bg_active,
        ))
        .with_size(kit_size)
        .disabled(btn.disabled)
        .rounded(radius_md())
        .h(height)
        .px(pad_x)
        .font_weight(WEIGHT_MEDIUM)
        .bg(bg_normal)
        .text_color(text_color_normal);

    if let Some(min_w) = min_w {
        el = el.min_w(min_w);
    }

    if btn.full_width {
        el = el.w_full();
    }

    if btn.disabled {
        // Discord `:disabled { opacity: 0.5 }`: keep exact normal colors and
        // fade the whole button. The kit wires no hover/active/click while
        // disabled, matching the previous implementation.
        el = el.opacity(0.5);
    } else if let Some(on_click) = btn.on_click {
        el = el
            .cursor_pointer()
            .on_click(move |ev, window, cx| {
                (on_click)(ev, window, cx);
            });
    } else {
        el = el.cursor_pointer();
    }

    // Content order (leading icon, label, trailing icon) is preserved by
    // passing everything as ordered children; the kit centers them with its
    // content row.
    if let Some(icon) = btn.icon {
        el = el.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .child(icon),
        );
    }

    if let Some(label) = btn.label {
        el = el.child(div().flex().items_center().child(label));
    }

    if let Some(trailing) = btn.trailing_icon {
        el = el.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .child(trailing),
        );
    }

    el.into_any_element()
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        render_icon_button(self, cx)
    }
}

/// Builds the kit button for [`IconButton`]. Discord icon-only buttons are
/// 32x32 with radius 8px, transparent background, #ABACB2 glyph, 16%/20%
/// translucent-gray hover/active backgrounds and a white glyph on hover.
/// Same refinement rules as [`render_button`]: no `.hover()` on the kit
/// button, hover/active colors via the `Custom` variant.
fn render_icon_button(btn: IconButton, cx: &mut App) -> AnyElement {
    let (icon_normal, bg_normal, bg_hover, fg_hover) = if btn.danger {
        // Measured `critical-secondary` pattern: red text on translucent gray.
        (danger_text(), rgba(0x00000000), secondary_bg_hover(), danger_text())
    } else if btn.active {
        // Toggled state has no measured Discord sample; values carried over.
        (white(), rgba(0xffffff24), rgba(0xffffff33), white())
    } else {
        (icon_muted(), rgba(0x00000000), secondary_bg_hover(), white())
    };
    let bg_active = if btn.danger || btn.active {
        bg_hover
    } else {
        secondary_bg_active()
    };

    let mut el = KitButton::new(btn.id)
        .with_variant(kit_custom(cx, bg_normal, fg_hover, bg_hover, bg_active))
        .with_size(Size::Medium)
        .disabled(btn.disabled)
        .rounded(radius_md())
        .w(btn.size)
        .h(btn.size)
        .font_weight(WEIGHT_MEDIUM)
        .bg(bg_normal)
        .text_color(icon_normal);

    if btn.disabled {
        el = el.opacity(0.5);
    } else {
        el = el.cursor_pointer();

        if let Some(on_click) = btn.on_click {
            el = el.on_click(move |ev, window, cx| {
                (on_click)(ev, window, cx);
            });
        }
    }

    if let Some(tooltip) = btn.tooltip_text {
        el = el.tooltip(tooltip);
    }

    el.child(
        div()
            .flex()
            .items_center()
            .justify_center()
            .text_size(btn.icon_size)
            .child(btn.icon),
    )
    .into_any_element()
}
