use std::collections::HashMap;

/// Bitwise permission flags (Discord official values).
pub const VIEW_CHANNEL: u64 = 1 << 10;
pub const SEND_MESSAGES: u64 = 1 << 11;
pub const MANAGE_MESSAGES: u64 = 1 << 13;
pub const CONNECT: u64 = 1 << 20;
pub const SPEAK: u64 = 1 << 21;
pub const ADMINISTRATOR: u64 = 1 << 3;

/// All permissions set. Used for owner/administrator.
pub const ALL_PERMISSIONS: u64 = u64::MAX;

/// Type of a permission overwrite target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteType {
    Role,
    Member,
}

/// A channel permission overwrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionOverwrite {
    pub id: String,
    pub overwrite_type: OverwriteType,
    pub allow: u64,
    pub deny: u64,
}

impl PermissionOverwrite {
    pub fn new(
        id: impl Into<String>,
        overwrite_type: OverwriteType,
        allow: u64,
        deny: u64,
    ) -> Self {
        Self {
            id: id.into(),
            overwrite_type,
            allow,
            deny,
        }
    }
}

/// Minimal role permission data needed for computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolePermissions {
    pub id: String,
    pub permissions: u64,
}

impl RolePermissions {
    pub fn new(id: impl Into<String>, permissions: u64) -> Self {
        Self {
            id: id.into(),
            permissions,
        }
    }
}

/// Parses a Discord permission bitfield string into u64.
/// Returns 0 for invalid/empty values.
pub fn parse_permissions(value: &str) -> u64 {
    value.parse::<u64>().unwrap_or(0)
}

/// Returns true if `permissions` contains all bits in `flag`.
pub fn has_permission(permissions: u64, flag: u64) -> bool {
    if flag == 0 {
        return true;
    }
    permissions & flag == flag
}

/// Computes effective channel permissions using Discord's official algorithm.
///
/// Algorithm:
/// 1. Owner gets ALL_PERMISSIONS.
/// 2. Base = @everyone role permissions.
/// 3. OR in every member role permission.
/// 4. If ADMINISTRATOR is present, return ALL_PERMISSIONS.
/// 5. Apply @everyone channel overwrite (deny then allow).
/// 6. Apply combined role channel overwrites (deny OR, allow OR, then deny then allow).
/// 7. Apply member channel overwrite (deny then allow).
/// 8. If ADMINISTRATOR is present at any point where relevant, ADMINISTRATOR bypasses overwrites;
///    this implementation returns ALL_PERMISSIONS when ADMINISTRATOR is in base permissions.
pub fn compute_permissions(
    guild_owner_id: &str,
    user_id: &str,
    guild_id: &str,
    member_role_ids: &[String],
    guild_roles: &[RolePermissions],
    channel_overwrites: &[PermissionOverwrite],
) -> u64 {
    if user_id == guild_owner_id {
        return ALL_PERMISSIONS;
    }

    let role_map: HashMap<&str, u64> = guild_roles
        .iter()
        .map(|r| (r.id.as_str(), r.permissions))
        .collect();

    // Base: @everyone role
    let mut permissions = role_map.get(guild_id).copied().unwrap_or(0);

    // Apply member roles
    for role_id in member_role_ids {
        if let Some(role_perms) = role_map.get(role_id.as_str()) {
            permissions |= *role_perms;
        }
    }

    // Administrator bypasses channel overwrites
    if has_permission(permissions, ADMINISTRATOR) {
        return ALL_PERMISSIONS;
    }

    // Channel overwrites
    apply_channel_overwrites(permissions, guild_id, user_id, member_role_ids, channel_overwrites)
}

/// Computes base guild permissions (no channel overwrites).
/// Owner and ADMINISTRATOR return ALL_PERMISSIONS.
pub fn compute_base_permissions(
    guild_owner_id: &str,
    user_id: &str,
    guild_id: &str,
    member_role_ids: &[String],
    guild_roles: &[RolePermissions],
) -> u64 {
    if user_id == guild_owner_id {
        return ALL_PERMISSIONS;
    }

    let role_map: HashMap<&str, u64> = guild_roles
        .iter()
        .map(|r| (r.id.as_str(), r.permissions))
        .collect();

    let mut permissions = role_map.get(guild_id).copied().unwrap_or(0);
    for role_id in member_role_ids {
        if let Some(role_perms) = role_map.get(role_id.as_str()) {
            permissions |= *role_perms;
        }
    }

    if has_permission(permissions, ADMINISTRATOR) {
        ALL_PERMISSIONS
    } else {
        permissions
    }
}

fn apply_channel_overwrites(
    mut permissions: u64,
    guild_id: &str,
    user_id: &str,
    member_role_ids: &[String],
    channel_overwrites: &[PermissionOverwrite],
) -> u64 {
    // @everyone overwrite
    if let Some(overwrite) = channel_overwrites
        .iter()
        .find(|o| o.overwrite_type == OverwriteType::Role && o.id == guild_id)
    {
        permissions &= !overwrite.deny;
        permissions |= overwrite.allow;
    }

    // Role overwrites (combined)
    let mut role_allow: u64 = 0;
    let mut role_deny: u64 = 0;
    let mut has_role_overwrite = false;
    for overwrite in channel_overwrites.iter().filter(|o| {
        o.overwrite_type == OverwriteType::Role
            && o.id != guild_id
            && member_role_ids.iter().any(|rid| rid == &o.id)
    }) {
        role_allow |= overwrite.allow;
        role_deny |= overwrite.deny;
        has_role_overwrite = true;
    }
    if has_role_overwrite {
        permissions &= !role_deny;
        permissions |= role_allow;
    }

    // Member overwrite
    if let Some(overwrite) = channel_overwrites
        .iter()
        .find(|o| o.overwrite_type == OverwriteType::Member && o.id == user_id)
    {
        permissions &= !overwrite.deny;
        permissions |= overwrite.allow;
    }

    permissions
}
