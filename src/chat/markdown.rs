//! Discord RS - Markdown Parser & GPUI Native Renderer
//! Implements canonical Discord Markdown formatting:
//! - Bold: `**texto**`
//! - Italic: `*texto*` or `_texto_`
//! - Bold Italic: `***texto***`
//! - Underline: `__texto__`
//! - Strikethrough: `~~texto~~`
//! - Inline Code: `` `código` ``
//! - Code Block: ```` ```lang\ncode\n``` ````
//! - Spoilers: `||texto||`
//! - User / Channel / Role Mentions: `@user`, `@everyone`, `@here`, `<@id>`, `<#id>`, `<@&id>`, `#canal`
//! - Links: `[label](https://...)`, raw URLs and `<https://...>`
//! - Dynamic timestamps: `<t:unix[:tTdDfFRsS]>`
//! - Custom emoji: `<:name:id>` / `<a:name:id>` and jumbo Unicode emoji
//! - Quotes: `> texto` and `>>> texto`
//! - Subtext: `-# texto pequeno`
//! - Headings: `# H1`, `## H2`, `### H3`

use chrono::{DateTime, Datelike, Local, Utc};
use gpui::prelude::*;
use gpui::*;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::domain::message::{Attachment, Embed, EmbedAuthor, EmbedField, EmbedFooter};
use crate::tokens::colors::*;
use crate::tokens::kit_theme::CSS_LINK;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;
use std::collections::HashMap;

/// Resolved names for mention rendering. Keys are Discord snowflake IDs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MentionLookup {
    pub users: HashMap<String, String>,
    pub channels: HashMap<String, String>,
    pub roles: HashMap<String, String>,
}

impl MentionLookup {
    pub fn user_name<'a>(&'a self, id: &str, fallback: &'a str) -> &'a str {
        self.users.get(id).map(String::as_str).unwrap_or(fallback)
    }

    pub fn channel_name<'a>(&'a self, id: &str, fallback: &'a str) -> &'a str {
        self.channels.get(id).map(String::as_str).unwrap_or(fallback)
    }

    pub fn role_name<'a>(&'a self, id: &str, fallback: &'a str) -> &'a str {
        self.roles.get(id).map(String::as_str).unwrap_or(fallback)
    }
}

// ============================================================================
// AST Models
// ============================================================================

/// Represents inline formatted segments inside a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSpan {
    Text(String),
    Bold(Vec<InlineSpan>),
    Italic(Vec<InlineSpan>),
    BoldItalic(Vec<InlineSpan>),
    Underline(Vec<InlineSpan>),
    Strikethrough(Vec<InlineSpan>),
    InlineCode(String),
    Spoiler(Vec<InlineSpan>),
    UserMention {
        id: Option<String>,
        name: String,
    },
    ChannelMention {
        id: Option<String>,
        name: String,
    },
    RoleMention {
        id: Option<String>,
        name: String,
    },
    Link {
        label: String,
        url: String,
    },
    Emoji(String),
    CustomEmoji {
        name: String,
        id: String,
        animated: bool,
    },
    /// Discord dynamic timestamp: `<t:unix[:fmt]>`.
    Timestamp {
        unix: i64,
        format: String,
    },
}

/// A single item inside an ordered or unordered markdown list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// Indentation level derived from leading spaces (2 spaces = 1 level).
    pub indent: usize,
    pub inlines: Vec<InlineSpan>,
}

/// Represents block-level elements in Discord markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    Heading {
        level: u8,
        inlines: Vec<InlineSpan>,
    },
    Paragraph(Vec<InlineSpan>),
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    BlockQuote(Vec<MarkdownBlock>),
    Subtext(Vec<InlineSpan>),
    OrderedList {
        start: u64,
        items: Vec<ListItem>,
    },
    UnorderedList(Vec<ListItem>),
}

// ============================================================================
// Regular Expressions for Parsing
// ============================================================================

static RE_USER_MENTION_SNOWFLAKE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^<@!?([0-9]+)>").expect("invalid snowflake regex"));

static RE_CHANNEL_MENTION_SNOWFLAKE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^<#([0-9]+)>").expect("invalid channel snowflake regex"));

static RE_ROLE_MENTION_SNOWFLAKE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^<@&([0-9]+)>").expect("invalid role snowflake regex"));

static RE_NAMED_USER_MENTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@([a-zA-Z0-9_\.\-]+)").expect("invalid user mention regex"));

static RE_NAMED_CHANNEL_MENTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#([a-zA-Z0-9_\-]+)").expect("invalid channel mention regex"));

static RE_MD_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[([^\]]+)\]\((https?://[^\s)]+)\)").expect("invalid md link regex")
});

static RE_ANGLE_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^<(https?://[^<>\s]+)>").expect("invalid angle autolink regex"));

static RE_RAW_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(https?://[^\s<]+)").expect("invalid raw url regex"));

static RE_CUSTOM_EMOJI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<(a)?:([a-zA-Z0-9_]+):([0-9]+)>").expect("invalid custom emoji regex")
});

static RE_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<t:(-?[0-9]+)(?::([tTdDfFRsS]))?>").expect("invalid timestamp regex")
});

/// Spoiler reveal state is keyed by its actual content. Keeping this outside the
/// element tree lets the stateless markdown renderer preserve a click reveal
/// across GPUI re-renders without inventing a second source of message data.
static REVEALED_SPOILERS: LazyLock<std::sync::Mutex<HashSet<String>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashSet::with_capacity(64)));

pub static MARKDOWN_AST_CACHE: LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, Vec<MarkdownBlock>>>,
> = LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::with_capacity(512)));

/// Parses raw Discord message markdown into a list of structured blocks with AST memoization.
pub fn parse_markdown(input: &str) -> Vec<MarkdownBlock> {
    if input.trim().is_empty() {
        return Vec::new();
    }

    if let Ok(cache) = MARKDOWN_AST_CACHE.lock() {
        if let Some(cached) = cache.get(input) {
            return cached.clone();
        }
    }

    let result = parse_markdown_uncached(input);

    if let Ok(mut cache) = MARKDOWN_AST_CACHE.lock() {
        if cache.len() >= 1000 {
            cache.clear();
        }
        cache.insert(input.to_string(), result.clone());
    }

    result
}

fn parse_markdown_uncached(input: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut offset = 0;

    while offset < input.len() {
        let (line_end, next_line) = line_bounds(input, offset);
        let line = input[offset..line_end].trim_end_matches('\r');
        if line.starts_with("```") {
            let language = {
                let name = line[3..].trim();
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            };
            let content_start = next_line;
            let mut scan = content_start;
            let mut content_end = input.len();
            let mut next_offset = input.len();
            let mut closed = false;

            while scan < input.len() {
                let (end, after_end) = line_bounds(input, scan);
                let candidate = input[scan..end].trim_end_matches('\r');
                if candidate.trim() == "```" {
                    content_end = scan;
                    next_offset = after_end;
                    closed = true;
                    break;
                }
                scan = after_end;
            }

            let mut content = &input[content_start..content_end];
            if closed {
                // The newline immediately before the closing fence belongs to
                // the delimiter. Remove only that one, preserving real blank lines.
                content = content
                    .strip_suffix("\r\n")
                    .or_else(|| content.strip_suffix('\n'))
                    .unwrap_or(content);
            }
            blocks.push(MarkdownBlock::CodeBlock {
                language,
                content: content.to_string(),
            });
            offset = next_offset;
        } else {
            let text_start = offset;
            let mut scan = next_line;
            while scan < input.len() {
                let (end, after_end) = line_bounds(input, scan);
                let candidate = input[scan..end].trim_end_matches('\r');
                if candidate.starts_with("```") {
                    break;
                }
                scan = after_end;
            }
            blocks.extend(parse_text_blocks(&input[text_start..scan]));
            offset = scan;
        }
    }

    blocks
}

fn line_bounds(input: &str, offset: usize) -> (usize, usize) {
    match input[offset..].find('\n') {
        Some(relative_end) => {
            let end = offset + relative_end;
            (end, end + 1)
        }
        None => (input.len(), input.len()),
    }
}

fn parse_ordered_list_line(line: &str) -> Option<(u64, usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let trimmed = &line[indent..];
    let dot = trimmed.find(". ")?;
    if dot == 0 {
        return None;
    }
    let number_part = &trimmed[..dot];
    if !number_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let start = number_part.parse::<u64>().ok()?;
    let content = &trimmed[dot + 2..];
    Some((start, indent / 2, content))
}

fn parse_unordered_list_line(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let trimmed = &line[indent..];
    // Discord subtext `-# ` must not become a bullet.
    if trimmed.starts_with("-# ") {
        return None;
    }
    let content = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))?;
    Some((indent / 2, content))
}

/// Parses lines of text outside of code blocks.
fn parse_text_blocks(text: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Multi-line blockquote: `>>> ` consumes all subsequent lines
        if line.starts_with(">>> ") || line == ">>>" {
            let first_line = line.strip_prefix(">>> ").unwrap_or("");
            let mut quote_content = String::new();
            if !first_line.is_empty() {
                quote_content.push_str(first_line);
            }
            for next_line in &lines[i + 1..] {
                if !quote_content.is_empty() {
                    quote_content.push('\n');
                }
                quote_content.push_str(next_line);
            }
            let inner_blocks = parse_text_blocks(&quote_content);
            blocks.push(MarkdownBlock::BlockQuote(inner_blocks));
            break; // Consumed until the end
        }

        // Single-line blockquote: `> `
        if line.starts_with("> ") || line == ">" {
            let mut quote_lines = Vec::new();
            while i < lines.len() && (lines[i].starts_with("> ") || lines[i] == ">") {
                let stripped = lines[i].strip_prefix("> ").unwrap_or("");
                quote_lines.push(stripped);
                i += 1;
            }
            let quote_text = quote_lines.join("\n");
            let inner_blocks = parse_text_blocks(&quote_text);
            blocks.push(MarkdownBlock::BlockQuote(inner_blocks));
            continue;
        }

        // Headings: `# `, `## `, `### `
        if let Some(rest) = line.strip_prefix("# ") {
            blocks.push(MarkdownBlock::Heading {
                level: 1,
                inlines: parse_inlines(rest),
            });
            i += 1;
            continue;
        } else if let Some(rest) = line.strip_prefix("## ") {
            blocks.push(MarkdownBlock::Heading {
                level: 2,
                inlines: parse_inlines(rest),
            });
            i += 1;
            continue;
        } else if let Some(rest) = line.strip_prefix("### ") {
            blocks.push(MarkdownBlock::Heading {
                level: 3,
                inlines: parse_inlines(rest),
            });
            i += 1;
            continue;
        }

        // Subtext: `-# `
        if let Some(rest) = line.strip_prefix("-# ") {
            blocks.push(MarkdownBlock::Subtext(parse_inlines(rest)));
            i += 1;
            continue;
        }

        // Ordered list: `1. item`, `3. item`
        if parse_ordered_list_line(line).is_some() {
            let (start, _, _) = parse_ordered_list_line(line).expect("checked above");
            let mut items = Vec::new();
            while i < lines.len() {
                if let Some((_, item_indent, item_content)) = parse_ordered_list_line(lines[i]) {
                    items.push((item_indent, parse_inlines(item_content)));
                    i += 1;
                } else {
                    break;
                }
            }
            blocks.push(MarkdownBlock::OrderedList {
                start,
                items: items
                    .into_iter()
                    .map(|(indent, inlines)| ListItem { indent, inlines })
                    .collect(),
            });
            continue;
        }

        // Unordered list: `- item`, `* item`
        if parse_unordered_list_line(line).is_some() {
            let mut items = Vec::new();
            while i < lines.len() {
                if let Some((item_indent, item_content)) = parse_unordered_list_line(lines[i]) {
                    items.push((item_indent, parse_inlines(item_content)));
                    i += 1;
                } else {
                    break;
                }
            }
            blocks.push(MarkdownBlock::UnorderedList(
                items
                    .into_iter()
                    .map(|(indent, inlines)| ListItem { indent, inlines })
                    .collect(),
            ));
            continue;
        }

        // Empty line
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // Standard paragraph
        blocks.push(MarkdownBlock::Paragraph(parse_inlines(line)));
        i += 1;
    }

    blocks
}

/// Parses inline text formatting (bold, italic, code, spoilers, mentions, links).
pub fn parse_inlines(input: &str) -> Vec<InlineSpan> {
    let mut inlines = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut idx = 0;
    let mut buffer = String::new();

    while idx < len {
        let remaining_str: String = chars[idx..].iter().collect();

        // Backslash escapes are literal and must be handled before formatting.
        if chars[idx] == '\\' {
            if let Some(next) = chars.get(idx + 1).copied() {
                if is_escapable(next) {
                    buffer.push(next);
                    idx += 2;
                    continue;
                }
            }
        }

        // Keep complete Unicode emoji sequences together so single-emoji
        // messages can use Discord's jumbo presentation.
        if is_emoji_base(chars[idx]) {
            let end = unicode_emoji_sequence_end(&chars, idx);
            let has_emoji_modifier = end > idx + 1;
            let is_keycap = chars.get(idx + 1) == Some(&'\u{FE0F}');
            if has_emoji_modifier || is_keycap || !chars[idx].is_ascii() {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let emoji: String = chars[idx..end].iter().collect();
                inlines.push(InlineSpan::Emoji(emoji));
                idx = end;
                continue;
            }
        }

        // 1. Spoilers: `||text||`
        if chars[idx..].starts_with(&['|', '|']) {
            if let Some(end_offset) = find_delimiter(&chars[idx + 2..], &['|', '|']) {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 2..idx + 2 + end_offset].iter().collect();
                inlines.push(InlineSpan::Spoiler(parse_inlines(&inner_str)));
                idx = idx + 2 + end_offset + 2;
                continue;
            }
        }

        // 2. Inline Code: matching backtick runs, including ``code ` tick``
        if chars[idx] == '`' {
            let run_length = {
                let mut end = idx;
                while end < len && chars[end] == '`' {
                    end += 1;
                }
                end - idx
            };
            if let Some(end_offset) = find_backtick_run(&chars[idx + run_length..], run_length) {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let code_content: String = chars[idx + run_length..idx + run_length + end_offset]
                    .iter()
                    .collect();
                inlines.push(InlineSpan::InlineCode(code_content));
                idx = idx + run_length + end_offset + run_length;
                continue;
            }
        }

        // 3. Bold Italic: `***text***`
        if chars[idx..].starts_with(&['*', '*', '*']) {
            if let Some(end_offset) = find_delimiter(&chars[idx + 3..], &['*', '*', '*']) {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 3..idx + 3 + end_offset].iter().collect();
                inlines.push(InlineSpan::BoldItalic(parse_inlines(&inner_str)));
                idx = idx + 3 + end_offset + 3;
                continue;
            }
        }

        // 4. Bold: `**text**`
        if chars[idx..].starts_with(&['*', '*']) {
            if let Some(end_offset) = find_delimiter(&chars[idx + 2..], &['*', '*']) {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 2..idx + 2 + end_offset].iter().collect();
                inlines.push(InlineSpan::Bold(parse_inlines(&inner_str)));
                idx = idx + 2 + end_offset + 2;
                continue;
            }
        }

        // 5. Underline: `__text__`
        if chars[idx..].starts_with(&['_', '_']) {
            if let Some(end_offset) = find_delimiter(&chars[idx + 2..], &['_', '_']) {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 2..idx + 2 + end_offset].iter().collect();
                inlines.push(InlineSpan::Underline(parse_inlines(&inner_str)));
                idx = idx + 2 + end_offset + 2;
                continue;
            }
        }

        // 6. Strikethrough: `~~text~~`
        if chars[idx..].starts_with(&['~', '~']) {
            if let Some(end_offset) = find_delimiter(&chars[idx + 2..], &['~', '~']) {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 2..idx + 2 + end_offset].iter().collect();
                inlines.push(InlineSpan::Strikethrough(parse_inlines(&inner_str)));
                idx = idx + 2 + end_offset + 2;
                continue;
            }
        }

        // 7. Italic: `*text*`
        if chars[idx] == '*' && (idx + 1 < len && chars[idx + 1] != ' ') {
            if let Some(end_offset) = find_single_char(&chars[idx + 1..], '*') {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 1..idx + 1 + end_offset].iter().collect();
                inlines.push(InlineSpan::Italic(parse_inlines(&inner_str)));
                idx = idx + 1 + end_offset + 1;
                continue;
            }
        }

        // 8. Italic with underscore: `_text_`
        if chars[idx] == '_' && (idx == 0 || chars[idx - 1] == ' ' || chars[idx - 1] == '\n') {
            if let Some(end_offset) = find_single_char(&chars[idx + 1..], '_') {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let inner_str: String = chars[idx + 1..idx + 1 + end_offset].iter().collect();
                inlines.push(InlineSpan::Italic(parse_inlines(&inner_str)));
                idx = idx + 1 + end_offset + 1;
                continue;
            }
        }

        // 9. Mentions, Links & Custom Emojis (Snowflake, named, custom emoji, markdown links, raw urls)
        if chars[idx] == '<'
            || chars[idx] == '@'
            || chars[idx] == '#'
            || chars[idx] == '['
            || remaining_str.starts_with("http")
        {
            // Discord custom emoji `<:name:id>` and `<a:name:id>`
            if let Some(cap) = RE_CUSTOM_EMOJI.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let animated = cap.get(1).is_some();
                let name = cap.get(2).unwrap().as_str().to_string();
                let id = cap.get(3).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::CustomEmoji { name, id, animated });
                idx += full.chars().count();
                continue;
            }

            // Discord dynamic timestamp `<t:unix>` or `<t:unix:fmt>`
            if let Some(cap) = RE_TIMESTAMP.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                if let Some(unix_match) = cap.get(1) {
                    if let Ok(unix) = unix_match.as_str().parse::<i64>() {
                        if DateTime::<Utc>::from_timestamp(unix, 0).is_some() {
                            if !buffer.is_empty() {
                                inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                            }
                            inlines.push(InlineSpan::Timestamp {
                                unix,
                                format: cap
                                    .get(2)
                                    .map_or_else(|| "f".to_string(), |m| m.as_str().to_string()),
                            });
                            idx += full.chars().count();
                            continue;
                        }
                    }
                }
            }

            // Snowflake user mention `<@12345>`
            if let Some(cap) = RE_USER_MENTION_SNOWFLAKE.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let id = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::UserMention {
                    id: Some(id.clone()),
                    // No fabricated display name: until member data resolves the
                    // mention, show the real snowflake carried by Discord.
                    name: id,
                });
                idx += full.chars().count();
                continue;
            }

            // Snowflake channel mention `<#12345>`
            if let Some(cap) = RE_CHANNEL_MENTION_SNOWFLAKE.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let id = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::ChannelMention {
                    id: Some(id.clone()),
                    name: id,
                });
                idx += full.chars().count();
                continue;
            }

            // Snowflake role mention `<@&12345>`
            if let Some(cap) = RE_ROLE_MENTION_SNOWFLAKE.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let id = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::RoleMention {
                    id: Some(id.clone()),
                    name: id,
                });
                idx += full.chars().count();
                continue;
            }

            // Named user mention `@quill`, `@everyone`, `@here`.
            // An `@` inside an email local-part is not a mention.
            let previous_is_email_local = idx > 0 && is_email_local_part_char(chars[idx - 1]);
            if !previous_is_email_local {
                if let Some(cap) = RE_NAMED_USER_MENTION.captures(&remaining_str) {
                    let full = cap.get(0).unwrap().as_str();
                    let name = cap.get(1).unwrap().as_str().to_string();
                    if !buffer.is_empty() {
                        inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                    }
                    inlines.push(InlineSpan::UserMention { id: None, name });
                    idx += full.chars().count();
                    continue;
                }
            }

            // Named channel mention `#geral`, `#chat`
            if let Some(cap) = RE_NAMED_CHANNEL_MENTION.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let name = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::ChannelMention { id: None, name });
                idx += full.chars().count();
                continue;
            }

            // Angle-bracket autolink `<https://example.com/path>`.
            if let Some(cap) = RE_ANGLE_URL.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let url = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::Link {
                    label: url.clone(),
                    url,
                });
                idx += full.chars().count();
                continue;
            }

            // Markdown link `[label](url)`
            if let Some(cap) = RE_MD_LINK.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let label = cap.get(1).unwrap().as_str().to_string();
                let url = cap.get(2).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::Link { label, url });
                idx += full.chars().count();
                continue;
            }

            // Raw URL `https://...`; leave sentence punctuation as Text.
            if let Some(cap) = RE_RAW_URL.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let url = trim_url_punctuation(full);
                if !url.is_empty() {
                    if !buffer.is_empty() {
                        inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                    }
                    inlines.push(InlineSpan::Link {
                        label: url.to_string(),
                        url: url.to_string(),
                    });
                    idx += url.chars().count();
                    continue;
                }
            }
        }

        // Regular character
        buffer.push(chars[idx]);
        idx += 1;
    }

    if !buffer.is_empty() {
        inlines.push(InlineSpan::Text(buffer));
    }

    inlines
}

fn is_escapable(character: char) -> bool {
    matches!(
        character,
        '*' | '_' | '~' | '`' | '|' | '>' | '\\' | '(' | ')' | '[' | ']'
    )
}

fn is_email_local_part_char(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '.' | '!'
                | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '*'
                | '+'
                | '-'
                | '/'
                | '='
                | '?'
                | '^'
                | '_'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
        )
}

fn is_emoji_base(character: char) -> bool {
    matches!(
        character,
        '\u{1F000}'..='\u{1FAFF}'
            | '\u{2600}'..='\u{27BF}'
            | '\u{2B00}'..='\u{2BFF}'
            | '0'..='9'
            | '#'
            | '*'
    )
}

fn unicode_emoji_sequence_end(chars: &[char], start: usize) -> usize {
    let base = chars[start];
    let mut end = start + 1;
    if ('\u{1F1E6}'..='\u{1F1FF}').contains(&base) {
        if chars
            .get(end)
            .copied()
            .is_some_and(|next| ('\u{1F1E6}'..='\u{1F1FF}').contains(&next))
        {
            end += 1;
        }
        return end;
    }
    if (base.is_ascii_digit() || base == '#' || base == '*')
        && chars.get(end) == Some(&'\u{FE0F}')
        && chars.get(end + 1) == Some(&'\u{20E3}')
    {
        return end + 2;
    }

    loop {
        match chars.get(end).copied() {
            Some('\u{FE0E}' | '\u{FE0F}') => end += 1,
            Some('\u{1F3FB}'..='\u{1F3FF}') => end += 1,
            Some('\u{200D}') if chars.get(end + 1).copied().is_some_and(is_emoji_base) => end += 2,
            _ => break,
        }
    }
    end
}

fn find_backtick_run(slice: &[char], run_length: usize) -> Option<usize> {
    let mut index = 0;
    while index < slice.len() {
        if slice[index] == '\n' {
            return None;
        }
        if slice[index] == '`' {
            let start = index;
            while index < slice.len() && slice[index] == '`' {
                index += 1;
            }
            if index - start == run_length {
                return Some(start);
            }
        } else {
            index += 1;
        }
    }
    None
}

fn trim_url_punctuation(url: &str) -> &str {
    let mut end = url.len();
    loop {
        let Some(last) = url[..end].chars().next_back() else {
            break;
        };
        let remove = match last {
            '.' | ',' | '!' | '?' | ';' | ':' => true,
            ')' | ']' | '}' => {
                let (open, close) = match last {
                    ')' => ('(', ')'),
                    ']' => ('[', ']'),
                    _ => ('{', '}'),
                };
                url[..end].matches(close).count() > url[..end].matches(open).count()
            }
            _ => false,
        };
        if !remove {
            break;
        }
        end -= last.len_utf8();
    }
    &url[..end]
}

fn find_single_char(slice: &[char], target: char) -> Option<usize> {
    for (i, &c) in slice.iter().enumerate() {
        if c == target {
            return Some(i);
        }
        if c == '\n' {
            return None; // Inline elements don't cross line breaks
        }
    }
    None
}

fn find_delimiter(slice: &[char], delimiter: &[char]) -> Option<usize> {
    let dlen = delimiter.len();
    if slice.len() < dlen {
        return None;
    }
    for i in 0..=slice.len() - dlen {
        if slice[i] == '\n' {
            return None;
        }
        if slice[i..i + dlen] == *delimiter {
            return Some(i);
        }
    }
    None
}

// ============================================================================
// Discord Timestamps
// ============================================================================

fn pt_month_name(month: u32) -> &'static str {
    match month {
        1 => "janeiro",
        2 => "fevereiro",
        3 => "março",
        4 => "abril",
        5 => "maio",
        6 => "junho",
        7 => "julho",
        8 => "agosto",
        9 => "setembro",
        10 => "outubro",
        11 => "novembro",
        12 => "dezembro",
        _ => "",
    }
}

fn pt_weekday_name(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "segunda-feira",
        chrono::Weekday::Tue => "terça-feira",
        chrono::Weekday::Wed => "quarta-feira",
        chrono::Weekday::Thu => "quinta-feira",
        chrono::Weekday::Fri => "sexta-feira",
        chrono::Weekday::Sat => "sábado",
        chrono::Weekday::Sun => "domingo",
    }
}

fn format_relative_timestamp(then: DateTime<Local>, now: DateTime<Local>) -> String {
    let seconds = now.signed_duration_since(then).num_seconds();
    let future = seconds < 0;
    let absolute = seconds.unsigned_abs();
    let (amount, unit) = if absolute < 5 {
        return "agora".to_string();
    } else if absolute < 60 {
        (absolute, if absolute == 1 { "segundo" } else { "segundos" })
    } else if absolute < 3600 {
        let value = absolute / 60;
        (value, if value == 1 { "minuto" } else { "minutos" })
    } else if absolute < 86_400 {
        let value = absolute / 3600;
        (value, if value == 1 { "hora" } else { "horas" })
    } else if absolute < 2_592_000 {
        let value = absolute / 86_400;
        (value, if value == 1 { "dia" } else { "dias" })
    } else if absolute < 31_536_000 {
        let value = absolute / 2_592_000;
        (value, if value == 1 { "mês" } else { "meses" })
    } else {
        let value = absolute / 31_536_000;
        (value, if value == 1 { "ano" } else { "anos" })
    };

    if future {
        format!("em {} {}", amount, unit)
    } else {
        format!("há {} {}", amount, unit)
    }
}

/// Formats a Discord `<t:unix:fmt>` timestamp in the user's local timezone.
/// Supported format codes follow Discord's documented `t`, `T`, `d`, `D`,
/// `f`, `F`, and `R` styles.
pub fn format_discord_timestamp(unix: i64, format: &str) -> String {
    let Some(then) = DateTime::<Utc>::from_timestamp(unix, 0) else {
        return unix.to_string();
    };
    let local = then.with_timezone(&Local);
    let date = format!(
        "{} de {} de {}",
        local.day(),
        pt_month_name(local.month()),
        local.year()
    );

    match format {
        "t" => local.format("%H:%M").to_string(),
        "T" => local.format("%H:%M:%S").to_string(),
        "d" => local.format("%d/%m/%Y").to_string(),
        "s" => local.format("%d/%m/%Y").to_string(),
        "S" => local.format("%d/%m/%Y %H:%M").to_string(),
        "D" => date,
        "R" => format_relative_timestamp(local, Local::now()),
        "F" => format!(
            "{}, {} às {}",
            pt_weekday_name(local.weekday()),
            date,
            local.format("%H:%M")
        ),
        _ => format!("{} às {}", date, local.format("%H:%M")),
    }
}

fn discord_link_color() -> Rgba {
    let hex = CSS_LINK.trim_start_matches('#');
    rgb(u32::from_str_radix(hex, 16).unwrap_or(0x4d96ee))
}

/// Allows only http/https URLs to be opened from markdown.
fn sanitize_http_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn inline_plain_text(span: &InlineSpan) -> String {
    match span {
        InlineSpan::Text(text) | InlineSpan::InlineCode(text) | InlineSpan::Emoji(text) => {
            text.clone()
        }
        InlineSpan::Bold(children)
        | InlineSpan::Italic(children)
        | InlineSpan::BoldItalic(children)
        | InlineSpan::Underline(children)
        | InlineSpan::Strikethrough(children)
        | InlineSpan::Spoiler(children) => children.iter().map(inline_plain_text).collect(),
        InlineSpan::UserMention { name, .. } => format!("@{}", name),
        InlineSpan::ChannelMention { name, .. } => format!("#{}", name),
        InlineSpan::RoleMention { name, .. } => format!("@{}", name),
        InlineSpan::Link { label, .. } => label.clone(),
        InlineSpan::CustomEmoji { name, .. } => format!(":{}:", name),
        InlineSpan::Timestamp { unix, format } => format_discord_timestamp(*unix, format),
    }
}

fn spoiler_key(children: &[InlineSpan]) -> String {
    children.iter().map(inline_plain_text).collect()
}

fn spoiler_is_revealed(key: &str) -> bool {
    REVEALED_SPOILERS
        .lock()
        .map(|revealed| revealed.contains(key))
        .unwrap_or(false)
}

fn toggle_spoiler(key: &str) {
    if let Ok(mut revealed) = REVEALED_SPOILERS.lock() {
        if !revealed.remove(key) {
            if revealed.len() >= 512 {
                revealed.clear();
            }
            revealed.insert(key.to_string());
        }
    }
}

// ============================================================================
// GPUI Native Renderer
// ============================================================================

/// Renders a full Discord message text natively into a GPUI `Div` element,
/// reusing MARKDOWN_AST_CACHE directly without cloning the AST vector.
pub fn render_markdown(input: &str) -> Div {
    render_markdown_with(input, &MentionLookup::default())
}

/// Renders markdown with mention resolution from real channel/member/role data.
pub fn render_markdown_with(input: &str, mentions: &MentionLookup) -> Div {
    if input.trim().is_empty() {
        return div().w_full().min_w(px(0.0));
    }

    // Direct read from MARKDOWN_AST_CACHE: zero AST cloning or regex parsing
    if let Ok(cache) = MARKDOWN_AST_CACHE.lock() {
        if let Some(blocks) = cache.get(input) {
            let mut container = div()
                .flex()
                .flex_col()
                .gap(space_1())
                .w_full()
                .min_w(px(0.0))
                .overflow_hidden();

            for block in blocks {
                container = container.child(render_block_with(block, mentions));
            }
            return container;
        }
    }

    let blocks = parse_markdown(input);

    let mut container = div()
        .flex()
        .flex_col()
        .gap(space_1())
        .w_full()
        .min_w(px(0.0))
        .overflow_hidden();

    for block in &blocks {
        container = container.child(render_block_with(block, mentions));
    }

    container
}

/// Pre-warms the markdown AST cache for a message content string
pub fn prewarm_markdown_cache(input: &str) {
    if !input.trim().is_empty() {
        let _ = parse_markdown(input);
    }
}

/// Renders a single markdown block element.
pub fn render_block(block: &MarkdownBlock) -> Div {
    render_block_with(block, &MentionLookup::default())
}

fn render_block_with(block: &MarkdownBlock, mentions: &MentionLookup) -> Div {
    match block {
        MarkdownBlock::Heading { level, inlines } => {
            let (font_sz, font_wt, color) = match level {
                1 => (font_size_2xl(), WEIGHT_BOLD, text_titles()),
                2 => (font_size_xl(), WEIGHT_BOLD, text_titles()),
                _ => (font_size_lg(), WEIGHT_BOLD, text_titles()),
            };

            div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .text_size(font_sz)
                .font_weight(font_wt)
                .text_color(color)
                .my(space_1())
                .children(
                    inlines
                        .iter()
                        .map(|span| render_inline_with(span, mentions)),
                )
        }

        MarkdownBlock::Paragraph(inlines) => {
            let meaningful_count = inlines
                .iter()
                .filter(|span| match span {
                    InlineSpan::Text(t) => !t.trim().is_empty(),
                    _ => true,
                })
                .count();
            let is_single_emoji = meaningful_count == 1
                && inlines.iter().any(|span| {
                    matches!(span, InlineSpan::CustomEmoji { .. } | InlineSpan::Emoji(_))
                });

            div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .text_size(font_size_base())
                .text_color(text_primary())
                .line_height(line_height_base())
                .children(
                    inlines
                        .iter()
                        .map(|span| render_inline_span_with(span, is_single_emoji, mentions)),
                )
        }

        MarkdownBlock::CodeBlock { language, content } => {
            let mut block_container = div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .bg(rgb(HEX_BG_CHANNELS))
                .border_1()
                .border_color(rgb(HEX_BG_SERVER))
                .rounded(radius_md())
                .my(space_1())
                .overflow_hidden();

            if let Some(lang) = language {
                block_container = block_container.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .bg(rgb(HEX_BG_USER_BAR))
                        .px(space_4())
                        .py(space_1())
                        .border_b_1()
                        .border_color(rgb(HEX_BG_SERVER))
                        .text_size(font_size_xs())
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_muted())
                        .child(lang.to_uppercase()),
                );
            }

            block_container.child(
                div()
                    .p(space_4())
                    .text_size(font_size_sm())
                    .text_color(text_primary())
                    .font_family(FONT_FAMILY_CODE)
                    .child(content.clone()),
            )
        }

        MarkdownBlock::BlockQuote(blocks) => div()
            .w_full()
            .min_w(px(0.0))
            .max_w(relative(0.9))
            .flex()
            .flex_row()
            .my(space_2())
            .child(
                div()
                    .w(px(4.0))
                    .flex_shrink_0()
                    .bg(borders())
                    .rounded(radius_sm()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .py(space_6())
                    .pl(space_4())
                    .pr(space_4())
                    .gap(space_1())
                    .children(blocks.iter().map(|b| render_block_with(b, mentions))),
            ),

        MarkdownBlock::OrderedList { start, items } => render_ordered_list(*start, items, mentions),

        MarkdownBlock::UnorderedList(items) => render_unordered_list(items, mentions),

        MarkdownBlock::Subtext(inlines) => div()
            .w_full()
            .min_w(px(0.0))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .text_size(font_size_xs())
            .text_color(text_muted())
            .children(
                inlines
                    .iter()
                    .map(|span| render_inline_with(span, mentions)),
            ),
    }
}

fn render_ordered_list(start: u64, items: &[ListItem], mentions: &MentionLookup) -> Div {
    let mut list = div()
        .w_full()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .my(space_1());
    for (idx, item) in items.iter().enumerate() {
        let number = start.saturating_add(idx as u64);
        let indent_px = px((item.indent as f32) * 16.0);
        list = list.child(
            div()
                .flex()
                .flex_row()
                .w_full()
                .min_w(px(0.0))
                .pl(indent_px)
                .child(
                    div()
                        .min_w(px(24.0))
                        .flex_shrink_0()
                        .text_color(text_primary())
                        .child(format!("{}.", number)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .flex_1()
                        .min_w(px(0.0))
                        .items_baseline()
                        .text_color(text_primary())
                        .children(
                            item.inlines
                                .iter()
                                .map(|span| render_inline_with(span, mentions)),
                        ),
                ),
        );
    }
    list
}

fn render_unordered_list(items: &[ListItem], mentions: &MentionLookup) -> Div {
    let mut list = div()
        .w_full()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .my(space_1());
    for item in items {
        let indent_px = px((item.indent as f32) * 16.0);
        list = list.child(
            div()
                .flex()
                .flex_row()
                .w_full()
                .min_w(px(0.0))
                .pl(indent_px)
                .child(
                    div()
                        .min_w(px(24.0))
                        .flex_shrink_0()
                        .text_color(text_primary())
                        .child("•"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .flex_1()
                        .min_w(px(0.0))
                        .items_baseline()
                        .text_color(text_primary())
                        .children(
                            item.inlines
                                .iter()
                                .map(|span| render_inline_with(span, mentions)),
                        ),
                ),
        );
    }
    list
}

/// Renders a single inline formatted element with optional single emoji flag.
pub fn render_inline_span(inline: &InlineSpan, is_single_emoji: bool) -> Div {
    render_inline_span_with(inline, is_single_emoji, &MentionLookup::default())
}

fn render_inline_span_with(
    inline: &InlineSpan,
    is_single_emoji: bool,
    mentions: &MentionLookup,
) -> Div {
    match inline {
        InlineSpan::Text(text) => div().text_color(text_primary()).child(text.clone()),

        InlineSpan::Bold(children) => div()
            .font_weight(WEIGHT_BOLD)
            .text_color(text_titles())
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .children(
                children
                    .iter()
                    .map(|child| render_inline_with(child, mentions)),
            ),

        InlineSpan::Italic(children) => div()
            .italic()
            .text_color(text_primary())
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .children(
                children
                    .iter()
                    .map(|child| render_inline_with(child, mentions)),
            ),

        InlineSpan::BoldItalic(children) => div()
            .font_weight(WEIGHT_BOLD)
            .italic()
            .text_color(text_titles())
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .children(
                children
                    .iter()
                    .map(|child| render_inline_with(child, mentions)),
            ),

        InlineSpan::Underline(children) => div()
            .underline()
            .text_decoration_color(text_primary())
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .children(
                children
                    .iter()
                    .map(|child| render_inline_with(child, mentions)),
            ),

        InlineSpan::Strikethrough(children) => div()
            .line_through()
            .text_color(text_primary())
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .children(
                children
                    .iter()
                    .map(|child| render_inline_with(child, mentions)),
            ),

        InlineSpan::InlineCode(code) => div()
            .bg(bg_user_bar())
            .text_color(text_primary())
            .rounded(radius_xs())
            .px(space_2())
            .py(px(1.0))
            .mx(space_1())
            .text_size(px(13.0))
            .font_family(FONT_FAMILY_CODE)
            .child(code.clone()),

        InlineSpan::Spoiler(children) => {
            let key = spoiler_key(children);
            let revealed = spoiler_is_revealed(&key);
            let key_for_click = key.clone();
            let content = if revealed {
                let mut content = div().flex().flex_row().flex_wrap().items_baseline();
                for child in children {
                    content = content.child(render_inline_with(child, mentions));
                }
                content
            } else {
                div().text_color(bg_user_bar()).child(key.clone())
            };

            div()
                .bg(bg_user_bar())
                .hover(|s| s.bg(bg_chat_hover()))
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    toggle_spoiler(&key_for_click);
                    cx.stop_propagation();
                    cx.refresh_windows();
                })
                .child(content)
        }

        InlineSpan::UserMention { id, name } => {
            let display = id
                .as_deref()
                .map(|id| mentions.user_name(id, "usuário desconhecido").to_string())
                .unwrap_or_else(|| name.clone());
            let mut mention = div()
                .bg(mention_bg())
                .hover(|s| s.bg(mention_hover()))
                .text_color(blurple())
                .font_weight(WEIGHT_MEDIUM)
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .child(format!("@{}", display));
            if let Some(user_id) = id {
                let user_id = user_id.clone();
                mention = mention.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |event, _window, cx| {
                        if let Some(state) = cx
                            .try_global::<crate::app::state::AppStateGlobal>()
                            .map(|g| g.0.clone())
                        {
                            let user_id = user_id.clone();
                            state.update(cx, |state, cx| {
                                state.open_user_profile(&user_id, Some(event.position));
                                cx.notify();
                            });
                        }
                        cx.stop_propagation();
                    },
                );
            }
            mention
        }

        InlineSpan::ChannelMention { id, name } => {
            let display = id
                .as_deref()
                .map(|id| mentions.channel_name(id, "canal desconhecido").to_string())
                .unwrap_or_else(|| name.clone());
            let mut mention = div()
                .bg(mention_bg())
                .hover(|s| s.bg(mention_hover()))
                .text_color(blurple())
                .font_weight(WEIGHT_MEDIUM)
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .child(format!("#{}", display));
            if let Some(channel_id) = id {
                let channel_id = channel_id.clone();
                mention = mention.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |_event, _window, cx| {
                        if let Some(state) = cx
                            .try_global::<crate::app::state::AppStateGlobal>()
                            .map(|g| g.0.clone())
                        {
                            let channel_id = channel_id.clone();
                            state.update(cx, |state, cx| {
                                let guild_id = state
                                    .find_channel(&channel_id)
                                    .map(|(guild, _)| guild.id.clone());
                                if let Some(guild_id) = guild_id {
                                    state.selected_guild_id = Some(guild_id);
                                }
                                state.select_channel(channel_id);
                                cx.notify();
                            });
                        }
                        cx.stop_propagation();
                    },
                );
            }
            mention
        }

        InlineSpan::RoleMention { id, name } => {
            let display = id
                .as_deref()
                .map(|id| mentions.role_name(id, "cargo desconhecido").to_string())
                .unwrap_or_else(|| name.clone());
            div()
                .bg(mention_bg())
                .hover(|s| s.bg(mention_hover()))
                .text_color(blurple())
                .font_weight(WEIGHT_MEDIUM)
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .child(format!("@{}", display))
        }

        InlineSpan::Link { label, url } => {
            let link_color = discord_link_color();
            let safe_url = sanitize_http_url(url);
            let mut link = div().text_color(link_color).child(label.clone());
            if let Some(url) = safe_url {
                link = link
                    .hover(move |s| s.underline().text_decoration_color(link_color))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        cx.open_url(&url);
                        cx.stop_propagation();
                    });
            }
            link
        }

        InlineSpan::Timestamp { unix, format } => div()
            .text_color(text_primary())
            .child(format_discord_timestamp(*unix, format)),

        InlineSpan::Emoji(emoji) => {
            let mut el = div().mx(space_1());
            if is_single_emoji {
                el = el.text_size(px(40.0));
            }
            el.child(emoji.clone())
        }

        InlineSpan::CustomEmoji { name, id, animated } => {
            let ext = if *animated { "gif" } else { "png" };
            let (img_sz, cdn_sz) = if is_single_emoji {
                (px(48.0), 48)
            } else {
                (px(22.0), 48)
            };
            let fallback_name = format!(":{}:", name);
            let cdn_url = format!(
                "https://cdn.discordapp.com/emojis/{}.{}?size={}&quality=lossless",
                id, ext, cdn_sz
            );
            div()
                .mx(space_1())
                .size(img_sz)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(cdn_url)
                        .id(ElementId::from(SharedString::from(format!(
                            "md-emoji-img-{}",
                            id
                        ))))
                        .size_full()
                        .rounded(radius_xs())
                        .with_fallback(move || {
                            div()
                                .text_size(font_size_sm())
                                .child(fallback_name.clone())
                                .into_any_element()
                        }),
                )
        }
    }
}

/// Renders a single inline formatted element.
pub fn render_inline(inline: &InlineSpan) -> Div {
    render_inline_with(inline, &MentionLookup::default())
}

fn render_inline_with(inline: &InlineSpan, mentions: &MentionLookup) -> Div {
    render_inline_span_with(inline, false, mentions)
}

// ============================================================================
// Rich Message Media (rendered by ChatView without duplicating message_list)
// ============================================================================

fn format_attachment_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn attachment_display_size(attachment: &Attachment) -> Option<(Pixels, Pixels)> {
    // Reuse the existing chat media width cap. Height follows the real API
    // dimensions so images are never stretched or assigned invented dimensions.
    let max_width = 400.0_f32;
    let width = attachment.width.filter(|width| *width > 0)?;
    let height = attachment.height.filter(|height| *height > 0)?;
    let scale = (max_width / width as f32).min(1.0);
    Some((px(width as f32 * scale), px(height as f32 * scale)))
}

/// Renders real image previews and downloadable file cards from Discord CDN data.
pub fn render_rich_attachments(attachments: &[Attachment]) -> Div {
    let mut container = div()
        .flex()
        .flex_col()
        .gap(space_2())
        .my(space_2())
        .max_w(px(520.0));
    if attachments.is_empty() {
        return container;
    }

    for attachment in attachments {
        let url = attachment.url.clone();
        let filename = attachment.filename.clone();
        if attachment.is_image() {
            let display_size = attachment_display_size(attachment);
            let fallback_filename = filename.clone();
            let mut image = img(url.clone())
                .id(ElementId::Name(
                    format!("message-attachment-{}", attachment.id).into(),
                ))
                .max_w(px(400.0))
                .max_h(px(400.0))
                .rounded(radius_md())
                .cursor_pointer()
                .with_fallback(move || {
                    div()
                        .p(space_4())
                        .max_w(px(400.0))
                        .bg(bg_channels())
                        .rounded(radius_md())
                        .text_color(text_muted())
                        .child(fallback_filename.clone())
                        .into_any_element()
                })
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    cx.open_url(&url);
                    cx.stop_propagation();
                });
            if let Some((width, height)) = display_size {
                image = image.w(width).h(height);
            }
            container = container.child(
                div()
                    .flex()
                    .flex_col()
                    .max_w(px(400.0))
                    .gap(space_1())
                    .child(image)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .text_size(font_size_xs())
                            .text_color(text_muted())
                            .child(filename)
                            .child(format_attachment_size(attachment.size)),
                    ),
            );
        } else {
            let url = attachment.url.clone();
            container = container.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space_3())
                    .max_w(px(400.0))
                    .p(space_3())
                    .bg(bg_channels())
                    .border_1()
                    .border_color(bg_server())
                    .rounded(radius_md())
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        cx.open_url(&url);
                        cx.stop_propagation();
                    })
                    .child(div().text_size(font_size_lg()).child("📄"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(font_size_sm())
                                    .font_weight(WEIGHT_MEDIUM)
                                    .text_color(text_titles())
                                    .child(filename),
                            )
                            .child(
                                div()
                                    .text_size(font_size_xs())
                                    .text_color(text_muted())
                                    .child(format_attachment_size(attachment.size)),
                            ),
                    ),
            );
        }
    }

    container
}

fn render_embed_author(author: &EmbedAuthor) -> Div {
    let mut row = div().flex().flex_row().items_center().gap(space_2());
    if let Some(icon_url) = &author.icon_url {
        row = row.child(
            img(icon_url.clone())
                .id(ElementId::Name(
                    format!("embed-author-{:x}", stable_text_hash(&author.name)).into(),
                ))
                .size(avatar_sm())
                .rounded(radius_full())
                .with_fallback(|| div().size(avatar_sm()).into_any_element()),
        );
    }
    row.child(
        div()
            .text_size(font_size_xs())
            .font_weight(WEIGHT_MEDIUM)
            .text_color(text_primary())
            .child(author.name.clone()),
    )
}

fn stable_text_hash(value: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn render_embed_fields(fields: &[EmbedField], mentions: &MentionLookup) -> Div {
    let mut row = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(space_4())
        .my(space_1());
    for field in fields {
        // Preserve the existing embed field widths already used by message_list.
        let width = if field.inline { px(150.0) } else { px(480.0) };
        row = row.child(
            div()
                .flex()
                .flex_col()
                .w(width)
                .child(
                    div()
                        .text_size(font_size_xs())
                        .font_weight(WEIGHT_BOLD)
                        .text_color(text_primary())
                        .child(field.name.clone()),
                )
                .child(render_markdown_with(&field.value, mentions)),
        );
    }
    row
}

fn render_embed_footer(footer: &EmbedFooter) -> Div {
    let mut row = div().flex().flex_row().items_center().gap(space_2());
    if let Some(icon_url) = &footer.icon_url {
        row = row.child(
            img(icon_url.clone())
                .id(ElementId::Name(
                    format!("embed-footer-{:x}", stable_text_hash(&footer.text)).into(),
                ))
                .size(avatar_sm())
                .rounded(radius_xs())
                .with_fallback(|| div().size(avatar_sm()).into_any_element()),
        );
    }
    row.child(
        div()
            .text_size(font_size_xs())
            .text_color(text_muted())
            .child(footer.text.clone()),
    )
}

/// Renders rich embeds with their real color, author, title, description, fields,
/// thumbnail, image and footer. URLs always come from the Discord payload.
pub fn render_rich_embeds(embeds: &[Embed]) -> Div {
    render_rich_embeds_with(embeds, &MentionLookup::default())
}

pub fn render_rich_embeds_with(embeds: &[Embed], mentions: &MentionLookup) -> Div {
    let mut container = div()
        .flex()
        .flex_col()
        .gap(space_2())
        .my(space_2())
        .max_w(px(520.0));
    if embeds.is_empty() {
        return container;
    }

    for embed in embeds {
        let mut card = div()
            .flex()
            .flex_col()
            .max_w(px(520.0))
            .bg(bg_channels())
            .rounded(radius_sm())
            .p(space_4())
            .gap(space_2());
        if let Some(color) = embed.color {
            card = card.border_l_4().border_color(rgb(color));
        }

        if let Some(author) = &embed.author {
            card = card.child(render_embed_author(author));
        }
        if let Some(title) = &embed.title {
            let url = embed.url.clone();
            let title_color = if url.is_some() {
                discord_link_color()
            } else {
                text_titles()
            };
            let mut title_element = div()
                .text_size(font_size_base())
                .font_weight(WEIGHT_BOLD)
                .text_color(title_color)
                .child(title.clone());
            if let Some(url) = url {
                if let Some(safe_url) = sanitize_http_url(&url) {
                    title_element = title_element.cursor_pointer().on_mouse_down(
                        MouseButton::Left,
                        move |_event, _window, cx| {
                            cx.open_url(&safe_url);
                            cx.stop_propagation();
                        },
                    );
                }
            }
            card = card.child(title_element);
        }
        if let Some(description) = &embed.description {
            card = card.child(render_markdown_with(description, mentions));
        }
        if !embed.fields.is_empty() {
            card = card.child(render_embed_fields(&embed.fields, mentions));
        }

        if let Some(image_url) = &embed.image_url {
            card = card.child(
                img(image_url.clone())
                    .id(ElementId::Name(
                        format!("embed-image-{:x}", stable_text_hash(image_url)).into(),
                    ))
                    .max_w(px(400.0))
                    .max_h(px(200.0))
                    .rounded(radius_sm())
                    .with_fallback(|| div().max_w(px(400.0)).max_h(px(200.0)).into_any_element()),
            );
        }

        if let Some(thumbnail_url) = &embed.thumbnail_url {
            let thumbnail_url = thumbnail_url.clone();
            let safe_thumb = sanitize_http_url(&thumbnail_url);
            let mut thumb = img(thumbnail_url.clone())
                .id(ElementId::Name(
                    format!("embed-thumb-{:x}", stable_text_hash(&thumbnail_url)).into(),
                ))
                .size(avatar_lg())
                .rounded(radius_sm())
                .with_fallback(|| div().size(avatar_lg()).into_any_element());
            if let Some(url) = safe_thumb {
                thumb = thumb.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |_event, _window, cx| {
                        cx.open_url(&url);
                        cx.stop_propagation();
                    },
                );
            }
            card = card.child(div().flex().flex_row().child(thumb));
        }

        if let Some(footer) = &embed.footer {
            card = card.child(render_embed_footer(footer));
        }
        if let Some(timestamp) = embed.timestamp {
            card = card.child(
                div()
                    .text_size(font_size_xs())
                    .text_color(text_muted())
                    .child(crate::chat::message_list::format_message_full_timestamp(
                        timestamp,
                    )),
            );
        }

        container = container.child(card);
    }

    container
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_parse_bold_and_italic() {
        let input = "Este é um texto **em negrito** e *em itálico*.";
        let inlines = parse_inlines(input);
        assert_eq!(inlines.len(), 5);
        assert_eq!(inlines[0], InlineSpan::Text("Este é um texto ".into()));
        assert_eq!(
            inlines[1],
            InlineSpan::Bold(vec![InlineSpan::Text("em negrito".into())])
        );
        assert_eq!(inlines[2], InlineSpan::Text(" e ".into()));
        assert_eq!(
            inlines[3],
            InlineSpan::Italic(vec![InlineSpan::Text("em itálico".into())])
        );
        assert_eq!(inlines[4], InlineSpan::Text(".".into()));
    }

    #[::core::prelude::v1::test]
    fn test_parse_spoilers() {
        let input = "O segredo é ||segredo revelado||!";
        let inlines = parse_inlines(input);
        assert_eq!(inlines.len(), 3);
        assert_eq!(inlines[0], InlineSpan::Text("O segredo é ".into()));
        assert_eq!(
            inlines[1],
            InlineSpan::Spoiler(vec![InlineSpan::Text("segredo revelado".into())])
        );
        assert_eq!(inlines[2], InlineSpan::Text("!".into()));
    }

    #[::core::prelude::v1::test]
    fn test_parse_inline_code() {
        let input = "Use o comando `cargo check` para verificar.";
        let inlines = parse_inlines(input);
        assert_eq!(inlines.len(), 3);
        assert_eq!(inlines[1], InlineSpan::InlineCode("cargo check".into()));
    }

    #[::core::prelude::v1::test]
    fn test_parse_mentions() {
        let input = "Olá @quill e #geral, veja <@123456789>!";
        let inlines = parse_inlines(input);
        assert!(inlines
            .iter()
            .any(|i| matches!(i, InlineSpan::UserMention { name, .. } if name == "quill")));
        assert!(inlines
            .iter()
            .any(|i| matches!(i, InlineSpan::ChannelMention { name, .. } if name == "geral")));
        assert!(inlines
            .iter()
            .any(|i| matches!(i, InlineSpan::UserMention { id: Some(_), .. })));
    }

    #[::core::prelude::v1::test]
    fn test_parse_code_blocks() {
        let input =
            "Aqui está o código:\n```rust\nfn main() {\n    println!(\"GPUI\");\n}\n```\nFim.";
        let blocks = parse_markdown(input);
        assert_eq!(blocks.len(), 3);
        match &blocks[1] {
            MarkdownBlock::CodeBlock { language, content } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert!(content.contains("println!"));
            }
            _ => panic!("Expected CodeBlock"),
        }
    }

    #[::core::prelude::v1::test]
    fn test_parse_quotes() {
        let input = "> Citação rápida\nTexto normal";
        let blocks = parse_markdown(input);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0], MarkdownBlock::BlockQuote(_)));
    }

    #[::core::prelude::v1::test]
    fn test_parse_custom_emojis() {
        let input = "Olha esse <:ferris:123456789> e animado <a:party_blob:987654321>!";
        let inlines = parse_inlines(input);
        assert_eq!(inlines.len(), 5);
        assert_eq!(inlines[0], InlineSpan::Text("Olha esse ".into()));
        assert_eq!(
            inlines[1],
            InlineSpan::CustomEmoji {
                name: "ferris".into(),
                id: "123456789".into(),
                animated: false,
            }
        );
        assert_eq!(inlines[2], InlineSpan::Text(" e animado ".into()));
        assert_eq!(
            inlines[3],
            InlineSpan::CustomEmoji {
                name: "party_blob".into(),
                id: "987654321".into(),
                animated: true,
            }
        );
        assert_eq!(inlines[4], InlineSpan::Text("!".into()));
    }

    #[::core::prelude::v1::test]
    fn test_render_single_custom_emoji_and_inline() {
        let single_input = "<a:hype:112233>";
        let blocks = parse_markdown(single_input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 1);
                assert_eq!(
                    inlines[0],
                    InlineSpan::CustomEmoji {
                        name: "hype".into(),
                        id: "112233".into(),
                        animated: true,
                    }
                );
            }
            _ => panic!("Expected Paragraph"),
        }
        let _rendered_single = render_markdown(single_input);

        let mixed_input = "Texto com <:custom:445566> junto.";
        let _rendered_mixed = render_markdown(mixed_input);
    }

    #[::core::prelude::v1::test]
    fn test_parse_dynamic_timestamps_and_role_mentions() {
        let inlines = parse_inlines("Reunião <t:1791732600:f> cargo <@&987654321>");
        assert!(inlines.iter().any(|span| matches!(
            span,
            InlineSpan::Timestamp { unix: 1791732600, format } if format == "f"
        )));
        assert!(inlines.iter().any(|span| matches!(
            span,
            InlineSpan::RoleMention { id: Some(id), name } if id == "987654321" && name == id
        )));
    }

    #[::core::prelude::v1::test]
    fn test_format_dynamic_timestamp_uses_local_timezone() {
        let unix = 1_791_732_600;
        let local = DateTime::<Utc>::from_timestamp(unix, 0)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(
            format_discord_timestamp(unix, "t"),
            local.format("%H:%M").to_string()
        );
        assert_eq!(
            format_discord_timestamp(unix, "T"),
            local.format("%H:%M:%S").to_string()
        );
        assert_eq!(
            format_discord_timestamp(unix, "d"),
            local.format("%d/%m/%Y").to_string()
        );
        assert!(
            format_discord_timestamp(unix, "R").starts_with("há ")
                || format_discord_timestamp(unix, "R") == "agora"
                || format_discord_timestamp(unix, "R").starts_with("em ")
        );
    }

    #[::core::prelude::v1::test]
    fn test_parse_underline_and_strikethrough() {
        let inlines = parse_inlines("__sublinhado__ ~~riscado~~");
        assert!(inlines
            .iter()
            .any(|span| matches!(span, InlineSpan::Underline(_))));
        assert!(inlines
            .iter()
            .any(|span| matches!(span, InlineSpan::Strikethrough(_))));
    }

    #[::core::prelude::v1::test]
    fn test_spoiler_reveal_state_round_trip() {
        let key = "synthetic-spoiler-state";
        let _ = REVEALED_SPOILERS.lock().unwrap().remove(key);
        assert!(!spoiler_is_revealed(key));
        toggle_spoiler(key);
        assert!(spoiler_is_revealed(key));
        toggle_spoiler(key);
        assert!(!spoiler_is_revealed(key));
    }

    #[::core::prelude::v1::test]
    fn test_unicode_emoji_sequences_are_single_spans() {
        for emoji in ["🦀", "👍🏽", "👩‍💻", "🇧🇷", "#️⃣"] {
            assert_eq!(
                parse_inlines(emoji),
                vec![InlineSpan::Emoji(emoji.to_string())],
                "{}",
                emoji
            );
        }
        assert_eq!(parse_inlines("1"), vec![InlineSpan::Text("1".into())]);
    }

    #[::core::prelude::v1::test]
    fn test_raw_url_trims_only_sentence_punctuation() {
        let inlines = parse_inlines("Veja https://example.com/path.");
        assert_eq!(
            inlines,
            vec![
                InlineSpan::Text("Veja ".into()),
                InlineSpan::Link {
                    label: "https://example.com/path".into(),
                    url: "https://example.com/path".into(),
                },
                InlineSpan::Text(".".into()),
            ]
        );

        let balanced = parse_inlines("(https://example.com/a_(b))");
        assert!(balanced.iter().any(|span| matches!(
            span,
            InlineSpan::Link { url, .. } if url == "https://example.com/a_(b)"
        )));
    }

    #[::core::prelude::v1::test]
    fn test_angle_bracket_autolink() {
        assert_eq!(
            parse_inlines("<https://example.com/path>"),
            vec![InlineSpan::Link {
                label: "https://example.com/path".into(),
                url: "https://example.com/path".into(),
            }]
        );
    }

    #[::core::prelude::v1::test]
    fn test_unclosed_code_fence_consumes_raw_content_to_eof() {
        let input = "```rust\nlet x = **não formatado**;\n\n";
        let blocks = parse_markdown(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::CodeBlock { language, content } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert_eq!(content, "let x = **não formatado**;\n\n");
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[::core::prelude::v1::test]
    fn test_double_backtick_inline_code_preserves_inner_tick() {
        assert_eq!(
            parse_inlines("``code ` tick``"),
            vec![InlineSpan::InlineCode("code ` tick".into())]
        );
    }

    #[::core::prelude::v1::test]
    fn test_backslash_escapes_disable_formatting() {
        let inlines = parse_inlines(r"\*literal\* \|\|segredo\|\| \`código\`");
        assert_eq!(
            inlines,
            vec![InlineSpan::Text("*literal* ||segredo|| `código`".into())]
        );
    }

    #[::core::prelude::v1::test]
    fn test_email_is_not_a_user_mention() {
        assert_eq!(
            parse_inlines("contato@example.com"),
            vec![InlineSpan::Text("contato@example.com".into())]
        );
    }

    #[::core::prelude::v1::test]
    fn test_short_timestamp_styles_are_accepted() {
        let inlines = parse_inlines("<t:1791732600:s> <t:1791732600:S>");
        assert!(inlines
            .iter()
            .any(|span| matches!(span, InlineSpan::Timestamp { format, .. } if format == "s")));
        assert!(inlines
            .iter()
            .any(|span| matches!(span, InlineSpan::Timestamp { format, .. } if format == "S")));
    }

    #[::core::prelude::v1::test]
    fn test_attachment_without_api_dimensions_has_no_invented_size() {
        let attachment = Attachment {
            id: "attachment-synthetic-no-size".into(),
            filename: "synthetic.png".into(),
            size: 1024,
            url: "https://cdn.discordapp.com/attachments/synthetic/no-size.png".into(),
            proxy_url: None,
            content_type: Some("image/png".into()),
            width: None,
            height: None,
        };
        assert_eq!(attachment_display_size(&attachment), None);
    }

    #[::core::prelude::v1::test]
    fn test_render_rich_attachments_and_embeds() {
        let attachment = Attachment {
            id: "attachment-synthetic".into(),
            filename: "synthetic.png".into(),
            size: 1024,
            url: "https://cdn.discordapp.com/attachments/synthetic/image.png".into(),
            proxy_url: None,
            content_type: Some("image/png".into()),
            width: Some(800),
            height: Some(600),
        };
        let embed = Embed {
            title: Some("Título sintético".into()),
            description: Some("Descrição **real** do payload".into()),
            url: Some("https://example.com/synthetic".into()),
            color: Some(0x5865f2),
            author: None,
            fields: vec![EmbedField {
                name: "Campo".into(),
                value: "Valor".into(),
                inline: true,
            }],
            footer: Some(EmbedFooter {
                text: "Rodapé".into(),
                icon_url: None,
            }),
            image_url: Some("https://cdn.discordapp.com/attachments/synthetic/embed.png".into()),
            thumbnail_url: Some(
                "https://cdn.discordapp.com/attachments/synthetic/thumb.png".into(),
            ),
            timestamp: None,
        };
        let _ = render_rich_attachments(&[attachment]);
        let _ = render_rich_embeds(&[embed]);
    }
}
