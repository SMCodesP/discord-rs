use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

pub const AGE_RESTRICTED_CHANNEL_PROMPT: &str =
    "This channel contains adult content marked as age-restricted. Do you wish to proceed?";
pub const AGE_RESTRICTED_GUILD_PROMPT: &str =
    "This server contains adult content marked as age-restricted. Do you wish to proceed?";
pub const AGE_RESTRICTED_GUILD_BLOCKED: &str =
    "You must be over legal age in order to view age-restricted servers";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgeRestrictionKind {
    Channel,
    Guild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NsfwLevel {
    #[default]
    Default,
    Explicit,
    Safe,
    AgeRestricted,
}

impl NsfwLevel {
    pub fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Default),
            1 => Some(Self::Explicit),
            2 => Some(Self::Safe),
            3 => Some(Self::AgeRestricted),
            _ => None,
        }
    }

    pub fn code(self) -> u64 {
        match self {
            Self::Default => 0,
            Self::Explicit => 1,
            Self::Safe => 2,
            Self::AgeRestricted => 3,
        }
    }

    pub fn is_age_restricted(self) -> bool {
        matches!(self, Self::AgeRestricted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgeRestriction {
    pub channel_nsfw: bool,
    pub guild_nsfw: bool,
    pub guild_nsfw_level: NsfwLevel,
}

impl AgeRestriction {
    pub fn new(channel_nsfw: bool, guild_nsfw: bool, guild_nsfw_level: NsfwLevel) -> Self {
        Self {
            channel_nsfw,
            guild_nsfw,
            guild_nsfw_level,
        }
    }

    pub fn from_channel_json(channel: &Value) -> Self {
        Self {
            channel_nsfw: read_bool(channel, "nsfw"),
            guild_nsfw: false,
            guild_nsfw_level: NsfwLevel::Default,
        }
    }

    pub fn from_guild_json(guild: &Value) -> Self {
        Self {
            channel_nsfw: false,
            guild_nsfw: read_bool(guild, "nsfw"),
            guild_nsfw_level: read_nsfw_level(guild),
        }
    }

    pub fn from_channel_and_guild_json(channel: &Value, guild: Option<&Value>) -> Self {
        let mut restriction = Self::from_channel_json(channel);
        if let Some(guild) = guild {
            let guild_restriction = Self::from_guild_json(guild);
            restriction.guild_nsfw = guild_restriction.guild_nsfw;
            restriction.guild_nsfw_level = guild_restriction.guild_nsfw_level;
        }
        restriction
    }

    pub fn with_guild_json(mut self, guild: &Value) -> Self {
        let guild_restriction = Self::from_guild_json(guild);
        self.guild_nsfw = guild_restriction.guild_nsfw;
        self.guild_nsfw_level = guild_restriction.guild_nsfw_level;
        self
    }

    pub fn guild_restricted(&self) -> bool {
        self.guild_nsfw || self.guild_nsfw_level.is_age_restricted()
    }

    pub fn channel_restricted(&self) -> bool {
        self.channel_nsfw
    }

    pub fn is_restricted(&self) -> bool {
        self.guild_restricted() || self.channel_restricted()
    }

    pub fn first_restriction(&self) -> Option<AgeRestrictionKind> {
        if self.guild_restricted() {
            Some(AgeRestrictionKind::Guild)
        } else if self.channel_restricted() {
            Some(AgeRestrictionKind::Channel)
        } else {
            None
        }
    }

    pub fn prompt(&self) -> Option<&'static str> {
        self.first_restriction().map(prompt_for)
    }
}

fn prompt_for(kind: AgeRestrictionKind) -> &'static str {
    match kind {
        AgeRestrictionKind::Channel => AGE_RESTRICTED_CHANNEL_PROMPT,
        AgeRestrictionKind::Guild => AGE_RESTRICTED_GUILD_PROMPT,
    }
}

fn read_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn read_nsfw_level(value: &Value) -> NsfwLevel {
    let Some(raw) = value.get("nsfw_level") else {
        return NsfwLevel::Default;
    };
    let code = raw
        .as_u64()
        .or_else(|| raw.as_str().and_then(|text| text.parse::<u64>().ok()));
    code.and_then(NsfwLevel::from_code).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgeGateRequest {
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub restriction: AgeRestriction,
}

impl AgeGateRequest {
    pub fn new(
        channel_id: Option<impl Into<String>>,
        guild_id: Option<impl Into<String>>,
        restriction: AgeRestriction,
    ) -> Self {
        Self {
            channel_id: channel_id.map(Into::into),
            guild_id: guild_id.map(Into::into),
            restriction,
        }
    }

    pub fn from_channel_json(channel: &Value) -> Option<Self> {
        let channel_id = channel.get("id")?.as_str()?.to_string();
        let guild_id = channel
            .get("guild_id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        Some(Self {
            channel_id: Some(channel_id),
            guild_id,
            restriction: AgeRestriction::from_channel_json(channel),
        })
    }

    pub fn from_guild_json(guild: &Value) -> Option<Self> {
        let guild_id = guild.get("id")?.as_str()?.to_string();
        Some(Self {
            channel_id: None,
            guild_id: Some(guild_id),
            restriction: AgeRestriction::from_guild_json(guild),
        })
    }

    pub fn from_channel_and_guild_json(channel: &Value, guild: Option<&Value>) -> Option<Self> {
        let channel_id = channel.get("id")?.as_str()?.to_string();
        let guild_id = guild
            .and_then(|value| value.get("id"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| {
                channel
                    .get("guild_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            });
        Some(Self {
            channel_id: Some(channel_id),
            guild_id,
            restriction: AgeRestriction::from_channel_and_guild_json(channel, guild),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgeGatePreference {
    #[default]
    Ask,
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeGateStatus {
    NotRequired,
    Pending(AgeRestrictionKind),
    Allowed,
    Denied(AgeRestrictionKind),
}

impl AgeGateStatus {
    pub fn can_render(self) -> bool {
        matches!(self, Self::NotRequired | Self::Allowed)
    }

    pub fn prompt(self) -> Option<&'static str> {
        match self {
            Self::Pending(kind) => Some(prompt_for(kind)),
            _ => None,
        }
    }

    pub fn denial_message(self) -> Option<&'static str> {
        match self {
            Self::Denied(AgeRestrictionKind::Guild) => Some(AGE_RESTRICTED_GUILD_BLOCKED),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AgeGateScope {
    Channel,
    Guild,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AgeGateKey {
    scope: AgeGateScope,
    id: String,
}

impl AgeGateKey {
    fn channel(id: &str) -> Self {
        Self {
            scope: AgeGateScope::Channel,
            id: id.to_string(),
        }
    }

    fn guild(id: &str) -> Self {
        Self {
            scope: AgeGateScope::Guild,
            id: id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgeGateDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgeGateSession {
    account_id: Option<String>,
    preference: AgeGatePreference,
    decisions: HashMap<AgeGateKey, AgeGateDecision>,
}

impl Default for AgeGateSession {
    fn default() -> Self {
        Self {
            account_id: None,
            preference: AgeGatePreference::Ask,
            decisions: HashMap::new(),
        }
    }
}

impl AgeGateSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_account(account_id: impl Into<String>) -> Self {
        Self {
            account_id: Some(account_id.into()),
            ..Self::default()
        }
    }

    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    pub fn preference(&self) -> AgeGatePreference {
        self.preference
    }

    pub fn set_preference(&mut self, preference: AgeGatePreference) {
        self.preference = preference;
    }

    pub fn set_account(&mut self, account_id: Option<String>) {
        if self.account_id != account_id {
            self.account_id = account_id;
            self.decisions.clear();
        }
    }

    pub fn clear_decisions(&mut self) {
        self.decisions.clear();
    }

    pub fn decision(
        &self,
        kind: AgeRestrictionKind,
        id: &str,
    ) -> Option<AgeGateDecision> {
        let key = match kind {
            AgeRestrictionKind::Channel => AgeGateKey::channel(id),
            AgeRestrictionKind::Guild => AgeGateKey::guild(id),
        };
        self.decisions.get(&key).copied()
    }

    pub fn remember(
        &mut self,
        kind: AgeRestrictionKind,
        id: impl Into<String>,
        decision: AgeGateDecision,
    ) {
        let key = match kind {
            AgeRestrictionKind::Channel => AgeGateKey::channel(&id.into()),
            AgeRestrictionKind::Guild => AgeGateKey::guild(&id.into()),
        };
        self.decisions.insert(key, decision);
    }

    pub fn status(&self, request: &AgeGateRequest) -> AgeGateStatus {
        let restriction = &request.restriction;
        if !restriction.is_restricted() {
            return AgeGateStatus::NotRequired;
        }
        match self.preference {
            AgeGatePreference::Allow => return AgeGateStatus::Allowed,
            AgeGatePreference::Deny => {
                return AgeGateStatus::Denied(
                    restriction
                        .first_restriction()
                        .unwrap_or(AgeRestrictionKind::Channel),
                );
            }
            AgeGatePreference::Ask => {}
        }
        if restriction.guild_restricted() {
            match request
                .guild_id
                .as_deref()
                .and_then(|id| self.decision(AgeRestrictionKind::Guild, id))
            {
                Some(AgeGateDecision::Deny) => {
                    return AgeGateStatus::Denied(AgeRestrictionKind::Guild);
                }
                Some(AgeGateDecision::Allow) => {}
                None => return AgeGateStatus::Pending(AgeRestrictionKind::Guild),
            }
        }
        if restriction.channel_restricted() {
            match request
                .channel_id
                .as_deref()
                .and_then(|id| self.decision(AgeRestrictionKind::Channel, id))
            {
                Some(AgeGateDecision::Deny) => {
                    return AgeGateStatus::Denied(AgeRestrictionKind::Channel);
                }
                Some(AgeGateDecision::Allow) => {}
                None => return AgeGateStatus::Pending(AgeRestrictionKind::Channel),
            }
        }
        AgeGateStatus::Allowed
    }

    pub fn can_render(&self, request: &AgeGateRequest) -> bool {
        self.status(request).can_render()
    }

    pub fn confirm(&mut self, request: &AgeGateRequest) -> bool {
        match self.status(request) {
            AgeGateStatus::Pending(AgeRestrictionKind::Guild) => {
                let Some(id) = request.guild_id.clone() else {
                    return false;
                };
                self.remember(AgeRestrictionKind::Guild, id, AgeGateDecision::Allow);
                true
            }
            AgeGateStatus::Pending(AgeRestrictionKind::Channel) => {
                let Some(id) = request.channel_id.clone() else {
                    return false;
                };
                self.remember(AgeRestrictionKind::Channel, id, AgeGateDecision::Allow);
                true
            }
            AgeGateStatus::Allowed | AgeGateStatus::NotRequired => true,
            AgeGateStatus::Denied(_) => false,
        }
    }

    pub fn deny(&mut self, request: &AgeGateRequest) -> bool {
        match self.status(request) {
            AgeGateStatus::Pending(AgeRestrictionKind::Guild) => {
                let Some(id) = request.guild_id.clone() else {
                    return false;
                };
                self.remember(AgeRestrictionKind::Guild, id, AgeGateDecision::Deny);
                true
            }
            AgeGateStatus::Pending(AgeRestrictionKind::Channel) => {
                let Some(id) = request.channel_id.clone() else {
                    return false;
                };
                self.remember(AgeRestrictionKind::Channel, id, AgeGateDecision::Deny);
                true
            }
            AgeGateStatus::Denied(_) => true,
            AgeGateStatus::Allowed | AgeGateStatus::NotRequired => false,
        }
    }

    pub fn retry_after_denial(&mut self, request: &AgeGateRequest) -> bool {
        match self.status(request) {
            AgeGateStatus::Denied(AgeRestrictionKind::Guild) => {
                let Some(id) = &request.guild_id else {
                    return false;
                };
                self.decisions.remove(&AgeGateKey::guild(id));
                true
            }
            AgeGateStatus::Denied(AgeRestrictionKind::Channel) => {
                let Some(id) = &request.channel_id else {
                    return false;
                };
                self.decisions.remove(&AgeGateKey::channel(id));
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAction {
    SendMessage,
    AddReaction,
    StartTyping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutParseError {
    value: String,
}

impl TimeoutParseError {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for TimeoutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid communication_disabled_until timestamp: {}",
            self.value
        )
    }
}

impl Error for TimeoutParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutState {
    NotTimedOut,
    Active {
        until: DateTime<Utc>,
        remaining: chrono::Duration,
    },
    Expired {
        until: DateTime<Utc>,
    },
}

impl TimeoutState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Active { .. })
    }

    pub fn until(self) -> Option<DateTime<Utc>> {
        match self {
            Self::NotTimedOut => None,
            Self::Active { until, .. } | Self::Expired { until } => Some(until),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutWarning {
    pub until: DateTime<Utc>,
    pub remaining: chrono::Duration,
}

impl TimeoutWarning {
    pub fn message(&self) -> String {
        format!("You are timed out until {}.", self.until.to_rfc3339())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberTimeout {
    until: Option<DateTime<Utc>>,
}

impl MemberTimeout {
    pub fn none() -> Self {
        Self { until: None }
    }

    pub fn until(&self) -> Option<DateTime<Utc>> {
        self.until
    }

    pub fn parse(raw: Option<&str>) -> Result<Self, TimeoutParseError> {
        match raw {
            None => Ok(Self::none()),
            Some(value) => parse_timeout(value).map(|until| Self { until: Some(until) }),
        }
    }

    pub fn from_member_json(member: &Value) -> Result<Self, TimeoutParseError> {
        match member.get("communication_disabled_until") {
            None | Some(Value::Null) => Ok(Self::none()),
            Some(Value::String(value)) => Self::parse(Some(value)),
            Some(other) => Err(TimeoutParseError::new(other.to_string())),
        }
    }

    pub fn update_raw(&mut self, raw: Option<&str>) -> Result<(), TimeoutParseError> {
        let parsed = Self::parse(raw)?;
        self.until = parsed.until;
        Ok(())
    }

    pub fn update_from_member_json(&mut self, member: &Value) -> Result<(), TimeoutParseError> {
        let parsed = Self::from_member_json(member)?;
        self.until = parsed.until;
        Ok(())
    }

    pub fn state(&self, now: DateTime<Utc>) -> TimeoutState {
        match self.until {
            None => TimeoutState::NotTimedOut,
            Some(until) if now < until => TimeoutState::Active {
                until,
                remaining: until.signed_duration_since(now),
            },
            Some(until) => TimeoutState::Expired { until },
        }
    }

    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.state(now).is_active()
    }

    pub fn remaining(&self, now: DateTime<Utc>) -> Option<chrono::Duration> {
        match self.state(now) {
            TimeoutState::Active { remaining, .. } => Some(remaining),
            _ => None,
        }
    }

    pub fn warning(&self, now: DateTime<Utc>) -> Option<TimeoutWarning> {
        match self.state(now) {
            TimeoutState::Active { until, remaining } => Some(TimeoutWarning { until, remaining }),
            _ => None,
        }
    }

    pub fn clear_if_expired(&mut self, now: DateTime<Utc>) -> bool {
        match self.until {
            Some(until) if now >= until => {
                self.until = None;
                true
            }
            _ => false,
        }
    }
}

fn parse_timeout(value: &str) -> Result<DateTime<Utc>, TimeoutParseError> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|_| TimeoutParseError::new(value))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardBlock {
    AgeGate(AgeGateStatus),
    TimedOut(TimeoutWarning),
}

impl GuardBlock {
    pub fn message(&self) -> Option<String> {
        match self {
            Self::AgeGate(status) => status
                .prompt()
                .or_else(|| status.denial_message())
                .map(ToString::to_string),
            Self::TimedOut(warning) => Some(warning.message()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionGuard {
    Allowed,
    Blocked(GuardBlock),
}

impl ActionGuard {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn block(&self) -> Option<&GuardBlock> {
        match self {
            Self::Allowed => None,
            Self::Blocked(block) => Some(block),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelGuard {
    pub age: AgeGateSession,
    pub timeout: MemberTimeout,
}

impl ChannelGuard {
    pub fn new(age: AgeGateSession, timeout: MemberTimeout) -> Self {
        Self { age, timeout }
    }

    pub fn can_render(&self, request: &AgeGateRequest) -> bool {
        self.age.can_render(request)
    }

    pub fn check(
        &self,
        _action: ChatAction,
        request: &AgeGateRequest,
        now: DateTime<Utc>,
    ) -> ActionGuard {
        let age_status = self.age.status(request);
        if !age_status.can_render() {
            return ActionGuard::Blocked(GuardBlock::AgeGate(age_status));
        }
        if let Some(warning) = self.timeout.warning(now) {
            return ActionGuard::Blocked(GuardBlock::TimedOut(warning));
        }
        ActionGuard::Allowed
    }
}
