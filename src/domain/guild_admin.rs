//! Guild administration domain models: role hierarchy validation,
//! moderation payloads, invite management, and audit log parsing.
//!
//! Sources:
//! - Roles / hierarchy: https://discord.com/developers/docs/topics/permissions#permission-hierarchy
//! - Modify Guild Member (timeout): https://discord.com/developers/docs/resources/guild#modify-guild-member
//! - Create Guild Ban: https://discord.com/developers/docs/resources/guild#create-guild-ban
//! - Remove Guild Member: https://discord.com/developers/docs/resources/guild#remove-guild-member
//! - Invites: https://discord.com/developers/docs/resources/invite
//! - Audit Log: https://discord.com/developers/docs/resources/audit-log

use serde::{Deserialize, Serialize};

use crate::domain::guild::Role;
use crate::domain::hierarchy::compare_role_rank;

/// Error returned when a guild administration action violates role hierarchy rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleHierarchyError {
    /// The actor is not a guild member context we can evaluate.
    ActorNotMember,
    /// The target is not a guild member context we can evaluate.
    TargetNotMember,
    /// Actor cannot act on the guild owner.
    TargetIsOwner,
    /// Actor cannot act on themselves for this action.
    CannotActOnSelf,
    /// Actor's highest role is not strictly above target's highest role.
    TargetNotLower {
        actor_highest_position: i32,
        target_highest_position: i32,
    },
    /// Actor cannot manage a role at or above their own highest role.
    RoleNotLower {
        actor_highest_position: i32,
        role_position: i32,
    },
    /// The role being managed does not exist in the guild role set.
    RoleNotFound { role_id: String },
}

impl std::fmt::Display for RoleHierarchyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActorNotMember => write!(f, "actor is not a guild member"),
            Self::TargetNotMember => write!(f, "target is not a guild member"),
            Self::TargetIsOwner => write!(f, "cannot act on the guild owner"),
            Self::CannotActOnSelf => write!(f, "cannot act on yourself"),
            Self::TargetNotLower {
                actor_highest_position,
                target_highest_position,
            } => write!(
                f,
                "target role hierarchy is not lower (actor={}, target={})",
                actor_highest_position, target_highest_position
            ),
            Self::RoleNotLower {
                actor_highest_position,
                role_position,
            } => write!(
                f,
                "role is not below actor (actor={}, role={})",
                actor_highest_position, role_position
            ),
            Self::RoleNotFound { role_id } => write!(f, "role not found: {}", role_id),
        }
    }
}

impl std::error::Error for RoleHierarchyError {}

/// Context needed to validate guild role hierarchy for moderation/management actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildRoleHierarchy<'a> {
    pub guild_id: &'a str,
    pub owner_id: &'a str,
    pub roles: &'a [Role],
}

impl<'a> GuildRoleHierarchy<'a> {
    pub fn new(guild_id: &'a str, owner_id: &'a str, roles: &'a [Role]) -> Self {
        Self {
            guild_id,
            owner_id,
            roles,
        }
    }

    /// Returns the highest role from the provided role IDs, based on Discord role rank.
    /// Unknown role IDs are ignored. If none are known, falls back to @everyone if present.
    pub fn highest_role_for_member(&self, member_role_ids: &[String]) -> Option<&'a Role> {
        let mut highest: Option<&Role> = None;
        for role_id in member_role_ids {
            if let Some(role) = self.roles.iter().find(|r| &r.id == role_id) {
                highest = Some(match highest {
                    Some(current) if compare_role_rank(current, role).is_ge() => current,
                    _ => role,
                });
            }
        }
        if highest.is_some() {
            return highest;
        }
        self.roles.iter().find(|r| r.id == self.guild_id)
    }

    fn highest_position_for_member(&self, member_role_ids: &[String]) -> i32 {
        self.highest_role_for_member(member_role_ids)
            .map(|r| r.position)
            .unwrap_or(0)
    }

    /// Validates that `actor_id` can act on `target_id` (kick/ban/timeout).
    ///
    /// Discord rules:
    /// - Owner can act on anyone except themselves for self-actions where disallowed by caller.
    /// - Nobody can act on the owner.
    /// - Actor's highest role must be strictly higher than target's highest role.
    pub fn can_act_on_member(
        &self,
        actor_id: &str,
        actor_role_ids: &[String],
        target_id: &str,
        target_role_ids: &[String],
        allow_self: bool,
    ) -> Result<(), RoleHierarchyError> {
        if !allow_self && actor_id == target_id {
            return Err(RoleHierarchyError::CannotActOnSelf);
        }
        if target_id == self.owner_id {
            return Err(RoleHierarchyError::TargetIsOwner);
        }
        if actor_id == self.owner_id {
            return Ok(());
        }

        let actor_highest = self.highest_role_for_member(actor_role_ids);
        let target_highest = self.highest_role_for_member(target_role_ids);

        // If actor has no resolvable role context, they cannot outrank anyone.
        let Some(actor_highest) = actor_highest else {
            return Err(RoleHierarchyError::ActorNotMember);
        };
        let Some(target_highest) = target_highest else {
            return Err(RoleHierarchyError::TargetNotMember);
        };

        if compare_role_rank(actor_highest, target_highest).is_gt() {
            Ok(())
        } else {
            Err(RoleHierarchyError::TargetNotLower {
                actor_highest_position: actor_highest.position,
                target_highest_position: target_highest.position,
            })
        }
    }

    /// Validates that `actor_id` can manage (create/edit/assign) `role_id`.
    /// Actor must be owner or have highest role strictly above the target role.
    pub fn can_manage_role(
        &self,
        actor_id: &str,
        actor_role_ids: &[String],
        role_id: &str,
    ) -> Result<(), RoleHierarchyError> {
        if actor_id == self.owner_id {
            return Ok(());
        }
        let Some(target_role) = self.roles.iter().find(|r| r.id == role_id) else {
            return Err(RoleHierarchyError::RoleNotFound {
                role_id: role_id.to_string(),
            });
        };
        let Some(actor_highest) = self.highest_role_for_member(actor_role_ids) else {
            return Err(RoleHierarchyError::ActorNotMember);
        };
        if compare_role_rank(actor_highest, target_role).is_gt() {
            Ok(())
        } else {
            Err(RoleHierarchyError::RoleNotLower {
                actor_highest_position: actor_highest.position,
                role_position: target_role.position,
            })
        }
    }

    /// Convenience: returns actor highest role position (0 if unresolved).
    pub fn actor_highest_position(&self, actor_role_ids: &[String]) -> i32 {
        self.highest_position_for_member(actor_role_ids)
    }
}

/// Payload for PUT /guilds/{guild.id}/bans/{user.id}
/// https://discord.com/developers/docs/resources/guild#create-guild-ban-json-params
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CreateGuildBanPayload {
    /// Number of seconds to delete messages for, between 0 and 604800 (7 days).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_message_seconds: Option<u32>,
}

impl CreateGuildBanPayload {
    pub const MAX_DELETE_MESSAGE_SECONDS: u32 = 604_800;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_delete_message_seconds(mut self, seconds: u32) -> Self {
        self.delete_message_seconds = Some(seconds);
        self
    }

    pub fn validate(&self) -> Result<(), GuildAdminValidationError> {
        if let Some(seconds) = self.delete_message_seconds {
            if seconds > Self::MAX_DELETE_MESSAGE_SECONDS {
                return Err(GuildAdminValidationError::DeleteMessageSecondsOutOfRange {
                    value: seconds,
                    max: Self::MAX_DELETE_MESSAGE_SECONDS,
                });
            }
        }
        Ok(())
    }
}

/// Request metadata for DELETE /guilds/{guild.id}/members/{user.id}
/// Discord accepts an optional `X-Audit-Log-Reason` header; the body is empty.
/// https://discord.com/developers/docs/resources/guild#remove-guild-member
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KickGuildMemberRequest {
    pub guild_id: String,
    pub user_id: String,
    /// Optional audit log reason (sent via X-Audit-Log-Reason header).
    pub reason: Option<String>,
}

impl KickGuildMemberRequest {
    pub fn new(guild_id: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            guild_id: guild_id.into(),
            user_id: user_id.into(),
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        let reason = reason.into();
        self.reason = if reason.is_empty() { None } else { Some(reason) };
        self
    }

    pub fn validate(&self) -> Result<(), GuildAdminValidationError> {
        if self.guild_id.trim().is_empty() {
            return Err(GuildAdminValidationError::MissingField("guild_id"));
        }
        if self.user_id.trim().is_empty() {
            return Err(GuildAdminValidationError::MissingField("user_id"));
        }
        if let Some(reason) = &self.reason {
            if reason.chars().count() > AUDIT_LOG_REASON_MAX_CHARS {
                return Err(GuildAdminValidationError::ReasonTooLong {
                    len: reason.chars().count(),
                    max: AUDIT_LOG_REASON_MAX_CHARS,
                });
            }
        }
        Ok(())
    }
}

/// Payload for PATCH /guilds/{guild.id}/members/{user.id} when applying a communication timeout.
/// https://discord.com/developers/docs/resources/guild#modify-guild-member-json-params
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TimeoutGuildMemberPayload {
    /// ISO8601 timestamp until which the member is timed out; null to remove timeout.
    #[serde(default)]
    pub communication_disabled_until: Option<String>,
}

impl TimeoutGuildMemberPayload {
    pub fn set(until_iso8601: impl Into<String>) -> Self {
        Self {
            communication_disabled_until: Some(until_iso8601.into()),
        }
    }

    pub fn clear() -> Self {
        Self {
            communication_disabled_until: None,
        }
    }

    pub fn validate(&self) -> Result<(), GuildAdminValidationError> {
        if let Some(ts) = &self.communication_disabled_until {
            if ts.trim().is_empty() {
                return Err(GuildAdminValidationError::MissingField(
                    "communication_disabled_until",
                ));
            }
            // Basic ISO8601 shape check: expect 'T' and timezone or 'Z'.
            let has_t = ts.contains('T');
            let has_tz = ts.ends_with('Z') || ts.contains('+') || ts.rfind('-').is_some_and(|i| i > 10);
            if !has_t || !has_tz {
                return Err(GuildAdminValidationError::InvalidTimestamp(ts.clone()));
            }
        }
        Ok(())
    }

    pub fn is_clear(&self) -> bool {
        self.communication_disabled_until.is_none()
    }
}

pub const AUDIT_LOG_REASON_MAX_CHARS: usize = 512;

/// Validation errors for guild admin payloads/requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildAdminValidationError {
    DeleteMessageSecondsOutOfRange { value: u32, max: u32 },
    MissingField(&'static str),
    ReasonTooLong { len: usize, max: usize },
    InvalidTimestamp(String),
    MaxAgeOutOfRange { value: u32, max: u32 },
    MaxUsesOutOfRange { value: u8, max: u8 },
}

impl std::fmt::Display for GuildAdminValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeleteMessageSecondsOutOfRange { value, max } => write!(
                f,
                "delete_message_seconds {} out of range (0..={})",
                value, max
            ),
            Self::MissingField(field) => write!(f, "missing required field: {}", field),
            Self::ReasonTooLong { len, max } => {
                write!(f, "audit log reason length {} exceeds {}", len, max)
            }
            Self::InvalidTimestamp(ts) => write!(f, "invalid ISO8601 timestamp: {}", ts),
            Self::MaxAgeOutOfRange { value, max } => {
                write!(f, "max_age {} out of range (0..={})", value, max)
            }
            Self::MaxUsesOutOfRange { value, max } => {
                write!(f, "max_uses {} out of range (0..={})", value, max)
            }
        }
    }
}

impl std::error::Error for GuildAdminValidationError {}

/// Invite target type.
/// https://discord.com/developers/docs/resources/invite#invite-object-invite-target-types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum InviteTargetType {
    Stream = 1,
    EmbeddedApplication = 2,
}

impl TryFrom<u8> for InviteTargetType {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Stream),
            2 => Ok(Self::EmbeddedApplication),
            other => Err(format!("Unknown InviteTargetType: {other}")),
        }
    }
}

impl From<InviteTargetType> for u8 {
    fn from(value: InviteTargetType) -> Self {
        value as u8
    }
}

/// Invite type.
/// https://discord.com/developers/docs/resources/invite#invite-object-invite-types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum InviteType {
    Guild = 0,
    GroupDm = 1,
    Friend = 2,
}

impl TryFrom<u8> for InviteType {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Guild),
            1 => Ok(Self::GroupDm),
            2 => Ok(Self::Friend),
            other => Err(format!("Unknown InviteType: {other}")),
        }
    }
}

impl From<InviteType> for u8 {
    fn from(value: InviteType) -> Self {
        value as u8
    }
}

/// Payload for POST /channels/{channel.id}/invites
/// https://discord.com/developers/docs/resources/channel#create-channel-invite-json-params
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CreateChannelInvitePayload {
    /// Duration of invite in seconds before expiry, or 0 for never. 0 to 604800 (7 days). Default 86400 (24h).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u32>,
    /// Max number of uses or 0 for unlimited. 0 to 100. Default 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u8>,
    /// Whether this invite only grants temporary membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary: Option<bool>,
    /// If true, don't try to reuse a similar invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique: Option<bool>,
    /// The target type of this voice channel invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<InviteTargetType>,
    /// The id of the user whose stream to display for this invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_user_id: Option<String>,
    /// The id of the embedded application to open for this invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_application_id: Option<String>,
}

impl CreateChannelInvitePayload {
    pub const MAX_AGE_MAX_SECONDS: u32 = 604_800;
    pub const MAX_USES_MAX: u8 = 100;
    pub const DEFAULT_MAX_AGE_SECONDS: u32 = 86_400;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_age(mut self, max_age: u32) -> Self {
        self.max_age = Some(max_age);
        self
    }

    pub fn with_max_uses(mut self, max_uses: u8) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    pub fn with_temporary(mut self, temporary: bool) -> Self {
        self.temporary = Some(temporary);
        self
    }

    pub fn with_unique(mut self, unique: bool) -> Self {
        self.unique = Some(unique);
        self
    }

    pub fn with_target(mut self, target_type: InviteTargetType) -> Self {
        self.target_type = Some(target_type);
        self
    }

    pub fn validate(&self) -> Result<(), GuildAdminValidationError> {
        if let Some(max_age) = self.max_age {
            if max_age > Self::MAX_AGE_MAX_SECONDS {
                return Err(GuildAdminValidationError::MaxAgeOutOfRange {
                    value: max_age,
                    max: Self::MAX_AGE_MAX_SECONDS,
                });
            }
        }
        if let Some(max_uses) = self.max_uses {
            if max_uses > Self::MAX_USES_MAX {
                return Err(GuildAdminValidationError::MaxUsesOutOfRange {
                    value: max_uses,
                    max: Self::MAX_USES_MAX,
                });
            }
        }
        Ok(())
    }
}

/// Partial invite metadata as returned by Discord invite endpoints.
/// https://discord.com/developers/docs/resources/invite#invite-object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InviteMetadata {
    pub code: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub inviter_id: Option<String>,
    #[serde(default)]
    pub target_type: Option<InviteTargetType>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub uses: Option<u32>,
    #[serde(default)]
    pub max_uses: Option<u32>,
    #[serde(default)]
    pub max_age: Option<u32>,
    #[serde(default)]
    pub temporary: Option<bool>,
}

impl InviteMetadata {
    pub fn is_expired_at(&self, now_iso8601: &str) -> bool {
        match &self.expires_at {
            Some(exp) => exp.as_str() <= now_iso8601,
            None => false,
        }
    }

    pub fn is_exhausted(&self) -> bool {
        match (self.uses, self.max_uses) {
            (Some(uses), Some(max)) if max > 0 => uses >= max,
            _ => false,
        }
    }
}

/// Audit log entry action types (subset covering moderation + role + invite management).
/// https://discord.com/developers/docs/resources/audit-log#audit-log-entry-object-audit-log-events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
#[repr(u16)]
pub enum AuditLogActionType {
    GuildUpdate = 1,
    ChannelCreate = 10,
    ChannelUpdate = 11,
    ChannelDelete = 12,
    ChannelOverwriteCreate = 13,
    ChannelOverwriteUpdate = 14,
    ChannelOverwriteDelete = 15,
    MemberKick = 20,
    MemberPrune = 21,
    MemberBanAdd = 22,
    MemberBanRemove = 23,
    MemberUpdate = 24,
    MemberRoleUpdate = 25,
    MemberMove = 26,
    MemberDisconnect = 27,
    BotAdd = 28,
    RoleCreate = 30,
    RoleUpdate = 31,
    RoleDelete = 32,
    InviteCreate = 40,
    InviteUpdate = 41,
    InviteDelete = 42,
    MessageDelete = 72,
    MessageBulkDelete = 73,
    MessagePin = 74,
    MessageUnpin = 75,
    /// Any action type not modeled explicitly; preserves the raw code.
    Unknown(u16),
}

impl TryFrom<u16> for AuditLogActionType {
    type Error = std::convert::Infallible;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::GuildUpdate,
            10 => Self::ChannelCreate,
            11 => Self::ChannelUpdate,
            12 => Self::ChannelDelete,
            13 => Self::ChannelOverwriteCreate,
            14 => Self::ChannelOverwriteUpdate,
            15 => Self::ChannelOverwriteDelete,
            20 => Self::MemberKick,
            21 => Self::MemberPrune,
            22 => Self::MemberBanAdd,
            23 => Self::MemberBanRemove,
            24 => Self::MemberUpdate,
            25 => Self::MemberRoleUpdate,
            26 => Self::MemberMove,
            27 => Self::MemberDisconnect,
            28 => Self::BotAdd,
            30 => Self::RoleCreate,
            31 => Self::RoleUpdate,
            32 => Self::RoleDelete,
            40 => Self::InviteCreate,
            41 => Self::InviteUpdate,
            42 => Self::InviteDelete,
            72 => Self::MessageDelete,
            73 => Self::MessageBulkDelete,
            74 => Self::MessagePin,
            75 => Self::MessageUnpin,
            other => Self::Unknown(other),
        })
    }
}

impl From<AuditLogActionType> for u16 {
    fn from(value: AuditLogActionType) -> Self {
        match value {
            AuditLogActionType::GuildUpdate => 1,
            AuditLogActionType::ChannelCreate => 10,
            AuditLogActionType::ChannelUpdate => 11,
            AuditLogActionType::ChannelDelete => 12,
            AuditLogActionType::ChannelOverwriteCreate => 13,
            AuditLogActionType::ChannelOverwriteUpdate => 14,
            AuditLogActionType::ChannelOverwriteDelete => 15,
            AuditLogActionType::MemberKick => 20,
            AuditLogActionType::MemberPrune => 21,
            AuditLogActionType::MemberBanAdd => 22,
            AuditLogActionType::MemberBanRemove => 23,
            AuditLogActionType::MemberUpdate => 24,
            AuditLogActionType::MemberRoleUpdate => 25,
            AuditLogActionType::MemberMove => 26,
            AuditLogActionType::MemberDisconnect => 27,
            AuditLogActionType::BotAdd => 28,
            AuditLogActionType::RoleCreate => 30,
            AuditLogActionType::RoleUpdate => 31,
            AuditLogActionType::RoleDelete => 32,
            AuditLogActionType::InviteCreate => 40,
            AuditLogActionType::InviteUpdate => 41,
            AuditLogActionType::InviteDelete => 42,
            AuditLogActionType::MessageDelete => 72,
            AuditLogActionType::MessageBulkDelete => 73,
            AuditLogActionType::MessagePin => 74,
            AuditLogActionType::MessageUnpin => 75,
            AuditLogActionType::Unknown(other) => other,
        }
    }
}

/// Audit log change key/value.
/// https://discord.com/developers/docs/resources/audit-log#audit-log-change-object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AuditLogChange {
    /// New value for the key. Type depends on key.
    #[serde(default)]
    pub new_value: Option<serde_json::Value>,
    /// Old value for the key.
    #[serde(default)]
    pub old_value: Option<serde_json::Value>,
    /// Name of the changed entity field.
    pub key: String,
}

/// Optional extra info for specific audit log entry types.
/// https://discord.com/developers/docs/resources/audit-log#audit-log-entry-object-optional-audit-entry-info
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AuditLogEntryOptionalInfo {
    #[serde(default)]
    pub delete_member_days: Option<String>,
    #[serde(default)]
    pub members_removed: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub count: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub role_name: Option<String>,
}

/// A single audit log entry.
/// https://discord.com/developers/docs/resources/audit-log#audit-log-entry-object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// ID of the affected entity (snowflake or other).
    #[serde(default)]
    pub target_id: Option<String>,
    /// Changes made to the target.
    #[serde(default)]
    pub changes: Vec<AuditLogChange>,
    /// User who made the changes.
    #[serde(default)]
    pub user_id: Option<String>,
    /// ID of the entry.
    pub id: String,
    /// Type of action.
    pub action_type: AuditLogActionType,
    /// Optional extra info for certain action types.
    #[serde(default)]
    pub options: Option<AuditLogEntryOptionalInfo>,
    /// Reason for the change (0-512 chars).
    #[serde(default)]
    pub reason: Option<String>,
}

/// Audit log response container.
/// https://discord.com/developers/docs/resources/audit-log#audit-log-object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AuditLog {
    #[serde(default)]
    pub audit_log_entries: Vec<AuditLogEntry>,
    /// Partial integration objects (kept raw; out of scope for this model).
    #[serde(default)]
    pub integrations: Vec<serde_json::Value>,
    /// Threads found in audit log (kept raw).
    #[serde(default)]
    pub threads: Vec<serde_json::Value>,
    /// Users found in audit log (kept raw).
    #[serde(default)]
    pub users: Vec<serde_json::Value>,
    /// Webhooks found in audit log (kept raw).
    #[serde(default)]
    pub webhooks: Vec<serde_json::Value>,
}

impl AuditLog {
    /// Parse an audit log payload from Discord API JSON.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value.clone())
    }

    /// Find entries by action type.
    pub fn entries_by_action(&self, action: AuditLogActionType) -> Vec<&AuditLogEntry> {
        self.audit_log_entries
            .iter()
            .filter(|e| e.action_type == action)
            .collect()
    }

    /// Find entries targeting a specific entity ID.
    pub fn entries_for_target(&self, target_id: &str) -> Vec<&AuditLogEntry> {
        self.audit_log_entries
            .iter()
            .filter(|e| e.target_id.as_deref() == Some(target_id))
            .collect()
    }
}
