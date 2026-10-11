//! Discord RS - TextInput Component
//! Discord-styled single-line text field over the gpui-kit [`Input`] engine.
//! The kit owns selection, clipboard, IME, undo history and the text itself
//! (kept in an `Entity<InputState>` owned by the calling view); this wrapper
//! only draws the Discord chrome around it:
//! - Top label (uppercase, required asterisk, optional error message)
//! - Fixed 40px box (Discord input background, state border, small radius)
//! - Clear button, "Colar" paste button, secret eye toggle (all Discord icons)
//! - Placeholder/disabled/focus visuals from `src/tokens/`
//!
//! Event flow: callers subscribe to `InputEvent::{Change, PressEnter}` on the
//! state they own (see `RootView`), instead of the old controlled
//! `.value()/.on_change()/.on_enter()` props.

use gpui::component::Theme;
use gpui::component::input::{Input as KitInput, InputState};
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

pub type TextInputChangeCallback = Arc<dyn Fn(String, &mut Window, &mut App) + Send + Sync + 'static>;
pub type TextInputActionCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>;
pub type TextInputMaskCallback = Arc<dyn Fn(bool, &mut Window, &mut App) + Send + Sync + 'static>;

/// Activates the Discord kit theme once, so the kit engine draws text,
/// placeholder, caret and selection with Discord colors instead of the kit
/// light default. `main.rs` cannot be touched by the input wave, so the kit
/// consumer does it here; a later `install` call from `main.rs` is idempotent
/// and simply re-applies the same theme.
fn ensure_discord_theme(cx: &mut App) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.load(Ordering::Relaxed) {
        return;
    }
    // Only when kit globals exist (production always; bare test contexts skip).
    if cx.try_global::<Theme>().is_some() {
        crate::tokens::kit_theme::install(cx);
        DONE.store(true, Ordering::Relaxed);
    }
}

/// Discord styled text input field over a caller-owned kit [`InputState`].
#[derive(IntoElement)]
pub struct TextInput {
    id: ElementId,
    state: Entity<InputState>,
    label: Option<SharedString>,
    required: bool,
    error: Option<SharedString>,
    placeholder: Option<SharedString>,
    is_secret: bool,
    disabled: bool,
    show_clear: bool,
    show_paste: bool,
    full_width: bool,
}

impl TextInput {
    /// Creates a new text input bound to a caller-owned kit state.
    /// The caller creates the state (e.g. in its view), subscribes to
    /// `InputEvent` for `Change`/`PressEnter`, and syncs programmatic value
    /// changes via `InputState::set_value`.
    pub fn new(id: impl Into<ElementId>, state: &Entity<InputState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            label: None,
            required: false,
            error: None,
            placeholder: None,
            is_secret: false,
            disabled: false,
            show_clear: true,
            show_paste: true,
            full_width: true,
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

    /// Sets placeholder string (forwarded to the kit state when it differs)
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Designates field as secret/password/token (shows the eye toggle)
    pub fn secret(mut self, is_secret: bool) -> Self {
        self.is_secret = is_secret;
        self
    }

    /// Sets disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
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

    pub fn is_secret_field(&self) -> bool {
        self.is_secret
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

/// Top label + required asterisk + error row. `None` when no label/error.
fn render_label_row(
    label: Option<SharedString>,
    required: bool,
    error: Option<SharedString>,
    has_error: bool,
) -> Option<AnyElement> {
    if label.is_none() && error.is_none() {
        return None;
    }
    let mut label_row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .text_size(font_size_xs())
        .font_weight(FontWeight::BOLD);

    if let Some(label) = label {
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

        if required {
            label_el = label_el.child(div().text_color(status_dnd()).child("*"));
        }

        label_row = label_row.child(label_el);
    }

    if let Some(err) = error {
        label_row = label_row.child(
            div()
                .text_color(status_dnd())
                .text_size(font_size_xs())
                .font_weight(FontWeight::MEDIUM)
                .child(format!("- {}", err)),
        );
    }

    Some(label_row.into_any_element())
}

/// Clear (X) button. Deletes via the kit engine (emits `Change`).
fn render_clear_button(state: &Entity<InputState>) -> AnyElement {
    let st = state.clone();
    div()
        .size(px(20.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(text_muted())
        .hover(|s| s.bg(channel_hover()).text_color(white()))
        .child(icon_close(px(12.0), text_muted()))
        .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
            st.update(cx, |s, cx| s.clean(window, cx));
            let fh = st.read(cx).focus_handle(cx);
            window.focus(&fh, cx);
        })
        .into_any_element()
}

/// "Colar" paste button. Inserts the clipboard at the cursor (emits `Change`).
fn render_paste_button(id: ElementId, state: &Entity<InputState>) -> AnyElement {
    let st = state.clone();
    div()
        .id((id, "paste-action-btn"))
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
        .child(div().child("Colar"))
        .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
            if let Some(clipboard) = cx.read_from_clipboard() {
                if let Some(text) = clipboard.text() {
                    st.update(cx, |s, cx| s.insert(text, window, cx));
                }
            }
            let fh = st.read(cx).focus_handle(cx);
            window.focus(&fh, cx);
        })
        .into_any_element()
}

/// Secret eye toggle. Flips the kit mask state (rendered text follows).
fn render_mask_button(state: &Entity<InputState>, masked: bool) -> AnyElement {
    let st = state.clone();
    let eye_icon = if masked {
        icon_eye(px(16.0), text_muted())
    } else {
        icon_eye_off(px(16.0), text_muted())
    };
    div()
        .size(px(24.0))
        .rounded(radius_sm())
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(text_muted())
        .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
        .child(eye_icon)
        .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
            st.update(cx, |s, cx| s.toggle_masked(window, cx));
        })
        .into_any_element()
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        ensure_discord_theme(cx);

        // Forward placeholder to the kit state when it differs.
        if let Some(ref ph) = self.placeholder {
            if self.state.read(cx).presentation().placeholder() != ph {
                let ph = ph.clone();
                self.state.update(cx, |s, cx| {
                    s.set_placeholder(ph, window, cx);
                });
            }
        }

        let has_error = self.error.is_some();
        let is_empty = self.state.read(cx).value().is_empty();
        let masked = self.state.read(cx).presentation().is_masked();
        let is_focused = self.state.read(cx).focus_handle(cx).is_focused(window);

        // Border color logic based on state (same tokens as before).
        let border_color = if has_error {
            status_dnd() // Red #f23f43
        } else if is_focused {
            blurple() // Blurple #5865f2
        } else {
            from_hex(HEX_BG_SERVER) // Dark #1e1f22
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
        if let Some(row) = render_label_row(self.label, self.required, self.error, has_error) {
            container = container.child(row);
        }

        // 2. Input Box Pill (Discord chrome; the kit engine draws text inside).
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
            input_box = input_box.opacity(0.5).cursor_not_allowed();
        } else {
            input_box = input_box.cursor_text();

            if !is_focused && !has_error {
                input_box = input_box.hover(|s| s.border_color(borders()));
            }

            // Clicking the box padding focuses the kit engine.
            let st = self.state.clone();
            input_box = input_box.on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                let fh = st.read(cx).focus_handle(cx);
                if !fh.is_focused(window) {
                    window.focus(&fh, cx);
                }
            });
        }

        // Kit engine: stripped of its own chrome (appearance/border/focus
        // ring/clear button); Discord box above provides all of it. Height,
        // text size and horizontal padding overridden to Discord measures.
        let kit = KitInput::new(&self.state)
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
            .cleanable(false)
            .disabled(self.disabled)
            .h(px(40.0))
            .w_auto()
            .flex_1()
            .px(px(0.0))
            .text_size(font_size_base());
        input_box = input_box.child(kit);

        // 3. Right Action Buttons (Clear + Colar + Secret Mask Toggle).
        let mut right_actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_2());

        if !is_empty && self.show_clear && !self.disabled {
            right_actions = right_actions.child(render_clear_button(&self.state));
        }

        if self.show_paste && !self.disabled {
            right_actions =
                right_actions.child(render_paste_button(self.id.clone(), &self.state));
        }

        if self.is_secret && !self.disabled {
            right_actions = right_actions.child(render_mask_button(&self.state, masked));
        }

        input_box = input_box.child(right_actions);
        container = container.child(input_box);

        container
    }
}
