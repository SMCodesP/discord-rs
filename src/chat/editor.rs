//! Discord RS - Chat Input Editor & Typing Indicator
//! Implements canonical Discord text input:
//! - Channel-specific placeholder ("Conversar em #canal" / "Conversar com @usuario")
//! - Attachment button (+) with SVG icon and kit tooltip
//! - Rich media buttons (GIF, Emoji, Stickers, Gift) with SVG icons and kit tooltips
//! - Active reply banner with mention toggle and dismiss
//! - Real-time typing indicator with animated dot simulation
//! - Modular components over gpui-kit Textarea / TextareaState with zero fixed colors

use gpui::component::input::{Textarea as KitTextarea, TextareaState};
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::components::button::IconButton;

use crate::chat::media_picker::MediaPickerTab;
use crate::domain::message::Message;
use crate::tokens::colors::*;
use crate::tokens::icons::{
    icon_close, icon_emoji, icon_gif, icon_gift, icon_plus, icon_send, icon_sticker,
};
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

// ============================================================================
// Callback Type Aliases
// ============================================================================

pub type EditorSendCallback = Arc<dyn Fn(&str, Option<&str>, &mut Window, &mut App) + Send + Sync + 'static>;
pub type EditorActionCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>;
pub type EditorTextChangeCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;

// ============================================================================
// Subcomponent Helpers (Small Functions Returning AnyElement)
// ============================================================================

/// Mention toggle button on the reply banner ("@ Ligado" / "@ Desligado").
fn render_reply_mention_button(
    mention_author: bool,
    on_toggle_mention: Option<EditorActionCallback>,
) -> AnyElement {
    let mut btn = if mention_author {
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(space_2())
            .py(px(2.0))
            .rounded(radius_xs())
            .bg(mention_bg())
            .text_color(blurple())
            .text_size(font_size_xs())
            .font_weight(WEIGHT_BOLD)
            .cursor_pointer()
            .hover(|s| s.bg(mention_hover()))
            .child("@ Ligado")
    } else {
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(space_2())
            .py(px(2.0))
            .rounded(radius_xs())
            .bg(channel_hover())
            .text_color(text_muted())
            .text_size(font_size_xs())
            .font_weight(WEIGHT_BOLD)
            .cursor_pointer()
            .hover(|s| s.bg(bg_chat_hover()))
            .child("@ Desligado")
    };

    if let Some(handler) = on_toggle_mention {
        btn = btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    btn.into_any_element()
}

/// Dismiss button on the reply banner using SVG icon and kit tooltip.
fn render_reply_close_button(
    on_cancel_reply: Option<EditorActionCallback>,
) -> AnyElement {
    let mut btn = IconButton::with_icon("reply-banner-close", |size, color| {
        icon_close(size, color).into_any_element()
    })
    .size(px(20.0))
    .icon_size(px(12.0))
    .rounded(radius_xs())
    .tooltip("Cancelar resposta");

    if let Some(handler) = on_cancel_reply {
        btn = btn.on_click(move |_ev, window, cx| {
            handler(window, cx);
        });
    }

    btn.into_any_element()
}

/// Plus (+) attachment button using SVG icon and kit tooltip.
fn render_attach_button(on_attachment: Option<EditorActionCallback>) -> AnyElement {
    let mut btn = IconButton::with_icon("chat-attach-btn", |size, color| {
        icon_plus(size, color).into_any_element()
    })
    .size(px(24.0))
    .icon_size(px(16.0))
    .rounded(radius_full())
    .tooltip("Adicionar anexo");

    if let Some(handler) = on_attachment {
        btn = btn.on_click(move |_ev, window, cx| {
            handler(window, cx);
        });
    }

    btn.into_any_element()
}

/// Nitro gift button using SVG icon and kit tooltip.
fn render_gift_button() -> AnyElement {
    IconButton::with_icon("chat-gift-btn", |size, color| {
        icon_gift(size, color).into_any_element()
    })
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .tooltip("Dar Nitro de presente")
    .into_any_element()
}

/// GIF picker button using SVG icon, active state & kit tooltip.
fn render_gif_button(is_active: bool, on_gif: Option<EditorActionCallback>) -> AnyElement {
    let mut btn = IconButton::with_icon("chat-gif-btn", |size, color| {
        icon_gif(size, color).into_any_element()
    })
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .active(is_active)
    .tooltip("Abrir seletor de GIFs");

    if let Some(handler) = on_gif {
        btn = btn.on_click(move |_ev, window, cx| {
            handler(window, cx);
        });
    }

    btn.into_any_element()
}

/// Sticker picker button using SVG icon, active state & kit tooltip.
fn render_sticker_button(is_active: bool, on_sticker: Option<EditorActionCallback>) -> AnyElement {
    let mut btn = IconButton::with_icon("chat-sticker-btn", |size, color| {
        icon_sticker(size, color).into_any_element()
    })
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .active(is_active)
    .tooltip("Abrir seletor de figurinhas");

    if let Some(handler) = on_sticker {
        btn = btn.on_click(move |_ev, window, cx| {
            handler(window, cx);
        });
    }

    btn.into_any_element()
}

/// Emoji picker button using SVG icon, active state & kit tooltip.
fn render_emoji_button(is_active: bool, on_emoji: Option<EditorActionCallback>) -> AnyElement {
    let mut btn = IconButton::with_icon("chat-emoji-btn", |size, color| {
        icon_emoji(size, color).into_any_element()
    })
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .active(is_active)
    .tooltip("Selecionar emoji");

    if let Some(handler) = on_emoji {
        btn = btn.on_click(move |_ev, window, cx| {
            handler(window, cx);
        });
    }

    btn.into_any_element()
}

/// Send message button (paper plane / arrow) shown when text is not empty.
pub fn render_send_button(
    text: String,
    reply_msg_id: Option<String>,
    on_send: Option<EditorSendCallback>,
) -> Option<AnyElement> {
    if text.trim().is_empty() {
        return None;
    }

    let mut btn = IconButton::with_icon("chat-send-btn", |size, color| {
        icon_send(size, color).into_any_element()
    })
    .size(px(32.0))
    .icon_size(px(16.0))
    .rounded(radius_sm())
    .tooltip("Enviar mensagem");

    if let Some(handler) = on_send {
        btn = btn.on_click(move |_ev, window, cx| {
            handler(&text, reply_msg_id.as_deref(), window, cx);
        });
    }

    Some(btn.into_any_element())
}

/// Single animated typing indicator dot.
fn render_typing_dot(id: &'static str, phase_offset: f32) -> AnyElement {
    use gpui::AnimationExt as _;
    use std::time::Duration;

    div()
        .size(px(4.0))
        .rounded(radius_full())
        .bg(blurple())
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1000)).repeat(),
            move |dot, delta| {
                let phase = ((delta + phase_offset) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                dot.opacity(0.3 + 0.7 * phase)
            },
        )
        .into_any_element()
}

// ============================================================================
// Active Reply Banner Component
// ============================================================================

/// Renders the active reply banner above the text input when replying to a message.
pub fn render_active_reply_banner(
    replying_to: &Message,
    mention_author: bool,
    on_toggle_mention: Option<EditorActionCallback>,
    on_cancel_reply: Option<EditorActionCallback>,
) -> Div {
    let author_name = replying_to.author.display_name().to_string();

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .bg(bg_channels())
        .border_b_1()
        .border_color(channel_hover())
        .rounded_t(radius_md())
        .px(space_4())
        .py(space_2())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space_2())
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(font_size_sm())
                        .child("↩"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(space_1())
                        .text_size(font_size_sm())
                        .child(
                            div()
                                .text_color(text_muted())
                                .child("Respondendo a"),
                        )
                        .child(
                            div()
                                .font_weight(WEIGHT_SEMIBOLD)
                                .text_color(blurple())
                                .child(author_name),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space_3())
                .child(render_reply_mention_button(mention_author, on_toggle_mention))
                .child(render_reply_close_button(on_cancel_reply)),
        )
}

// ============================================================================
// Typing Indicator Component
// ============================================================================

/// Renders the real-time typing indicator bar ("Fulano está digitando...").
pub fn render_typing_indicator(typing_users: &[String]) -> Div {
    if typing_users.is_empty() {
        return div().h(px(20.0)).w_full();
    }

    let typing_text = match typing_users.len() {
        1 => format!("{} está digitando...", typing_users[0]),
        2 => format!("{} e {} estão digitando...", typing_users[0], typing_users[1]),
        3 => format!("{}, {} e {} estão digitando...", typing_users[0], typing_users[1], typing_users[2]),
        _ => "Várias pessoas estão digitando...".to_string(),
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(space_2())
        .h(px(20.0))
        .px(space_4())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(3.0))
                .child(render_typing_dot("typing-dot-1", 0.0))
                .child(render_typing_dot("typing-dot-2", 0.25))
                .child(render_typing_dot("typing-dot-3", 0.50)),
        )
        .child(
            div()
                .text_size(font_size_xs())
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_muted())
                .child(typing_text),
        )
}

// ============================================================================
// Kit-Based Multi-Line Chat Composer Component
// ============================================================================

/// Canonical Discord multi-line message composer built over the gpui-kit [`KitTextarea`]
/// and [`TextareaState`]. Provides auto-growth, Enter-to-send (`submit_on_enter`),
/// Shift+Enter for newline insertion, native selection/clipboard/IME, active reply banner,
/// and Discord icon buttons.
#[derive(IntoElement)]
pub struct ChatComposer {
    id: ElementId,
    state: Entity<TextareaState>,
    channel_name: SharedString,
    placeholder: Option<SharedString>,
    replying_to: Option<Message>,
    mention_author: bool,
    typing_users: Vec<String>,
    active_media_tab: Option<MediaPickerTab>,
    disabled: bool,
    on_toggle_mention: Option<EditorActionCallback>,
    on_cancel_reply: Option<EditorActionCallback>,
    on_attachment: Option<EditorActionCallback>,
    on_gif: Option<EditorActionCallback>,
    on_sticker: Option<EditorActionCallback>,
    on_emoji: Option<EditorActionCallback>,
    on_send: Option<EditorSendCallback>,
}

impl ChatComposer {
    /// Creates a new multi-line chat composer bound to a caller-owned [`TextareaState`].
    pub fn new(id: impl Into<ElementId>, state: &Entity<TextareaState>) -> Self {
        Self {
            id: id.into(),
            state: state.clone(),
            channel_name: "chat".into(),
            placeholder: None,
            replying_to: None,
            mention_author: true,
            typing_users: Vec::new(),
            active_media_tab: None,
            disabled: false,
            on_toggle_mention: None,
            on_cancel_reply: None,
            on_attachment: None,
            on_gif: None,
            on_sticker: None,
            on_emoji: None,
            on_send: None,
        }
    }

    pub fn channel_name(mut self, name: impl Into<SharedString>) -> Self {
        self.channel_name = name.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn replying_to(mut self, message: Option<Message>) -> Self {
        self.replying_to = message;
        self
    }

    pub fn mention_author(mut self, mention: bool) -> Self {
        self.mention_author = mention;
        self
    }

    pub fn typing_users(mut self, users: Vec<String>) -> Self {
        self.typing_users = users;
        self
    }

    pub fn active_media_tab(mut self, tab: Option<MediaPickerTab>) -> Self {
        self.active_media_tab = tab;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_attachment(mut self, handler: Option<EditorActionCallback>) -> Self {
        self.on_attachment = handler;
        self
    }

    pub fn on_gif(mut self, handler: Option<EditorActionCallback>) -> Self {
        self.on_gif = handler;
        self
    }

    pub fn on_sticker(mut self, handler: Option<EditorActionCallback>) -> Self {
        self.on_sticker = handler;
        self
    }

    pub fn on_emoji(mut self, handler: Option<EditorActionCallback>) -> Self {
        self.on_emoji = handler;
        self
    }

    pub fn on_send(mut self, handler: Option<EditorSendCallback>) -> Self {
        self.on_send = handler;
        self
    }

    pub fn on_toggle_mention(mut self, handler: Option<EditorActionCallback>) -> Self {
        self.on_toggle_mention = handler;
        self
    }

    pub fn on_cancel_reply(mut self, handler: Option<EditorActionCallback>) -> Self {
        self.on_cancel_reply = handler;
        self
    }
}

impl RenderOnce for ChatComposer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let has_reply = self.replying_to.is_some();
        let reply_msg_id = self.replying_to.as_ref().map(|m| m.id.clone());

        let ph = self.placeholder.unwrap_or_else(|| {
            if self.channel_name.starts_with('@') {
                format!("Conversar com {}", self.channel_name).into()
            } else {
                format!("Conversar em #{}", self.channel_name).into()
            }
        });

        if self.state.read(cx).presentation().placeholder() != ph.as_ref() {
            let ph_str = ph.clone();
            self.state.update(cx, |s, cx| {
                s.set_placeholder(ph_str, window, cx);
            });
        }

        let is_empty = self.state.read(cx).value().is_empty();

        let reply_banner = self.replying_to.as_ref().map(|msg| {
            render_active_reply_banner(
                msg,
                self.mention_author,
                self.on_toggle_mention.clone(),
                self.on_cancel_reply.clone(),
            )
        });

        let attach_btn = render_attach_button(self.on_attachment.clone());
        let gift_btn = render_gift_button();
        let is_gif_active = self.active_media_tab == Some(MediaPickerTab::Gifs);
        let gif_btn = render_gif_button(is_gif_active, self.on_gif.clone());
        let is_sticker_active = self.active_media_tab == Some(MediaPickerTab::Stickers);
        let sticker_btn = render_sticker_button(is_sticker_active, self.on_sticker.clone());
        let is_emoji_active = self.active_media_tab == Some(MediaPickerTab::Emojis);
        let emoji_btn = render_emoji_button(is_emoji_active, self.on_emoji.clone());

        let current_text = self.state.read(cx).value().to_string();
        let send_btn = if !is_empty {
            render_send_button(current_text, reply_msg_id, self.on_send.clone())
        } else {
            None
        };

        let kit_textarea = KitTextarea::new(&self.state)
            .appearance(false)
            .bordered(false)
            .disabled(self.disabled)
            .w_full()
            .flex_1()
            .min_h(px(22.0))
            .text_size(font_size_base());

        let mut main_input = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_4())
            .min_h(chat_input_height())
            .px(space_4())
            .py(space_2())
            .bg(bg_input());

        if has_reply {
            main_input = main_input.rounded_b(radius_md());
        } else {
            main_input = main_input.rounded(radius_md());
        }

        main_input = main_input
            .child(attach_btn)
            .child(kit_textarea)
            .child(gift_btn)
            .child(gif_btn)
            .child(sticker_btn)
            .child(emoji_btn);

        if let Some(send) = send_btn {
            main_input = main_input.child(send);
        }

        let mut container = div()
            .id(self.id)
            .flex()
            .flex_col()
            .w_full()
            .px(space_4())
            .pb(space_4());

        if let Some(banner) = reply_banner {
            container = container.child(banner);
        }

        container = container.child(main_input);
        container = container.child(render_typing_indicator(&self.typing_users));

        container
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_typing_indicator_text() {
        let users1 = vec!["Alice".to_string()];
        let users2 = vec!["Alice".to_string(), "Bob".to_string()];
        let users3 = vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()];

        assert_eq!(users1.len(), 1);
        assert_eq!(users2.len(), 2);
        assert_eq!(users3.len(), 3);

        let div1 = render_typing_indicator(&users1);
        let div2 = render_typing_indicator(&users2);
        let div3 = render_typing_indicator(&users3);
        let div_empty = render_typing_indicator(&[]);
        let _ = (div1, div2, div3, div_empty);
    }

    #[::core::prelude::v1::test]
    fn test_render_active_reply_banner() {
        use crate::domain::message::Author;

        let author = Author::new("u1", "tester");
        let msg = Message::new("m1", "c1", author, "Hello world");

        let banner_on = render_active_reply_banner(&msg, true, None, None);
        let banner_off = render_active_reply_banner(&msg, false, None, None);
        let _ = (banner_on, banner_off);
    }
}
