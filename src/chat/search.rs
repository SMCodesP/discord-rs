//! Discord RS - Search Engine and Filter Parser
//!
//! Provides Discord-compatible full-text search and faceted filter parsing:
//! - `from:<user>`: Filter by author username or ID
//! - `mentions:<user>`: Filter by mentioned user
//! - `has:link|embed|file|video|image|sound`: Filter by attachment/embed types
//! - `in:<channel>`: Filter by channel name or ID
//! - `before:<YYYY-MM-DD>`: Messages sent before date
//! - `after:<YYYY-MM-DD>`: Messages sent after date
//! - `during:<YYYY-MM-DD>`: Messages sent on specific date
//! - `pinned:true|false`: Filter by pinned status
//! - Free text search tokens
//!
//! Also defines DTOs for Discord REST API search endpoints:
//! `GET /api/v10/channels/{channel_id}/messages/search`
//! `GET /api/v10/guilds/{guild_id}/messages/search`

use chrono::NaiveDate;
use gpui::*;
use serde::{Deserialize, Serialize};

use crate::app::state::AppState;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::domain::Message;
use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;

/// Filter for presence of attachments, links, or media types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HasFilter {
    Link,
    Embed,
    File,
    Video,
    Image,
    Sound,
    Sticker,
}

impl HasFilter {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "link" | "url" => Some(Self::Link),
            "embed" => Some(Self::Embed),
            "file" => Some(Self::File),
            "video" => Some(Self::Video),
            "image" | "photo" | "picture" => Some(Self::Image),
            "sound" | "audio" => Some(Self::Sound),
            "sticker" | "stickers" => Some(Self::Sticker),
            _ => None,
        }
    }

    pub fn matches(&self, message: &Message) -> bool {
        match self {
            Self::Link => {
                let content_lower = message.content.to_lowercase();
                content_lower.contains("http://") || content_lower.contains("https://")
            }
            Self::Embed => !message.embeds.is_empty(),
            Self::File => !message.attachments.is_empty(),
            Self::Image => {
                message.attachments.iter().any(|a| a.is_image())
                    || message.embeds.iter().any(|e| e.image_url.is_some() || e.thumbnail_url.is_some())
            }
            Self::Video => message.attachments.iter().any(|a| {
                let ct = a.content_type.as_deref().unwrap_or("");
                let fn_lower = a.filename.to_lowercase();
                ct.starts_with("video/")
                    || fn_lower.ends_with(".mp4")
                    || fn_lower.ends_with(".webm")
                    || fn_lower.ends_with(".mov")
            }),
            Self::Sound => message.attachments.iter().any(|a| {
                let ct = a.content_type.as_deref().unwrap_or("");
                let fn_lower = a.filename.to_lowercase();
                ct.starts_with("audio/")
                    || fn_lower.ends_with(".mp3")
                    || fn_lower.ends_with(".ogg")
                    || fn_lower.ends_with(".wav")
                    || fn_lower.ends_with(".flac")
            }),
            Self::Sticker => !message.sticker_items.is_empty(),
        }
    }
}

/// Parsed structured search query.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Raw unparsed query string
    pub raw: String,
    /// Free-text search terms (e.g. "rust", "release")
    pub text_terms: Vec<String>,
    /// Filter by author: `from:username`
    pub from: Option<String>,
    /// Filter by mentions: `mentions:username`
    pub mentions: Option<String>,
    /// Media / link filters: `has:link`, `has:file`, etc.
    pub has_filters: Vec<HasFilter>,
    /// Channel filter: `in:channel-name`
    pub in_channel: Option<String>,
    /// Messages before date: `before:YYYY-MM-DD`
    pub before: Option<NaiveDate>,
    /// Messages after date: `after:YYYY-MM-DD`
    pub after: Option<NaiveDate>,
    /// Messages during date: `during:YYYY-MM-DD`
    pub during: Option<NaiveDate>,
    /// Filter pinned messages: `pinned:true` or `pinned:false`
    pub pinned: Option<bool>,
}

fn tokenize_search_input(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        if ch == '"' {
            chars.next();
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '"' {
                    break;
                }
                s.push(c);
            }
            if !s.is_empty() {
                tokens.push(s);
            }
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                chars.next();
                if c == '"' {
                    while let Some(&qc) = chars.peek() {
                        chars.next();
                        if qc == '"' {
                            break;
                        }
                        s.push(qc);
                    }
                } else {
                    s.push(c);
                }
            }
            if !s.is_empty() {
                tokens.push(s);
            }
        }
    }
    tokens
}

fn parse_search_date(val: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(val, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(val, "%Y/%m/%d"))
        .ok()
}

impl SearchQuery {
    /// Parses a raw search string following Discord's faceted query syntax.
    /// Handles both `prefix:val` and `prefix: val`, quoted values, and free-text terms.
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        let mut query = Self {
            raw: trimmed.to_string(),
            ..Default::default()
        };

        if trimmed.is_empty() {
            return query;
        }

        let raw_tokens = tokenize_search_input(trimmed);
        let mut idx = 0;

        while idx < raw_tokens.len() {
            let token = &raw_tokens[idx];

            let (prefix, val_opt) = if let Some(stripped) = token.strip_prefix("from:") {
                ("from:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("mentions:") {
                ("mentions:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("has:") {
                ("has:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("in:") {
                ("in:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("before:") {
                ("before:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("after:") {
                ("after:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("during:") {
                ("during:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else if let Some(stripped) = token.strip_prefix("pinned:") {
                ("pinned:", if stripped.is_empty() { None } else { Some(stripped.to_string()) })
            } else {
                ("", Some(token.clone()))
            };

            if prefix.is_empty() {
                query.text_terms.push(token.to_lowercase());
                idx += 1;
                continue;
            }

            // If value was attached (e.g. from:alice), use it; otherwise take next token if available
            let filter_val = if let Some(v) = val_opt {
                v
            } else if idx + 1 < raw_tokens.len() {
                idx += 1;
                raw_tokens[idx].clone()
            } else {
                idx += 1;
                continue;
            };

            match prefix {
                "from:" => {
                    let clean = filter_val.trim_start_matches('@').trim().to_lowercase();
                    if !clean.is_empty() {
                        query.from = Some(clean);
                    }
                }
                "mentions:" => {
                    let clean = filter_val.trim_start_matches('@').trim().to_lowercase();
                    if !clean.is_empty() {
                        query.mentions = Some(clean);
                    }
                }
                "has:" => {
                    if let Some(has) = HasFilter::parse(&filter_val) {
                        query.has_filters.push(has);
                    }
                }
                "in:" => {
                    let clean = filter_val.trim_start_matches('#').trim().to_lowercase();
                    if !clean.is_empty() {
                        query.in_channel = Some(clean);
                    }
                }
                "before:" => {
                    if let Some(d) = parse_search_date(&filter_val) {
                        query.before = Some(d);
                    }
                }
                "after:" => {
                    if let Some(d) = parse_search_date(&filter_val) {
                        query.after = Some(d);
                    }
                }
                "during:" => {
                    if let Some(d) = parse_search_date(&filter_val) {
                        query.during = Some(d);
                    }
                }
                "pinned:" => {
                    match filter_val.to_lowercase().as_str() {
                        "true" | "yes" | "1" => query.pinned = Some(true),
                        "false" | "no" | "0" => query.pinned = Some(false),
                        _ => {}
                    }
                }
                _ => {}
            }

            idx += 1;
        }

        query
    }

    /// Whether this query has any active constraints or terms.
    pub fn is_empty(&self) -> bool {
        self.text_terms.is_empty()
            && self.from.is_none()
            && self.mentions.is_none()
            && self.has_filters.is_empty()
            && self.in_channel.is_none()
            && self.before.is_none()
            && self.after.is_none()
            && self.during.is_none()
            && self.pinned.is_none()
    }

    /// Serializes query constraints into REST API query parameters for Discord search endpoints:
    /// `GET /api/v10/channels/{channel_id}/messages/search` or `GET /api/v10/guilds/{guild_id}/messages/search`
    pub fn to_api_query_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if !self.text_terms.is_empty() {
            params.push(("content", self.text_terms.join(" ")));
        }
        if let Some(ref from) = self.from {
            params.push(("author_id", from.clone()));
        }
        if let Some(ref mentions) = self.mentions {
            params.push(("mentions", mentions.clone()));
        }
        if let Some(ref ch) = self.in_channel {
            params.push(("channel_id", ch.clone()));
        }
        for has in &self.has_filters {
            let s = match has {
                HasFilter::Link => "link",
                HasFilter::Embed => "embed",
                HasFilter::File => "file",
                HasFilter::Video => "video",
                HasFilter::Image => "image",
                HasFilter::Sound => "sound",
                HasFilter::Sticker => "sticker",
            };
            params.push(("has", s.to_string()));
        }
        if let Some(pinned) = self.pinned {
            params.push(("pinned", pinned.to_string()));
        }
        params
    }

    /// Checks if a message matches all query criteria.
    pub fn matches(&self, message: &Message, channel_name: Option<&str>) -> bool {
        // 1. Text terms matching (all free text terms must be found in content, attachments, author, embeds, or stickers)
        let content_lower = message.content.to_lowercase();
        for term in &self.text_terms {
            let in_content = content_lower.contains(term);
            let in_attachment = message
                .attachments
                .iter()
                .any(|a| a.filename.to_lowercase().contains(term));
            let in_author = message.author.username.to_lowercase().contains(term)
                || message
                    .author
                    .global_name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(term);
            let in_embed = message.embeds.iter().any(|e| {
                e.title.as_deref().unwrap_or("").to_lowercase().contains(term)
                    || e.description.as_deref().unwrap_or("").to_lowercase().contains(term)
                    || e.fields.iter().any(|f| {
                        f.name.to_lowercase().contains(term)
                            || f.value.to_lowercase().contains(term)
                    })
                    || e.author.as_ref().map(|a| a.name.to_lowercase().contains(term)).unwrap_or(false)
                    || e.footer.as_ref().map(|f| f.text.to_lowercase().contains(term)).unwrap_or(false)
            });
            let in_sticker = message
                .sticker_items
                .iter()
                .any(|s| s.name.to_lowercase().contains(term));

            if !in_content && !in_attachment && !in_author && !in_embed && !in_sticker {
                return false;
            }
        }

        // 2. Author filter (`from:`)
        if let Some(ref from) = self.from {
            let author_user = message.author.username.to_lowercase();
            let author_id = message.author.id.to_lowercase();
            let author_global = message
                .author
                .global_name
                .as_deref()
                .unwrap_or("")
                .to_lowercase();

            if !author_user.contains(from) && !author_id.contains(from) && !author_global.contains(from) {
                return false;
            }
        }

        // 3. Mentions filter (`mentions:`)
        if let Some(ref mention) = self.mentions {
            let mention_id_tag = format!("<@{}>", mention);
            let mention_nick_tag = format!("<@!{}>", mention);
            let mention_name_tag = format!("@{}", mention);
            if !content_lower.contains(&mention_id_tag)
                && !content_lower.contains(&mention_nick_tag)
                && !content_lower.contains(&mention_name_tag)
                && !content_lower.contains(mention)
            {
                return false;
            }
        }

        // 4. Has filters (`has:link`, `has:file`, `has:sticker`, etc.)
        for filter in &self.has_filters {
            if !filter.matches(message) {
                return false;
            }
        }

        // 5. Channel filter (`in:`)
        if let Some(ref target_ch) = self.in_channel {
            let mut matched = false;
            if message.channel_id.to_lowercase() == *target_ch {
                matched = true;
            } else if let Some(ch_name) = channel_name {
                let name_lower = ch_name.to_lowercase();
                if name_lower == *target_ch || name_lower.contains(target_ch) {
                    matched = true;
                }
            }
            if !matched {
                return false;
            }
        }

        // 6. Date filters (`before:`, `after:`, `during:`)
        let msg_date = message.timestamp.date_naive();
        if let Some(before) = self.before {
            if msg_date >= before {
                return false;
            }
        }
        if let Some(after) = self.after {
            if msg_date <= after {
                return false;
            }
        }
        if let Some(during) = self.during {
            if msg_date != during {
                return false;
            }
        }

        // 7. Pinned filter (`pinned:true|false`)
        if let Some(pinned) = self.pinned {
            if message.pinned != pinned {
                return false;
            }
        }

        true
    }
}

/// A search result group containing the matched hit message and surrounding context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResultGroup {
    /// Channel where the hit was found
    pub channel_id: String,
    /// Channel display name (if known)
    pub channel_name: Option<String>,
    /// The primary message matching the search query
    pub hit_message: Message,
    /// Messages preceding the hit message (for context display in sidebar)
    pub context_before: Vec<Message>,
    /// Messages following the hit message (for context display in sidebar)
    pub context_after: Vec<Message>,
}

/// Discord REST API search response schema.
/// Returned by `GET /api/v10/channels/{channel_id}/messages/search` and `GET /api/v10/guilds/{guild_id}/messages/search`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchPayload {
    /// Total count of matching messages across the scope
    pub total_results: u64,
    /// Array of result groups (each group contains the hit message and context messages)
    pub messages: Vec<Vec<ApiSearchMessage>>,
    /// Discord analytics tracking ID
    #[serde(default)]
    pub analytics_id: Option<String>,
}

impl SearchPayload {
    /// Converts REST API response payload into UI-compatible SearchResultGroup structures
    pub fn into_result_groups(self) -> Vec<SearchResultGroup> {
        let mut groups = Vec::new();
        for group in self.messages {
            let hit_idx = group.iter().position(|m| m.hit).unwrap_or(0);
            if let Some(hit_msg) = group.get(hit_idx) {
                let context_before = group[..hit_idx].iter().map(|m| m.message.clone()).collect();
                let context_after = if hit_idx + 1 < group.len() {
                    group[hit_idx + 1..].iter().map(|m| m.message.clone()).collect()
                } else {
                    Vec::new()
                };
                groups.push(SearchResultGroup {
                    channel_id: hit_msg.message.channel_id.clone(),
                    channel_name: None,
                    hit_message: hit_msg.message.clone(),
                    context_before,
                    context_after,
                });
            }
        }
        groups
    }
}

/// Individual message element in API search result groups with the `hit` flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiSearchMessage {
    #[serde(flatten)]
    pub message: Message,
    /// True if this message is the search hit, false if it's surrounding context
    #[serde(default)]
    pub hit: bool,
}

/// Search sorting order (newest first vs oldest first)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSortOrder {
    #[default]
    Desc,
    Asc,
}

impl SearchSortOrder {
    pub fn as_api_str(&self) -> &'static str {
        match self {
            Self::Desc => "desc",
            Self::Asc => "asc",
        }
    }
}

/// Discord REST API search response.
/// Can be either ready results (HTTP 200) or an indexing state (HTTP 202).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SearchApiResponse {
    Ready(SearchPayload),
    Indexing { retry_after: f64 },
}

/// In-memory local search engine for fast client-side query execution over cached messages.
pub struct LocalSearchEngine;

impl LocalSearchEngine {
    /// Executes a search query over a slice of messages, returning grouped results with context.
    pub fn search_channel_messages(
        messages: &[Message],
        query: &SearchQuery,
        channel_id: &str,
        channel_name: Option<&str>,
        context_window: usize,
    ) -> Vec<SearchResultGroup> {
        if query.is_empty() || messages.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();

        for (idx, msg) in messages.iter().enumerate() {
            if query.matches(msg, channel_name) {
                // Collect context before
                let start_idx = idx.saturating_sub(context_window);
                let context_before = messages[start_idx..idx].to_vec();

                // Collect context after
                let end_idx = (idx + 1 + context_window).min(messages.len());
                let context_after = if idx + 1 < messages.len() {
                    messages[idx + 1..end_idx].to_vec()
                } else {
                    Vec::new()
                };

                results.push(SearchResultGroup {
                    channel_id: channel_id.to_string(),
                    channel_name: channel_name.map(ToString::to_string),
                    hit_message: msg.clone(),
                    context_before,
                    context_after,
                });
            }
        }

        // Return latest hits first (Discord default order is descending timestamp)
        results.reverse();
        results
    }
}

// ============================================================================
// Search Flyout UI & Filter Chips Component
// ============================================================================

/// Interactive Discord filter chips
pub const DISCORD_SEARCH_CHIPS: &[(&str, &str)] = &[
    ("from:", "usuário"),
    ("has:", "link, anexo..."),
    ("in:", "canal"),
    ("before:", "YYYY-MM-DD"),
    ("during:", "YYYY-MM-DD"),
    ("after:", "YYYY-MM-DD"),
    ("mentions:", "usuário"),
    ("pinned:", "true/false"),
];

/// Renders message text highlighting search query matches with Discord yellow (#fee75c) and dark text (#1e1f22)
pub fn render_highlighted_text(content: &str, query: &SearchQuery) -> Div {
    let terms: Vec<String> = query
        .text_terms
        .iter()
        .map(|t| t.to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() || content.is_empty() {
        return div()
            .text_color(text_primary())
            .text_size(px(14.0))
            .child(content.to_string());
    }

    let content_lower = content.to_lowercase();
    let mut matches = Vec::new();
    for term in &terms {
        let mut start = 0;
        while let Some(pos) = content_lower[start..].find(term.as_str()) {
            let s = start + pos;
            let e = s + term.len();
            matches.push((s, e));
            start = e;
        }
    }

    if matches.is_empty() {
        return div()
            .text_color(text_primary())
            .text_size(px(14.0))
            .child(content.to_string());
    }

    matches.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in matches {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                last.1 = last.1.max(e);
            } else {
                merged.push((s, e));
            }
        } else {
            merged.push((s, e));
        }
    }

    let mut container = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_baseline()
        .gap(px(1.0))
        .text_size(px(14.0));

    let mut last_idx = 0;
    for (s, e) in merged {
        if s > last_idx && s <= content.len() {
            if let Some(sub) = content.get(last_idx..s) {
                container = container.child(
                    div()
                        .text_color(text_primary())
                        .child(sub.to_string()),
                );
            }
        }
        if let Some(sub) = content.get(s..e) {
            container = container.child(
                div()
                    // COR ANTIGA SEM ORIGEM: legacy_fee75c (0xfee75c)
                    .bg(legacy_fee75c())
                    .text_color(bg_server())
                    .font_weight(FontWeight::BOLD)
                    .rounded(px(2.0))
                    .px(px(2.0))
                    .child(sub.to_string()),
            );
        }
        last_idx = e;
    }
    if last_idx < content.len() {
        if let Some(sub) = content.get(last_idx..) {
            container = container.child(
                div()
                    .text_color(text_primary())
                    .child(sub.to_string()),
            );
        }
    }

    container
}



/// Popover content showing Discord's faceted search filters (search-popover.md spec: 356x335, r8, padding 12px 8px)
pub fn render_search_popover_content(app_entity: Entity<AppState>) -> AnyElement {
    let filters = [
        ("from:", "de um usuário específico", "from: usuário"),
        ("in:", "em um canal específico", "in: canal"),
        ("has:", "contém link, anexo ou mídia", "has: link, embed, file..."),
        ("mentions:", "menciona um usuário", "mentions: usuário"),
        ("before:", "enviadas antes de uma data", "before: YYYY-MM-DD"),
        ("during:", "enviadas em uma data específica", "during: YYYY-MM-DD"),
        ("after:", "enviadas após uma data", "after: YYYY-MM-DD"),
        ("pinned:", "mensagens fixadas", "pinned: true/false"),
    ];

    div()
        .w(px(356.0))
        .p(px(8.0))
        .py(px(12.0))
        .rounded(px(8.0))
        .bg(bg_floating())
        .border_1()
        .border_color(channel_hover())
        .shadow_lg()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .px(px(8.0))
                .pb(px(4.0))
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(text_muted())
                .child("OPÇÕES DE FILTRO"),
        )
        .children(filters.iter().map(|&(prefix, desc, example)| {
            let entity = app_entity.clone();
            div()
                .h(px(28.0))
                .px(px(8.0))
                .rounded(px(8.0))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .hover(|s| s.bg(channel_hover()))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity.update(cx, |state, cx| {
                        let current = state.search_query.trim();
                        if current.is_empty() {
                            state.search_query = format!("{} ", prefix);
                        } else if !current.ends_with(prefix) {
                            state.search_query = format!("{} {} ", current, prefix);
                        }
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_color(blurple())
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(12.0))
                                .child(prefix),
                        )
                        .child(
                            div()
                                .text_color(text_muted())
                                .text_size(px(12.0))
                                .child(desc),
                        ),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(10.0))
                        .child(example),
                )
        }))
        .into_any_element()
}



/// Renders the close button (✕) for the flyout panel
fn render_search_flyout_close_button(app_entity: Entity<AppState>) -> AnyElement {
    div()
        .size(px(28.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(text_muted())
        .hover(|s| s.bg(channel_hover()).text_color(white()))
        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
            app_entity.update(cx, |state, cx| {
                state.clear_search();
                state.close_search();
                cx.notify();
            });
        })
        .child(icon_close(px(14.0), text_muted()))
        .into_any_element()
}

/// Renders the top header row of the search flyout: Results count + Sort tabs (Recentes / Antigos) + Close button
fn render_search_flyout_header(
    app_entity: Entity<AppState>,
    state: &AppState,
) -> AnyElement {
    let total = state.search_total_results();
    let is_searching = state.is_searching;
    let has_error = state.search_error().is_some();
    let sort_order = state.search_sort_order();

    let count_text = if is_searching {
        "Buscando...".to_string()
    } else if has_error {
        "Erro na busca".to_string()
    } else if total == 1 {
        "1 Resultado".to_string()
    } else {
        format!("{} Resultados", total)
    };

    let entity_recent = app_entity.clone();
    let entity_old = app_entity.clone();

    div()
        .h(px(48.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(border_subtle())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .text_color(text_titles())
                .font_weight(FontWeight::BOLD)
                .text_size(px(14.0))
                .child(count_text),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .bg(if sort_order == SearchSortOrder::Desc {
                            channel_hover()
                        } else {
                            gpui::rgba(0x00000000)
                        })
                        .text_color(if sort_order == SearchSortOrder::Desc {
                            text_titles()
                        } else {
                            text_muted()
                        })
                        .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            let maybe_req = entity_recent.update(cx, |state, cx| {
                                state.set_search_sort_order(SearchSortOrder::Desc);
                                let q = state.search_query.clone();
                                let req = state.begin_search(&q);
                                cx.notify();
                                req
                            });
                            if let Some(req) = maybe_req {
                                let app_weak = entity_recent.downgrade();
                                cx.spawn(|cx: &mut AsyncApp| {
                                    let cx = cx.clone();
                                    async move {
                                        req.run_loop(app_weak, cx).await;
                                    }
                                })
                                .detach();
                            }
                        })
                        .child("Recentes"),
                )
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .bg(if sort_order == SearchSortOrder::Asc {
                            channel_hover()
                        } else {
                            gpui::rgba(0x00000000)
                        })
                        .text_color(if sort_order == SearchSortOrder::Asc {
                            text_titles()
                        } else {
                            text_muted()
                        })
                        .hover(|s| s.bg(channel_hover()).text_color(text_titles()))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            let maybe_req = entity_old.update(cx, |state, cx| {
                                state.set_search_sort_order(SearchSortOrder::Asc);
                                let q = state.search_query.clone();
                                let req = state.begin_search(&q);
                                cx.notify();
                                req
                            });
                            if let Some(req) = maybe_req {
                                let app_weak = entity_old.downgrade();
                                cx.spawn(|cx: &mut AsyncApp| {
                                    let cx = cx.clone();
                                    async move {
                                        req.run_loop(app_weak, cx).await;
                                    }
                                })
                                .detach();
                            }
                        })
                        .child("Antigos"),
                ),
        )
        .child(render_search_flyout_close_button(app_entity))
        .into_any_element()
}

/// Renders the empty search guide state showing available filters
fn render_empty_guide_state(app_entity: Entity<AppState>) -> Vec<AnyElement> {
    let mut guide = vec![
        div()
            .text_color(text_muted())
            .font_weight(FontWeight::BOLD)
            .text_size(px(12.0))
            .child("EU ESTOU PROCURANDO POR...")
            .into_any_element(),
    ];

    let filter_guides = [
        ("from:", "procura mensagens de um usuário específico (ex: from:ferran)"),
        ("has:", "filtra por mídias: link, embed, file, video, image, sound, sticker"),
        ("in:", "filtra mensagens em um canal específico (ex: in:geral)"),
        ("before:", "mensagens enviadas antes de uma data (YYYY-MM-DD)"),
        ("during:", "mensagens enviadas em uma data específica (YYYY-MM-DD)"),
        ("after:", "mensagens enviadas após uma data (YYYY-MM-DD)"),
        ("mentions:", "filtra mensagens que mencionam um usuário"),
        ("pinned:", "mensagens fixadas: pinned:true ou pinned:false"),
    ];

    for (filter_key, desc) in filter_guides {
        let entity = app_entity.clone();
        guide.push(
            div()
                .p(px(8.0))
                .rounded(px(4.0))
                .bg(bg_user_bar())
                .hover(|s| s.bg(channel_hover()))
                .border_1()
                .border_color(border_subtle())
                .cursor_pointer()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.search_query = format!("{} ", filter_key);
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_color(blurple())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child(filter_key),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(11.0))
                        .child(desc),
                )
                .into_any_element(),
        );
    }
    guide
}

/// Renders searching state while local or remote search is executing
fn render_searching_state() -> Vec<AnyElement> {
    vec![
        div()
            .p(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_search(px(32.0), blurple())),
            )
            .child(
                div()
                    .text_color(text_titles())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(15.0))
                    .child("Buscando mensagens..."),
            )
            .child(
                div()
                    .text_color(text_muted())
                    .text_size(px(13.0))
                    .child("Consultando histórico de mensagens na API do Discord."),
            )
            .into_any_element(),
    ]
}

/// Renders indexing state (HTTP 202 Accepted, retry_after)
fn render_indexing_state(retry_after: f64) -> Vec<AnyElement> {
    vec![
        div()
            .p(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_search(px(32.0), blurple())),
            )
            .child(
                div()
                    .text_color(text_titles())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(15.0))
                    .child("Indexando mensagens..."),
            )
            .child(
                div()
                    .text_color(text_muted())
                    .text_size(px(13.0))
                    .child(format!(
                        "O Discord está preparando o índice de busca deste canal/servidor (tentando novamente em {:.1}s).",
                        retry_after
                    )),
            )
            .into_any_element(),
    ]
}

/// Renders error state when search fails or client is disconnected
fn render_search_error_state(error_message: &str) -> Vec<AnyElement> {
    vec![
        div()
            .p(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_info(px(32.0), status_dnd())),
            )
            .child(
                div()
                    .text_color(text_titles())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(15.0))
                    .child("Erro na busca"),
            )
            .child(
                div()
                    .text_color(text_muted())
                    .text_size(px(13.0))
                    .child(error_message.to_string()),
            )
            .into_any_element(),
    ]
}

/// Renders empty state when query returns 0 hits
fn render_no_results_state() -> Vec<AnyElement> {
    vec![
        div()
            .p(px(24.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .text_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_search(px(32.0), text_muted())),
            )
            .child(
                div()
                    .text_color(text_titles())
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(15.0))
                    .child("Nenhum resultado encontrado"),
            )
            .child(
                div()
                    .text_color(text_muted())
                    .text_size(px(13.0))
                    .child("Tente buscar com outros termos ou remover alguns filtros."),
            )
            .into_any_element(),
    ]
}

/// Renders a single search result hit message card
fn render_search_result_card(
    app_entity: Entity<AppState>,
    hit: &SearchResultGroup,
    parsed_query: &SearchQuery,
) -> AnyElement {
    let entity = app_entity.clone();
    let channel_id = hit.channel_id.clone();
    let message_id = hit.hit_message.id.clone();
    let channel_label = hit.channel_name.clone().unwrap_or_else(|| hit.channel_id.clone());
    let author_name = hit.hit_message.author.display_name().to_string();
    let timestamp_str = hit.hit_message.timestamp.format("%d/%m/%Y %H:%M").to_string();

    let jump_cid = channel_id.clone();
    let jump_mid = message_id.clone();
    let jump_entity = entity.clone();

    div()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(bg_user_bar())
        .border_1()
        .border_color(border_subtle())
        .hover(|s| s.border_color(channel_hover()).bg(bg_chat_hover()))
        .flex()
        .flex_col()
        .gap(px(8.0))
        // Top Row: Origin Channel + "Ir para mensagem" Button via crate::components::Button
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(icon_hash(px(14.0), blurple()))
                        .child(
                            div()
                                .text_color(channels_default())
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(12.0))
                                .child(channel_label),
                        ),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("jump-to-msg-{}", message_id)),
                        "Ir para mensagem",
                    )
                    .size(ButtonSize::Sm)
                    .variant(ButtonVariant::Gray)
                    .on_click(move |_ev, _window, cx| {
                        jump_entity.update(cx, |state, cx| {
                            state.jump_to_message(&jump_cid, &jump_mid);
                            cx.notify();
                        });
                    }),
                ),
        )
        // Author & Timestamp Row: using Avatar component!
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    Avatar::new(author_name.clone())
                        .size(AvatarSize::Px24)
                        .opt_src(hit.hit_message.author.avatar_url())
                        .opt_decoration(hit.hit_message.author.avatar_decoration_data.clone()),
                )
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(13.0))
                        .child(author_name),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(11.0))
                        .child(timestamp_str),
                ),
        )
        // Matched Hit Content (with Highlighted Terms in #fee75c / dark text)
        .child(render_highlighted_text(&hit.hit_message.content, parsed_query))
        .into_any_element()
}

/// Renders the scrollable results area or empty states
fn render_search_results_scroll_area(
    app_entity: Entity<AppState>,
    state: &AppState,
    parsed_query: &SearchQuery,
) -> AnyElement {
    let is_empty_query = state.search_query.trim().is_empty();
    let has_no_results = state.search_results.is_empty();

    let items: Vec<AnyElement> = if is_empty_query {
        render_empty_guide_state(app_entity)
    } else if state.is_searching {
        if state.is_search_indexing() {
            render_indexing_state(state.search_retry_after().unwrap_or(2.5))
        } else {
            render_searching_state()
        }
    } else if let Some(ref err) = state.search_error() {
        render_search_error_state(err)
    } else if has_no_results {
        render_no_results_state()
    } else {
        state
            .search_results
            .iter()
            .map(|hit| render_search_result_card(app_entity.clone(), hit, parsed_query))
            .collect()
    };

    div()
        .id("search-results-scroll")
        .flex_1()
        .overflow_y_scroll()
        .p(px(12.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .children(items)
        .into_any_element()
}

/// Renders bottom pagination dock matching Discord's .paginationDock_a98f3b
fn render_search_pagination_dock(
    app_entity: Entity<AppState>,
    state: &AppState,
) -> Option<AnyElement> {
    let total = state.search_total_results();
    if total <= 25 {
        return None;
    }

    let offset = state.search_offset();
    let current_page = (offset / 25) + 1;
    let total_pages = (total + 24) / 25;

    let can_prev = offset >= 25;
    let can_next = (offset as u64 + 25) < total;

    let prev_entity = app_entity.clone();
    let next_entity = app_entity.clone();

    Some(
        div()
            .h(px(48.0))
            .px(px(16.0))
            .border_t_1()
            .border_color(border_subtle())
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_color(text_muted())
                    .text_size(px(12.0))
                    .child(format!("Página {} de {}", current_page, total_pages)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        Button::new("search-prev-page", "Anterior")
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Gray)
                            .disabled(!can_prev)
                            .on_click(move |_ev, _window, cx| {
                                let maybe_req = prev_entity.update(cx, |state, cx| {
                                    let new_offset = state.search_offset().saturating_sub(25);
                                    state.set_search_offset(new_offset);
                                    let q = state.search_query.clone();
                                    let req = state.begin_search(&q);
                                    cx.notify();
                                    req
                                });
                                if let Some(req) = maybe_req {
                                    let app_weak = prev_entity.downgrade();
                                    cx.spawn(|cx: &mut AsyncApp| {
                                        let cx = cx.clone();
                                        async move {
                                            req.run_loop(app_weak, cx).await;
                                        }
                                    })
                                    .detach();
                                }
                            }),
                    )
                    .child(
                        Button::new("search-next-page", "Próxima")
                            .size(ButtonSize::Sm)
                            .variant(ButtonVariant::Gray)
                            .disabled(!can_next)
                            .on_click(move |_ev, _window, cx| {
                                let maybe_req = next_entity.update(cx, |state, cx| {
                                    let new_offset = state.search_offset() + 25;
                                    state.set_search_offset(new_offset);
                                    let q = state.search_query.clone();
                                    let req = state.begin_search(&q);
                                    cx.notify();
                                    req
                                });
                                if let Some(req) = maybe_req {
                                    let app_weak = next_entity.downgrade();
                                    cx.spawn(|cx: &mut AsyncApp| {
                                        let cx = cx.clone();
                                        async move {
                                            req.run_loop(app_weak, cx).await;
                                        }
                                    })
                                    .detach();
                                }
                            }),
                    ),
            )
            .into_any_element(),
    )
}

/// Renders the complete Discord Search Flyout Panel (418px width, Discord dark channels background)
pub fn render_search_flyout(
    app_entity: Entity<AppState>,
    state: &AppState,
    _focus_handle: &FocusHandle,
) -> Div {
    let parsed_query = SearchQuery::parse(&state.search_query);

    div()
        .w(px(418.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg_channels())
        .border_l_1()
        .border_color(bg_server())
        .flex_shrink_0()
        .child(render_search_flyout_header(
            app_entity.clone(),
            state,
        ))
        .child(render_search_results_scroll_area(
            app_entity.clone(),
            state,
            &parsed_query,
        ))
        .children(render_search_pagination_dock(
            app_entity,
            state,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Author;

    #[::core::prelude::v1::test]
    fn test_search_query_parsing() {
        let q = SearchQuery::parse("from:ferran has:link in:geral pinned:true release notes");
        assert_eq!(q.from.as_deref(), Some("ferran"));
        assert_eq!(q.in_channel.as_deref(), Some("geral"));
        assert_eq!(q.pinned, Some(true));
        assert_eq!(q.has_filters, vec![HasFilter::Link]);
        assert_eq!(q.text_terms, vec!["release", "notes"]);
    }

    #[::core::prelude::v1::test]
    fn test_search_query_matching() {
        let q = SearchQuery::parse("from:ana has:embed performance");
        let author_ana = Author::new("u2", "ana");
        let mut msg = Message::new("m1", "c1", author_ana, "Discutindo performance do GPUI");
        msg.embeds.push(crate::domain::Embed {
            title: Some("Bench".to_string()),
            ..Default::default()
        });

        assert!(q.matches(&msg, Some("geral")));

        let author_bob = Author::new("u3", "bob");
        let msg2 = Message::new("m2", "c1", author_bob, "Discutindo performance do GPUI");
        assert!(!q.matches(&msg2, Some("geral")));
    }

    #[::core::prelude::v1::test]
    fn test_local_search_engine_with_context() {
        let author = Author::new("u1", "carlos");
        let m1 = Message::new("m1", "c1", author.clone(), "Primeira mensagem de contexto");
        let m2 = Message::new("m2", "c1", author.clone(), "Alvo encontrado com chave secreta");
        let m3 = Message::new("m3", "c1", author.clone(), "Terceira mensagem de contexto");

        let messages = vec![m1.clone(), m2.clone(), m3.clone()];
        let query = SearchQuery::parse("chave secreta");

        let results = LocalSearchEngine::search_channel_messages(
            &messages,
            &query,
            "c1",
            Some("geral"),
            1,
        );

        assert_eq!(results.len(), 1);
        let hit = &results[0];
        assert_eq!(hit.hit_message.id, "m2");
        assert_eq!(hit.context_before.len(), 1);
        assert_eq!(hit.context_before[0].id, "m1");
        assert_eq!(hit.context_after.len(), 1);
        assert_eq!(hit.context_after[0].id, "m3");
    }

    #[::core::prelude::v1::test]
    fn test_has_filter_parsing_all() {
        assert_eq!(HasFilter::parse("link"), Some(HasFilter::Link));
        assert_eq!(HasFilter::parse("url"), Some(HasFilter::Link));
        assert_eq!(HasFilter::parse("embed"), Some(HasFilter::Embed));
        assert_eq!(HasFilter::parse("file"), Some(HasFilter::File));
        assert_eq!(HasFilter::parse("video"), Some(HasFilter::Video));
        assert_eq!(HasFilter::parse("image"), Some(HasFilter::Image));
        assert_eq!(HasFilter::parse("photo"), Some(HasFilter::Image));
        assert_eq!(HasFilter::parse("sound"), Some(HasFilter::Sound));
        assert_eq!(HasFilter::parse("audio"), Some(HasFilter::Sound));
        assert_eq!(HasFilter::parse("unknown"), None);
    }

    #[::core::prelude::v1::test]
    fn test_has_filter_matching_media() {
        let author = Author::new("u1", "user1");
        let mut msg = Message::new("m1", "c1", author, "Hello");

        assert!(!HasFilter::File.matches(&msg));
        assert!(!HasFilter::Link.matches(&msg));

        // Link
        msg.content = "Confira https://discord.com para mais".to_string();
        assert!(HasFilter::Link.matches(&msg));

        // Attachment
        let att = crate::domain::message::Attachment {
            id: "a1".to_string(),
            filename: "audio.mp3".to_string(),
            size: 1024,
            url: "http://a.com/audio.mp3".to_string(),
            proxy_url: None,
            content_type: Some("audio/mpeg".to_string()),
            width: None,
            height: None,
        };
        msg.attachments.push(att);
        assert!(HasFilter::File.matches(&msg));
        assert!(HasFilter::Sound.matches(&msg));
    }

    #[::core::prelude::v1::test]
    fn test_date_and_pinned_filters() {
        let q = SearchQuery::parse("before:2026-02-01 after:2026-01-01 pinned:true");
        assert_eq!(
            q.before,
            Some(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap())
        );
        assert_eq!(
            q.after,
            Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
        );
        assert_eq!(q.pinned, Some(true));

        let author = Author::new("u1", "user1");
        let mut msg = Message::new("m1", "c1", author, "pinned note");
        msg.pinned = true;
        // timestamp is set to now by Message::new, let's test matching
        let query_pinned = SearchQuery::parse("pinned:true");
        assert!(query_pinned.matches(&msg, None));

        let query_not_pinned = SearchQuery::parse("pinned:false");
        assert!(!query_not_pinned.matches(&msg, None));
    }

    #[::core::prelude::v1::test]
    fn test_empty_query_behavior() {
        let q = SearchQuery::parse("");
        assert!(q.is_empty());

        let author = Author::new("u1", "user1");
        let msg = Message::new("m1", "c1", author, "content");
        let results = LocalSearchEngine::search_channel_messages(
            &[msg],
            &q,
            "c1",
            None,
            2,
        );
        assert!(results.is_empty());
    }

    #[::core::prelude::v1::test]
    fn test_mentions_filter() {
        let q = SearchQuery::parse("mentions:ferran");
        assert_eq!(q.mentions.as_deref(), Some("ferran"));

        let author = Author::new("u1", "user1");
        let msg_match = Message::new("m1", "c1", author.clone(), "Ei @ferran você viu isso?");
        assert!(q.matches(&msg_match, None));

        let msg_no_match = Message::new("m2", "c1", author, "Mensagem sem menção");
        assert!(!q.matches(&msg_no_match, None));
    }

    #[::core::prelude::v1::test]
    fn test_space_separated_filter_syntax() {
        let q = SearchQuery::parse("from: alice in: geral has: sticker during: 2026-03-15 pinned: true");
        assert_eq!(q.from.as_deref(), Some("alice"));
        assert_eq!(q.in_channel.as_deref(), Some("geral"));
        assert_eq!(q.has_filters, vec![HasFilter::Sticker]);
        assert_eq!(q.during, Some(NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()));
        assert_eq!(q.pinned, Some(true));
    }

    #[::core::prelude::v1::test]
    fn test_quoted_filter_and_terms() {
        let q = SearchQuery::parse("from: \"bob marley\" \"crash on start\" beta");
        assert_eq!(q.from.as_deref(), Some("bob marley"));
        assert_eq!(q.text_terms, vec!["crash on start", "beta"]);
    }

    #[::core::prelude::v1::test]
    fn test_sticker_filter_matching() {
        let author = Author::new("u1", "user1");
        let mut msg = Message::new("m1", "c1", author, "mensagem com sticker");
        assert!(!HasFilter::Sticker.matches(&msg));

        msg.sticker_items.push(crate::domain::message::MessageStickerItem::new("s1", "wave", 1));
        assert!(HasFilter::Sticker.matches(&msg));

        let q = SearchQuery::parse("wave");
        assert!(q.matches(&msg, None));
    }

    #[::core::prelude::v1::test]
    fn test_embed_content_matching() {
        let author = Author::new("u1", "user1");
        let mut msg = Message::new("m1", "c1", author, "sem nada no corpo");
        let q = SearchQuery::parse("anúncio");
        assert!(!q.matches(&msg, None));

        msg.embeds.push(crate::domain::Embed {
            title: Some("Grande Anúncio da Comunidade".to_string()),
            description: Some("Novo servidor disponível".to_string()),
            ..Default::default()
        });
        assert!(q.matches(&msg, None));
    }

    #[::core::prelude::v1::test]
    fn test_search_payload_into_result_groups() {
        let author = Author::new("u1", "user1");
        let m1 = Message::new("m1", "c1", author.clone(), "contexto antes");
        let m2 = Message::new("m2", "c1", author.clone(), "hit alvo");
        let m3 = Message::new("m3", "c1", author.clone(), "contexto depois");

        let payload = SearchPayload {
            total_results: 1,
            messages: vec![vec![
                ApiSearchMessage { message: m1.clone(), hit: false },
                ApiSearchMessage { message: m2.clone(), hit: true },
                ApiSearchMessage { message: m3.clone(), hit: false },
            ]],
            analytics_id: Some("tr-123".to_string()),
        };

        let groups = payload.into_result_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].hit_message.id, "m2");
        assert_eq!(groups[0].context_before.len(), 1);
        assert_eq!(groups[0].context_before[0].id, "m1");
        assert_eq!(groups[0].context_after.len(), 1);
        assert_eq!(groups[0].context_after[0].id, "m3");
    }

    #[::core::prelude::v1::test]
    fn test_to_api_query_params() {
        let q = SearchQuery::parse("from:123 has:link in:456 pinned:true bug");
        let params = q.to_api_query_params();
        assert!(params.contains(&("author_id", "123".to_string())));
        assert!(params.contains(&("channel_id", "456".to_string())));
        assert!(params.contains(&("has", "link".to_string())));
        assert!(params.contains(&("pinned", "true".to_string())));
        assert!(params.contains(&("content", "bug".to_string())));
    }
}
