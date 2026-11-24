//! T-102 - Polls: model/parser/percentages/multiselect and GPUI card smoke tests.
//! Fixtures use synthetic Discord-shaped payloads; no mock provider data.

use chrono::{TimeZone, Utc};
use discord_rs::chat::polls::{parse_poll_from_json, render_poll_card, vote_percentage, PollLayoutType};
use serde_json::json;

#[test]
fn poll_parser_reads_creation_like_official_payload() {
    let payload = json!({
        "question": {"text": "Qual feature priorizar?"},
        "answers": [
            {"answer_id": 1, "poll_media": {"text": "Polls", "emoji": {"id": null, "name": "📊"}}},
            {"answer_id": 2, "poll_media": {"text": "Forward", "emoji": {"id": "123", "name": "custom"}}},
            {"answer_id": 3, "poll_media": {"text": "Stage"}}
        ],
        "duration": 24,
        "allow_multiselect": false,
        "layout_type": 1
    });
    // Discord message.poll on creation may omit results/expiry in some fixtures; parser must not invent them.
    let poll = parse_poll_from_json(&payload).expect("poll should parse");
    assert_eq!(poll.question_text(), "Qual feature priorizar?");
    assert_eq!(poll.answers.len(), 3);
    assert!(!poll.allow_multiselect);
    assert_eq!(poll.layout_type, PollLayoutType::Default);
    assert_eq!(poll.total_votes(), None);
    assert_eq!(poll.answer_percentage(1), None);
    assert_eq!(poll.answer_percentage(2), None);
    assert_eq!(poll.answer_count(3), None);
}

#[test]
fn poll_percentages_use_real_counts_only() {
    assert_eq!(vote_percentage(0, 0), 0);
    assert_eq!(vote_percentage(1, 2), 50);
    assert_eq!(vote_percentage(1, 3), 33);
    assert_eq!(vote_percentage(2, 3), 66);

    let payload = json!({
        "question": {"text": "Vote"},
        "answers": [
            {"answer_id": 10, "poll_media": {"text": "A"}},
            {"answer_id": 20, "poll_media": {"text": "B"}},
            {"answer_id": 30, "poll_media": {"text": "C"}}
        ],
        "allow_multiselect": true,
        "layout_type": 1,
        "results": {
            "is_finalized": false,
            "answer_counts": [
                {"id": 10, "count": 5, "me_voted": true},
                {"id": 20, "count": 5, "me_voted": true},
                {"id": 30, "count": 0, "me_voted": false}
            ]
        }
    });
    let poll = parse_poll_from_json(&payload).expect("poll");
    assert!(poll.allow_multiselect);
    assert_eq!(poll.total_votes(), Some(10));
    assert_eq!(poll.answer_percentage(10), Some(50));
    assert_eq!(poll.answer_percentage(20), Some(50));
    assert_eq!(poll.answer_percentage(30), Some(0));
    assert!(poll.me_voted_for(10));
    assert!(poll.me_voted_for(20));
    assert!(!poll.me_voted_for(30));
}

#[test]
fn poll_tie_expiry_and_finalized_states() {
    let expiry = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let payload = json!({
        "question": {"text": "Empate?"},
        "answers": [
            {"answer_id": 1, "poll_media": {"text": "Sim"}},
            {"answer_id": 2, "poll_media": {"text": "Não"}}
        ],
        "expiry": "2026-11-24T12:00:00+00:00",
        "allow_multiselect": false,
        "layout_type": 1,
        "results": {
            "is_finalized": true,
            "answer_counts": [
                {"id": 1, "count": 7, "me_voted": false},
                {"id": 2, "count": 7, "me_voted": true}
            ]
        }
    });
    let poll = parse_poll_from_json(&payload).expect("poll");
    assert_eq!(poll.answer_percentage(1), Some(50));
    assert_eq!(poll.answer_percentage(2), Some(50));
    assert!(poll.is_finalized());
    assert!(poll.is_expired_at(expiry));
    assert!(poll.is_expired_at(expiry + chrono::Duration::seconds(1)));
    assert!(!poll.is_expired_at(expiry - chrono::Duration::seconds(1)));
}

#[test]
fn poll_card_renders_from_real_data() {
    let payload = json!({
        "question": {"text": "Card smoke"},
        "answers": [
            {"answer_id": 1, "poll_media": {"text": "A"}},
            {"answer_id": 2, "poll_media": {"text": "B"}}
        ],
        "allow_multiselect": true,
        "layout_type": 1,
        "results": {
            "is_finalized": false,
            "answer_counts": [
                {"id": 1, "count": 2, "me_voted": true},
                {"id": 2, "count": 1, "me_voted": false}
            ]
        }
    });
    let poll = parse_poll_from_json(&payload).expect("poll");
    let _card = render_poll_card(&poll, Utc::now());
}

#[test]
fn poll_without_results_does_not_invent_percentages() {
    let payload = json!({
        "question": {"text": "Sem resultados"},
        "answers": [
            {"answer_id": 1, "poll_media": {"text": "A"}},
            {"answer_id": 2, "poll_media": {"text": "B"}}
        ],
        "allow_multiselect": false,
        "layout_type": 1
    });
    let poll = parse_poll_from_json(&payload).expect("poll");
    assert_eq!(poll.results, None);
    assert_eq!(poll.total_votes(), None);
    assert_eq!(poll.answer_percentage(1), None);
    assert_eq!(poll.answer_percentage(2), None);
    let _card = render_poll_card(&poll, Utc::now());
}
