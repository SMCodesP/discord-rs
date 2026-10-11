//! Integration tests for Chat Editor, Reply Banner, and Typing Indicator

use discord_rs::chat::editor::{
    render_active_reply_banner, render_chat_editor, render_typing_indicator, ChatEditorConfig,
    ChatEditorView,
};
use discord_rs::domain::message::{Author, Message};

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
fn test_chat_editor_view_methods() {
    let mut editor = ChatEditorView::new("general");
    assert_eq!(editor.channel_name, "general");
    assert_eq!(editor.text, "");
    assert!(editor.replying_to.is_none());
    assert!(editor.mention_author);
    assert!(editor.typing_users.is_empty());

    editor.set_text("Hello Discord!");
    assert_eq!(editor.text, "Hello Discord!");

    editor.set_typing_users(vec!["Alice".into(), "Bob".into()]);
    assert_eq!(editor.typing_users.len(), 2);

    let author = Author::new("2002", "bob");
    let msg = Message::new("msg_2", "chan_1", author, "Ping");
    editor.set_replying_to(Some(msg));
    assert!(editor.replying_to.is_some());
}

#[test]
fn test_chat_editor_config_rendering() {
    let config = ChatEditorConfig {
        channel_name: "anúncios".into(),
        text: "Mensagem de teste".into(),
        replying_to: None,
        mention_author: true,
        typing_users: vec!["Mod".into()],
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
    };

    let element = render_chat_editor(config);
    let _ = element;
}

#[test]
fn test_chat_editor_config_dm_placeholder() {
    let config = ChatEditorConfig {
        channel_name: "@Alice".into(),
        text: "".into(),
        replying_to: None,
        mention_author: false,
        typing_users: vec![],
        focus_handle: None,
        is_focused: true,
        on_text_change: None,
        on_send: None,
        on_attachment: None,
        on_gif: None,
        on_sticker: None,
        on_emoji: None,
        on_toggle_mention: None,
        on_cancel_reply: None,
        active_media_tab: None,
    };

    let element = render_chat_editor(config);
    let _ = element;
}
