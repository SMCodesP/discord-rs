//! Discord RS - Polls (T-102)
//! Native poll model, parsing, percentage calculation, and GPUI card rendering.
//! Data comes only from Discord API/Gateway payloads; no mock/fabricated values.

use chrono::{DateTime, Utc};
use gpui::*;
use serde::{Deserialize, Serialize};

use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Discord poll layout type. Currently only default (1) is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PollLayoutType {
    #[default]
    Default,
    Unknown(u8),
}

impl From<u8> for PollLayoutType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Default,
            other => Self::Unknown(other),
        }
    }
}

impl From<PollLayoutType> for u8 {
    fn from(value: PollLayoutType) -> Self {
        match value {
            PollLayoutType::Default => 1,
            PollLayoutType::Unknown(v) => v,
        }
    }
}

impl Serialize for PollLayoutType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        u8::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PollLayoutType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = u8::deserialize(deserializer)?;
        Ok(PollLayoutType::from(v))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PollMedia {
    pub text: Option<String>,
    #[serde(default)]
    pub emoji: Option<PollEmoji>,
}

impl PollMedia {
    pub fn text_or_empty(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PollEmoji {
    pub id: Option<String>,
    pub name: Option<String>,
}

impl PollEmoji {
    pub fn display(&self) -> String {
        if let Some(name) = &self.name {
            if !name.is_empty() {
                return name.clone();
            }
        }
        if let Some(id) = &self.id {
            return format!("emoji:{id}");
        }
        String::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollAnswer {
    pub answer_id: i32,
    #[serde(default)]
    pub poll_media: PollMedia,
}

impl PollAnswer {
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(emoji) = &self.poll_media.emoji {
            let e = emoji.display();
            if !e.is_empty() {
                parts.push(e);
            }
        }
        let text = self.poll_media.text_or_empty().trim();
        if !text.is_empty() {
            parts.push(text.to_string());
        }
        if parts.is_empty() {
            format!("Answer {}", self.answer_id)
        } else {
            parts.join(" ")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PollAnswerCount {
    pub id: i32,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub me_voted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PollResults {
    #[serde(default)]
    pub is_finalized: bool,
    #[serde(default)]
    pub answer_counts: Vec<PollAnswerCount>,
}

impl PollResults {
    pub fn count_for(&self, answer_id: i32) -> Option<u32> {
        self.answer_counts
            .iter()
            .find(|c| c.id == answer_id)
            .map(|c| c.count)
    }

    pub fn me_voted_for(&self, answer_id: i32) -> bool {
        self.answer_counts
            .iter()
            .find(|c| c.id == answer_id)
            .is_some_and(|c| c.me_voted)
    }

    pub fn total_votes(&self) -> u32 {
        self.answer_counts.iter().map(|c| c.count).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Poll {
    pub question: PollMedia,
    #[serde(default)]
    pub answers: Vec<PollAnswer>,
    #[serde(default)]
    pub expiry: Option<DateTime<Utc>>,
    #[serde(default)]
    pub allow_multiselect: bool,
    #[serde(default)]
    pub layout_type: PollLayoutType,
    #[serde(default)]
    pub results: Option<PollResults>,
}

impl Poll {
    pub fn question_text(&self) -> &str {
        self.question.text_or_empty()
    }

    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expiry.is_some_and(|e| now >= e)
    }

    pub fn is_finalized(&self) -> bool {
        self.results.as_ref().is_some_and(|r| r.is_finalized)
    }

    pub fn total_votes(&self) -> Option<u32> {
        self.results.as_ref().map(|r| r.total_votes())
    }

    /// Returns vote percentage for an answer, or None when results are unavailable.
    /// Uses integer-rounded percentage from real counts; no fabricated values.
    pub fn answer_percentage(&self, answer_id: i32) -> Option<u32> {
        let results = self.results.as_ref()?;
        let total = results.total_votes();
        let count = results.count_for(answer_id)?;
        Some(vote_percentage(count, total))
    }

    pub fn answer_count(&self, answer_id: i32) -> Option<u32> {
        self.results.as_ref()?.count_for(answer_id)
    }

    pub fn me_voted_for(&self, answer_id: i32) -> bool {
        self.results
            .as_ref()
            .is_some_and(|r| r.me_voted_for(answer_id))
    }
}

/// Calculate percentage from real counts. Returns 0 when total is 0.
pub fn vote_percentage(count: u32, total: u32) -> u32 {
    if total == 0 {
        return 0;
    }
    ((count as u64 * 100) / total as u64) as u32
}

/// Parse a Discord poll object from message JSON.
/// Returns None when payload is absent or missing required fields.
pub fn parse_poll_from_json(val: &serde_json::Value) -> Option<Poll> {
    let question_val = val.get("question")?;
    let question = parse_poll_media(question_val);

    let answers_val = val.get("answers")?.as_array()?;
    let mut answers = Vec::new();
    for a in answers_val {
        let answer_id = a.get("answer_id").and_then(|v| v.as_i64())? as i32;
        let poll_media = a
            .get("poll_media")
            .map(parse_poll_media)
            .unwrap_or_default();
        answers.push(PollAnswer {
            answer_id,
            poll_media,
        });
    }

    let expiry = val
        .get("expiry")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let allow_multiselect = val
        .get("allow_multiselect")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let layout_type = val
        .get("layout_type")
        .and_then(|v| v.as_u64())
        .map(|v| PollLayoutType::from(v as u8))
        .unwrap_or_default();
    let results = val.get("results").and_then(parse_poll_results);

    Some(Poll {
        question,
        answers,
        expiry,
        allow_multiselect,
        layout_type,
        results,
    })
}

fn parse_poll_media(val: &serde_json::Value) -> PollMedia {
    let text = val
        .get("text")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let emoji = val.get("emoji").and_then(|e| {
        if e.is_null() {
            return None;
        }
        let id = e.get("id").and_then(|v| v.as_str()).map(ToString::to_string);
        let name = e
            .get("name")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        if id.is_none() && name.is_none() {
            None
        } else {
            Some(PollEmoji { id, name })
        }
    });
    PollMedia { text, emoji }
}

fn parse_poll_results(val: &serde_json::Value) -> Option<PollResults> {
    let is_finalized = val
        .get("is_finalized")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut answer_counts = Vec::new();
    if let Some(arr) = val.get("answer_counts").and_then(|v| v.as_array()) {
        for c in arr {
            let Some(id) = c.get("id").and_then(|v| v.as_i64()) else {
                continue;
            };
            let count = c.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let me_voted = c.get("me_voted").and_then(|v| v.as_bool()).unwrap_or(false);
            answer_counts.push(PollAnswerCount {
                id: id as i32,
                count,
                me_voted,
            });
        }
    }
    Some(PollResults {
        is_finalized,
        answer_counts,
    })
}

/// Renders a poll card for the message list. Uses only data present in `poll`.
pub fn render_poll_card(poll: &Poll, now: DateTime<Utc>) -> Div {
    let total_votes = poll.total_votes();
    let is_finalized = poll.is_finalized();
    let is_expired = poll.is_expired_at(now);
    let has_results = poll.results.is_some();

    let mut card = div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(520.0))
        .bg(bg_channels())
        .border_1()
        .border_color(borders())
        .rounded(radius_md())
        .p(space_4())
        .gap(space_2())
        .mt(space_2());

    let question = poll.question_text().trim();
    if !question.is_empty() {
        card = card.child(
            div()
                .text_size(font_size_base())
                .font_weight(WEIGHT_BOLD)
                .text_color(text_titles())
                .child(question.to_string()),
        );
    }

    let mut answers_col = div().flex().flex_col().gap(space_2()).w_full();
    for answer in &poll.answers {
        answers_col = answers_col.child(render_poll_answer(poll, answer, has_results));
    }
    card = card.child(answers_col);

    let mut meta_parts: Vec<String> = Vec::new();
    if let Some(total) = total_votes {
        meta_parts.push(format!("{total} vote{}", if total == 1 { "" } else { "s" }));
    }
    if poll.allow_multiselect {
        meta_parts.push("multiple answers".to_string());
    }
    if is_finalized || is_expired {
        meta_parts.push("poll closed".to_string());
    }
    if !meta_parts.is_empty() {
        card = card.child(
            div()
                .text_size(font_size_xs())
                .text_color(text_muted())
                .child(meta_parts.join(" · ")),
        );
    }

    card
}

fn render_poll_answer(poll: &Poll, answer: &PollAnswer, has_results: bool) -> Div {
    let label = answer.label();
    let me_voted = poll.me_voted_for(answer.answer_id);
    let pct = poll.answer_percentage(answer.answer_id).unwrap_or(0);
    let count = poll.answer_count(answer.answer_id);

    let border = if me_voted { blurple() } else { borders() };

    let mut row = div()
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .border_1()
        .border_color(border)
        .rounded(radius_md())
        .overflow_hidden()
        .bg(bg_chat());

    // Progress fill only when results exist; width derived from real percentage.
    if has_results {
        let fill_color = if me_voted {
            with_alpha(HEX_BLURPLE, 0.30)
        } else {
            with_alpha(HEX_WHITE, 0.08)
        };
        row = row.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(relative(pct as f32 / 100.0))
                .bg(fill_color),
        );
    }

    let mut top = div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(space_2())
        .px(space_4())
        .py(space_2());

    top = top.child(
        div()
            .flex_1()
            .text_size(font_size_sm())
            .text_color(if me_voted { text_titles() } else { text_primary() })
            .font_weight(if me_voted { WEIGHT_MEDIUM } else { WEIGHT_NORMAL })
            .child(label),
    );

    if has_results {
        let mut right = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_2())
            .text_size(font_size_xs())
            .text_color(text_muted());
        if let Some(c) = count {
            right = right.child(format!("{c}"));
        }
        right = right.child(format!("{pct}%"));
        top = top.child(right);
    }

    row.child(top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn vote_percentage_handles_zero_and_rounding() {
        assert_eq!(vote_percentage(0, 0), 0);
        assert_eq!(vote_percentage(1, 3), 33);
        assert_eq!(vote_percentage(2, 3), 66);
        assert_eq!(vote_percentage(3, 3), 100);
    }

    #[::core::prelude::v1::test]
    fn parse_poll_from_json_reads_official_shape() {
        let value = json!({
            "question": {"text": "Best lang?"},
            "answers": [
                {"answer_id": 1, "poll_media": {"text": "Rust", "emoji": {"name": "🦀"}}},
                {"answer_id": 2, "poll_media": {"text": "Go"}}
            ],
            "expiry": "2026-11-25T00:00:00+00:00",
            "allow_multiselect": true,
            "layout_type": 1,
            "results": {
                "is_finalized": false,
                "answer_counts": [
                    {"id": 1, "count": 3, "me_voted": true},
                    {"id": 2, "count": 1, "me_voted": false}
                ]
            }
        });
        let poll = parse_poll_from_json(&value).expect("poll");
        assert_eq!(poll.question_text(), "Best lang?");
        assert!(poll.allow_multiselect);
        assert_eq!(poll.answers.len(), 2);
        assert_eq!(poll.answer_percentage(1), Some(75));
        assert_eq!(poll.answer_percentage(2), Some(25));
        assert!(poll.me_voted_for(1));
        assert!(!poll.me_voted_for(2));
        assert_eq!(poll.total_votes(), Some(4));
    }

    #[::core::prelude::v1::test]
    fn parse_poll_without_results_has_no_fabricated_percentages() {
        let value = json!({
            "question": {"text": "Pick one"},
            "answers": [{"answer_id": 1, "poll_media": {"text": "A"}}],
            "allow_multiselect": false,
            "layout_type": 1
        });
        let poll = parse_poll_from_json(&value).expect("poll");
        assert_eq!(poll.total_votes(), None);
        assert_eq!(poll.answer_percentage(1), None);
        assert!(!poll.is_finalized());
    }
}
