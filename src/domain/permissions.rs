use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

/// Bitwise permission flags (Discord official values).
pub const VIEW_CHANNEL: u64 = 1 << 10;
pub const SEND_MESSAGES: u64 = 1 << 11;
pub const MANAGE_MESSAGES: u64 = 1 << 13;
pub const ADD_REACTIONS: u64 = 1 << 6;
pub const ATTACH_FILES: u64 = 1 << 15;
pub const READ_MESSAGE_HISTORY: u64 = 1 << 16;
pub const EMBED_LINKS: u64 = 1 << 14;
pub const CONNECT: u64 = 1 << 20;
pub const SPEAK: u64 = 1 << 21;
pub const ADMINISTRATOR: u64 = 1 << 3;
pub const MANAGE_CHANNELS: u64 = 1 << 4;
pub const MANAGE_GUILD: u64 = 1 << 5;
pub const BYPASS_SLOWMODE: u64 = 1u64 << 52;

// Thread/voice permissions cleared while a member is timed out. Discord rejects
// timeouts for administrators and the guild owner, so only other members receive
// this mask after their normal permission calculation.
pub const CREATE_PUBLIC_THREADS: u64 = 1 << 35;
pub const CREATE_PRIVATE_THREADS: u64 = 1 << 36;
pub const SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;
pub const REQUEST_TO_SPEAK: u64 = 1 << 32;
pub const SEND_VOICE_MESSAGES: u64 = 1 << 46;

/// Permissions removed from a timed-out member.
pub const TIMEOUT_DENIED: u64 = SEND_MESSAGES
    | ADD_REACTIONS
    | SPEAK
    | CREATE_PUBLIC_THREADS
    | CREATE_PRIVATE_THREADS
    | SEND_MESSAGES_IN_THREADS
    | REQUEST_TO_SPEAK
    | SEND_VOICE_MESSAGES;

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

impl Serialize for PermissionOverwrite {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("PermissionOverwrite", 4)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field(
            "type",
            &match self.overwrite_type {
                OverwriteType::Role => 0u8,
                OverwriteType::Member => 1u8,
            },
        )?;
        state.serialize_field("allow", &self.allow.to_string())?;
        state.serialize_field("deny", &self.deny.to_string())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PermissionOverwrite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Bitfield {
            String(String),
            Number(u64),
        }

        impl Bitfield {
            fn value(&self) -> u64 {
                match self {
                    Bitfield::String(value) => parse_permissions(value),
                    Bitfield::Number(value) => *value,
                }
            }
        }

        #[derive(Deserialize)]
        struct RawOverwrite {
            id: String,
            #[serde(rename = "type", default)]
            overwrite_type: u8,
            #[serde(default)]
            allow: Option<Bitfield>,
            #[serde(default)]
            deny: Option<Bitfield>,
        }

        let raw = RawOverwrite::deserialize(deserializer)?;
        let overwrite_type = match raw.overwrite_type {
            0 => OverwriteType::Role,
            1 => OverwriteType::Member,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "Unknown permission overwrite type: {}",
                    other
                )));
            }
        };
        Ok(Self {
            id: raw.id,
            overwrite_type,
            allow: raw.allow.map(|value| value.value()).unwrap_or(0),
            deny: raw.deny.map(|value| value.value()).unwrap_or(0),
        })
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

/// Resolves whether a timed-out member should be treated as timed out at `now`.
/// Discord leaves expired timestamps in the member object, so the date must be
/// compared with the current instant instead of relying on the field being null.
pub fn is_timeout_active(value: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> bool {
    value
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|until| until.with_timezone(&chrono::Utc) > now)
        .unwrap_or(false)
}

/// `VIEW_CHANNEL` gates every other channel capability.
pub fn can_view_channel(permissions: u64) -> bool {
    has_permission(permissions, VIEW_CHANNEL)
}

/// Sending requires reading the channel first.
pub fn can_send_messages(permissions: u64) -> bool {
    can_view_channel(permissions) && has_permission(permissions, SEND_MESSAGES)
}

/// Reacting requires reading the channel and `ADD_REACTIONS`.
pub fn can_add_reactions(permissions: u64) -> bool {
    can_view_channel(permissions) && has_permission(permissions, ADD_REACTIONS)
}

/// Attaching files is implicitly denied when `SEND_MESSAGES` is denied.
pub fn can_attach_files(permissions: u64) -> bool {
    can_send_messages(permissions) && has_permission(permissions, ATTACH_FILES)
}

/// Slowmode bypass: ADMINISTRATOR/owner already have all permissions; the dedicated
/// BYPASS_SLOWMODE bit is also honored when present.
pub fn can_bypass_slowmode(permissions: u64) -> bool {
    has_permission(permissions, ADMINISTRATOR) || has_permission(permissions, BYPASS_SLOWMODE)
}

/// Joining voice requires `VIEW_CHANNEL` and `CONNECT`.
pub fn can_connect(permissions: u64) -> bool {
    can_view_channel(permissions) && has_permission(permissions, CONNECT)
}

/// Speaking requires joining the voice channel and `SPEAK`.
pub fn can_speak(permissions: u64) -> bool {
    can_connect(permissions) && has_permission(permissions, SPEAK)
}

/// Guild-level inputs used to derive the current member's channel permissions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildPermissionContext {
    pub guild_id: String,
    pub owner_id: String,
    pub user_id: String,
    pub member_role_ids: Vec<String>,
    pub roles: Vec<RolePermissions>,
    pub timeout_active: bool,
    /// False when the guild lacks role data (e.g. a bot-only REST snapshot).
    /// Callers must not hide channels from an unknown baseline.
    pub known: bool,
}

impl GuildPermissionContext {
    pub fn new(
        guild_id: impl Into<String>,
        owner_id: impl Into<String>,
        user_id: impl Into<String>,
        member_role_ids: Vec<String>,
        roles: Vec<RolePermissions>,
        timeout_active: bool,
    ) -> Self {
        let known = !roles.is_empty();
        Self {
            guild_id: guild_id.into(),
            owner_id: owner_id.into(),
            user_id: user_id.into(),
            member_role_ids,
            roles,
            timeout_active,
            known,
        }
    }

    /// Effective permissions for a guild channel, including its parent category
    /// overwrites (applied first) and the channel's own overwrites.
    pub fn channel_permissions(
        &self,
        channel_overwrites: &[PermissionOverwrite],
        parent_overwrites: &[PermissionOverwrite],
    ) -> u64 {
        compute_channel_permissions(
            &self.owner_id,
            &self.user_id,
            &self.guild_id,
            &self.member_role_ids,
            &self.roles,
            channel_overwrites,
            parent_overwrites,
            self.timeout_active,
        )
    }
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

/// Computes effective channel permissions including parent category overwrites
/// and an optional communication timeout.
///
/// Parent category overwrites are applied first, then the channel's own
/// overwrites, matching Discord's client behavior. Owner and administrator keep
/// `ALL_PERMISSIONS`; Discord rejects timeouts for both, so the timeout mask is
/// only applied to regular members.
pub fn compute_channel_permissions(
    guild_owner_id: &str,
    user_id: &str,
    guild_id: &str,
    member_role_ids: &[String],
    guild_roles: &[RolePermissions],
    channel_overwrites: &[PermissionOverwrite],
    parent_overwrites: &[PermissionOverwrite],
    timeout_active: bool,
) -> u64 {
    if user_id == guild_owner_id {
        return ALL_PERMISSIONS;
    }

    let base = compute_base_permissions(
        guild_owner_id,
        user_id,
        guild_id,
        member_role_ids,
        guild_roles,
    );
    if has_permission(base, ADMINISTRATOR) {
        return ALL_PERMISSIONS;
    }

    let with_parent = apply_channel_overwrites(
        base,
        guild_id,
        user_id,
        member_role_ids,
        parent_overwrites,
    );
    let mut permissions = apply_channel_overwrites(
        with_parent,
        guild_id,
        user_id,
        member_role_ids,
        channel_overwrites,
    );

    if timeout_active {
        permissions &= !TIMEOUT_DENIED;
    }

    permissions
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
