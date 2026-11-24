use chrono::{DateTime, Duration, TimeZone, Utc};
use serde_json::json;

use discord_rs::chat::channel_guards::{
    ActionGuard, AgeGateDecision, AgeGatePreference, AgeGateRequest, AgeGateSession, AgeGateStatus,
    AgeRestriction, AgeRestrictionKind, ChannelGuard, ChatAction, GuardBlock, MemberTimeout,
    NsfwLevel, TimeoutState, AGE_RESTRICTED_CHANNEL_PROMPT, AGE_RESTRICTED_GUILD_BLOCKED,
    AGE_RESTRICTED_GUILD_PROMPT,
};

fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .expect("valid UTC timestamp")
}

fn channel_json(id: &str, guild_id: &str, nsfw: bool) -> serde_json::Value {
    json!({
        "id": id,
        "guild_id": guild_id,
        "name": "synthetic-channel",
        "type": 0,
        "nsfw": nsfw
    })
}

fn guild_json(id: &str, nsfw: bool, nsfw_level: serde_json::Value) -> serde_json::Value {
    json!({
        "id": id,
        "name": "synthetic-guild",
        "nsfw": nsfw,
        "nsfw_level": nsfw_level
    })
}

#[test]
fn unrestricted_channel_renders_without_prompt() {
    let channel = channel_json("100000000000000001", "200000000000000001", false);
    let guild = guild_json("200000000000000001", false, json!(0));
    let request = AgeGateRequest::from_channel_and_guild_json(&channel, Some(&guild)).unwrap();
    let session = AgeGateSession::new();

    assert_eq!(session.status(&request), AgeGateStatus::NotRequired);
    assert!(session.can_render(&request));
    assert_eq!(request.restriction.guild_nsfw_level, NsfwLevel::Default);

    let guard = ChannelGuard::new(session, MemberTimeout::none());
    assert_eq!(
        guard.check(
            ChatAction::SendMessage,
            &request,
            utc(2026, 11, 24, 12, 0, 0)
        ),
        ActionGuard::Allowed
    );
}

#[test]
fn nsfw_channel_requires_confirmation_before_render() {
    let channel = channel_json("100000000000000002", "200000000000000001", true);
    let guild = guild_json("200000000000000001", false, json!(0));
    let request = AgeGateRequest::from_channel_and_guild_json(&channel, Some(&guild)).unwrap();
    let mut session = AgeGateSession::new();

    assert_eq!(
        session.status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Channel)
    );
    assert!(!session.can_render(&request));
    assert_eq!(
        session.status(&request).prompt(),
        Some(AGE_RESTRICTED_CHANNEL_PROMPT)
    );

    assert!(session.confirm(&request));
    assert_eq!(session.status(&request), AgeGateStatus::Allowed);
    assert!(session.can_render(&request));

    let sibling = AgeGateRequest::from_channel_json(&channel_json(
        "100000000000000003",
        "200000000000000001",
        true,
    ))
    .unwrap();
    assert_eq!(
        session.status(&sibling),
        AgeGateStatus::Pending(AgeRestrictionKind::Channel)
    );
}

#[test]
fn denied_channel_stays_blocked_until_explicit_retry() {
    let channel = channel_json("100000000000000004", "200000000000000001", true);
    let request = AgeGateRequest::from_channel_and_guild_json(&channel, None).unwrap();
    let mut session = AgeGateSession::new();

    assert!(session.deny(&request));
    assert_eq!(
        session.status(&request),
        AgeGateStatus::Denied(AgeRestrictionKind::Channel)
    );
    assert!(!session.can_render(&request));
    assert!(!session.confirm(&request));

    assert!(session.retry_after_denial(&request));
    assert_eq!(
        session.status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Channel)
    );
    assert!(session.confirm(&request));
    assert!(session.can_render(&request));
}

#[test]
fn age_restricted_guild_gates_channels_and_reports_legal_age_message() {
    let channel = channel_json("100000000000000005", "200000000000000005", false);
    let guild = guild_json("200000000000000005", false, json!(3));
    let request = AgeGateRequest::from_channel_and_guild_json(&channel, Some(&guild)).unwrap();
    let mut session = AgeGateSession::new();

    assert!(request.restriction.guild_restricted());
    assert_eq!(
        session.status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Guild)
    );
    assert_eq!(
        session.status(&request).prompt(),
        Some(AGE_RESTRICTED_GUILD_PROMPT)
    );
    assert!(!session.can_render(&request));

    assert!(session.deny(&request));
    assert_eq!(
        session.status(&request),
        AgeGateStatus::Denied(AgeRestrictionKind::Guild)
    );
    assert_eq!(
        session.status(&request).denial_message(),
        Some(AGE_RESTRICTED_GUILD_BLOCKED)
    );

    assert!(session.retry_after_denial(&request));
    assert!(session.confirm(&request));
    assert!(session.can_render(&request));
}

#[test]
fn guild_nsfw_flag_and_string_level_both_restrict() {
    let guild_level = guild_json("200000000000000006", false, json!("3"));
    let request = AgeGateRequest::from_guild_json(&guild_level).unwrap();
    assert_eq!(request.restriction.guild_nsfw_level, NsfwLevel::AgeRestricted);
    assert_eq!(
        AgeGateSession::new().status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Guild)
    );

    let guild_flag = guild_json("200000000000000007", true, json!(0));
    let flagged = AgeGateRequest::from_guild_json(&guild_flag).unwrap();
    assert!(flagged.restriction.guild_restricted());
    assert_eq!(
        AgeGateSession::new().status(&flagged),
        AgeGateStatus::Pending(AgeRestrictionKind::Guild)
    );
}

#[test]
fn guild_and_channel_gates_require_each_confirmation() {
    let channel = channel_json("100000000000000008", "200000000000000008", true);
    let guild = guild_json("200000000000000008", true, json!(3));
    let request = AgeGateRequest::from_channel_and_guild_json(&channel, Some(&guild)).unwrap();
    let mut session = AgeGateSession::new();

    assert_eq!(
        session.status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Guild)
    );
    assert!(session.confirm(&request));
    assert_eq!(
        session.status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Channel)
    );
    assert!(!session.can_render(&request));
    assert!(session.confirm(&request));
    assert_eq!(session.status(&request), AgeGateStatus::Allowed);
}

#[test]
fn user_preference_is_applied_before_prompting() {
    let channel = channel_json("100000000000000009", "200000000000000009", true);
    let request = AgeGateRequest::from_channel_json(&channel).unwrap();

    let mut denied = AgeGateSession::new();
    denied.set_preference(AgeGatePreference::Deny);
    assert_eq!(
        denied.status(&request),
        AgeGateStatus::Denied(AgeRestrictionKind::Channel)
    );
    assert!(!denied.can_render(&request));

    let mut allowed = AgeGateSession::new();
    allowed.set_preference(AgeGatePreference::Allow);
    assert_eq!(allowed.status(&request), AgeGateStatus::Allowed);
    assert!(allowed.can_render(&request));
}

#[test]
fn decisions_are_scoped_to_the_account_and_reset_on_account_switch() {
    let channel = channel_json("100000000000000010", "200000000000000010", true);
    let request = AgeGateRequest::from_channel_json(&channel).unwrap();
    let mut session = AgeGateSession::for_account("900000000000000001");

    assert_eq!(session.account_id(), Some("900000000000000001"));
    assert!(session.confirm(&request));
    assert_eq!(
        session.decision(AgeRestrictionKind::Channel, "100000000000000010"),
        Some(AgeGateDecision::Allow)
    );
    assert!(session.can_render(&request));

    session.set_account(Some("900000000000000002".to_string()));
    assert_eq!(session.account_id(), Some("900000000000000002"));
    assert_eq!(
        session.status(&request),
        AgeGateStatus::Pending(AgeRestrictionKind::Channel)
    );
    assert!(!session.can_render(&request));
}

#[test]
fn active_timeout_blocks_send_reaction_and_typing_with_real_remaining() {
    let member = json!({
        "user": {"id": "900000000000000003"},
        "communication_disabled_until": "2026-11-24T12:00:00.000000+00:00"
    });
    let timeout = MemberTimeout::from_member_json(&member).unwrap();
    let now = utc(2026, 11, 24, 11, 59, 30);

    assert_eq!(
        timeout.state(now),
        TimeoutState::Active {
            until: utc(2026, 11, 24, 12, 0, 0),
            remaining: Duration::seconds(30)
        }
    );
    assert_eq!(timeout.remaining(now), Some(Duration::seconds(30)));

    let warning = timeout.warning(now).expect("active timeout warning");
    assert_eq!(warning.until, utc(2026, 11, 24, 12, 0, 0));
    assert_eq!(warning.remaining, Duration::seconds(30));
    assert!(warning.message().contains("2026-11-24T12:00:00"));

    let channel = channel_json("100000000000000011", "200000000000000011", false);
    let request = AgeGateRequest::from_channel_json(&channel).unwrap();
    let guard = ChannelGuard::new(AgeGateSession::new(), timeout);

    for action in [
        ChatAction::SendMessage,
        ChatAction::AddReaction,
        ChatAction::StartTyping,
    ] {
        match guard.check(action, &request, now) {
            ActionGuard::Blocked(GuardBlock::TimedOut(reported)) => {
                assert_eq!(reported, warning);
            }
            other => panic!("expected timeout block for {action:?}, got {other:?}"),
        }
    }
}

#[test]
fn timeout_releases_exactly_at_expiration_and_is_cleared() {
    let until = utc(2026, 11, 24, 12, 0, 0);
    let mut timeout = MemberTimeout::parse(Some("2026-11-24T12:00:00Z")).unwrap();

    assert!(timeout.is_active(utc(2026, 11, 24, 11, 59, 59)));
    assert_eq!(
        timeout.state(until),
        TimeoutState::Expired { until }
    );
    assert_eq!(timeout.remaining(until), None);
    assert!(!timeout.is_active(until));
    assert!(timeout.warning(until).is_none());
    assert!(timeout.clear_if_expired(until));
    assert_eq!(timeout.state(until), TimeoutState::NotTimedOut);
    assert!(!timeout.clear_if_expired(until));

    let channel = channel_json("100000000000000012", "200000000000000012", false);
    let request = AgeGateRequest::from_channel_json(&channel).unwrap();
    let guard = ChannelGuard::new(AgeGateSession::new(), timeout);
    assert_eq!(
        guard.check(ChatAction::SendMessage, &request, until),
        ActionGuard::Allowed
    );
}

#[test]
fn timeout_updates_and_removal_follow_gateway_payloads() {
    let mut timeout = MemberTimeout::from_member_json(&json!({
        "communication_disabled_until": "2026-11-24T12:00:00+00:00"
    }))
    .unwrap();
    assert_eq!(timeout.until(), Some(utc(2026, 11, 24, 12, 0, 0)));

    timeout
        .update_from_member_json(&json!({
            "communication_disabled_until": "2026-11-24T12:30:00+00:00"
        }))
        .unwrap();
    assert_eq!(timeout.until(), Some(utc(2026, 11, 24, 12, 30, 0)));

    let before_invalid = timeout.until();
    let error = timeout.update_raw(Some("not-a-timestamp")).unwrap_err();
    assert_eq!(error.value(), "not-a-timestamp");
    assert_eq!(timeout.until(), before_invalid);

    timeout
        .update_from_member_json(&json!({"communication_disabled_until": null}))
        .unwrap();
    assert_eq!(timeout.until(), None);
    assert_eq!(
        timeout.state(utc(2026, 11, 24, 12, 0, 0)),
        TimeoutState::NotTimedOut
    );
}

#[test]
fn missing_or_malformed_timeout_field_never_fabricates_state() {
    let missing = MemberTimeout::from_member_json(&json!({
        "user": {"id": "900000000000000004"}
    }))
    .unwrap();
    assert_eq!(missing.until(), None);

    let malformed = MemberTimeout::from_member_json(&json!({
        "communication_disabled_until": 123
    }));
    assert!(malformed.is_err());
    assert_eq!(malformed.unwrap_err().value(), "123");
}

#[test]
fn timeout_timestamp_normalizes_timezone_offsets() {
    let timeout = MemberTimeout::parse(Some("2026-11-24T09:00:00-03:00")).unwrap();
    assert_eq!(timeout.until(), Some(utc(2026, 11, 24, 12, 0, 0)));
    assert_eq!(
        timeout.remaining(utc(2026, 11, 24, 11, 59, 0)),
        Some(Duration::minutes(1))
    );
}

#[test]
fn age_gate_precedes_timeout_and_both_must_clear() {
    let channel = channel_json("100000000000000013", "200000000000000013", true);
    let request = AgeGateRequest::from_channel_json(&channel).unwrap();
    let timeout = MemberTimeout::parse(Some("2026-11-24T12:00:00Z")).unwrap();
    let mut age = AgeGateSession::new();
    let now = utc(2026, 11, 24, 11, 59, 0);

    let guard = ChannelGuard::new(age.clone(), timeout.clone());
    assert!(!guard.can_render(&request));
    assert!(matches!(
        guard.check(ChatAction::SendMessage, &request, now),
        ActionGuard::Blocked(GuardBlock::AgeGate(AgeGateStatus::Pending(
            AgeRestrictionKind::Channel
        )))
    ));

    assert!(age.confirm(&request));
    let guard = ChannelGuard::new(age, timeout);
    assert!(guard.can_render(&request));
    assert!(matches!(
        guard.check(ChatAction::SendMessage, &request, now),
        ActionGuard::Blocked(GuardBlock::TimedOut(_))
    ));

    assert_eq!(
        guard.check(
            ChatAction::SendMessage,
            &request,
            utc(2026, 11, 24, 12, 0, 0)
        ),
        ActionGuard::Allowed
    );
}

#[test]
fn restriction_json_defaults_do_not_invent_nsfw_flags() {
    let channel = json!({"id": "100000000000000014"});
    let request = AgeGateRequest::from_channel_json(&channel).unwrap();
    assert_eq!(request.channel_id.as_deref(), Some("100000000000000014"));
    assert_eq!(request.guild_id, None);
    assert!(!request.restriction.is_restricted());
    assert_eq!(
        AgeGateSession::new().status(&request),
        AgeGateStatus::NotRequired
    );

    let restriction = AgeRestriction::from_channel_and_guild_json(
        &channel,
        Some(&json!({"id": "200000000000000014", "nsfw_level": 99})),
    );
    assert_eq!(restriction.guild_nsfw_level, NsfwLevel::Default);
    assert!(!restriction.guild_restricted());
}
