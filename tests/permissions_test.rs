use discord_rs::domain::permissions::{
    compute_base_permissions, compute_permissions, has_permission, parse_permissions,
    OverwriteType, PermissionOverwrite, RolePermissions, ADMINISTRATOR, ALL_PERMISSIONS, CONNECT,
    MANAGE_MESSAGES, SEND_MESSAGES, SPEAK, VIEW_CHANNEL,
};

fn role(id: &str, permissions: u64) -> RolePermissions {
    RolePermissions::new(id, permissions)
}

fn overwrite(id: &str, kind: OverwriteType, allow: u64, deny: u64) -> PermissionOverwrite {
    PermissionOverwrite::new(id, kind, allow, deny)
}

#[test]
fn parse_permissions_handles_valid_and_invalid() {
    assert_eq!(parse_permissions("0"), 0);
    assert_eq!(parse_permissions("3072"), VIEW_CHANNEL | SEND_MESSAGES);
    assert_eq!(parse_permissions("not-a-number"), 0);
}

#[test]
fn owner_gets_all_permissions() {
    let guild_id = "g1";
    let owner_id = "owner";
    let roles = vec![role(guild_id, 0)];
    let perms = compute_permissions(owner_id, owner_id, guild_id, &[], &roles, &[]);
    assert_eq!(perms, ALL_PERMISSIONS);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, MANAGE_MESSAGES));
}

#[test]
fn administrator_bypasses_channel_overwrites() {
    let guild_id = "g1";
    let user_id = "u1";
    let admin_role_id = "r_admin";
    let roles = vec![
        role(guild_id, VIEW_CHANNEL | SEND_MESSAGES),
        role(admin_role_id, ADMINISTRATOR),
    ];
    let overwrites = vec![
        overwrite(guild_id, OverwriteType::Role, 0, VIEW_CHANNEL | SEND_MESSAGES),
        overwrite(user_id, OverwriteType::Member, 0, MANAGE_MESSAGES),
    ];

    let perms = compute_permissions(
        "owner",
        user_id,
        guild_id,
        &[admin_role_id.to_string()],
        &roles,
        &overwrites,
    );
    assert_eq!(perms, ALL_PERMISSIONS);
    assert!(has_permission(perms, SEND_MESSAGES));
    assert!(has_permission(perms, MANAGE_MESSAGES));
}

#[test]
fn everyone_role_provides_base_permissions() {
    let guild_id = "g1";
    let roles = vec![role(guild_id, VIEW_CHANNEL | SEND_MESSAGES)];
    let perms = compute_permissions("owner", "u1", guild_id, &[], &roles, &[]);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, SEND_MESSAGES));
    assert!(!has_permission(perms, MANAGE_MESSAGES));
}

#[test]
fn member_roles_are_combined_with_everyone() {
    let guild_id = "g1";
    let roles = vec![
        role(guild_id, VIEW_CHANNEL),
        role("r1", SEND_MESSAGES),
        role("r2", CONNECT),
    ];
    let member_roles = vec!["r1".to_string(), "r2".to_string()];
    let perms = compute_permissions("owner", "u1", guild_id, &member_roles, &roles, &[]);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, SEND_MESSAGES));
    assert!(has_permission(perms, CONNECT));
    assert!(!has_permission(perms, SPEAK));
}

#[test]
fn everyone_channel_overwrite_applies_deny_then_allow() {
    let guild_id = "g1";
    let roles = vec![role(guild_id, VIEW_CHANNEL | SEND_MESSAGES)];
    let overwrites = vec![overwrite(
        guild_id,
        OverwriteType::Role,
        MANAGE_MESSAGES,
        SEND_MESSAGES,
    )];
    let perms = compute_permissions("owner", "u1", guild_id, &[], &roles, &overwrites);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(!has_permission(perms, SEND_MESSAGES));
    assert!(has_permission(perms, MANAGE_MESSAGES));
}

#[test]
fn role_overwrites_combine_allow_and_deny() {
    let guild_id = "g1";
    let roles = vec![
        role(guild_id, VIEW_CHANNEL),
        role("r1", 0),
        role("r2", 0),
    ];
    let member_roles = vec!["r1".to_string(), "r2".to_string()];
    let overwrites = vec![
        overwrite("r1", OverwriteType::Role, SEND_MESSAGES, 0),
        overwrite("r2", OverwriteType::Role, CONNECT, SEND_MESSAGES),
    ];
    let perms = compute_permissions("owner", "u1", guild_id, &member_roles, &roles, &overwrites);
    assert!(has_permission(perms, VIEW_CHANNEL));
    // Role denies are ORed then applied, role allows ORed then applied - allow wins over deny at same level
    assert!(has_permission(perms, SEND_MESSAGES));
    assert!(has_permission(perms, CONNECT));
}

#[test]
fn member_overwrite_overrides_role_overwrite() {
    let guild_id = "g1";
    let roles = vec![role(guild_id, VIEW_CHANNEL), role("r1", 0)];
    let member_roles = vec!["r1".to_string()];
    let overwrites = vec![
        overwrite("r1", OverwriteType::Role, 0, SEND_MESSAGES),
        overwrite("u1", OverwriteType::Member, SEND_MESSAGES, 0),
    ];
    let perms = compute_permissions("owner", "u1", guild_id, &member_roles, &roles, &overwrites);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, SEND_MESSAGES));
}

#[test]
fn channel_overwrites_ignore_roles_not_owned_by_member() {
    let guild_id = "g1";
    let roles = vec![role(guild_id, VIEW_CHANNEL), role("r1", 0), role("r2", 0)];
    let member_roles = vec!["r1".to_string()];
    let overwrites = vec![
        overwrite("r2", OverwriteType::Role, 0, VIEW_CHANNEL), // member doesn't have r2
        overwrite("r1", OverwriteType::Role, SEND_MESSAGES, 0),
    ];
    let perms = compute_permissions("owner", "u1", guild_id, &member_roles, &roles, &overwrites);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, SEND_MESSAGES));
}

#[test]
fn compute_base_permissions_without_channel_overwrites() {
    let guild_id = "g1";
    let roles = vec![
        role(guild_id, VIEW_CHANNEL),
        role("r1", SEND_MESSAGES | CONNECT),
    ];
    let perms = compute_base_permissions("owner", "u1", guild_id, &["r1".to_string()], &roles);
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, SEND_MESSAGES));
    assert!(has_permission(perms, CONNECT));
    assert!(!has_permission(perms, MANAGE_MESSAGES));
}

#[test]
fn administrator_in_base_permissions_returns_all() {
    let guild_id = "g1";
    let roles = vec![role(guild_id, 0), role("r1", ADMINISTRATOR)];
    let perms = compute_base_permissions("owner", "u1", guild_id, &["r1".to_string()], &roles);
    assert_eq!(perms, ALL_PERMISSIONS);
}

#[test]
fn has_permission_checks_flag_bits() {
    let perms = VIEW_CHANNEL | SEND_MESSAGES | CONNECT | SPEAK;
    assert!(has_permission(perms, VIEW_CHANNEL));
    assert!(has_permission(perms, SEND_MESSAGES));
    assert!(has_permission(perms, CONNECT));
    assert!(has_permission(perms, SPEAK));
    assert!(!has_permission(perms, MANAGE_MESSAGES));
    assert!(!has_permission(perms, ADMINISTRATOR));
}

#[test]
fn role_overwrite_deny_is_applied_before_allow_at_same_role_level() {
    // Discord: role overwrites are combined - deny ORed, allow ORed, then deny applied, then allow applied
    let guild_id = "g1";
    let roles = vec![role(guild_id, VIEW_CHANNEL), role("r1", 0)];
    let member_roles = vec!["r1".to_string()];
    let overwrites = vec![overwrite(
        "r1",
        OverwriteType::Role,
        SEND_MESSAGES,
        SEND_MESSAGES,
    )];
    let perms = compute_permissions("owner", "u1", guild_id, &member_roles, &roles, &overwrites);
    // allow wins over deny at same role-overwrite level
    assert!(has_permission(perms, SEND_MESSAGES));
}
