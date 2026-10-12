//! Discord RS - Banner Component
//! Status and alert banners with Info, Success, Warning, and Danger variants.
//! Designed for displaying authentication feedback, demonstration modes, and connection warnings.

use gpui::prelude::*;
use gpui::*;

use crate::tokens::colors::{white, *};
use crate::tokens::icons::{icon_check, icon_close, icon_info, icon_warning};
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Visual variants for Discord alert and status banners
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BannerVariant {
    /// Information / Demo mode banner (Blurple #5865F2)
    #[default]
    Info,
    /// Success / Connected status banner (Green #23A55A)
    Success,
    /// Warning / Rate limit / Pending status banner (Yellow #F0B232)
    Warning,
    /// Error / Authentication failure banner (Red #F23F43)
    Danger,
}

impl BannerVariant {
    /// Returns the primary accent color associated with this variant
    pub fn accent_color(&self) -> Rgba {
        match self {
            Self::Info => blurple(),
            Self::Success => status_online(),
            Self::Warning => status_ausente(),
            Self::Danger => status_dnd(),
        }
    }

    /// Default translucent background with ~15% opacity
    pub fn bg_color(&self) -> Rgba {
        match self {
            Self::Info => with_alpha(HEX_BLURPLE, 0.15),
            Self::Success => with_alpha(HEX_STATUS_ONLINE, 0.15),
            Self::Warning => with_alpha(HEX_STATUS_AUSENTE, 0.15),
            Self::Danger => with_alpha(HEX_STATUS_DND, 0.15),
        }
    }

    /// Canonical icon symbol for the variant
    pub fn default_icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ️",
            Self::Success => "✅",
            Self::Warning => "⚠️",
            Self::Danger => "❌",
        }
    }
}

/// Alert, notification, and status banner element
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    variant: BannerVariant,
    title: Option<SharedString>,
    message: SharedString,
    icon: Option<SharedString>,
    action_label: Option<SharedString>,
    dismissible: bool,
    full_width: bool,
    on_action: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_dismiss: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Banner {
    /// Creates a new informational banner with an ID and message
    pub fn new(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            variant: BannerVariant::Info,
            title: None,
            message: message.into(),
            icon: None,
            action_label: None,
            dismissible: false,
            full_width: true,
            on_action: None,
            on_dismiss: None,
        }
    }

    /// Creates an Info banner (e.g. "Atualização disponível")
    pub fn info(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self::new(id, message).variant(BannerVariant::Info)
    }

    /// Creates a Success banner (e.g. "Conectado como @rustacean")
    pub fn success(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self::new(id, message).variant(BannerVariant::Success)
    }

    /// Creates a Warning banner (e.g. "A conexão está instável")
    pub fn warning(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self::new(id, message).variant(BannerVariant::Warning)
    }

    /// Creates a Danger banner (e.g. "Falha ao autenticar token")
    pub fn danger(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self::new(id, message).variant(BannerVariant::Danger)
    }

    /// Sets the variant
    pub fn variant(mut self, variant: BannerVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets an optional bold header/title
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Overrides the default icon with a custom symbol or emoji
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Adds an action button with a label and click callback
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action_label = Some(label.into());
        self.on_action = Some(Box::new(handler));
        self
    }

    /// Enables the dismiss ("✕") button
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// Sets dismiss callback
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.dismissible = true;
        self.on_dismiss = Some(Box::new(handler));
        self
    }

    /// Whether banner should expand to 100% width
    pub fn full_width(mut self, full: bool) -> Self {
        self.full_width = full;
        self
    }

    pub fn get_variant(&self) -> BannerVariant {
        self.variant
    }

    pub fn get_title(&self) -> Option<&str> {
        self.title.as_ref().map(|s| s.as_ref())
    }

    pub fn get_message(&self) -> &str {
        self.message.as_ref()
    }

    pub fn get_id(&self) -> &ElementId {
        &self.id
    }

    pub fn is_dismissible(&self) -> bool {
        self.dismissible
    }

    pub fn is_full_width(&self) -> bool {
        self.full_width
    }

    pub fn get_action_label(&self) -> Option<&str> {
        self.action_label.as_ref().map(|s| s.as_ref())
    }
}

impl RenderOnce for Banner {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let accent = self.variant.accent_color();
        let bg = self.variant.bg_color();
        let icon_el = if let Some(custom) = self.icon {
            div()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(18.0))
                .child(custom)
        } else {
            let hsla_accent: Hsla = accent.into();
            match self.variant {
                BannerVariant::Info => icon_info(px(20.0), hsla_accent),
                BannerVariant::Success => icon_check(px(20.0), hsla_accent),
                BannerVariant::Warning => icon_warning(px(20.0), hsla_accent),
                BannerVariant::Danger => icon_close(px(20.0), hsla_accent),
            }
        };

        let mut container = div()
            .id(self.id)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .p(space_4())
            .rounded(radius_md())
            .bg(bg)
            .border_1()
            .border_color(accent)
            .gap(space_4());

        if self.full_width {
            container = container.w_full();
        }

        // Left Side: Icon + Message Content
        // overflow_hidden on both flex containers lets long messages wrap
        // inside the box instead of overflowing it (same technique as the
        // kit Alert: flex_1 + overflow_hidden on each text ancestor).
        let mut left_side = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_4())
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden();

        // Banner Icon
        left_side = left_side.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(20.0))
                .flex_shrink_0()
                .child(icon_el),
        );

        // Text Content (Title + Message)
        let mut text_column = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden();

        if let Some(title) = self.title {
            text_column = text_column.child(
                div()
                    .text_color(text_titles())
                    .font_weight(FontWeight::BOLD)
                    .text_size(font_size_sm())
                    .child(title),
            );
        }

        text_column = text_column.child(
            div()
                .text_color(text_primary())
                .text_size(font_size_sm())
                .child(self.message),
        );

        left_side = left_side.child(text_column);
        container = container.child(left_side);

        // Right Side: Action Button + Dismiss Button
        let mut right_side = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_3())
            .flex_shrink_0();

        // Optional Action Button
        let on_action = self.on_action;
        if let Some(action_label) = self.action_label {
            let mut action_btn = div()
                .id("banner-action-btn")
                .px(space_3())
                .py(space_1())
                .rounded(radius_sm())
                .bg(accent)
                .text_color(white())
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(font_size_xs())
                .cursor_pointer()
                .hover(|s| s.opacity(0.85))
                .child(action_label);

            if let Some(handler) = on_action {
                action_btn = action_btn.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                    handler(window, cx);
                });
            }

            right_side = right_side.child(action_btn);
        }

        // Optional Dismiss Button (✕)
        if self.dismissible {
            let on_dismiss = self.on_dismiss;
            let mut dismiss_btn = div()
                .size(px(24.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(text_muted())
                .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
                .child(icon_close(px(14.0), text_muted()));

            if let Some(handler) = on_dismiss {
                dismiss_btn =
                    dismiss_btn.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                        handler(window, cx);
                    });
            }

            right_side = right_side.child(dismiss_btn);
        }

        container = container.child(right_side);

        container
    }
}
