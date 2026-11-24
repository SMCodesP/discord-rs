use chrono::{TimeZone, Utc};
use discord_rs::chat::invite_card::{
    extract_invite_code, extract_invite_links, invite_api_path, is_invite_url, parse_invite_error,
    parse_invite_from_json, parse_invite_json_str, parse_invite_url, render_invite_card,
    Invite, InviteAvailability, InviteErrorKind, InviteGuild, InviteParseError, InviteType,
};
use serde_json::json;

fn full_payload() -> serde_json::Value {
    json!({
        "code": "sintetico-abc",
        "guild": {
            "id": "100000000000000001",
            "name": "Servidor Sintetico",
            "icon": "icone_sintetico",
            "splash": "splash_sintetico",
            "description": "Descricao sintetica do servidor",
            "features": ["COMMUNITY"],
            "verification_level": 2,
            "nsfw": false,
            "premium_tier": 2,
            "premium_subscription_count": 8
        },
        "channel": {
            "id": "100000000000000002",
            "type": 0,
            "name": "geral"
        },
        "inviter": {
            "id": "100000000000000003",
            "username": "usuario.sintetico",
            "discriminator": "0",
            "global_name": "Usuario Sintetico",
            "avatar": "avatar_sintetico",
            "bot": false
        },
        "approximate_member_count": 42,
        "approximate_presence_count": 7,
        "expires_at": "2026-12-01T12:00:00+00:00",
        "created_at": "2026-11-20T09:30:00+00:00",
        "temporary": true,
        "uses": 3,
        "max_uses": 10,
        "type": 0,
        "flags": 0
    })
}

#[test]
fn parses_supported_invite_domains_and_forms() {
    for url in [
        "https://discord.gg/sintetico-abc",
        "discord.gg/sintetico-abc",
        "http://www.discord.gg/sintetico-abc",
        "https://discord.com/invite/sintetico-abc",
        "https://discordapp.com/invite/sintetico-abc",
        "https://ptb.discord.com/invite/sintetico-abc",
        "https://canary.discord.com/invite/sintetico-abc",
    ] {
        let link = parse_invite_url(url).expect(url);
        assert_eq!(link.code, "sintetico-abc");
        assert!(is_invite_url(url));
        assert_eq!(link.api_path(), "/invites/sintetico-abc");
    }
}

#[test]
fn rejects_foreign_domains_and_malformed_paths() {
    for url in [
        "https://notdiscord.gg/sintetico",
        "https://discord.gg.evil.example/sintetico",
        "https://discord.com/channels/100/200",
        "https://discord.gg/",
        "https://discord.com/invite/",
        "https://discord.gg/sintetico/extra",
        "https://discord.gg/abc%20def",
    ] {
        assert!(parse_invite_url(url).is_none(), "{url}");
    }
    assert!(parse_invite_url("https://discord.gg/sintetico?event=1").is_some());
}

#[test]
fn extracts_from_markdown_dedupes_and_ignores_foreign_hosts() {
    let text = concat!(
        "Convite: <https://discord.gg/sintetico-abc>, ",
        "[link](https://discord.com/invite/def456) e ",
        "https://discord.gg/sintetico-abc. ",
        "Nao e convite: notdiscord.gg/falso e https://discord.gg.evil.example/falso"
    );
    let links = extract_invite_links(text);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].code, "sintetico-abc");
    assert_eq!(links[1].code, "def456");
    assert_eq!(extract_invite_code(text).as_deref(), Some("sintetico-abc"));

    assert!(extract_invite_links("notdiscord.gg/falso").is_empty());
    assert!(extract_invite_links("discord.gg.evil.example/falso").is_empty());
    assert!(extract_invite_links("sem link nenhum").is_empty());
}

#[test]
fn parses_full_official_invite_payload() {
    let invite = parse_invite_from_json(&full_payload()).expect("invite should parse");
    assert_eq!(invite.code, "sintetico-abc");
    assert_eq!(invite.invite_type, InviteType::Guild);
    assert_eq!(invite.title(), "Servidor Sintetico");
    assert_eq!(
        invite.description(),
        Some("Descricao sintetica do servidor")
    );
    assert_eq!(invite.approximate_member_count, Some(42));
    assert_eq!(invite.approximate_presence_count, Some(7));
    assert_eq!(invite.members_label().as_deref(), Some("42 members · 7 online"));
    assert_eq!(invite.uses, Some(3));
    assert_eq!(invite.max_uses, Some(10));
    assert_eq!(invite.usage_label().as_deref(), Some("3/10 uses"));
    assert!(invite.temporary);
    assert_eq!(invite.flags, Some(0));

    let guild = invite.guild.as_ref().expect("guild");
    assert_eq!(guild.verification_level, Some(2));
    assert_eq!(guild.premium_tier, Some(2));
    assert_eq!(guild.premium_subscription_count, Some(8));
    assert_eq!(guild.features, vec!["COMMUNITY".to_string()]);
    assert_eq!(
        guild.icon_url(96).as_deref(),
        Some("https://cdn.discordapp.com/icons/100000000000000001/icone_sintetico.png?size=96")
    );
    assert_eq!(
        guild.splash_url(640).as_deref(),
        Some("https://cdn.discordapp.com/splashes/100000000000000001/splash_sintetico.png?size=640")
    );

    let channel = invite.channel.as_ref().expect("channel");
    assert_eq!(channel.label().as_deref(), Some("#geral"));

    let inviter = invite.inviter.as_ref().expect("inviter");
    assert_eq!(inviter.display_name(), "Usuario Sintetico");
    assert_eq!(
        inviter.avatar_url(64).as_deref(),
        Some("https://cdn.discordapp.com/avatars/100000000000000003/avatar_sintetico.png?size=64")
    );

    let before_expiry = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    assert_eq!(
        invite.availability_at(before_expiry),
        InviteAvailability::Available
    );
    assert_eq!(
        invite.state_label(before_expiry).as_deref(),
        Some("Temporary membership")
    );
    let _card = render_invite_card(&invite, before_expiry);
}

#[test]
fn minimal_payload_does_not_invent_counts_or_relations() {
    let payload = json!({"code": "minimal1"});
    let invite = parse_invite_from_json(&payload).expect("minimal invite");
    assert_eq!(invite.code, "minimal1");
    assert_eq!(invite.guild, None);
    assert_eq!(invite.channel, None);
    assert_eq!(invite.inviter, None);
    assert_eq!(invite.approximate_member_count, None);
    assert_eq!(invite.approximate_presence_count, None);
    assert_eq!(invite.expires_at, None);
    assert_eq!(invite.uses, None);
    assert_eq!(invite.max_uses, None);
    assert_eq!(invite.invite_type, InviteType::Guild);
    assert_eq!(invite.members_label(), None);
    assert_eq!(invite.usage_label(), None);
    assert_eq!(invite.description(), None);
    assert_eq!(invite.title(), "minimal1");
    assert_eq!(
        invite.availability_at(Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap()),
        InviteAvailability::Available
    );
    assert_eq!(
        invite.state_label(Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap()),
        None
    );
    let _card = render_invite_card(
        &invite,
        Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap(),
    );
}

#[test]
fn reports_expired_and_exhausted_invites_without_success() {
    let expired_at = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let payload = json!({
        "code": "expirado1",
        "expires_at": "2026-11-24T12:00:00+00:00",
        "uses": 10,
        "max_uses": 10,
        "temporary": false
    });
    let invite = parse_invite_from_json(&payload).expect("invite");
    assert!(invite.is_expired_at(expired_at));
    assert!(invite.is_exhausted());
    assert_eq!(
        invite.availability_at(expired_at),
        InviteAvailability::Expired
    );
    assert_eq!(
        invite.state_label(expired_at).as_deref(),
        Some("Invite expired")
    );
    let _card = render_invite_card(&invite, expired_at);

    let before_expiry = expired_at - chrono::Duration::seconds(1);
    assert!(!invite.is_expired_at(before_expiry));
    assert_eq!(
        invite.availability_at(before_expiry),
        InviteAvailability::Exhausted
    );
    assert_eq!(
        invite.state_label(before_expiry).as_deref(),
        Some("No uses left")
    );
}

#[test]
fn rejects_invalid_json_shapes_and_fields() {
    assert_eq!(
        parse_invite_from_json(&json!([1, 2, 3])),
        Err(InviteParseError::NotObject)
    );
    assert_eq!(
        parse_invite_from_json(&json!({})),
        Err(InviteParseError::MissingField("code"))
    );
    assert_eq!(
        parse_invite_from_json(&json!({"code": "abc/def"})),
        Err(InviteParseError::InvalidCode("abc/def".to_string()))
    );
    assert_eq!(
        parse_invite_from_json(&json!({"code": "ok", "guild": "nope"})),
        Err(InviteParseError::InvalidType("guild"))
    );
    assert_eq!(
        parse_invite_from_json(&json!({"code": "ok", "guild": {"id": "1"}})),
        Err(InviteParseError::MissingField("name"))
    );
    assert_eq!(
        parse_invite_from_json(&json!({"code": "ok", "expires_at": 123})),
        Err(InviteParseError::InvalidType("expires_at"))
    );
    assert_eq!(
        parse_invite_from_json(&json!({"code": "ok", "expires_at": "amanha"})),
        Err(InviteParseError::InvalidTimestamp("expires_at"))
    );
    assert_eq!(
        parse_invite_json_str("{invalid"),
        Err(InviteParseError::NotObject)
    );
}

#[test]
fn parses_api_error_status_and_retry_after() {
    let not_found = parse_invite_error(
        404,
        br#"{"message":"Unknown Invite","code":10006}"#,
    );
    assert_eq!(not_found.code, Some(10006));
    assert_eq!(not_found.message.as_deref(), Some("Unknown Invite"));
    assert_eq!(not_found.kind(), InviteErrorKind::NotFound);
    assert!(!not_found.is_retryable());

    let rate_limited = parse_invite_error(
        429,
        br#"{"message":"You are being rate limited.","retry_after":1.5,"global":false}"#,
    );
    assert_eq!(rate_limited.retry_after, Some(1.5));
    assert_eq!(rate_limited.kind(), InviteErrorKind::RateLimited);
    assert!(rate_limited.is_retryable());

    let verification = parse_invite_error(400, br#"{"code":40002,"message":"verification"}"#);
    assert_eq!(verification.kind(), InviteErrorKind::VerificationRequired);

    let forbidden = parse_invite_error(403, b"");
    assert_eq!(forbidden.kind(), InviteErrorKind::Forbidden);
    assert_eq!(forbidden.message, None);

    let unknown = parse_invite_error(502, b"upstream");
    assert_eq!(unknown.kind(), InviteErrorKind::Unknown);
    assert_eq!(unknown.code, None);
}

#[test]
fn builds_only_valid_invite_api_paths() {
    assert_eq!(
        invite_api_path("sintetico-abc").as_deref(),
        Some("/invites/sintetico-abc")
    );
    assert_eq!(invite_api_path(""), None);
    assert_eq!(invite_api_path("abc/def"), None);
    assert_eq!(invite_api_path("abc def"), None);
}

#[test]
fn guild_assets_require_real_hashes() {
    let guild = InviteGuild {
        id: "100000000000000001".to_string(),
        name: "Servidor Sintetico".to_string(),
        icon: None,
        splash: None,
        ..Default::default()
    };
    assert_eq!(guild.icon_url(96), None);
    assert_eq!(guild.splash_url(640), None);

    let direct = InviteGuild {
        id: "100000000000000001".to_string(),
        name: "Servidor Sintetico".to_string(),
        splash: Some("https://cdn.discordapp.com/splashes/1/direct.png".to_string()),
        ..Default::default()
    };
    assert_eq!(
        direct.splash_url(640).as_deref(),
        Some("https://cdn.discordapp.com/splashes/1/direct.png")
    );
}
