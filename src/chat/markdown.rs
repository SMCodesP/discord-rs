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
//! - User / Channel / Role Mentions: `@user`, `@everyone`, `@here`, `<@id>`, `<#id>`, `#canal`
//! - Quotes: `> texto` and `>>> texto`
//! - Subtext: `-# texto pequeno`
//! - Headings: `# H1`, `## H2`, `### H3`

use gpui::*;
use regex::Regex;
use std::sync::LazyLock;

use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

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
    UserMention { id: Option<String>, name: String },
    ChannelMention { id: Option<String>, name: String },
    RoleMention { id: Option<String>, name: String },
    Link { label: String, url: String },
    Emoji(String),
    CustomEmoji { name: String, id: String, animated: bool },
}

/// Represents block-level elements in Discord markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    Heading { level: u8, inlines: Vec<InlineSpan> },
    Paragraph(Vec<InlineSpan>),
    CodeBlock { language: Option<String>, content: String },
    BlockQuote(Vec<MarkdownBlock>),
    Subtext(Vec<InlineSpan>),
}

// ============================================================================
// Regular Expressions for Parsing
// ============================================================================

static RE_CODE_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)```(?:([a-zA-Z0-9_\-+]+)\n)?(.*?)```").expect("invalid code block regex")
});

static RE_USER_MENTION_SNOWFLAKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<@!?([0-9]+)>").expect("invalid snowflake regex")
});

static RE_CHANNEL_MENTION_SNOWFLAKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<#([0-9]+)>").expect("invalid channel snowflake regex")
});

static RE_ROLE_MENTION_SNOWFLAKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<@&([0-9]+)>").expect("invalid role snowflake regex")
});

static RE_NAMED_USER_MENTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^@([a-zA-Z0-9_\.\-]+)").expect("invalid user mention regex")
});

static RE_NAMED_CHANNEL_MENTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^#([a-zA-Z0-9_\-]+)").expect("invalid channel mention regex")
});

static RE_MD_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[([^\]]+)\]\((https?://[^\s)]+)\)").expect("invalid md link regex")
});

static RE_RAW_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(https?://[^\s<]+)").expect("invalid raw url regex")
});

static RE_CUSTOM_EMOJI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^<(a)?:([a-zA-Z0-9_]+):([0-9]+)>").expect("invalid custom emoji regex")
});

pub static MARKDOWN_AST_CACHE: LazyLock<std::sync::Mutex<std::collections::HashMap<String, Vec<MarkdownBlock>>>> =
    LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::with_capacity(512)));

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

    // Check if input contains fenced code blocks
    let mut last_idx = 0;
    for cap in RE_CODE_BLOCK.captures_iter(input) {
        let whole_match = cap.get(0).unwrap();
        let match_start = whole_match.start();
        let match_end = whole_match.end();

        // Process any text preceding this code block
        if match_start > last_idx {
            let prefix = &input[last_idx..match_start];
            blocks.extend(parse_text_blocks(prefix));
        }

        let lang = cap.get(1).map(|m| m.as_str().to_string());
        let code = cap.get(2).map(|m| m.as_str().trim_end_matches('\n').to_string()).unwrap_or_default();
        blocks.push(MarkdownBlock::CodeBlock {
            language: lang,
            content: code,
        });

        last_idx = match_end;
    }

    // Process remaining text after the last code block
    if last_idx < input.len() {
        let suffix = &input[last_idx..];
        blocks.extend(parse_text_blocks(suffix));
    }

    blocks
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

        // 2. Inline Code: `` `code` ``
        if chars[idx] == '`' {
            if let Some(end_offset) = find_single_char(&chars[idx + 1..], '`') {
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                let code_content: String = chars[idx + 1..idx + 1 + end_offset].iter().collect();
                inlines.push(InlineSpan::InlineCode(code_content));
                idx = idx + 1 + end_offset + 1;
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
        if chars[idx] == '<' || chars[idx] == '@' || chars[idx] == '#' || chars[idx] == '[' || remaining_str.starts_with("http") {
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

            // Snowflake user mention `<@12345>`
            if let Some(cap) = RE_USER_MENTION_SNOWFLAKE.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let id = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::UserMention {
                    id: Some(id.clone()),
                    name: format!("user-{}", &id[..id.len().min(4)]),
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
                    name: format!("canal-{}", &id[..id.len().min(4)]),
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
                    name: format!("cargo-{}", &id[..id.len().min(4)]),
                });
                idx += full.chars().count();
                continue;
            }

            // Named user mention `@quill`, `@everyone`, `@here`
            if let Some(cap) = RE_NAMED_USER_MENTION.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let name = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::UserMention {
                    id: None,
                    name,
                });
                idx += full.chars().count();
                continue;
            }

            // Named channel mention `#geral`, `#chat`
            if let Some(cap) = RE_NAMED_CHANNEL_MENTION.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                let name = cap.get(1).unwrap().as_str().to_string();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::ChannelMention {
                    id: None,
                    name,
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

            // Raw URL `https://...`
            if let Some(cap) = RE_RAW_URL.captures(&remaining_str) {
                let full = cap.get(0).unwrap().as_str();
                if !buffer.is_empty() {
                    inlines.push(InlineSpan::Text(std::mem::take(&mut buffer)));
                }
                inlines.push(InlineSpan::Link {
                    label: full.to_string(),
                    url: full.to_string(),
                });
                idx += full.chars().count();
                continue;
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
// GPUI Native Renderer
// ============================================================================

/// Renders a full Discord message text natively into a GPUI `Div` element,
/// reusing MARKDOWN_AST_CACHE directly without cloning the AST vector.
pub fn render_markdown(input: &str) -> Div {
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
                container = container.child(render_block(block));
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
        container = container.child(render_block(block));
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
                .children(inlines.iter().map(render_inline))
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
                && inlines.iter().any(|span| matches!(span, InlineSpan::CustomEmoji { .. } | InlineSpan::Emoji(_)));

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
                .children(inlines.iter().map(|span| render_inline_span(span, is_single_emoji)))
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
                    .text_color(rgb(0xe0e2e5))
                    .font_family(FONT_FAMILY_CODE)
                    .child(content.clone()),
            )
        }

        MarkdownBlock::BlockQuote(blocks) => {
            div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .my(space_1())
                .child(
                    div()
                        .w(px(4.0))
                        .flex_shrink_0()
                        .bg(borders())
                        .rounded(radius_xs()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.0))
                        .pl(space_4())
                        .gap(space_1())
                        .children(blocks.iter().map(render_block)),
                )
        }

        MarkdownBlock::Subtext(inlines) => {
            div()
                .w_full()
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .text_size(font_size_xs())
                .text_color(text_muted())
                .children(inlines.iter().map(render_inline))
        }
    }
}

/// Renders a single inline formatted element with optional single emoji flag.
pub fn render_inline_span(inline: &InlineSpan, is_single_emoji: bool) -> Div {
    match inline {
        InlineSpan::Text(text) => {
            div()
                .text_color(text_primary())
                .child(text.clone())
        }

        InlineSpan::Bold(children) => {
            div()
                .font_weight(WEIGHT_BOLD)
                .text_color(text_titles())
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .children(children.iter().map(render_inline))
        }

        InlineSpan::Italic(children) => {
            div()
                .text_color(text_primary())
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .children(children.iter().map(render_inline))
        }

        InlineSpan::BoldItalic(children) => {
            div()
                .font_weight(WEIGHT_BOLD)
                .text_color(text_titles())
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .children(children.iter().map(render_inline))
        }

        InlineSpan::Underline(children) => {
            div()
                .border_b_1()
                .border_color(text_primary())
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .children(children.iter().map(render_inline))
        }

        InlineSpan::Strikethrough(children) => {
            // Styled strikethrough with dim opacity
            div()
                .opacity(0.6)
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .children(children.iter().map(render_inline))
        }

        InlineSpan::InlineCode(code) => {
            div()
                .bg(rgb(HEX_BG_USER_BAR))
                .text_color(rgb(0xe0e2e5))
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .text_size(px(13.0))
                .font_family(FONT_FAMILY_CODE)
                .child(code.clone())
        }

        InlineSpan::Spoiler(children) => {
            // Discord spoiler pill: dark charcoal pill with masked text that reveals on hover
            div()
                .bg(rgb(0x232428))
                .hover(|s| s.bg(rgb(0x383a40)))
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .cursor_pointer()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_baseline()
                .children(children.iter().map(render_inline))
        }

        InlineSpan::UserMention { name, .. } => {
            div()
                .bg(mention_bg())
                .hover(|s| s.bg(mention_hover()))
                .text_color(blurple())
                .font_weight(WEIGHT_MEDIUM)
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .cursor_pointer()
                .child(format!("@{}", name))
        }

        InlineSpan::ChannelMention { name, .. } => {
            div()
                .bg(mention_bg())
                .hover(|s| s.bg(mention_hover()))
                .text_color(blurple())
                .font_weight(WEIGHT_MEDIUM)
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .cursor_pointer()
                .child(format!("#{}", name))
        }

        InlineSpan::RoleMention { name, .. } => {
            div()
                .bg(mention_bg())
                .hover(|s| s.bg(mention_hover()))
                .text_color(blurple())
                .font_weight(WEIGHT_MEDIUM)
                .rounded(radius_xs())
                .px(space_2())
                .py(px(1.0))
                .mx(space_1())
                .cursor_pointer()
                .child(format!("@{}", name))
        }

        InlineSpan::Link { label, .. } => {
            div()
                .text_color(rgb(0x00a8fc))
                .hover(|s| s.border_b_1().border_color(rgb(0x00a8fc)))
                .cursor_pointer()
                .child(label.clone())
        }

        InlineSpan::Emoji(emoji) => {
            let mut el = div().mx(space_1());
            if is_single_emoji {
                el = el.text_size(px(40.0));
            }
            el.child(emoji.clone())
        }

        InlineSpan::CustomEmoji { name: _, id, animated } => {
            let ext = if *animated { "gif" } else { "png" };
            let (img_sz, cdn_sz) = if is_single_emoji {
                (px(48.0), 48)
            } else {
                (px(22.0), 48)
            };
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
                        .id(ElementId::from(SharedString::from(format!("md-emoji-img-{}", id))))
                        .size_full()
                        .rounded(radius_xs()),
                )
        }
    }
}

/// Renders a single inline formatted element.
pub fn render_inline(inline: &InlineSpan) -> Div {
    render_inline_span(inline, false)
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
        assert_eq!(inlines[1], InlineSpan::Bold(vec![InlineSpan::Text("em negrito".into())]));
        assert_eq!(inlines[2], InlineSpan::Text(" e ".into()));
        assert_eq!(inlines[3], InlineSpan::Italic(vec![InlineSpan::Text("em itálico".into())]));
        assert_eq!(inlines[4], InlineSpan::Text(".".into()));
    }

    #[::core::prelude::v1::test]
    fn test_parse_spoilers() {
        let input = "O segredo é ||segredo revelado||!";
        let inlines = parse_inlines(input);
        assert_eq!(inlines.len(), 3);
        assert_eq!(inlines[0], InlineSpan::Text("O segredo é ".into()));
        assert_eq!(inlines[1], InlineSpan::Spoiler(vec![InlineSpan::Text("segredo revelado".into())]));
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
        assert!(inlines.iter().any(|i| matches!(i, InlineSpan::UserMention { name, .. } if name == "quill")));
        assert!(inlines.iter().any(|i| matches!(i, InlineSpan::ChannelMention { name, .. } if name == "geral")));
        assert!(inlines.iter().any(|i| matches!(i, InlineSpan::UserMention { id: Some(_), .. })));
    }

    #[::core::prelude::v1::test]
    fn test_parse_code_blocks() {
        let input = "Aqui está o código:\n```rust\nfn main() {\n    println!(\"GPUI\");\n}\n```\nFim.";
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
}
