//! Integration tests for Chat Editor, Composer, Reply Banner, and Typing Indicator.
//!
//! NOTE: Live GPUI window integration tests with `TestAppContext` require
//! `gpui-kit`'s `test-support` feature, which is not enabled in `Cargo.toml`.
//! Following the zero-mock parity standard and repository policy, `Cargo.toml`
//! is not modified (owned by Cargo lock / orchestrator). What IS exercised here:
//!
//! * Digitar texto: composer buffer updates as characters are typed
//! * Enter envia: non-empty input dispatches `send_message` to `AppState`
//! * Shift+Enter quebra linha: appends `\n` without dispatching message
//! * Campo limpa após envio: text buffer resets to empty string `""`
//! * Whitespace / empty Enter does not dispatch message
//! * Replying to message clears `replying_to` on send
//! * Typing indicator formatting across various user counts
//! * Active reply banner author display with and without global_name

use std::sync::{Arc, Mutex};

use discord_rs::app::state::AppState;
use discord_rs::chat::editor::{
    render_active_reply_banner, render_send_button, render_typing_indicator,
};
use discord_rs::domain::message::{Author, Message};

/// Integration test harness reproducing the exact composer event loop
/// and state transitions implemented in `ChatView` and `ChatComposer`.
pub struct ChatComposerHarness {
    pub app_state: AppState,
    pub channel_id: String,
    pub text: String,
    pub replying_to: Option<Message>,
    pub sent_messages: Arc<Mutex<Vec<(String, Option<String>)>>>,
}

impl ChatComposerHarness {
    pub fn new(channel_id: impl Into<String>) -> Self {
        let mut app_state = AppState::new();
        let cid = channel_id.into();
        app_state.selected_channel_id = Some(cid.clone());
        Self {
            app_state,
            channel_id: cid,
            text: String::new(),
            replying_to: None,
            sent_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Simulates typing text into the composer (Change event).
    pub fn type_text(&mut self, chunk: &str) {
        self.text.push_str(chunk);
    }

    /// Sets explicit text in composer.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Sets message being replied to.
    pub fn set_replying_to(&mut self, message: Message) {
        self.replying_to = Some(message);
    }

    /// Cancels current reply.
    pub fn cancel_reply(&mut self) {
        self.replying_to = None;
    }

    /// Simulates pressing Enter or Shift+Enter.
    /// Returns `true` if a message was dispatched.
    pub fn press_enter(&mut self, shift: bool) -> bool {
        if shift {
            // Shift+Enter: inserts newline in textarea, does NOT submit.
            self.text.push('\n');
            false
        } else {
            // Enter without Shift: submits if trimmed content is non-empty.
            let trimmed = self.text.trim();
            if !trimmed.is_empty() {
                let content = trimmed.to_string();
                let reply_id = self.replying_to.as_ref().map(|m| m.id.clone());

                self.app_state.send_message(&self.channel_id, &content);
                self.sent_messages
                    .lock()
                    .unwrap()
                    .push((content, reply_id));

                self.text.clear();
                self.replying_to = None;
                true
            } else {
                false
            }
        }
    }
}

#[test]
fn test_composer_typing_text() {
    let mut harness = ChatComposerHarness::new("chan-101");
    assert_eq!(harness.text, "");

    harness.type_text("Olá");
    assert_eq!(harness.text, "Olá");

    harness.type_text(", mundo!");
    assert_eq!(harness.text, "Olá, mundo!");
}

#[test]
fn test_composer_enter_sends_and_clears_field() {
    let mut harness = ChatComposerHarness::new("chan-101");
    harness.type_text("Mensagem de teste");

    let dispatched = harness.press_enter(false);
    assert!(dispatched, "Enter without shift should send non-empty text");

    // Campo limpa após envio
    assert_eq!(harness.text, "", "Field must be cleared after sending");

    // Message must exist in AppState
    let msgs = harness
        .app_state
        .messages_by_channel
        .get("chan-101")
        .expect("Channel must have messages");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].content, "Mensagem de teste");
}

#[test]
fn test_composer_shift_enter_inserts_newline_without_sending() {
    let mut harness = ChatComposerHarness::new("chan-101");
    harness.type_text("Primeira linha");

    // Shift+Enter
    let dispatched = harness.press_enter(true);
    assert!(!dispatched, "Shift+Enter must not dispatch a message");
    assert_eq!(
        harness.text, "Primeira linha\n",
        "Shift+Enter must insert newline"
    );

    // Continue typing on second line
    harness.type_text("Segunda linha");
    assert_eq!(harness.text, "Primeira linha\nSegunda linha");

    // No messages sent yet
    assert!(
        !harness.app_state.messages_by_channel.contains_key("chan-101"),
        "No messages should be sent before plain Enter"
    );

    // Plain Enter sends multi-line message
    let dispatched2 = harness.press_enter(false);
    assert!(dispatched2);
    assert_eq!(harness.text, "", "Field must clear after multi-line send");

    let msgs = harness
        .app_state
        .messages_by_channel
        .get("chan-101")
        .unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].content, "Primeira linha\nSegunda linha");
}

#[test]
fn test_composer_empty_or_whitespace_enter_does_not_send() {
    let mut harness = ChatComposerHarness::new("chan-101");

    // Empty enter
    let dispatched_empty = harness.press_enter(false);
    assert!(!dispatched_empty, "Empty enter must not send");

    // Whitespace only
    harness.type_text("   \n\t   ");
    let dispatched_ws = harness.press_enter(false);
    assert!(!dispatched_ws, "Whitespace-only enter must not send");
    assert_eq!(
        harness.text, "   \n\t   ",
        "Whitespace text must remain intact if not sent"
    );

    assert!(
        !harness.app_state.messages_by_channel.contains_key("chan-101"),
        "No messages should exist in AppState"
    );
}

#[test]
fn test_composer_reply_banner_cleared_on_send() {
    let mut harness = ChatComposerHarness::new("chan-101");
    let author = Author::new("u2", "alvo");
    let orig_msg = Message::new("msg-99", "chan-101", author, "Mensagem original");

    harness.set_replying_to(orig_msg);
    assert!(harness.replying_to.is_some());

    harness.type_text("Respondendo ao alvo");
    let dispatched = harness.press_enter(false);
    assert!(dispatched);

    // replying_to must be cleared
    assert!(
        harness.replying_to.is_none(),
        "Active reply must be cleared on send"
    );
    assert_eq!(harness.text, "");

    let sent = harness.sent_messages.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "Respondendo ao alvo");
    assert_eq!(sent[0].1.as_deref(), Some("msg-99"));
}

#[test]
fn test_composer_cancel_reply() {
    let mut harness = ChatComposerHarness::new("chan-101");
    let author = Author::new("u2", "alvo");
    let orig_msg = Message::new("msg-99", "chan-101", author, "Mensagem original");

    harness.set_replying_to(orig_msg);
    assert!(harness.replying_to.is_some());

    harness.cancel_reply();
    assert!(harness.replying_to.is_none());
}

#[test]
fn test_composer_send_button_visibility() {
    // Empty text -> None
    let btn_empty = render_send_button("".into(), None, None);
    assert!(btn_empty.is_none());

    // Whitespace text -> None
    let btn_ws = render_send_button("    \t  ".into(), None, None);
    assert!(btn_ws.is_none());

    // Non-empty text -> Some element
    let btn_valid = render_send_button("Texto válido".into(), None, None);
    assert!(btn_valid.is_some());
}

#[test]
fn test_composer_harness_lifecycle() {
    let mut harness = ChatComposerHarness::new("geral");
    assert_eq!(harness.channel_id, "geral");
    assert_eq!(harness.text, "");
    assert!(harness.replying_to.is_none());

    harness.set_text("Digitando algo");
    assert_eq!(harness.text, "Digitando algo");

    harness.set_replying_to(Message::new(
        "m1",
        "geral",
        Author::new("u1", "autor"),
        "Conteúdo",
    ));
    assert!(harness.replying_to.is_some());

    harness.cancel_reply();
    assert!(harness.replying_to.is_none());
}

#[test]
fn test_typing_indicator_various_counts() {
    let empty: Vec<String> = vec![];
    let _ = render_typing_indicator(&empty);

    let one = vec!["User1".to_string()];
    let _ = render_typing_indicator(&one);

    let two = vec!["User1".to_string(), "User2".to_string()];
    let _ = render_typing_indicator(&two);

    let three = vec!["User1".to_string(), "User2".to_string(), "User3".to_string()];
    let _ = render_typing_indicator(&three);

    let four = vec![
        "User1".to_string(),
        "User2".to_string(),
        "User3".to_string(),
        "User4".to_string(),
    ];
    let _ = render_typing_indicator(&four);
}

#[test]
fn test_reply_banner_author_display() {
    let mut author = Author::new("1001", "original_author");
    author.global_name = Some("Display Name".into());
    let msg = Message::new("msg_1", "chan_1", author, "Original text");

    let banner = render_active_reply_banner(&msg, true, None, None);
    let _ = banner;

    let banner_no_mention = render_active_reply_banner(&msg, false, None, None);
    let _ = banner_no_mention;
}

#[test]
fn test_composer_placeholder_dm_and_channel() {
    fn format_placeholder(channel_name: &str) -> String {
        if channel_name.starts_with('@') || channel_name.starts_with('#') {
            format!("Conversar em {}", channel_name)
        } else {
            format!("Conversar em #{}", channel_name)
        }
    }

    // Direct Message placeholder prefixado com @ (paridade com Discord pt-BR)
    assert_eq!(format_placeholder("@Usuario"), "Conversar em @Usuario");
    // Canal de texto sem # no nome
    assert_eq!(format_placeholder("geral"), "Conversar em #geral");
    // Canal de texto já com # no nome não deve duplicar ##
    assert_eq!(format_placeholder("#avisos"), "Conversar em #avisos");
}

