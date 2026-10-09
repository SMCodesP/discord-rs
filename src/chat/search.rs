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
use crate::domain::Message;
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

impl SearchQuery {
    /// Parses a raw search string following Discord's query syntax.
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        let mut query = Self {
            raw: trimmed.to_string(),
            ..Default::default()
        };

        if trimmed.is_empty() {
            return query;
        }

        for token in trimmed.split_whitespace() {
            if let Some(val) = token.strip_prefix("from:") {
                let clean_val = val.trim_start_matches('@').to_lowercase();
                if !clean_val.is_empty() {
                    query.from = Some(clean_val);
                }
            } else if let Some(val) = token.strip_prefix("mentions:") {
                let clean_val = val.trim_start_matches('@').to_lowercase();
                if !clean_val.is_empty() {
                    query.mentions = Some(clean_val);
                }
            } else if let Some(val) = token.strip_prefix("has:") {
                if let Some(has) = HasFilter::parse(val) {
                    query.has_filters.push(has);
                }
            } else if let Some(val) = token.strip_prefix("in:") {
                let clean_val = val.trim_start_matches('#').to_lowercase();
                if !clean_val.is_empty() {
                    query.in_channel = Some(clean_val);
                }
            } else if let Some(val) = token.strip_prefix("before:") {
                if let Ok(d) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
                    query.before = Some(d);
                }
            } else if let Some(val) = token.strip_prefix("after:") {
                if let Ok(d) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
                    query.after = Some(d);
                }
            } else if let Some(val) = token.strip_prefix("during:") {
                if let Ok(d) = NaiveDate::parse_from_str(val, "%Y-%m-%d") {
                    query.during = Some(d);
                }
            } else if let Some(val) = token.strip_prefix("pinned:") {
                match val.to_lowercase().as_str() {
                    "true" | "yes" | "1" => query.pinned = Some(true),
                    "false" | "no" | "0" => query.pinned = Some(false),
                    _ => {}
                }
            } else {
                query.text_terms.push(token.to_lowercase());
            }
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

    /// Checks if a message matches all query criteria.
    pub fn matches(&self, message: &Message, channel_name: Option<&str>) -> bool {
        // 1. Text terms matching (all free text terms must be found in content or attachment filenames)
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

            if !in_content && !in_attachment && !in_author {
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
            // Discord mentions format: <@123456> or literal @username in text
            let mention_id_tag = format!("<@{}>", mention);
            let mention_name_tag = format!("@{}", mention);
            if !content_lower.contains(&mention_id_tag)
                && !content_lower.contains(&mention_name_tag)
                && !content_lower.contains(mention)
            {
                return false;
            }
        }

        // 4. Has filters (`has:link`, `has:file`, etc.)
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

/// Individual message element in API search result groups with the `hit` flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiSearchMessage {
    #[serde(flatten)]
    pub message: Message,
    /// True if this message is the search hit, false if it's surrounding context
    #[serde(default)]
    pub hit: bool,
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
            .text_color(rgb(0xdbdee1))
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
            .text_color(rgb(0xdbdee1))
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
                        .text_color(rgb(0xdbdee1))
                        .child(sub.to_string()),
                );
            }
        }
        if let Some(sub) = content.get(s..e) {
            container = container.child(
                div()
                    .bg(rgb(0xfee75c))
                    .text_color(rgb(0x1e1f22))
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
                    .text_color(rgb(0xdbdee1))
                    .child(sub.to_string()),
            );
        }
    }

    container
}

/// Renders the complete Discord Search Flyout Panel (418px width, #2b2d31 background, #1e1f22 border)
pub fn render_search_flyout(
    app_entity: Entity<AppState>,
    state: &AppState,
    focus_handle: &FocusHandle,
) -> Div {
    let parsed_query = SearchQuery::parse(&state.search_query);

    div()
        .w(px(418.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(0x2b2d31))
        .border_l_1()
        .border_color(rgb(0x1e1f22))
        .flex_shrink_0()
        // =====================================================================
        // 1. TOP HEADER: Search Box + Close Button
        // =====================================================================
        .child(
            div()
                .h(px(48.0))
                .px(px(12.0))
                .border_b_1()
                .border_color(rgb(0x1e1f22))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                // Search Input Box Pill
                .child(
                    div()
                        .flex_1()
                        .h(px(32.0))
                        .px(px(8.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x1e1f22))
                        .border_1()
                        .border_color(rgb(0x35373c))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap(px(6.0))
                        .track_focus(focus_handle)
                        .cursor_text()
                        .on_mouse_down(MouseButton::Left, {
                            let fh = focus_handle.clone();
                            move |_ev, window, _cx| {
                                window.focus(&fh);
                            }
                        })
                        .on_key_down({
                            let entity = app_entity.clone();
                            let fh = focus_handle.clone();
                            move |ev: &KeyDownEvent, window, cx| {
                                window.focus(&fh);
                                let key = &ev.keystroke.key;
                                let is_ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;
                                if is_ctrl {
                                    return;
                                }
                                entity.update(cx, |state, cx| {
                                    match key.as_str() {
                                        "backspace" => {
                                            state.search_query.pop();
                                            let q = state.search_query.clone();
                                            state.execute_search(&q);
                                            cx.notify();
                                        }
                                        "escape" => {
                                            state.close_search();
                                            cx.notify();
                                        }
                                        "enter" => {
                                            let q = state.search_query.clone();
                                            state.execute_search(&q);
                                            cx.notify();
                                        }
                                        k if k.chars().count() == 1 => {
                                            state.search_query.push_str(k);
                                            let q = state.search_query.clone();
                                            state.execute_search(&q);
                                            cx.notify();
                                        }
                                        _ => {}
                                    }
                                });
                            }
                        })
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .overflow_hidden()
                                .child(icon_search(px(14.0), rgb(0x80848e)))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(if !state.search_query.is_empty() {
                                            rgb(0xf2f3f5)
                                        } else {
                                            rgb(0x80848e)
                                        })
                                        .child(if !state.search_query.is_empty() {
                                            state.search_query.clone()
                                        } else {
                                            "Buscar".to_string()
                                        }),
                                ),
                        )
                        .children(if !state.search_query.is_empty() {
                            let entity = app_entity.clone();
                            Some(
                                div()
                                    .size(px(16.0))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(0x80848e))
                                    .hover(|s| s.text_color(rgb(0xffffff)))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.clear_search();
                                            cx.notify();
                                        });
                                    })
                                    .child(icon_close(px(12.0), rgb(0x80848e))),
                            )
                        } else {
                            None
                        }),
                )
                // Close Flyout Button (✕)
                .child(
                    div()
                        .size(px(28.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(rgb(0x80848e))
                        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff)))
                        .on_mouse_down(MouseButton::Left, {
                            let entity = app_entity.clone();
                            move |_ev, _window, cx| {
                                entity.update(cx, |state, cx| {
                                    state.close_search();
                                    cx.notify();
                                });
                            }
                        })
                        .child(icon_close(px(14.0), rgb(0x80848e))),
                ),
        )
        // =====================================================================
        // 2. CHIPS SECTION: Results count + Interactive filter chips
        // =====================================================================
        .child(
            div()
                .px(px(12.0))
                .py(px(8.0))
                .border_b_1()
                .border_color(rgb(0x1e1f22))
                .flex()
                .flex_col()
                .gap(px(8.0))
                // Results Counter
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_color(rgb(0x80848e))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(11.0))
                                .child(if state.search_query.trim().is_empty() {
                                    "OPÇÕES DE PESQUISA".to_string()
                                } else {
                                    format!("{} RESULTADOS ENCONTRADOS", state.search_results.len())
                                }),
                        ),
                )
                // Interactive Filter Chips Row
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(6.0))
                        .children(DISCORD_SEARCH_CHIPS.iter().map(|&(prefix, hint)| {
                            let entity = app_entity.clone();
                            div()
                                .h(px(24.0))
                                .px(px(8.0))
                                .rounded(px(4.0))
                                .bg(rgb(0x1e1f22))
                                .border_1()
                                .border_color(rgb(0x35373c))
                                .hover(|s| s.bg(rgb(0x35373c)).border_color(rgb(0x5865f2)))
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.0))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |state, cx| {
                                        let current = state.search_query.trim();
                                        if current.is_empty() {
                                            state.search_query = format!("{} ", prefix);
                                        } else if !current.ends_with(prefix) {
                                            state.search_query = format!("{} {} ", current, prefix);
                                        }
                                        let q = state.search_query.clone();
                                        state.execute_search(&q);
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_color(rgb(0x5865f2))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(11.0))
                                        .child(prefix),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x949ba4))
                                        .text_size(px(10.0))
                                        .child(hint),
                                )
                        })),
                ),
        )
        // =====================================================================
        // 3. RESULTS SCROLL AREA / EMPTY GUIDE STATE
        // =====================================================================
        .child(
            div()
                .id("search-results-scroll")
                .flex_1()
                .overflow_y_scroll()
                .p(px(12.0))
                .flex()
                .flex_col()
                .gap(px(10.0))
                .children(if state.search_query.trim().is_empty() {
                    // Empty Guide State: Show available filter descriptions
                    let mut guide = vec![
                        div()
                            .text_color(rgb(0x949ba4))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(12.0))
                            .child("EU ESTOU PROCURANDO POR..."),
                    ];

                    let filter_guides = [
                        ("from:", "procura mensagens de um usuário específico (ex: from:ferran)"),
                        ("has:", "filtra por mídias: link, embed, file, video, image, sound"),
                        ("in:", "filtra mensagens em um canal específico (ex: in:geral)"),
                        ("before:", "mensagens enviadas antes de uma data (YYYY-MM-DD)"),
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
                                .bg(rgb(0x232428))
                                .hover(|s| s.bg(rgb(0x35373c)))
                                .border_1()
                                .border_color(rgb(0x1f2023))
                                .cursor_pointer()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |state, cx| {
                                        state.search_query = format!("{} ", filter_key);
                                        let q = state.search_query.clone();
                                        state.execute_search(&q);
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .text_color(rgb(0x5865f2))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(12.0))
                                        .child(filter_key),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x949ba4))
                                        .text_size(px(11.0))
                                        .child(desc),
                                ),
                        );
                    }
                    guide
                } else if state.search_results.is_empty() {
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
                                    .child(icon_search(px(32.0), rgb(0x80848e))),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xf2f3f5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(15.0))
                                    .child("Nenhum resultado encontrado"),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .text_size(px(13.0))
                                    .child("Tente buscar com outros termos ou remover alguns filtros."),
                            ),
                    ]
                } else {
                    // Result Cards
                    state.search_results.iter().map(|hit| {
                        let entity = app_entity.clone();
                        let channel_id = hit.channel_id.clone();
                        let message_id = hit.hit_message.id.clone();
                        let channel_label = hit.channel_name.clone().unwrap_or_else(|| hit.channel_id.clone());
                        let author_name = hit.hit_message.author.display_name().to_string();
                        let author_char = author_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                        let timestamp_str = hit.hit_message.timestamp.format("%d/%m/%Y %H:%M").to_string();

                        div()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x232428))
                            .border_1()
                            .border_color(rgb(0x1f2023))
                            .hover(|s| s.border_color(rgb(0x35373c)).bg(rgb(0x282a2e)))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            // Top Row: Origin Channel + "Ir para mensagem" Button
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
                                            .child(icon_hash(px(14.0), rgb(0x5865f2)))
                                            .child(
                                                div()
                                                    .text_color(rgb(0x949ba4))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(12.0))
                                                    .child(channel_label),
                                            ),
                                    )
                                    // Button: 'Ir para mensagem'
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(4.0))
                                            .rounded(px(4.0))
                                            .bg(rgb(0x35373c))
                                            .hover(|s| s.bg(rgb(0x5865f2)).text_color(rgb(0xffffff)))
                                            .text_color(rgb(0xdbdee1))
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::BOLD)
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, {
                                                let entity = entity.clone();
                                                let cid = channel_id.clone();
                                                let mid = message_id.clone();
                                                move |_ev, _window, cx| {
                                                    entity.update(cx, |state, cx| {
                                                        state.jump_to_message(&cid, &mid);
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .child("Ir para mensagem"),
                                    ),
                            )
                            // Author & Timestamp Row
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .size(px(24.0))
                                            .rounded_full()
                                            .bg(rgb(0x5865f2))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(rgb(0xffffff))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child(author_char),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0xf2f3f5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(13.0))
                                            .child(author_name),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .text_size(px(11.0))
                                            .child(timestamp_str),
                                    ),
                            )
                            // Matched Hit Content (with Highlighted Terms in #fee75c / dark text)
                            .child(
                                render_highlighted_text(&hit.hit_message.content, &parsed_query),
                            )
                    }).collect()
                }),
        )
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
}
