use discord_rs::domain::guild::Role;
use discord_rs::domain::guild_admin::{
    AuditLog, AuditLogActionType, CreateChannelInvitePayload, CreateGuildBanPayload,
    GuildAdminValidationError, GuildRoleHierarchy, InviteMetadata, InviteTargetType, InviteType,
    KickGuildMemberRequest, RoleHierarchyError, TimeoutGuildMemberPayload, AUDIT_LOG_REASON_MAX_CHARS,
};
use serde_json::json;

fn role(id: &str, name: &str, position: i32) -> Role {
    Role::new(id, name).with_position(position)
}

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn sample_roles(guild_id: &str) -> Vec<Role> {
    vec![
        role(guild_id, "@everyone", 0),
        role("role-mod", "Mod", 5),
        role("role-admin", "Admin", 10),
        role("role-owner-role", "OwnerRole", 20),
    ]
}

#[test]
fn role_hierarchy_owner_can_act_on_anyone_except_owner_target() {
    let guild_id = "guild-1";
    let roles = sample_roles(guild_id);
    let h = GuildRoleHierarchy::new(guild_id, "owner-1", &roles);

    assert!(h
        .can_act_on_member("owner-1", &ids(&["role-owner-role"]), "user-2", &ids(&["role-admin"]), false)
        .is_ok());
    let err = h
        .can_act_on_member("owner-1", &ids(&["role-owner-role"]), "owner-1", &ids(&["role-owner-role"]), false)
        .unwrap_err();
    assert_eq!(err, RoleHierarchyError::CannotActOnSelf);
}

#[test]
fn role_hierarchy_cannot_act_on_owner() {
    let guild_id = "guild-1";
    let roles = sample_roles(guild_id);
    let h = GuildRoleHierarchy::new(guild_id, "owner-1", &roles);

    let err = h
        .can_act_on_member("admin-actor", &ids(&["role-admin"]), "owner-1", &ids(&["role-owner-role"]), false)
        .unwrap_err();
    assert_eq!(err, RoleHierarchyError::TargetIsOwner);
}

#[test]
fn role_hierarchy_requires_strictly_higher_role() {
    let guild_id = "guild-1";
    let roles = sample_roles(guild_id);
    let h = GuildRoleHierarchy::new(guild_id, "owner-1", &roles);

    // Mod (5) cannot act on Admin (10)
    let err = h
        .can_act_on_member("mod-actor", &ids(&["role-mod"]), "admin-target", &ids(&["role-admin"]), false)
        .unwrap_err();
    assert!(matches!(err, RoleHierarchyError::TargetNotLower { .. }));

    // Admin (10) cannot act on Admin (10) - equal is not enough
    let err = h
        .can_act_on_member("admin-a", &ids(&["role-admin"]), "admin-b", &ids(&["role-admin"]), false)
        .unwrap_err();
    assert!(matches!(err, RoleHierarchyError::TargetNotLower { .. }));

    // Admin (10) can act on Mod (5)
    assert!(h
        .can_act_on_member("admin-a", &ids(&["role-admin"]), "mod-b", &ids(&["role-mod"]), false)
        .is_ok());
}

#[test]
fn role_hierarchy_uses_highest_role_and_ignores_unknown_roles() {
    let guild_id = "guild-1";
    let roles = sample_roles(guild_id);
    let h = GuildRoleHierarchy::new(guild_id, "owner-1", &roles);

    // Actor has mod and admin; highest is admin (10) - can act on mod.
    assert!(h
        .can_act_on_member(
            "actor",
            &ids(&["role-mod", "unknown-role", "role-admin"]),
            "target",
            &ids(&["role-mod"]),
            false
        )
        .is_ok());

    let highest = h.highest_role_for_member(&ids(&["unknown-role", "role-mod"])).unwrap();
    assert_eq!(highest.id, "role-mod");
}

#[test]
fn role_hierarchy_falls_back_to_everyone_when_no_known_roles() {
    let guild_id = "guild-1";
    let roles = sample_roles(guild_id);
    let h = GuildRoleHierarchy::new(guild_id, "owner-1", &roles);

    let highest = h.highest_role_for_member(&ids(&["unknown-role"])).unwrap();
    assert_eq!(highest.id, guild_id);
    assert_eq!(highest.position, 0);

    // everyone vs everyone -> cannot act
    let err = h
        .can_act_on_member("a", &ids(&[]), "b", &ids(&[]), false)
        .unwrap_err();
    assert!(matches!(err, RoleHierarchyError::TargetNotLower { .. }));
}

#[test]
fn role_hierarchy_can_manage_role() {
    let guild_id = "guild-1";
    let roles = sample_roles(guild_id);
    let h = GuildRoleHierarchy::new(guild_id, "owner-1", &roles);

    assert!(h.can_manage_role("owner-1", &ids(&[]), "role-admin").is_ok());
    assert!(h.can_manage_role("admin-actor", &ids(&["role-admin"]), "role-mod").is_ok());

    let err = h.can_manage_role("mod-actor", &ids(&["role-mod"]), "role-admin").unwrap_err();
    assert!(matches!(err, RoleHierarchyError::RoleNotLower { .. }));

    let err = h
        .can_manage_role("mod-actor", &ids(&["role-mod"]), "role-mod")
        .unwrap_err();
    assert!(matches!(err, RoleHierarchyError::RoleNotLower { .. }));

    let err = h.can_manage_role("admin-actor", &ids(&["role-admin"]), "missing").unwrap_err();
    assert!(matches!(err, RoleHierarchyError::RoleNotFound { .. }));
}

#[test]
fn ban_payload_validation_and_serde() {
    let payload = CreateGuildBanPayload::new().with_delete_message_seconds(86_400);
    assert!(payload.validate().is_ok());
    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["delete_message_seconds"], 86_400);
    let back: CreateGuildBanPayload = serde_json::from_value(json).unwrap();
    assert_eq!(back, payload);

    let max = CreateGuildBanPayload::new().with_delete_message_seconds(604_800);
    assert!(max.validate().is_ok());

    let over = CreateGuildBanPayload::new().with_delete_message_seconds(604_801);
    let err = over.validate().unwrap_err();
    assert!(matches!(
        err,
        GuildAdminValidationError::DeleteMessageSecondsOutOfRange { value: 604_801, max: 604_800 }
    ));

    let empty: CreateGuildBanPayload = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.delete_message_seconds, None);
    assert!(empty.validate().is_ok());
}

#[test]
fn kick_request_validation() {
    let req = KickGuildMemberRequest::new("guild-1", "user-1").with_reason("spam cleanup");
    assert!(req.validate().is_ok());
    assert_eq!(req.reason.as_deref(), Some("spam cleanup"));

    let no_reason = KickGuildMemberRequest::new("guild-1", "user-1").with_reason("");
    assert_eq!(no_reason.reason, None);
    assert!(no_reason.validate().is_ok());

    let missing_guild = KickGuildMemberRequest::new("  ", "user-1");
    assert_eq!(
        missing_guild.validate().unwrap_err(),
        GuildAdminValidationError::MissingField("guild_id")
    );

    let missing_user = KickGuildMemberRequest::new("guild-1", "");
    assert_eq!(
        missing_user.validate().unwrap_err(),
        GuildAdminValidationError::MissingField("user_id")
    );

    let long_reason = "a".repeat(AUDIT_LOG_REASON_MAX_CHARS + 1);
    let req = KickGuildMemberRequest::new("guild-1", "user-1").with_reason(long_reason);
    assert!(matches!(
        req.validate().unwrap_err(),
        GuildAdminValidationError::ReasonTooLong { .. }
    ));
}

#[test]
fn timeout_payload_validation_and_serde() {
    let set = TimeoutGuildMemberPayload::set("2026-11-24T12:00:00Z");
    assert!(set.validate().is_ok());
    assert!(!set.is_clear());
    let json = serde_json::to_value(&set).unwrap();
    assert_eq!(json["communication_disabled_until"], "2026-11-24T12:00:00Z");

    let clear = TimeoutGuildMemberPayload::clear();
    assert!(clear.validate().is_ok());
    assert!(clear.is_clear());
    let json = serde_json::to_value(&clear).unwrap();
    assert!(json["communication_disabled_until"].is_null());

    let round: TimeoutGuildMemberPayload = serde_json::from_str(
        r#"{"communication_disabled_until":"2026-11-24T12:00:00+00:00"}"#,
    )
    .unwrap();
    assert!(round.validate().is_ok());

    let bad = TimeoutGuildMemberPayload::set("not-a-timestamp");
    assert!(matches!(
        bad.validate().unwrap_err(),
        GuildAdminValidationError::InvalidTimestamp(_)
    ));

    let empty = TimeoutGuildMemberPayload::set("   ");
    assert_eq!(
        empty.validate().unwrap_err(),
        GuildAdminValidationError::MissingField("communication_disabled_until")
    );
}

#[test]
fn invite_payload_validation_and_serde() {
    let payload = CreateChannelInvitePayload::new()
        .with_max_age(3600)
        .with_max_uses(10)
        .with_temporary(true)
        .with_unique(true)
        .with_target(InviteTargetType::Stream);
    assert!(payload.validate().is_ok());

    let json = serde_json::to_value(&payload).unwrap();
    assert_eq!(json["max_age"], 3600);
    assert_eq!(json["max_uses"], 10);
    assert_eq!(json["temporary"], true);
    assert_eq!(json["unique"], true);
    assert_eq!(json["target_type"], 1);

    let back: CreateChannelInvitePayload = serde_json::from_value(json).unwrap();
    assert_eq!(back, payload);

    let max_age_err = CreateChannelInvitePayload::new().with_max_age(604_801);
    assert!(matches!(
        max_age_err.validate().unwrap_err(),
        GuildAdminValidationError::MaxAgeOutOfRange { .. }
    ));

    let max_uses_err = CreateChannelInvitePayload::new().with_max_uses(101);
    assert!(matches!(
        max_uses_err.validate().unwrap_err(),
        GuildAdminValidationError::MaxUsesOutOfRange { .. }
    ));

    // Boundary values
    assert!(CreateChannelInvitePayload::new().with_max_age(0).validate().is_ok());
    assert!(CreateChannelInvitePayload::new().with_max_age(604_800).validate().is_ok());
    assert!(CreateChannelInvitePayload::new().with_max_uses(0).validate().is_ok());
    assert!(CreateChannelInvitePayload::new().with_max_uses(100).validate().is_ok());
}

#[test]
fn invite_type_and_target_type_serde() {
    let t: InviteType = serde_json::from_str("0").unwrap();
    assert_eq!(t, InviteType::Guild);
    assert_eq!(serde_json::to_value(t).unwrap(), json!(0));
    let t: InviteType = serde_json::from_str("1").unwrap();
    assert_eq!(t, InviteType::GroupDm);
    let t: InviteType = serde_json::from_str("2").unwrap();
    assert_eq!(t, InviteType::Friend);
    let err = serde_json::from_str::<InviteType>("9").unwrap_err().to_string();
    assert!(err.contains("Unknown InviteType"));

    let tt: InviteTargetType = serde_json::from_str("1").unwrap();
    assert_eq!(tt, InviteTargetType::Stream);
    let tt: InviteTargetType = serde_json::from_str("2").unwrap();
    assert_eq!(tt, InviteTargetType::EmbeddedApplication);
    assert_eq!(serde_json::to_value(tt).unwrap(), json!(2));
}

#[test]
fn invite_metadata_expiry_and_exhaustion() {
    let invite = InviteMetadata {
        code: "abc123".to_string(),
        expires_at: Some("2026-11-24T12:00:00Z".to_string()),
        uses: Some(5),
        max_uses: Some(10),
        ..Default::default()
    };
    assert!(!invite.is_expired_at("2026-11-24T11:59:59Z"));
    assert!(invite.is_expired_at("2026-11-24T12:00:00Z"));
    assert!(invite.is_expired_at("2026-11-24T12:00:01Z"));
    assert!(!invite.is_exhausted());

    let exhausted = InviteMetadata {
        uses: Some(10),
        max_uses: Some(10),
        ..Default::default()
    };
    assert!(exhausted.is_exhausted());

    let unlimited = InviteMetadata {
        uses: Some(100),
        max_uses: Some(0),
        ..Default::default()
    };
    assert!(!unlimited.is_exhausted());

    let no_expiry = InviteMetadata::default();
    assert!(!no_expiry.is_expired_at("2099-01-01T00:00:00Z"));
}

#[test]
fn audit_log_parsing_and_queries() {
    let payload = json!({
        "audit_log_entries": [
            {
                "id": "1001",
                "action_type": 22,
                "target_id": "user-banned",
                "user_id": "mod-1",
                "reason": "rule violation",
                "changes": [
                    {"key": "communication_disabled_until", "old_value": null, "new_value": "2026-11-25T00:00:00Z"}
                ]
            },
            {
                "id": "1002",
                "action_type": 20,
                "target_id": "user-kicked",
                "user_id": "mod-2",
                "reason": "spam"
            },
            {
                "id": "1003",
                "action_type": 40,
                "target_id": "invite-code-1",
                "user_id": "mod-1",
                "options": {
                    "channel_id": "chan-1",
                    "code": "abc123"
                }
            },
            {
                "id": "1004",
                "action_type": 31,
                "target_id": "role-1",
                "user_id": "admin-1",
                "changes": [
                    {"key": "name", "old_value": "Old", "new_value": "New"},
                    {"key": "permissions", "old_value": "1024", "new_value": "3072"}
                ]
            },
            {
                "id": "1005",
                "action_type": 9999,
                "target_id": "something",
                "user_id": "admin-1"
            }
        ],
        "users": [{"id": "mod-1", "username": "mod1"}],
        "integrations": [],
        "webhooks": [],
        "threads": []
    });

    let log = AuditLog::from_json(&payload).expect("audit log should parse");
    assert_eq!(log.audit_log_entries.len(), 5);

    let bans = log.entries_by_action(AuditLogActionType::MemberBanAdd);
    assert_eq!(bans.len(), 1);
    assert_eq!(bans[0].target_id.as_deref(), Some("user-banned"));
    assert_eq!(bans[0].reason.as_deref(), Some("rule violation"));
    assert_eq!(bans[0].changes.len(), 1);
    assert_eq!(bans[0].changes[0].key, "communication_disabled_until");

    let kicks = log.entries_by_action(AuditLogActionType::MemberKick);
    assert_eq!(kicks.len(), 1);
    assert_eq!(kicks[0].user_id.as_deref(), Some("mod-2"));

    let invites = log.entries_by_action(AuditLogActionType::InviteCreate);
    assert_eq!(invites.len(), 1);
    let opts = invites[0].options.as_ref().unwrap();
    assert_eq!(opts.channel_id.as_deref(), Some("chan-1"));

    let role_updates = log.entries_for_target("role-1");
    assert_eq!(role_updates.len(), 1);
    assert_eq!(role_updates[0].action_type, AuditLogActionType::RoleUpdate);
    assert_eq!(role_updates[0].changes.len(), 2);

    let unknowns = log.entries_by_action(AuditLogActionType::Unknown(9999));
    assert_eq!(unknowns.len(), 1);

    // raw users preserved
    assert_eq!(log.users.len(), 1);
    assert_eq!(log.users[0]["username"], "mod1");
}

#[test]
fn audit_log_action_type_round_trip() {
    let cases = [
        (1u16, AuditLogActionType::GuildUpdate),
        (20, AuditLogActionType::MemberKick),
        (22, AuditLogActionType::MemberBanAdd),
        (23, AuditLogActionType::MemberBanRemove),
        (24, AuditLogActionType::MemberUpdate),
        (25, AuditLogActionType::MemberRoleUpdate),
        (30, AuditLogActionType::RoleCreate),
        (31, AuditLogActionType::RoleUpdate),
        (32, AuditLogActionType::RoleDelete),
        (40, AuditLogActionType::InviteCreate),
        (41, AuditLogActionType::InviteUpdate),
        (42, AuditLogActionType::InviteDelete),
    ];
    for (num, expected) in cases {
        let parsed = AuditLogActionType::try_from(num).unwrap();
        assert_eq!(parsed, expected);
        let back: u16 = parsed.into();
        assert_eq!(back, num);
        let json_val = serde_json::to_value(parsed).unwrap();
        assert_eq!(json_val, json!(num));
        let from_json: AuditLogActionType = serde_json::from_value(json_val).unwrap();
        assert_eq!(from_json, expected);
    }

    let unknown = AuditLogActionType::try_from(12345u16).unwrap();
    assert_eq!(unknown, AuditLogActionType::Unknown(12345));
    assert_eq!(u16::from(unknown), 12345);
}

#[test]
fn audit_log_missing_optional_fields_default() {
    let payload = json!({
        "audit_log_entries": [
            {
                "id": "1",
                "action_type": 20
            }
        ]
    });
    let log = AuditLog::from_json(&payload).unwrap();
    let entry = &log.audit_log_entries[0];
    assert_eq!(entry.target_id, None);
    assert_eq!(entry.user_id, None);
    assert_eq!(entry.reason, None);
    assert!(entry.changes.is_empty());
    assert_eq!(entry.options, None);
}
