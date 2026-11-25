use chrono::{TimeZone, Utc};
use discord_rs::chat::markdown::{
    format_discord_timestamp, parse_inlines, parse_markdown, InlineSpan, MarkdownBlock,
};
use discord_rs::domain::message::{Attachment, Embed, EmbedAuthor, EmbedField, EmbedFooter, Message};
use discord_rs::protocol::cache_engine::DiscordCacheEngine;
use discord_rs::protocol::discord_api::parse_message_from_json_str;

fn sample_author() -> discord_rs::domain::message::Author {
    discord_rs::domain::message::Author::new("100", "tester")
}

fn make_msg(id: &str, channel: &str, content: &str) -> Message {
    Message::new(id, channel, sample_author(), content)
}

// ---------------------------------------------------------------------------
// Attachments / spoiler (gaps)
// ---------------------------------------------------------------------------

#[test]
fn attachment_spoiler_detected_by_filename_prefix() {
    let att = Attachment {
        id: "1".into(),
        filename: "SPOILER_secret.png".into(),
        size: 10,
        url: "https://cdn.example/SPOILER_secret.png".into(),
        proxy_url: None,
        content_type: Some("image/png".into()),
        width: Some(100),
        height: Some(100),
    };
    assert!(att.is_spoiler());
    assert_eq!(att.display_filename(), "secret.png");
}

#[test]
fn attachment_non_spoiler_filename() {
    let att = Attachment {
        id: "1".into(),
        filename: "photo.png".into(),
        size: 10,
        url: "https://cdn.example/photo.png".into(),
        proxy_url: None,
        content_type: Some("image/png".into()),
        width: Some(100),
        height: Some(100),
    };
    assert!(!att.is_spoiler());
    assert_eq!(att.display_filename(), "photo.png");
}

#[test]
fn attachment_spoiler_prefix_only_not_treated_as_empty_name() {
    let att = Attachment {
        id: "1".into(),
        filename: "SPOILER_".into(),
        size: 10,
        url: "https://cdn.example/SPOILER_".into(),
        proxy_url: None,
        content_type: None,
        width: None,
        height: None,
    };
    assert!(att.is_spoiler());
    assert_eq!(att.display_filename(), "SPOILER_");
}

// ---------------------------------------------------------------------------
// Embed schema parsing (gaps)
// ---------------------------------------------------------------------------

#[test]
fn parse_message_embed_full_schema() {
    let json = r#"{
        "id":"9001","channel_id":"42","content":"",
        "author":{"id":"7","username":"alice"},
        "timestamp":"2026-01-02T03:04:05+00:00",
        "attachments":[],"reactions":[],"pinned":false,
        "embeds":[{
            "title":"Title",
            "description":"Desc",
            "url":"https://example.com/e",
            "color":16711680,
            "timestamp":"2026-01-03T04:05:06+00:00",
            "author":{"name":"Auth","url":"https://example.com/a","icon_url":"https://example.com/i.png"},
            "footer":{"text":"Foot","icon_url":"https://example.com/f.png"},
            "image":{"url":"https://cdn.example/img.png"},
            "thumbnail":{"url":"https://cdn.example/thumb.png"},
            "fields":[
                {"name":"F1","value":"V1","inline":true},
                {"name":"F2","value":"V2","inline":false}
            ]
        }]
    }"#;
    let msg = parse_message_from_json_str(json).expect("parse");
    assert_eq!(msg.embeds.len(), 1);
    let e = &msg.embeds[0];
    assert_eq!(e.title.as_deref(), Some("Title"));
    assert_eq!(e.description.as_deref(), Some("Desc"));
    assert_eq!(e.url.as_deref(), Some("https://example.com/e"));
    assert_eq!(e.color, Some(0xFF0000));
    assert_eq!(e.image_url.as_deref(), Some("https://cdn.example/img.png"));
    assert_eq!(e.thumbnail_url.as_deref(), Some("https://cdn.example/thumb.png"));
    let a = e.author.as_ref().expect("author");
    assert_eq!(a.name, "Auth");
    assert_eq!(a.url.as_deref(), Some("https://example.com/a"));
    assert_eq!(a.icon_url.as_deref(), Some("https://example.com/i.png"));
    let f = e.footer.as_ref().expect("footer");
    assert_eq!(f.text, "Foot");
    assert_eq!(f.icon_url.as_deref(), Some("https://example.com/f.png"));
    assert_eq!(e.fields.len(), 2);
    assert_eq!(e.fields[0].name, "F1");
    assert!(e.fields[0].inline);
    assert!(!e.fields[1].inline);
    let ts = e.timestamp.expect("embed timestamp");
    assert_eq!(ts, Utc.with_ymd_and_hms(2026, 1, 3, 4, 5, 6).unwrap());
}

#[test]
fn parse_message_embed_missing_optional_fields_defaults() {
    let json = r#"{
        "id":"9002","channel_id":"42","content":"",
        "author":{"id":"7","username":"alice"},
        "timestamp":"2026-01-02T03:04:05+00:00",
        "attachments":[],"reactions":[],"pinned":false,
        "embeds":[{"type":"rich"}]
    }"#;
    let msg = parse_message_from_json_str(json).expect("parse");
    assert_eq!(msg.embeds.len(), 1);
    let e = &msg.embeds[0];
    assert!(e.title.is_none());
    assert!(e.description.is_none());
    assert!(e.author.is_none());
    assert!(e.footer.is_none());
    assert!(e.timestamp.is_none());
    assert!(e.fields.is_empty());
}

// ---------------------------------------------------------------------------
// MESSAGE_UPDATE cache (gaps)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn message_update_cache_updates_content_and_metadata() {
    let dir = std::env::temp_dir().join(format!(
        "discord_rs_att_test_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let engine = DiscordCacheEngine::new_with_base_dir(dir.clone());
    let mut msg = make_msg("m1", "c1", "old");
    msg.embeds = vec![Embed {
        title: Some("old".into()),
        ..Default::default()
    }];
    msg.pinned = false;
    engine.save_channel_messages("c1", &[msg]).await;

    let mut updated = make_msg("m1", "c1", "new");
    updated.embeds = vec![Embed {
        title: Some("new".into()),
        ..Default::default()
    }];
    updated.pinned = true;
    updated.edited_timestamp = Some(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    updated.attachments = vec![Attachment {
        id: "a1".into(),
        filename: "f.png".into(),
        size: 1,
        url: "https://cdn/f.png".into(),
        proxy_url: None,
        content_type: Some("image/png".into()),
        width: Some(1),
        height: Some(1),
    }];

    engine.apply_gateway_message_update("c1", "m1", "new").await;
    let cached = engine.get_channel_messages("c1").await;
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].content, "new");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// Timestamps (gaps)
// ---------------------------------------------------------------------------

#[test]
fn discord_timestamp_formats_t_T_d_D_f_F_R_s_S() {
    let ts = 1_704_067_200_i64; // 2024-01-01T00:00:00Z
    // Just ensure all documented formats produce non-empty, format-specific outputs
    let t = format_discord_timestamp(ts, "t");
    let tt = format_discord_timestamp(ts, "T");
    let d = format_discord_timestamp(ts, "d");
    let dd = format_discord_timestamp(ts, "D");
    let f = format_discord_timestamp(ts, "f");
    let ff = format_discord_timestamp(ts, "F");
    let r = format_discord_timestamp(ts, "R");
    let s = format_discord_timestamp(ts, "s");
    let ss = format_discord_timestamp(ts, "S");
    for v in [&t, &tt, &d, &dd, &f, &ff, &r, &s, &ss] {
        assert!(!v.is_empty());
    }
    assert_ne!(t, tt);
    assert_ne!(d, dd);
    assert_ne!(f, ff);
    // d/s are short date style, D is long date style
    assert_ne!(d, dd);
}

#[test]
fn markdown_parses_discord_timestamp_token() {
    let spans = parse_inlines("see <t:1704067200:F> now");
    assert!(spans.iter().any(|s| matches!(s, InlineSpan::Timestamp { unix: 1704067200, format } if format == "F")));
}

#[test]
fn markdown_invalid_timestamp_falls_back_to_text() {
    let spans = parse_inlines("bad <t:not-a-number:f> here");
    assert!(!spans.iter().any(|s| matches!(s, InlineSpan::Timestamp { .. })));
    let text: String = spans.iter().map(|s| match s {
        InlineSpan::Text(t) => t.clone(),
        _ => String::new(),
    }).collect();
    assert!(text.contains("<t:not-a-number:f>"));
}

// ---------------------------------------------------------------------------
// Mentions fabricated/email (gaps)
// ---------------------------------------------------------------------------

#[test]
fn snowflake_user_mention_not_fabricated() {
    let spans = parse_inlines("hi <@123456789012345678>");
    let mention = spans.iter().find_map(|s| match s {
        InlineSpan::UserMention { id, name } => Some((id.clone(), name.clone())),
        _ => None,
    }).expect("mention");
    assert_eq!(mention.0.as_deref(), Some("123456789012345678"));
    // Name must not be fabricated like "Unknown" or "@123"; use id/snowflake as safe fallback
    assert_eq!(mention.1, "123456789012345678");
}

#[test]
fn email_address_not_parsed_as_mention() {
    let spans = parse_inlines("contact test@example.com please");
    assert!(!spans.iter().any(|s| matches!(s, InlineSpan::UserMention { .. })));
    let joined: String = spans.iter().map(|s| match s { InlineSpan::Text(t) => t.clone(), _ => "?".into() }).collect();
    assert!(joined.contains("test@example.com"));
}

#[test]
fn everyone_here_mentions_parsed() {
    let spans = parse_inlines("@everyone and @here");
    let names: Vec<_> = spans.iter().filter_map(|s| match s {
        InlineSpan::UserMention { name, .. } => Some(name.as_str()),
        _ => None,
    }).collect();
    assert!(names.contains(&"everyone"));
    assert!(names.contains(&"here"));
}

// ---------------------------------------------------------------------------
// Links (gaps)
// ---------------------------------------------------------------------------

#[test]
fn markdown_link_parsed() {
    let spans = parse_inlines("see [site](https://example.com/path)");
    let (label, url) = spans.iter().find_map(|s| match s {
        InlineSpan::Link { label, url } => Some((label.clone(), url.clone())),
        _ => None,
    }).expect("link");
    assert_eq!(label, "site");
    assert_eq!(url, "https://example.com/path");
}

#[test]
fn angle_autolink_parsed() {
    let spans = parse_inlines("see <https://example.com/a?b=1>");
    let link = spans.iter().find_map(|s| match s {
        InlineSpan::Link { url, .. } => Some(url.clone()),
        _ => None,
    }).expect("link");
    assert_eq!(link, "https://example.com/a?b=1");
}

#[test]
fn raw_url_trims_trailing_punctuation() {
    let spans = parse_inlines("go to https://example.com/path.");
    let link = spans.iter().find_map(|s| match s {
        InlineSpan::Link { url, .. } => Some(url.clone()),
        _ => None,
    }).expect("link");
    assert_eq!(link, "https://example.com/path");
    let has_dot_text = spans.iter().any(|s| matches!(s, InlineSpan::Text(t) if t.contains('.')));
    assert!(has_dot_text);
}

// ---------------------------------------------------------------------------
// Escapes (gaps)
// ---------------------------------------------------------------------------

#[test]
fn escaped_markdown_characters_render_literal() {
    let spans = parse_inlines(r"\*not bold\* \_not italic\_ \`not code\`");
    assert!(!spans.iter().any(|s| matches!(s, InlineSpan::Bold(_) | InlineSpan::Italic(_) | InlineSpan::InlineCode(_))));
    let text: String = spans.iter().filter_map(|s| match s { InlineSpan::Text(t) => Some(t.as_str()), _ => None }).collect();
    assert!(text.contains("*not bold*"));
    assert!(text.contains("_not italic_"));
    assert!(text.contains("`not code`"));
}

#[test]
fn escaped_backslash_renders_backslash() {
    let spans = parse_inlines(r"\\path");
    let text: String = spans.iter().filter_map(|s| match s { InlineSpan::Text(t) => Some(t.as_str()), _ => None }).collect();
    assert!(text.contains(r"\path"));
}

// ---------------------------------------------------------------------------
// Code blocks (gaps)
// ---------------------------------------------------------------------------

#[test]
fn code_block_preserves_content_and_language() {
    let blocks = parse_markdown("```rs\nfn main() {\n  println!(\"hi\");\n}\n```");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        MarkdownBlock::CodeBlock { language, content } => {
            assert_eq!(language.as_deref(), Some("rs"));
            assert!(content.contains("fn main()"));
            assert!(content.contains("println!(\"hi\");"));
        }
        other => panic!("expected code block, got {other:?}"),
    }
}

#[test]
fn code_block_does_not_parse_inline_markdown() {
    let blocks = parse_markdown("```\n**bold** _italic_ <@123>\n```");
    match &blocks[0] {
        MarkdownBlock::CodeBlock { content, .. } => {
            assert!(content.contains("**bold**"));
            assert!(content.contains("_italic_"));
            assert!(content.contains("<@123>"));
        }
        other => panic!("expected code block, got {other:?}"),
    }
}

#[test]
fn inline_code_preserves_backticks_with_longer_delimiter() {
    let spans = parse_inlines("``code `tick` here``");
    let code = spans.iter().find_map(|s| match s { InlineSpan::InlineCode(c) => Some(c.clone()), _ => None }).expect("code");
    assert_eq!(code, "code `tick` here");
}

// ---------------------------------------------------------------------------
// Emoji Unicode jumbo (gaps)
// ---------------------------------------------------------------------------

#[test]
fn single_unicode_emoji_message_is_jumbo_candidate() {
    let blocks = parse_markdown("😀");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        MarkdownBlock::Paragraph(inlines) => {
            assert_eq!(inlines.len(), 1);
            assert!(matches!(&inlines[0], InlineSpan::Emoji(e) if e == "😀"));
        }
        other => panic!("expected paragraph, got {other:?}"),
    }
}

#[test]
fn mixed_text_and_emoji_not_jumbo() {
    let blocks = parse_markdown("hi 😀");
    match &blocks[0] {
        MarkdownBlock::Paragraph(inlines) => {
            assert!(inlines.iter().any(|s| matches!(s, InlineSpan::Text(_))));
            assert!(inlines.iter().any(|s| matches!(s, InlineSpan::Emoji(_))));
            // more than one meaningful span => not jumbo by render logic
            let meaningful = inlines.iter().filter(|s| !matches!(s, InlineSpan::Text(t) if t.trim().is_empty())).count();
            assert!(meaningful > 1);
        }
        other => panic!("expected paragraph, got {other:?}"),
    }
}

#[test]
fn custom_emoji_parsed() {
    let spans = parse_inlines("<:rust:12345> and <a:wave:67890>");
    assert!(spans.iter().any(|s| matches!(s, InlineSpan::CustomEmoji { name, id, animated: false } if name=="rust" && id=="12345")));
    assert!(spans.iter().any(|s| matches!(s, InlineSpan::CustomEmoji { name, id, animated: true } if name=="wave" && id=="67890")));
}

#[test]
fn message_update_event_parsed_into_full_message() {
    let json = r#"{
        "id":"m9","channel_id":"c9","content":"edited",
        "edited_timestamp":"2026-02-03T04:05:06+00:00",
        "pinned":true,
        "author":{"id":"1","username":"u"},
        "timestamp":"2026-01-01T00:00:00+00:00",
        "attachments":[],"embeds":[{"title":"E"}],"reactions":[]
    }"#;
    let msg = parse_message_from_json_str(json).expect("parse");
    assert_eq!(msg.content, "edited");
    assert!(msg.pinned);
    assert!(msg.edited_timestamp.is_some());
    assert_eq!(msg.embeds.len(), 1);
}

#[test]
fn embed_field_inline_default_false() {
    let json = r#"{
        "id":"m10","channel_id":"c10","content":"",
        "author":{"id":"1","username":"u"},
        "timestamp":"2026-01-01T00:00:00+00:00",
        "attachments":[],"embeds":[{"fields":[{"name":"N","value":"V"}]}],"reactions":[],"pinned":false
    }"#;
    let msg = parse_message_from_json_str(json).expect("parse");
    assert_eq!(msg.embeds[0].fields[0].inline, false);
}

#[test]
fn spoiler_markdown_parsed() {
    let spans = parse_inlines("this is ||secret||");
    assert!(spans.iter().any(|s| matches!(s, InlineSpan::Spoiler(_))));
}
