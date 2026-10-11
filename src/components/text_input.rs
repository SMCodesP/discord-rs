//! Discord RS - TextInput Component
//! Native GPUI text input styled with Discord Dark Theme tokens.
//! Supports:
//! - Top label (uppercase, required asterisk, optional error message)
//! - Placeholder and text value display
//! - Focus, hover, and disabled visual states
//! - Secret / password / token masking toggle (bullet points vs visible text)
//! - Clear text button
//! - `on_change(String)` callback and keyboard typing support

use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

pub type TextInputChangeCallback = Arc<dyn Fn(String, &mut Window, &mut App) + Send + Sync + 'static>;
pub type TextInputActionCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>;
pub type TextInputMaskCallback = Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>;

/// Discord styled text input field
#[derive(IntoElement)]
pub struct TextInput {
    id: ElementId,
    label: Option<SharedString>,
    required: bool,
    error: Option<SharedString>,
    value: SharedString,
    placeholder: Option<SharedString>,
    is_secret: bool,
    masked: bool,
    disabled: bool,
    focused: bool,
    focus_handle: Option<FocusHandle>,
    show_clear: bool,
    show_paste: bool,
    full_width: bool,
    on_change: Option<TextInputChangeCallback>,
    on_toggle_mask: Option<TextInputMaskCallback>,
    on_clear: Option<TextInputActionCallback>,
    on_click: Option<TextInputActionCallback>,
    on_enter: Option<TextInputActionCallback>,
}

impl TextInput {
    /// Creates a new text input with a unique ID
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            required: false,
            error: None,
            value: "".into(),
            placeholder: None,
            is_secret: false,
            masked: true,
            disabled: false,
            focused: false,
            focus_handle: None,
            show_clear: true,
            show_paste: true,
            full_width: true,
            on_change: None,
            on_toggle_mask: None,
            on_clear: None,
            on_click: None,
            on_enter: None,
        }
    }

    /// Sets the top label (e.g. "TOKEN DE AUTENTICAÇÃO")
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Marks field as required (renders red asterisk next to label)
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Sets validation error message (highlights border in red and displays error)
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Sets current text value
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = value.into();
        self
    }

    /// Sets placeholder string
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Designates field as secret/password/token
    pub fn secret(mut self, is_secret: bool) -> Self {
        self.is_secret = is_secret;
        self
    }

    /// Sets whether secret is currently masked (true = ••••••, false = visible)
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// Sets disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Sets visual focus state
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Associates a GPUI FocusHandle to automatically track window focus and handle keyboard input
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.focus_handle = Some(handle.clone());
        self
    }

    /// Sets whether to show clear button when text is not empty
    pub fn show_clear(mut self, show_clear: bool) -> Self {
        self.show_clear = show_clear;
        self
    }

    /// Sets whether to show direct paste button
    pub fn show_paste(mut self, show_paste: bool) -> Self {
        self.show_paste = show_paste;
        self
    }

    /// Whether input should take 100% width
    pub fn full_width(mut self, full: bool) -> Self {
        self.full_width = full;
        self
    }

    /// Callback fired whenever the text value changes
    pub fn on_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    /// Callback fired when pressing Enter
    pub fn on_enter(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_enter = Some(Arc::new(handler));
        self
    }

    /// Callback fired when toggling secret mask
    pub fn on_toggle_mask(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_toggle_mask = Some(Arc::new(handler));
        self
    }

    /// Callback fired when clicking the clear button
    pub fn on_clear(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_clear = Some(Arc::new(handler));
        self
    }

    /// Callback fired when clicking the input area
    pub fn on_click(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }

    pub fn get_id(&self) -> &ElementId {
        &self.id
    }

    pub fn get_label(&self) -> Option<&str> {
        self.label.as_ref().map(|s| s.as_ref())
    }

    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn get_placeholder(&self) -> Option<&str> {
        self.placeholder.as_ref().map(|s| s.as_ref())
    }

    pub fn get_value(&self) -> &str {
        self.value.as_ref()
    }

    pub fn is_secret_field(&self) -> bool {
        self.is_secret
    }

    pub fn is_masked(&self) -> bool {
        self.masked
    }

    pub fn get_error(&self) -> Option<&str> {
        self.error.as_ref().map(|s| s.as_ref())
    }

    pub fn shows_clear(&self) -> bool {
        self.show_clear
    }

    pub fn shows_paste(&self) -> bool {
        self.show_paste
    }

    pub fn is_full_width(&self) -> bool {
        self.full_width
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let has_error = self.error.is_some();
        let focus_handle = self
            .focus_handle
            .clone()
            .unwrap_or_else(|| cx.focus_handle());
        let is_focused = self.focused || focus_handle.is_focused(window);

        // Border color logic based on state
        let border_color = if has_error {
            status_dnd() // Red #f23f43
        } else if is_focused {
            blurple() // Blurple #5865f2
        } else {
            from_hex(HEX_BG_SERVER) // Dark #1e1f22
        };

        // Compute display string (masked vs plain)
        let is_empty = self.value.is_empty();
        let display_text = if is_empty {
            self.placeholder.clone().unwrap_or_else(|| "".into())
        } else if self.is_secret && self.masked {
            let char_count = self.value.chars().count();
            "•".repeat(char_count.min(32)).into()
        } else {
            self.value.clone()
        };

        let mut container = div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .gap(space_2());

        if self.full_width {
            container = container.w_full();
        }

        // 1. Top Label & Error Row
        if self.label.is_some() || self.error.is_some() {
            let mut label_row = div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .text_size(font_size_xs())
                .font_weight(FontWeight::BOLD);

            // Left side: Label + Required Asterisk
            if let Some(label) = self.label {
                let mut label_el = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .text_color(if has_error {
                        status_dnd()
                    } else {
                        text_muted()
                    })
                    .child(label);

                if self.required {
                    label_el = label_el.child(
                        div()
                            .text_color(status_dnd())
                            .child("*"),
                    );
                }

                label_row = label_row.child(label_el);
            }

            // Right side: Error message
            if let Some(err) = self.error {
                label_row = label_row.child(
                    div()
                        .text_color(status_dnd())
                        .text_size(font_size_xs())
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("- {}", err)),
                );
            }

            container = container.child(label_row);
        }

        // 2. Input Box Pill
        let mut input_box = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(40.0))
            .w_full()
            .px(space_4())
            .rounded(radius_sm())
            .bg(rgb(HEX_BG_INPUT))
            .border_1()
            .border_color(border_color)
            .gap(space_2());

        if self.disabled {
            input_box = input_box
                .opacity(0.5)
                .cursor_not_allowed();
        } else {
            input_box = input_box.cursor_text();

            if !is_focused && !has_error {
                input_box = input_box.hover(|s| s.border_color(borders()));
            }

            // Track active focus on the input box
            input_box = input_box.track_focus(&focus_handle);

            // Click to explicitly focus the focus_handle
            let fh_click = focus_handle.clone();
            let on_click = self.on_click.clone();
            input_box = input_box.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                window.focus(&fh_click, cx);
                if let Some(ref click) = on_click {
                    click(window, cx);
                }
            });

            // Keyboard typing, backspace, and clipboard paste handling
            let fh_key = focus_handle.clone();
            let current_val = self.value.to_string();
            let on_change = self.on_change.clone();
            let on_enter = self.on_enter.clone();

            input_box = input_box.on_key_down(move |ev: &KeyDownEvent, window, cx| {
                window.focus(&fh_key, cx);

                let key = &ev.keystroke.key;
                let is_ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
                let is_paste = (is_ctrl && (key == "v" || key == "V"))
                    || (ev.keystroke.modifiers.shift && key == "insert");

                // Paste from clipboard (Ctrl+V, Command+V, Shift+Insert)
                if is_paste {
                    if let Some(clipboard) = cx.read_from_clipboard() {
                        if let Some(text) = clipboard.text() {
                            if let Some(ref cb) = on_change {
                                let mut new_str = current_val.clone();
                                new_str.push_str(&text);
                                cb(new_str, window, cx);
                            }
                        }
                    }
                    return;
                }

                // Ignore other Ctrl/Cmd shortcuts so control sequences aren't typed
                if is_ctrl {
                    return;
                }

                match key.as_str() {
                    "enter" => {
                        if let Some(ref enter_cb) = on_enter {
                            enter_cb(window, cx);
                        }
                    }
                    "backspace" => {
                        if let Some(ref cb) = on_change {
                            let mut new_str = current_val.clone();
                            new_str.pop();
                            cb(new_str, window, cx);
                        }
                    }
                    "space" => {
                        if let Some(ref cb) = on_change {
                            let mut new_str = current_val.clone();
                            new_str.push(' ');
                            cb(new_str, window, cx);
                        }
                    }
                    k if k.chars().count() == 1 => {
                        if let Some(ref cb) = on_change {
                            let mut new_str = current_val.clone();
                            new_str.push_str(k);
                            cb(new_str, window, cx);
                        }
                    }
                    _ => {}
                }
            });
        }

        // Center Text Value or Placeholder with visual cursor indicator
        let cursor_el = if is_focused {
            Some(
                div()
                    .w(px(2.0))
                    .h(px(16.0))
                    .bg(blurple())
                    .rounded(px(1.0))
                    .ml(px(2.0)),
            )
        } else {
            None
        };

        let mut text_element = div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .overflow_hidden()
            .text_size(font_size_base())
            .font_weight(FontWeight::NORMAL);

        if is_empty {
            text_element = text_element
                .text_color(text_muted())
                .child(display_text);
            if let Some(cursor) = cursor_el {
                text_element = text_element.child(cursor);
            }
        } else {
            text_element = text_element
                .text_color(text_primary())
                .child(display_text);
            if let Some(cursor) = cursor_el {
                text_element = text_element.child(cursor);
            }
        }

        input_box = input_box.child(text_element);

        // Right Action Buttons (Clear + Direct Paste + Secret Mask Toggle)
        let mut right_actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_2());

        // Clear Button (✕)
        if !is_empty && self.show_clear && !self.disabled {
            let on_clear = self.on_clear.clone();
            let on_change = self.on_change.clone();

            let mut clear_btn = div()
                .size(px(20.0))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(text_muted())
                .hover(|s| s.bg(channel_hover()).text_color(white()))
                .child(icon_close(px(12.0), text_muted()));

            clear_btn = clear_btn.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                if let Some(ref cb) = on_change {
                    cb("".to_string(), window, cx);
                }
                if let Some(ref clr) = on_clear {
                    clr(window, cx);
                }
            });

            right_actions = right_actions.child(clear_btn);
        }

        // Direct Action Button: Paste (Colar)
        if self.show_paste && !self.disabled {
            let on_change_paste = self.on_change.clone();
            let fh_paste = focus_handle.clone();
            let current_val_paste = self.value.to_string();

            let paste_btn = div()
                .id((self.id.clone(), "paste-action-btn"))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded(radius_xs())
                .bg(rgb(0x35373c))
                .text_color(text_muted())
                .text_size(font_size_xs())
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(|s| s.bg(rgb(0x404249)).text_color(text_titles()))
                .child(icon_paste(px(14.0), text_muted()))
                .child(
                    div()
                        .child("Colar"),
                )
                .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                    window.focus(&fh_paste, cx);
                    if let Some(clipboard) = cx.read_from_clipboard() {
                        if let Some(text) = clipboard.text() {
                            if let Some(ref cb) = on_change_paste {
                                let mut new_str = current_val_paste.clone();
                                new_str.push_str(&text);
                                cb(new_str, window, cx);
                            }
                        }
                    }
                });

            right_actions = right_actions.child(paste_btn);
        }

        // Secret Mask Toggle (Eye icon)
        if self.is_secret && !self.disabled {
            let next_masked = !self.masked;
            let on_toggle_mask = self.on_toggle_mask.clone();

            let eye_icon = if self.masked {
                icon_eye(px(16.0), text_muted())
            } else {
                icon_eye_off(px(16.0), text_muted())
            };

            let mut toggle_btn = div()
                .size(px(24.0))
                .rounded(radius_sm())
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(text_muted())
                .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
                .child(eye_icon);

            if let Some(handler) = on_toggle_mask {
                toggle_btn = toggle_btn.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                    handler(next_masked, window, cx);
                });
            }

            right_actions = right_actions.child(toggle_btn);
        }

        input_box = input_box.child(right_actions);
        container = container.child(input_box);

        container
    }
}
