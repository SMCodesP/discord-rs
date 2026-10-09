//! Discord RS - Modal Component
//! Modal dialog overlay with backdrop scrim, header, scrollable body, and action footer.

use gpui::prelude::*;
use gpui::*;

use crate::components::button::{Button, ButtonVariant, IconButton};
use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Discord modal dialog overlay
#[derive(IntoElement)]
pub struct Modal {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    width: Pixels,
    show_close_button: bool,
    children: Vec<AnyElement>,
    footer_elements: Vec<AnyElement>,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Modal {
    /// Creates a new modal dialog with an ID and title
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            width: px(440.0),
            show_close_button: true,
            children: Vec::new(),
            footer_elements: Vec::new(),
            on_close: None,
        }
    }

    /// Sets optional subtitle description
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Sets modal card width
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// Whether to show the top-right "✕" close button
    pub fn show_close_button(mut self, show: bool) -> Self {
        self.show_close_button = show;
        self
    }

    /// Sets close callback
    pub fn on_close(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    /// Appends a child element to the modal body
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element().into_any_element());
        self
    }

    /// Appends an action element to the modal footer
    pub fn footer_child(mut self, child: impl IntoElement) -> Self {
        self.footer_elements.push(child.into_element().into_any_element());
        self
    }

    /// Helper to set standard Cancel and Confirm footer buttons
    pub fn with_standard_footer(
        mut self,
        confirm_label: impl Into<SharedString>,
        confirm_variant: ButtonVariant,
        on_confirm: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let cancel_on_close = self.on_close.is_some();
        let cancel_btn = Button::ghost("modal_cancel", "Cancelar")
            .on_click(move |_ev, _window, cx| {
                if cancel_on_close {
                    // Handled if hooked
                }
                cx.stop_propagation();
            });

        let confirm_btn = Button::new("modal_confirm", confirm_label)
            .variant(confirm_variant)
            .on_click(on_confirm);

        self.footer_elements.push(cancel_btn.into_any_element());
        self.footer_elements.push(confirm_btn.into_any_element());
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_close_rc = self.on_close.map(std::rc::Rc::new);

        // Backdrop overlay
        let mut backdrop = div()
            .id(self.id)
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(HEX_MODAL_BACKDROP))
            .flex()
            .items_center()
            .justify_center()
            .cursor_default();

        if let Some(on_close) = on_close_rc.clone() {
            backdrop = backdrop.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                (on_close)(window, cx);
            });
        }

        // Modal Dialog Card
        let mut card = div()
            .w(self.width)
            .max_w(px(600.0))
            .bg(bg_chat()) // #313338
            .rounded(radius_lg()) // 12px
            .border_1()
            .border_color(rgba(0xffffff14))
            .overflow_hidden()
            .flex()
            .flex_col()
            .cursor_default()
            // Prevent backdrop click inside card
            .on_mouse_down(MouseButton::Left, |_ev, _window, cx| {
                cx.stop_propagation();
            });

        // Header Section
        let mut header = div()
            .px(space_12())
            .pt(space_12())
            .pb(space_4())
            .flex()
            .flex_col()
            .gap(space_1());

        let mut title_row = div()
            .flex()
            .items_center()
            .justify_between();

        title_row = title_row.child(
            div()
                .text_size(font_size_xl())
                .font_weight(FontWeight::BOLD)
                .text_color(text_header())
                .child(self.title),
        );

        if self.show_close_button {
            let mut close_btn = IconButton::new("modal_close_btn", "✕")
                .size(px(28.0))
                .icon_size(px(14.0));

            if let Some(on_close) = on_close_rc {
                close_btn = close_btn.on_click(move |_ev, window, cx| {
                    (on_close)(window, cx);
                });
            }

            title_row = title_row.child(close_btn);
        }

        header = header.child(title_row);

        if let Some(subtitle) = self.subtitle {
            header = header.child(
                div()
                    .text_size(font_size_sm())
                    .text_color(text_muted())
                    .child(subtitle),
            );
        }

        card = card.child(header);

        // Body Content
        let mut body = div()
            .px(space_12())
            .py(space_4())
            .flex()
            .flex_col()
            .gap(space_4())
            .text_size(font_size_base())
            .text_color(text_normal());

        for child in self.children {
            body = body.child(child);
        }

        card = card.child(body);

        // Footer Section (if any elements present)
        if !self.footer_elements.is_empty() {
            let mut footer = div()
                .px(space_12())
                .py(space_8())
                .bg(bg_channels()) // #2B2D31
                .flex()
                .items_center()
                .justify_end()
                .gap(space_4());

            for elem in self.footer_elements {
                footer = footer.child(elem);
            }

            card = card.child(footer);
        }

        backdrop.child(card)
    }
}
