use std::collections::HashSet;
use std::fmt;
use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use gpui::*;
use regex::Regex;
use serde_json::Value;

use crate::domain::channel::ChannelType;
use crate::protocol::cdn;
use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

pub const GET_INVITE_PATH_TEMPLATE: &str = "/invites/{code}";
pub const ACCEPT_INVITE_PATH_TEMPLATE: &str = "/invites/{code}";
pub const INVITE_CARD_WIDTH: f32 = 300.0;
pub const INVITE_CARD_SPLASH_HEIGHT: f32 = 64.0;

static RE_INVITE_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:^|[\s\(\[\{<"'])((?:https?://)?(?:[a-z0-9-]+\.)*(?:discord\.gg|discord(?:app)?\.com)/[^\s<>\(\)\[\]\{\}"'`]+)"#,
    )
    .expect("invalid invite url regex")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InviteDomain {
    DiscordGg,
    DiscordCom,
    DiscordApp,
}

impl InviteDomain {
    pub fn canonical_host(self) -> &'static str {
        match self {
            Self::DiscordGg => "discord.gg",
            Self::DiscordCom => "discord.com",
            Self::DiscordApp => "discordapp.com",
        }
    }

    pub fn as_str(self) -> &'static str {
        self.canonical_host()
    }
}

impl fmt::Display for InviteDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_host())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteLink {
    pub code: String,
    pub host: String,
    pub domain: InviteDomain,
    pub raw: String,
}

impl InviteLink {
    pub fn canonical_url(&self) -> String {
        format!("https://{}/{}", self.domain.canonical_host(), self.code)
    }

    pub fn api_path(&self) -> String {
        invite_api_path(&self.code).unwrap_or_default()
    }
}

fn strip_scheme(value: &str) -> &str {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("https://") {
        &value[8..]
    } else if lower.starts_with("http://") {
        &value[7..]
    } else {
        value
    }
}

fn trim_url_tail(value: &str) -> &str {
    value.trim().trim_end_matches(|c: char| {
        matches!(
            c,
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
        )
    })
}

fn normalize_invite_code(value: &str) -> Option<String> {
    let code = value.trim().trim_matches(|c: char| {
        matches!(
            c,
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
        )
    });
    if is_valid_invite_code(code) {
        Some(code.to_string())
    } else {
        None
    }
}

pub fn is_valid_invite_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 64
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn classify_host(host: &str) -> Option<InviteDomain> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    match host.as_str() {
        "discord.gg" | "www.discord.gg" => Some(InviteDomain::DiscordGg),
        "discord.com" | "www.discord.com" | "ptb.discord.com" | "canary.discord.com" => {
            Some(InviteDomain::DiscordCom)
        }
        "discordapp.com" | "www.discordapp.com" => Some(InviteDomain::DiscordApp),
        _ => None,
    }
}

pub fn parse_invite_url(raw: &str) -> Option<InviteLink> {
    let trimmed = trim_url_tail(raw);
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = strip_scheme(trimmed);
    let (host, path) = without_scheme.split_once('/')?;
    if host.is_empty() {
        return None;
    }
    let domain = classify_host(host)?;
    let path = path.split(|c| c == '?' || c == '#').next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let code = match domain {
        InviteDomain::DiscordGg => {
            if segments.len() != 1 {
                return None;
            }
            normalize_invite_code(segments[0])?
        }
        InviteDomain::DiscordCom | InviteDomain::DiscordApp => {
            if segments.len() != 2 || !segments[0].eq_ignore_ascii_case("invite") {
                return None;
            }
            normalize_invite_code(segments[1])?
        }
    };

    Some(InviteLink {
        code,
        host: host.trim_end_matches('.').to_ascii_lowercase(),
        domain,
        raw: trimmed.to_string(),
    })
}

pub fn is_invite_url(raw: &str) -> bool {
    parse_invite_url(raw).is_some()
}

pub fn extract_invite_links(text: &str) -> Vec<InviteLink> {
    let mut seen = HashSet::new();
    let mut links = Vec::new();
    for capture in RE_INVITE_URL.captures_iter(text) {
        let Some(candidate) = capture.get(1) else {
            continue;
        };
        let Some(link) = parse_invite_url(candidate.as_str()) else {
            continue;
        };
        if seen.insert(link.code.clone()) {
            links.push(link);
        }
    }
    links
}

pub fn extract_first_invite_link(text: &str) -> Option<InviteLink> {
    extract_invite_links(text).into_iter().next()
}

pub fn extract_invite_code(text: &str) -> Option<String> {
    extract_first_invite_link(text).map(|link| link.code)
}

pub fn invite_api_path(code: &str) -> Option<String> {
    if is_valid_invite_code(code) {
        Some(format!("/invites/{code}"))
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InviteType {
    #[default]
    Guild,
    GroupDm,
    Friend,
    Unknown(u8),
}

impl From<u8> for InviteType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Guild,
            1 => Self::GroupDm,
            2 => Self::Friend,
            other => Self::Unknown(other),
        }
    }
}

impl From<InviteType> for u8 {
    fn from(value: InviteType) -> Self {
        match value {
            InviteType::Guild => 0,
            InviteType::GroupDm => 1,
            InviteType::Friend => 2,
            InviteType::Unknown(v) => v,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InviteGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub splash: Option<String>,
    pub description: Option<String>,
    pub features: Vec<String>,
    pub verification_level: Option<u8>,
    pub nsfw: bool,
    pub premium_tier: Option<u8>,
    pub premium_subscription_count: Option<u64>,
}

impl InviteGuild {
    pub fn icon_url(&self, size: u16) -> Option<String> {
        cdn::guild_icon_url(&self.id, self.icon.as_deref(), size)
    }

    pub fn splash_url(&self, size: u16) -> Option<String> {
        let splash = self.splash.as_deref()?.trim();
        if splash.is_empty() {
            return None;
        }
        let lower = splash.to_ascii_lowercase();
        if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("data:")
            || lower.starts_with("file:")
        {
            return Some(splash.to_string());
        }
        let hash = strip_known_image_extension(splash);
        Some(format!(
            "{}/splashes/{}/{}.png?size={size}",
            cdn::CDN_BASE,
            self.id,
            hash
        ))
    }
}

fn strip_known_image_extension(hash: &str) -> &str {
    for extension in [".png", ".gif", ".webp", ".jpg", ".jpeg", ".avif"] {
        if hash.len() > extension.len() && hash.to_ascii_lowercase().ends_with(extension) {
            return &hash[..hash.len() - extension.len()];
        }
    }
    hash
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InviteChannel {
    pub id: String,
    pub name: Option<String>,
    pub channel_type: Option<u64>,
}

impl InviteChannel {
    pub fn channel_type_enum(&self) -> Option<ChannelType> {
        self.channel_type.and_then(ChannelType::from_discord_code)
    }

    pub fn label(&self) -> Option<String> {
        let name = self.name.as_deref()?.trim();
        if name.is_empty() {
            return None;
        }
        match self.channel_type_enum() {
            Some(kind) => {
                let symbol = kind.symbol();
                if symbol.is_empty() {
                    Some(name.to_string())
                } else {
                    Some(format!("{symbol}{name}"))
                }
            }
            None => Some(format!("#{name}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InviteInviter {
    pub id: String,
    pub username: String,
    pub discriminator: Option<String>,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub bot: bool,
}

impl InviteInviter {
    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.username)
    }

    pub fn avatar_url(&self, size: u16) -> Option<String> {
        cdn::avatar_url(&self.id, self.avatar.as_deref(), size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Invite {
    pub code: String,
    pub guild: Option<InviteGuild>,
    pub channel: Option<InviteChannel>,
    pub inviter: Option<InviteInviter>,
    pub approximate_member_count: Option<u64>,
    pub approximate_presence_count: Option<u64>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub temporary: bool,
    pub uses: Option<u64>,
    pub max_uses: Option<u64>,
    pub invite_type: InviteType,
    pub flags: Option<u64>,
}

impl Invite {
    pub fn title(&self) -> String {
        if let Some(guild) = &self.guild {
            let name = guild.name.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if let Some(inviter) = &self.inviter {
            let name = inviter.display_name().trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if let Some(channel) = &self.channel {
            if let Some(name) = channel.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                return name.to_string();
            }
        }
        self.code.clone()
    }

    pub fn description(&self) -> Option<&str> {
        self.guild
            .as_ref()?
            .description
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    }

    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|expires| now >= expires)
    }

    pub fn is_exhausted(&self) -> bool {
        match (self.uses, self.max_uses) {
            (Some(uses), Some(max_uses)) if max_uses > 0 => uses >= max_uses,
            _ => false,
        }
    }

    pub fn availability_at(&self, now: DateTime<Utc>) -> InviteAvailability {
        if self.is_expired_at(now) {
            InviteAvailability::Expired
        } else if self.is_exhausted() {
            InviteAvailability::Exhausted
        } else {
            InviteAvailability::Available
        }
    }

    pub fn usage_label(&self) -> Option<String> {
        match (self.uses, self.max_uses) {
            (Some(uses), Some(max_uses)) if max_uses > 0 => {
                Some(format!("{uses}/{max_uses} uses"))
            }
            (Some(uses), _) => Some(format!("{uses} uses")),
            _ => None,
        }
    }

    pub fn members_label(&self) -> Option<String> {
        let members = self.approximate_member_count?;
        let online = self.approximate_presence_count?;
        Some(format!(
            "{members} members · {online} online"
        ))
    }

    pub fn state_label(&self, now: DateTime<Utc>) -> Option<String> {
        match self.availability_at(now) {
            InviteAvailability::Expired => Some("Invite expired".to_string()),
            InviteAvailability::Exhausted => Some("No uses left".to_string()),
            InviteAvailability::Available if self.temporary => {
                Some("Temporary membership".to_string())
            }
            InviteAvailability::Available => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteAvailability {
    Available,
    Expired,
    Exhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InviteParseError {
    NotObject,
    MissingField(&'static str),
    InvalidCode(String),
    InvalidTimestamp(&'static str),
    InvalidType(&'static str),
}

impl fmt::Display for InviteParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotObject => f.write_str("invite payload is not an object"),
            Self::MissingField(field) => write!(f, "invite payload missing field: {field}"),
            Self::InvalidCode(code) => write!(f, "invalid invite code: {code}"),
            Self::InvalidTimestamp(field) => write!(f, "invalid invite timestamp: {field}"),
            Self::InvalidType(field) => write!(f, "invalid invite field type: {field}"),
        }
    }
}

impl std::error::Error for InviteParseError {}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
}

fn required_string(value: &Value, field: &'static str) -> Result<String, InviteParseError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
        .ok_or(InviteParseError::MissingField(field))
}

fn optional_u64(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}

fn optional_u8(value: &Value, field: &str) -> Option<u8> {
    optional_u64(value, field).map(|v| v.min(u8::MAX as u64) as u8)
}

fn optional_bool(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn optional_timestamp(
    value: &Value,
    field: &'static str,
) -> Result<Option<DateTime<Utc>>, InviteParseError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => DateTime::parse_from_rfc3339(raw)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(|_| InviteParseError::InvalidTimestamp(field)),
        Some(_) => Err(InviteParseError::InvalidType(field)),
    }
}

fn parse_invite_guild(value: &Value) -> Result<InviteGuild, InviteParseError> {
    let features = value
        .get("features")
        .and_then(Value::as_array)
        .map(|features| {
            features
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(InviteGuild {
        id: required_string(value, "id")?,
        name: required_string(value, "name")?,
        icon: optional_string(value, "icon"),
        splash: optional_string(value, "splash"),
        description: optional_string(value, "description"),
        features,
        verification_level: optional_u8(value, "verification_level"),
        nsfw: optional_bool(value, "nsfw"),
        premium_tier: optional_u8(value, "premium_tier"),
        premium_subscription_count: optional_u64(value, "premium_subscription_count"),
    })
}

fn parse_invite_channel(value: &Value) -> Result<InviteChannel, InviteParseError> {
    Ok(InviteChannel {
        id: required_string(value, "id")?,
        name: optional_string(value, "name"),
        channel_type: optional_u64(value, "type"),
    })
}

fn parse_invite_inviter(value: &Value) -> Result<InviteInviter, InviteParseError> {
    Ok(InviteInviter {
        id: required_string(value, "id")?,
        username: required_string(value, "username")?,
        discriminator: optional_string(value, "discriminator"),
        global_name: optional_string(value, "global_name"),
        avatar: optional_string(value, "avatar"),
        bot: optional_bool(value, "bot"),
    })
}

pub fn parse_invite_from_json(value: &Value) -> Result<Invite, InviteParseError> {
    if !value.is_object() {
        return Err(InviteParseError::NotObject);
    }
    let code = required_string(value, "code")?;
    if !is_valid_invite_code(&code) {
        return Err(InviteParseError::InvalidCode(code));
    }

    let guild = match value.get("guild") {
        None | Some(Value::Null) => None,
        Some(guild) if guild.is_object() => Some(parse_invite_guild(guild)?),
        Some(_) => return Err(InviteParseError::InvalidType("guild")),
    };
    let channel = match value.get("channel") {
        None | Some(Value::Null) => None,
        Some(channel) if channel.is_object() => Some(parse_invite_channel(channel)?),
        Some(_) => return Err(InviteParseError::InvalidType("channel")),
    };
    let inviter = match value.get("inviter") {
        None | Some(Value::Null) => None,
        Some(inviter) if inviter.is_object() => Some(parse_invite_inviter(inviter)?),
        Some(_) => return Err(InviteParseError::InvalidType("inviter")),
    };

    Ok(Invite {
        code,
        guild,
        channel,
        inviter,
        approximate_member_count: optional_u64(value, "approximate_member_count"),
        approximate_presence_count: optional_u64(value, "approximate_presence_count"),
        expires_at: optional_timestamp(value, "expires_at")?,
        created_at: optional_timestamp(value, "created_at")?,
        temporary: optional_bool(value, "temporary"),
        uses: optional_u64(value, "uses"),
        max_uses: optional_u64(value, "max_uses"),
        invite_type: optional_u64(value, "type")
            .map(|v| InviteType::from(v.min(u8::MAX as u64) as u8))
            .unwrap_or_default(),
        flags: optional_u64(value, "flags"),
    })
}

pub fn parse_invite_json_str(raw: &str) -> Result<Invite, InviteParseError> {
    let value: Value = serde_json::from_str(raw).map_err(|_| InviteParseError::NotObject)?;
    parse_invite_from_json(&value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InviteErrorKind {
    NotFound,
    RateLimited,
    Unauthorized,
    Forbidden,
    VerificationRequired,
    MissingAccess,
    MissingPermissions,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InviteApiError {
    pub status: u16,
    pub code: Option<u64>,
    pub message: Option<String>,
    pub retry_after: Option<f64>,
}

impl InviteApiError {
    pub fn kind(&self) -> InviteErrorKind {
        if self.status == 429 {
            return InviteErrorKind::RateLimited;
        }
        match self.code {
            Some(10006) => return InviteErrorKind::NotFound,
            Some(40002) => return InviteErrorKind::VerificationRequired,
            Some(50001) => return InviteErrorKind::MissingAccess,
            Some(50013) => return InviteErrorKind::MissingPermissions,
            _ => {}
        }
        match self.status {
            401 => InviteErrorKind::Unauthorized,
            403 => InviteErrorKind::Forbidden,
            404 => InviteErrorKind::NotFound,
            429 => InviteErrorKind::RateLimited,
            _ => InviteErrorKind::Unknown,
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self.kind(), InviteErrorKind::RateLimited)
    }
}

impl fmt::Display for InviteApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invite request failed with HTTP {}", self.status)?;
        if let Some(code) = self.code {
            write!(f, " (code {code})")?;
        }
        if let Some(message) = self.message.as_deref().filter(|m| !m.is_empty()) {
            write!(f, ": {message}")?;
        }
        Ok(())
    }
}

impl std::error::Error for InviteApiError {}

pub fn parse_invite_error_value(status: u16, value: Option<&Value>) -> InviteApiError {
    let code = value
        .and_then(|v| v.get("code"))
        .and_then(Value::as_u64);
    let message = value
        .and_then(|v| v.get("message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(ToString::to_string);
    let retry_after = value
        .and_then(|v| v.get("retry_after"))
        .and_then(Value::as_f64);
    InviteApiError {
        status,
        code,
        message,
        retry_after,
    }
}

pub fn parse_invite_error(status: u16, body: &[u8]) -> InviteApiError {
    let value = serde_json::from_slice::<Value>(body).ok();
    parse_invite_error_value(status, value.as_ref())
}

pub fn render_invite_card(invite: &Invite, now: DateTime<Utc>) -> Div {
    let mut card = div()
        .flex()
        .flex_col()
        .w(px(INVITE_CARD_WIDTH))
        .max_w(px(INVITE_CARD_WIDTH))
        .bg(bg_channels())
        .border_1()
        .border_color(borders())
        .rounded(radius_lg())
        .overflow_hidden();

    if let Some(splash) = invite
        .guild
        .as_ref()
        .and_then(|guild| guild.splash_url(640))
    {
        card = card.child(
            img(splash)
                .w_full()
                .h(px(INVITE_CARD_SPLASH_HEIGHT))
                .object_fit(ObjectFit::Cover),
        );
    }

    let mut body = div()
        .flex()
        .flex_col()
        .gap(space_3())
        .p(space_4())
        .w_full();

    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(space_3())
        .w_full();

    if let Some(icon) = invite.guild.as_ref().and_then(|guild| guild.icon_url(96)) {
        header = header.child(
            img(icon)
                .size(px(48.0))
                .rounded(radius_full())
                .object_fit(ObjectFit::Cover),
        );
    }

    let mut title_col = div().flex().flex_col().flex_1().gap(space_1());
    title_col = title_col.child(
        div()
            .text_size(font_size_base())
            .font_weight(WEIGHT_SEMIBOLD)
            .text_color(text_titles())
            .child(invite.title()),
    );
    if let Some(description) = invite.description() {
        title_col = title_col.child(
            div()
                .text_size(font_size_xs())
                .text_color(text_muted())
                .child(description.to_string()),
        );
    }
    header = header.child(title_col);
    body = body.child(header);

    if let Some(channel) = invite.channel.as_ref().and_then(InviteChannel::label) {
        body = body.child(
            div()
                .text_size(font_size_sm())
                .text_color(text_primary())
                .child(channel),
        );
    }

    let mut meta_parts = Vec::new();
    if let Some(members) = invite.members_label() {
        meta_parts.push(members);
    }
    if let Some(usage) = invite.usage_label() {
        meta_parts.push(usage);
    }
    if let Some(state) = invite.state_label(now) {
        meta_parts.push(state);
    }
    if !meta_parts.is_empty() {
        body = body.child(
            div()
                .text_size(font_size_xs())
                .text_color(text_muted())
                .child(meta_parts.join(" · ")),
        );
    }

    if let Some(inviter) = invite.inviter.as_ref() {
        let mut inviter_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_2())
            .w_full();
        if let Some(avatar) = inviter.avatar_url(64) {
            inviter_row = inviter_row.child(
                img(avatar)
                    .size(px(20.0))
                    .rounded(radius_full())
                    .object_fit(ObjectFit::Cover),
            );
        }
        inviter_row = inviter_row.child(
            div()
                .text_size(font_size_xs())
                .text_color(text_muted())
                .child(format!("Invited by {}", inviter.display_name())),
        );
        body = body.child(inviter_row);
    }

    card.child(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[::core::prelude::v1::test]
    fn parses_supported_domains() {
        for url in [
            "https://discord.gg/sintetico-abc",
            "discord.gg/sintetico-abc",
            "https://www.discord.gg/sintetico-abc",
            "https://discord.com/invite/sintetico-abc",
            "https://discordapp.com/invite/sintetico-abc",
            "https://canary.discord.com/invite/sintetico-abc",
        ] {
            let link = parse_invite_url(url).expect(url);
            assert_eq!(link.code, "sintetico-abc");
            assert!(is_valid_invite_code(&link.code));
        }
    }

    #[::core::prelude::v1::test]
    fn rejects_foreign_hosts_and_malformed_paths() {
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
    }
}
