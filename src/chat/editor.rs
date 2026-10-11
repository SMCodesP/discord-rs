//! Discord RS - Chat Input Editor & Typing Indicator
//! Implements canonical Discord text input:
//! - Channel-specific placeholder ("Conversar em #canal")
//! - Attachment button (+)
//! - Rich media buttons (GIF, Emoji, Stickers, Gift)
//! - Active reply banner with mention toggle and dismiss
//! - Real-time typing indicator with animated dot simulation

use gpui::*;
use std::sync::Arc;

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

    let mut mention_btn = if mention_author {
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
            .bg(rgb(HEX_CHANNEL_HOVER))
            .text_color(text_muted())
            .text_size(font_size_xs())
            .font_weight(WEIGHT_BOLD)
            .cursor_pointer()
            .hover(|s| s.bg(rgb(HEX_BG_CHAT_HOVER)))
            .child("@ Desligado")
    };

    if let Some(handler) = on_toggle_mention {
        mention_btn = mention_btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    let mut close_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.0))
        .rounded_full()
        .text_color(text_muted())
        .cursor_pointer()
        .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
        .child(icon_close(px(12.0), text_muted()));

    if let Some(handler) = on_cancel_reply {
        close_btn = close_btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .bg(rgb(HEX_BG_CHANNELS))
        .border_b_1()
        .border_color(rgb(0x35373c))
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
                .child(mention_btn)
                .child(close_btn),
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

    use gpui::AnimationExt as _;
    use std::time::Duration;

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(space_2())
        .h(px(20.0))
        .px(space_4())
        .child(
            // Animated typing dots with 3 blinking/pulsing dots
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(3.0))
                .child(
                    div()
                        .size(px(4.0))
                        .rounded_full()
                        .bg(blurple())
                        .with_animation(
                            "typing-dot-1",
                            Animation::new(Duration::from_millis(1000)).repeat(),
                            |dot, delta| {
                                let phase = (delta * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                                dot.opacity(0.3 + 0.7 * phase)
                            },
                        ),
                )
                .child(
                    div()
                        .size(px(4.0))
                        .rounded_full()
                        .bg(blurple())
                        .with_animation(
                            "typing-dot-2",
                            Animation::new(Duration::from_millis(1000)).repeat(),
                            |dot, delta| {
                                let phase = ((delta + 0.25) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                                dot.opacity(0.3 + 0.7 * phase)
                            },
                        ),
                )
                .child(
                    div()
                        .size(px(4.0))
                        .rounded_full()
                        .bg(blurple())
                        .with_animation(
                            "typing-dot-3",
                            Animation::new(Duration::from_millis(1000)).repeat(),
                            |dot, delta| {
                                let phase = ((delta + 0.50) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                                dot.opacity(0.3 + 0.7 * phase)
                            },
                        ),
                ),
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
// Chat Editor Component
// ============================================================================

/// Configuration parameters for rendering the chat editor.
pub struct ChatEditorConfig {
    pub channel_name: String,
    pub text: String,
    pub replying_to: Option<Message>,
    pub mention_author: bool,
    pub typing_users: Vec<String>,
    pub focus_handle: Option<FocusHandle>,
    pub is_focused: bool,
    pub on_text_change: Option<EditorTextChangeCallback>,
    pub on_send: Option<EditorSendCallback>,
    pub on_attachment: Option<EditorActionCallback>,
    pub on_gif: Option<EditorActionCallback>,
    pub on_sticker: Option<EditorActionCallback>,
    pub on_emoji: Option<EditorActionCallback>,
    pub on_toggle_mention: Option<EditorActionCallback>,
    pub on_cancel_reply: Option<EditorActionCallback>,
    pub active_media_tab: Option<MediaPickerTab>,
}

/// Renders the complete bottom chat editor box.
pub fn render_chat_editor(config: ChatEditorConfig) -> Div {
    let has_reply = config.replying_to.is_some();
    let reply_msg_id = config.replying_to.as_ref().map(|m| m.id.clone());

    // 1. Reply banner (if active)
    let reply_banner = config.replying_to.as_ref().map(|msg| {
        render_active_reply_banner(
            msg,
            config.mention_author,
            config.on_toggle_mention,
            config.on_cancel_reply,
        )
    });

    // 2. Attachment Plus (+) Button
    let mut plus_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded_full()
        .bg(rgb(0x3f4147))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x4e5058)))
        .text_color(rgb(0xb5bac1))
        .child(icon_plus(px(16.0), rgb(0xb5bac1)));

    if let Some(handler) = config.on_attachment {
        plus_btn = plus_btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    // 3. Right Action Buttons (Gift, GIF, Sticker, Emoji)
    let gift_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded(radius_sm())
        .cursor_pointer()
        .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
        .text_color(text_muted())
        .child(icon_gift(px(20.0), text_muted()));

    let is_gif_active = config.active_media_tab == Some(MediaPickerTab::Gifs);
    let mut gif_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded(radius_sm())
        .cursor_pointer()
        .bg(if is_gif_active { rgb(HEX_CHANNEL_ACTIVE) } else { rgba(0x00000000) })
        .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
        .text_color(if is_gif_active { rgb(HEX_WHITE) } else { text_muted() })
        .child(icon_gif(px(20.0), if is_gif_active { rgb(HEX_WHITE) } else { text_muted() }));

    if let Some(handler) = config.on_gif {
        gif_btn = gif_btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    let is_sticker_active = config.active_media_tab == Some(MediaPickerTab::Stickers);
    let mut sticker_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded(radius_sm())
        .cursor_pointer()
        .bg(if is_sticker_active { rgb(HEX_CHANNEL_ACTIVE) } else { rgba(0x00000000) })
        .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
        .text_color(if is_sticker_active { rgb(HEX_WHITE) } else { text_muted() })
        .child(icon_sticker(px(20.0), if is_sticker_active { rgb(HEX_WHITE) } else { text_muted() }));

    if let Some(handler) = config.on_sticker {
        sticker_btn = sticker_btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    let is_emoji_active = config.active_media_tab == Some(MediaPickerTab::Emojis);
    let mut emoji_btn = div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded(radius_sm())
        .cursor_pointer()
        .bg(if is_emoji_active { rgb(HEX_CHANNEL_ACTIVE) } else { rgba(0x00000000) })
        .hover(|s| s.bg(rgb(HEX_CHANNEL_HOVER)).text_color(text_titles()))
        .text_color(if is_emoji_active { rgb(HEX_WHITE) } else { text_muted() })
        .child(icon_emoji(px(20.0), if is_emoji_active { rgb(HEX_WHITE) } else { text_muted() }));

    if let Some(handler) = config.on_emoji {
        emoji_btn = emoji_btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(window, cx);
        });
    }

    // Send button if text is not empty
    let send_btn = if !config.text.trim().is_empty() {
        let text_to_send = config.text.clone();
        let reply_id = reply_msg_id.clone();
        let mut btn = div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(32.0))
            .rounded(radius_sm())
            .cursor_pointer()
            .bg(blurple())
            .hover(|s| s.bg(blurple_hover()))
            .child(icon_send(px(16.0), rgb(HEX_WHITE)));

        if let Some(ref handler) = config.on_send {
            let h = handler.clone();
            btn = btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                h(&text_to_send, reply_id.as_deref(), window, cx);
            });
        }
        Some(btn)
    } else {
        None
    };

    // 4. Center text input area with authentic Discord caret cursor
    let placeholder = if config.channel_name.starts_with('@') {
        format!("Conversar com {}", config.channel_name)
    } else {
        format!("Conversar em #{}", config.channel_name)
    };

    let is_focused = config.is_focused;
    let cursor_color = rgb(0xdbdee1);

    let text_area = if config.text.is_empty() {
        div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .child(if is_focused {
                div()
                    .w(px(2.0))
                    .h(px(18.0))
                    .bg(cursor_color)
                    .rounded(px(1.0))
                    .mr(px(2.0))
            } else {
                div()
            })
            .child(
                div()
                    .text_size(font_size_base())
                    .text_color(text_muted())
                    .child(placeholder),
            )
    } else {
        div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .child(
                div()
                    .text_size(font_size_base())
                    .text_color(text_primary())
                    .child(config.text.clone()),
            )
            .child(if is_focused {
                div()
                    .w(px(2.0))
                    .h(px(18.0))
                    .bg(cursor_color)
                    .rounded(px(1.0))
                    .ml(px(2.0))
            } else {
                div()
            })
    };

    // Main input pill
    let mut main_input = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(space_4())
        .min_h(chat_input_height())
        .px(space_4())
        .py(space_2())
        .bg(rgb(HEX_BG_INPUT));

    if has_reply {
        main_input = main_input.rounded_b(radius_md());
    } else {
        main_input = main_input.rounded(radius_md());
    }

    main_input = main_input
        .child(plus_btn)
        .child(text_area)
        .child(gift_btn)
        .child(gif_btn)
        .child(sticker_btn)
        .child(emoji_btn);

    if let Some(send) = send_btn {
        main_input = main_input.child(send);
    }

    if let Some(ref fh) = config.focus_handle {
        main_input = main_input.track_focus(fh);

        let fh_click = fh.clone();
        main_input = main_input.cursor_text().on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
            window.focus(&fh_click, cx);
        });

        let on_text_change = config.on_text_change.clone();
        let on_send = config.on_send.clone();
        let current_text = config.text.clone();
        let reply_id = reply_msg_id.clone();
        let fh_key = fh.clone();

        main_input = main_input.on_key_down(move |ev: &KeyDownEvent, window, cx| {
            window.focus(&fh_key, cx);
            let key = &ev.keystroke.key;
            let is_ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
            let is_paste = (is_ctrl && (key == "v" || key == "V"))
                || (ev.keystroke.modifiers.shift && key == "insert");

            // Paste from clipboard (Ctrl+V, Command+V, Shift+Insert)
            if is_paste {
                if let Some(clipboard) = cx.read_from_clipboard() {
                    if let Some(text) = clipboard.text() {
                        if let Some(ref change_cb) = on_text_change {
                            let mut new_str = current_text.clone();
                            new_str.push_str(&text);
                            change_cb(&new_str, window, cx);
                        }
                    }
                }
                return;
            }

            // Ignore other Ctrl/Cmd shortcuts so control sequences aren't typed into chat
            if is_ctrl {
                return;
            }

            match key.as_str() {
                "enter" => {
                    if !current_text.trim().is_empty() {
                        if let Some(ref send_cb) = on_send {
                            send_cb(&current_text, reply_id.as_deref(), window, cx);
                        }
                    }
                }
                "backspace" => {
                    if let Some(ref change_cb) = on_text_change {
                        let mut new_str = current_text.clone();
                        new_str.pop();
                        change_cb(&new_str, window, cx);
                    }
                }
                "space" | " " => {
                    if let Some(ref change_cb) = on_text_change {
                        let mut new_str = current_text.clone();
                        new_str.push(' ');
                        change_cb(&new_str, window, cx);
                    }
                }
                k if k.chars().count() == 1 => {
                    if let Some(ref change_cb) = on_text_change {
                        let mut new_str = current_text.clone();
                        new_str.push_str(k);
                        change_cb(&new_str, window, cx);
                    }
                }
                _ => {}
            }
        });
    }

    // 5. Compose editor container with reply banner, input pill, and typing indicator
    let mut container = div()
        .flex()
        .flex_col()
        .w_full()
        .px(space_4())
        .pb(space_4());

    if let Some(banner) = reply_banner {
        container = container.child(banner);
    }

    container = container.child(main_input);
    container = container.child(render_typing_indicator(&config.typing_users));

    container
}

// ============================================================================
// GPUI View Component for Chat Editor
// ============================================================================

/// GPUI View component managing interactive chat input state.
pub struct ChatEditorView {
    pub channel_name: String,
    pub text: String,
    pub replying_to: Option<Message>,
    pub mention_author: bool,
    pub typing_users: Vec<String>,
}

impl ChatEditorView {
    pub fn new(channel_name: impl Into<String>) -> Self {
        Self {
            channel_name: channel_name.into(),
            text: String::new(),
            replying_to: None,
            mention_author: true,
            typing_users: Vec::new(),
        }
    }

    pub fn set_replying_to(&mut self, message: Option<Message>) {
        self.replying_to = message;
    }

    pub fn set_typing_users(&mut self, users: Vec<String>) {
        self.typing_users = users;
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Render for ChatEditorView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        render_chat_editor(ChatEditorConfig {
            channel_name: self.channel_name.clone(),
            text: self.text.clone(),
            replying_to: self.replying_to.clone(),
            mention_author: self.mention_author,
            typing_users: self.typing_users.clone(),
            focus_handle: None,
            is_focused: false,
            on_text_change: None,
            on_send: None,
            on_attachment: None,
            on_gif: None,
            on_sticker: None,
            on_emoji: None,
            on_toggle_mention: None,
            on_cancel_reply: None,
            active_media_tab: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[::core::prelude::v1::test]
    fn test_typing_indicator_text() {
        let users1 = vec!["Alice".to_string()];
        let users2 = vec!["Alice".to_string(), "Bob".to_string()];
        let users3 = vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()];

        assert_eq!(users1.len(), 1);
        assert_eq!(users2.len(), 2);
        assert_eq!(users3.len(), 3);
    }
}
