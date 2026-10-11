//! Integration tests for Chat Editor, Reply Banner, and Typing Indicator

use discord_rs::chat::editor::{
    render_active_reply_banner, render_typing_indicator,
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
