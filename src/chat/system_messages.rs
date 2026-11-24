use chrono::{DateTime, Utc};
use gpui::prelude::*;
use gpui::*;
use serde_json::Value;

use crate::domain::message::{Attachment, Author, Embed, EmbedField, MessageStickerItem};
use crate::i18n::Locale;
use crate::tokens::colors::{
    bg_chat_hover, blurple, borders, status_dnd, status_online, text_muted, text_primary,
    text_titles,
};
use crate::tokens::icons::{
    icon_info, icon_members, icon_phone, icon_phone_hangup, icon_pin, icon_sparkles,
};
use crate::tokens::spacing::{radius_md, radius_sm, space_1, space_2, space_3, space_4, space_8};
use crate::tokens::typography::{
    font_size_base, font_size_sm, font_size_xs, line_height_base, line_height_sm, WEIGHT_BOLD,
    WEIGHT_MEDIUM, WEIGHT_SEMIBOLD,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemLocale {
    #[default]
    PtBr,
    EnUs,
}

impl From<Locale> for SystemLocale {
    fn from(locale: Locale) -> Self {
        match locale {
            Locale::PtBr => Self::PtBr,
            Locale::EnUs => Self::EnUs,
        }
    }
}

impl SystemLocale {
    pub fn current() -> Self {
        Self::from(crate::i18n::current_locale())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMessageIcon {
    MemberJoin,
    Boost,
    CallStarted,
    CallEnded,
    Pin,
    Unpin,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMessageTone {
    Positive,
    Boost,
    Call,
    Muted,
    Alert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessageKind {
    Join,
    Boost { tier: u8, count: u32 },
    Call {
        ended: bool,
        missed: bool,
        duration_seconds: Option<u64>,
    },
    Pin {
        pinned: bool,
    },
    Other(u8),
}

impl SystemMessageKind {
    pub fn from_type_code(type_code: u8) -> Self {
        match type_code {
            7 => Self::Join,
            8 => Self::Boost { tier: 1, count: 1 },
            9 => Self::Boost { tier: 2, count: 1 },
            10 => Self::Boost { tier: 3, count: 1 },
            3 => Self::Call {
                ended: false,
                missed: false,
                duration_seconds: None,
            },
            6 => Self::Pin { pinned: true },
            other => Self::Other(other),
        }
    }

    pub fn type_code(&self) -> u8 {
        match self {
            Self::Join => 7,
            Self::Boost { .. } => 8,
            Self::Call { .. } => 3,
            Self::Pin { .. } => 6,
            Self::Other(code) => *code,
        }
    }

    pub fn icon(&self) -> SystemMessageIcon {
        match self {
            Self::Join => SystemMessageIcon::MemberJoin,
            Self::Boost { .. } => SystemMessageIcon::Boost,
            Self::Call {
                ended: false,
                missed: false,
                ..
            } => SystemMessageIcon::CallStarted,
            Self::Call { .. } => SystemMessageIcon::CallEnded,
            Self::Pin { pinned: true } => SystemMessageIcon::Pin,
            Self::Pin { pinned: false } => SystemMessageIcon::Unpin,
            Self::Other(_) => SystemMessageIcon::Info,
        }
    }

    pub fn tone(&self) -> SystemMessageTone {
        match self {
            Self::Join => SystemMessageTone::Positive,
            Self::Boost { .. } => SystemMessageTone::Boost,
            Self::Call {
                ended: false,
                missed: false,
                ..
            } => SystemMessageTone::Call,
            Self::Call { missed: true, .. } => SystemMessageTone::Alert,
            Self::Call { .. } => SystemMessageTone::Muted,
            Self::Pin { pinned: true } => SystemMessageTone::Boost,
            Self::Pin { pinned: false } => SystemMessageTone::Muted,
            Self::Other(_) => SystemMessageTone::Muted,
        }
    }

    pub fn is_join(&self) -> bool {
        matches!(self, Self::Join)
    }

    pub fn is_boost(&self) -> bool {
        matches!(self, Self::Boost { .. })
    }

    pub fn is_call(&self) -> bool {
        matches!(self, Self::Call { .. })
    }

    pub fn is_pin(&self) -> bool {
        matches!(self, Self::Pin { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemMessage {
    pub message_id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub kind: SystemMessageKind,
    pub actor: Option<Author>,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub reference_message_id: Option<String>,
}

impl SystemMessage {
    pub fn from_json(value: &Value) -> Option<Self> {
        Self::from_json_for_viewer(value, None)
    }

    pub fn from_json_for_viewer(value: &Value, viewer_id: Option<&str>) -> Option<Self> {
        let message_id = value.get("id")?.as_str()?.to_string();
        let channel_id = value.get("channel_id")?.as_str()?.to_string();
        let type_code = value.get("type").and_then(Value::as_u64)? as u8;
        let mut kind = SystemMessageKind::from_type_code(type_code);
        let content = value
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if let SystemMessageKind::Boost { tier, .. } = kind {
            kind = SystemMessageKind::Boost {
                tier,
                count: boost_count(&content).unwrap_or(1),
            };
        }

        if let SystemMessageKind::Call { .. } = kind {
            let ended = call_ended(value);
            let duration_seconds = call_duration_seconds(value);
            let missed = ended && call_missed(value, viewer_id);
            kind = SystemMessageKind::Call {
                ended,
                missed,
                duration_seconds,
            };
        }

        if let SystemMessageKind::Pin { .. } = kind {
            kind = SystemMessageKind::Pin {
                pinned: pin_is_pinned(&content),
            };
        }

        Some(Self {
            message_id,
            channel_id,
            guild_id: value
                .get("guild_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            kind,
            actor: parse_author(value.get("author")),
            content,
            timestamp: parse_timestamp(value).unwrap_or(DateTime::<Utc>::UNIX_EPOCH),
            reference_message_id: value
                .get("message_reference")
                .and_then(|reference| reference.get("message_id"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
        })
    }

    pub fn is_system(&self) -> bool {
        !matches!(self.kind, SystemMessageKind::Other(0))
    }

    pub fn actor_id(&self) -> Option<&str> {
        self.actor.as_ref().map(|actor| actor.id.as_str())
    }

    pub fn actor_name(&self) -> Option<String> {
        self.actor.as_ref().map(|actor| actor.display_name().to_string())
    }

    pub fn view(&self, locale: SystemLocale) -> SystemMessageView {
        let actor_name = self.actor_name();
        let text = format_system_text(&self.kind, actor_name.as_deref(), &self.content, locale);

        SystemMessageView {
            message_id: self.message_id.clone(),
            channel_id: self.channel_id.clone(),
            guild_id: self.guild_id.clone(),
            kind: self.kind.clone(),
            icon: self.kind.icon(),
            tone: self.kind.tone(),
            text,
            actor_id: self.actor_id().map(ToString::to_string),
            actor_name,
            target_message_id: self.reference_message_id.clone(),
            timestamp: self.timestamp,
            call_duration_seconds: match self.kind {
                SystemMessageKind::Call {
                    duration_seconds, ..
                } => duration_seconds,
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SystemMessageView {
    pub message_id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub kind: SystemMessageKind,
    pub icon: SystemMessageIcon,
    pub tone: SystemMessageTone,
    pub text: String,
    pub actor_id: Option<String>,
    pub actor_name: Option<String>,
    pub target_message_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub call_duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageReferenceType {
    Default,
    Forward,
    Other(u8),
}

impl MessageReferenceType {
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Default,
            1 => Self::Forward,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageReference {
    pub reference_type: MessageReferenceType,
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub fail_if_not_exists: bool,
}

impl MessageReference {
    pub fn from_json(value: &Value) -> Option<Self> {
        let reference = value.get("message_reference")?;
        let reference_type = reference
            .get("type")
            .and_then(Value::as_u64)
            .map(|code| MessageReferenceType::from_code(code as u8))
            .unwrap_or(MessageReferenceType::Default);

        Some(Self {
            reference_type,
            message_id: reference
                .get("message_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            channel_id: reference
                .get("channel_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            guild_id: reference
                .get("guild_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            fail_if_not_exists: reference
                .get("fail_if_not_exists")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    pub fn is_forward(&self) -> bool {
        self.reference_type == MessageReferenceType::Forward
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageSnapshot {
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub message_type: u8,
    pub author: Option<Author>,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub edited_timestamp: Option<DateTime<Utc>>,
    pub attachments: Vec<Attachment>,
    pub embeds: Vec<Embed>,
    pub sticker_items: Vec<MessageStickerItem>,
}

impl MessageSnapshot {
    pub fn from_json(value: &Value) -> Option<Self> {
        let message = value.get("message").unwrap_or(value);
        let content = message
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        Some(Self {
            message_id: message
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            channel_id: message
                .get("channel_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            guild_id: message
                .get("guild_id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            message_type: message.get("type").and_then(Value::as_u64).unwrap_or(0) as u8,
            author: parse_author(message.get("author")),
            content,
            timestamp: parse_timestamp(message),
            edited_timestamp: parse_edited_timestamp(message),
            attachments: parse_attachments(message),
            embeds: parse_embeds(message),
            sticker_items: parse_sticker_items(message),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForwardedMessage {
    pub snapshots: Vec<MessageSnapshot>,
    pub reference: Option<MessageReference>,
}

impl ForwardedMessage {
    pub fn from_json(value: &Value) -> Option<Self> {
        let reference = MessageReference::from_json(value);
        let mut snapshots = Vec::new();

        if let Some(raw_snapshots) = value.get("message_snapshots").and_then(Value::as_array) {
            for raw in raw_snapshots {
                if let Some(snapshot) = MessageSnapshot::from_json(raw) {
                    snapshots.push(snapshot);
                }
            }
        }

        let reference_is_forward = reference
            .as_ref()
            .map(MessageReference::is_forward)
            .unwrap_or(false);

        if snapshots.is_empty() && !reference_is_forward {
            return None;
        }

        Some(Self {
            snapshots,
            reference,
        })
    }

    pub fn is_forwarded(&self) -> bool {
        !self.snapshots.is_empty()
            || self
                .reference
                .as_ref()
                .map(MessageReference::is_forward)
                .unwrap_or(false)
    }

    pub fn card(&self, options: ForwardedRenderOptions) -> ForwardedCard {
        let label = match options.locale {
            SystemLocale::PtBr => "Encaminhado",
            SystemLocale::EnUs => "Forwarded",
        }
        .to_string();

        let restriction_notice = if options.age_restricted {
            Some(
                match options.locale {
                    SystemLocale::PtBr => {
                        "Encaminhado de um canal com restrição de idade"
                    }
                    SystemLocale::EnUs => {
                        "Forwarded from an age-restricted channel"
                    }
                }
                .to_string(),
            )
        } else {
            None
        };

        let origin = if options.origin_accessible {
            self.reference.as_ref().and_then(|reference| {
                reference.message_id.as_ref().and_then(|message_id| {
                    reference.channel_id.as_ref().map(|channel_id| ForwardedOriginView {
                        message_id: message_id.clone(),
                        channel_id: channel_id.clone(),
                        guild_id: reference.guild_id.clone(),
                        url: discord_message_url(
                            reference.guild_id.as_deref(),
                            channel_id,
                            message_id,
                        ),
                    })
                })
            })
        } else {
            None
        };

        let snapshots = self
            .snapshots
            .iter()
            .map(|snapshot| {
                let restricted = options.age_restricted;
                ForwardedSnapshotView {
                    message_id: snapshot.message_id.clone(),
                    author_id: snapshot.author.as_ref().map(|author| author.id.clone()),
                    author_name: snapshot
                        .author
                        .as_ref()
                        .map(|author| author.display_name().to_string()),
                    author_avatar_url: snapshot
                        .author
                        .as_ref()
                        .and_then(Author::avatar_url),
                    content: if restricted {
                        String::new()
                    } else {
                        snapshot.content.clone()
                    },
                    timestamp: snapshot.timestamp,
                    edited_timestamp: snapshot.edited_timestamp,
                    attachments: if restricted {
                        Vec::new()
                    } else {
                        snapshot.attachments.clone()
                    },
                    embeds: if restricted {
                        Vec::new()
                    } else {
                        snapshot.embeds.clone()
                    },
                    sticker_items: if restricted {
                        Vec::new()
                    } else {
                        snapshot.sticker_items.clone()
                    },
                }
            })
            .collect();

        ForwardedCard {
            label,
            snapshots,
            origin,
            restriction_notice,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardedRenderOptions {
    pub locale: SystemLocale,
    pub origin_accessible: bool,
    pub age_restricted: bool,
}

impl Default for ForwardedRenderOptions {
    fn default() -> Self {
        Self {
            locale: SystemLocale::default(),
            origin_accessible: true,
            age_restricted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForwardedOriginView {
    pub message_id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForwardedSnapshotView {
    pub message_id: Option<String>,
    pub author_id: Option<String>,
    pub author_name: Option<String>,
    pub author_avatar_url: Option<String>,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub edited_timestamp: Option<DateTime<Utc>>,
    pub attachments: Vec<Attachment>,
    pub embeds: Vec<Embed>,
    pub sticker_items: Vec<MessageStickerItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForwardedCard {
    pub label: String,
    pub snapshots: Vec<ForwardedSnapshotView>,
    pub origin: Option<ForwardedOriginView>,
    pub restriction_notice: Option<String>,
}

impl ForwardedCard {
    pub fn has_visible_content(&self) -> bool {
        self.snapshots.iter().any(|snapshot| {
            !snapshot.content.trim().is_empty()
                || !snapshot.attachments.is_empty()
                || !snapshot.embeds.is_empty()
                || !snapshot.sticker_items.is_empty()
        })
    }
}

fn format_system_text(
    kind: &SystemMessageKind,
    actor_name: Option<&str>,
    content: &str,
    locale: SystemLocale,
) -> String {
    match kind {
        SystemMessageKind::Join => actor_name
            .map(|name| {
                replace_user(
                    match locale {
                        SystemLocale::PtBr => "{user} entrou no servidor!",
                        SystemLocale::EnUs => "{user} joined the server!",
                    },
                    name,
                )
            })
            .unwrap_or_else(|| content.to_string()),
        SystemMessageKind::Boost { tier, count } => actor_name
            .map(|name| {
                if *tier > 1 {
                    return replace_template(
                        match locale {
                            SystemLocale::PtBr => {
                                "{user} impulsionou o servidor para o Nível {tier}!"
                            }
                            SystemLocale::EnUs => {
                                "{user} just boosted the server to Level {tier}!"
                            }
                        },
                        name,
                        *count,
                        *tier,
                    );
                }
                if *count > 1 {
                    return replace_template(
                        match locale {
                            SystemLocale::PtBr => {
                                "{user} impulsionou o servidor {count} vezes!"
                            }
                            SystemLocale::EnUs => {
                                "{user} just boosted the server {count} times!"
                            }
                        },
                        name,
                        *count,
                        *tier,
                    );
                }
                replace_user(
                    match locale {
                        SystemLocale::PtBr => "{user} impulsionou o servidor!",
                        SystemLocale::EnUs => "{user} just boosted the server!",
                    },
                    name,
                )
            })
            .unwrap_or_else(|| content.to_string()),
        SystemMessageKind::Call {
            ended,
            missed,
            duration_seconds,
        } => {
            if *missed {
                return match locale {
                    SystemLocale::PtBr => "Chamada perdida.",
                    SystemLocale::EnUs => "Missed call.",
                }
                .to_string();
            }
            if *ended {
                let mut text = match locale {
                    SystemLocale::PtBr => "Chamada encerrada.",
                    SystemLocale::EnUs => "Call ended.",
                }
                .to_string();
                if let Some(seconds) = duration_seconds {
                    text.push(' ');
                    text.push_str(&format_duration(*seconds));
                }
                return text;
            }
            actor_name
                .map(|name| {
                    replace_user(
                        match locale {
                            SystemLocale::PtBr => "{user} iniciou uma chamada.",
                            SystemLocale::EnUs => "{user} started a call.",
                        },
                        name,
                    )
                })
                .unwrap_or_else(|| content.to_string())
        }
        SystemMessageKind::Pin { pinned } => actor_name
            .map(|name| {
                replace_user(
                    match (locale, pinned) {
                        (SystemLocale::PtBr, true) => {
                            "{user} fixou uma mensagem neste canal."
                        }
                        (SystemLocale::PtBr, false) => {
                            "{user} desafixou uma mensagem neste canal."
                        }
                        (SystemLocale::EnUs, true) => {
                            "{user} pinned a message to this channel."
                        }
                        (SystemLocale::EnUs, false) => {
                            "{user} unpinned a message from this channel."
                        }
                    },
                    name,
                )
            })
            .unwrap_or_else(|| content.to_string()),
        SystemMessageKind::Other(_) => content.to_string(),
    }
}

fn replace_user(template: &str, user: &str) -> String {
    template.replace("{user}", user)
}

fn replace_template(template: &str, user: &str, count: u32, tier: u8) -> String {
    template
        .replace("{user}", user)
        .replace("{count}", &count.to_string())
        .replace("{tier}", &tier.to_string())
}

fn format_duration(seconds: u64) -> String {
    let minutes = seconds / 60;
    let remaining = seconds % 60;
    format!("{minutes}:{remaining:02}")
}

fn call_ended(value: &Value) -> bool {
    value
        .get("call")
        .and_then(|call| call.get("ended_timestamp"))
        .map(|timestamp| !timestamp.is_null())
        .unwrap_or(false)
}

fn call_duration_seconds(value: &Value) -> Option<u64> {
    let started = value.get("timestamp")?.as_str()?;
    let ended = value
        .get("call")
        .and_then(|call| call.get("ended_timestamp"))
        .and_then(Value::as_str)?;
    let started = parse_rfc3339(started)?;
    let ended = parse_rfc3339(ended)?;
    let seconds = (ended - started).num_seconds();
    (seconds >= 0).then_some(seconds as u64)
}

fn call_missed(value: &Value, viewer_id: Option<&str>) -> bool {
    let Some(viewer_id) = viewer_id else {
        return false;
    };
    let Some(participants) = value
        .get("call")
        .and_then(|call| call.get("participants"))
        .and_then(Value::as_array)
    else {
        return false;
    };
    !participants
        .iter()
        .filter_map(Value::as_str)
        .any(|participant| participant == viewer_id)
}

fn pin_is_pinned(content: &str) -> bool {
    let lower = content.to_lowercase();
    !lower.contains("unpin") && !lower.contains("desafix")
}

fn boost_count(content: &str) -> Option<u32> {
    let lower = content.to_lowercase();
    if !lower.contains("times") && !lower.contains("vezes") {
        return None;
    }
    let digits: String = content
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn parse_author(value: Option<&Value>) -> Option<Author> {
    let value = value?;
    let id = value.get("id")?.as_str()?;
    let username = value.get("username")?.as_str()?;
    let mut author = Author::new(id, username);
    author.global_name = value
        .get("global_name")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    author.avatar = value
        .get("avatar")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    author.bot = value.get("bot").and_then(Value::as_bool).unwrap_or(false);
    Some(author)
}

fn parse_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    value
        .get("timestamp")
        .or_else(|| value.get("created_at"))
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
}

fn parse_edited_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    value
        .get("edited_timestamp")
        .or_else(|| value.get("edited_at"))
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
}

fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn parse_attachments(value: &Value) -> Vec<Attachment> {
    let mut attachments = Vec::new();
    let Some(raw_attachments) = value.get("attachments").and_then(Value::as_array) else {
        return attachments;
    };
    for raw in raw_attachments {
        let (Some(id), Some(filename), Some(url)) = (
            raw.get("id").and_then(Value::as_str),
            raw.get("filename").and_then(Value::as_str),
            raw.get("url").and_then(Value::as_str),
        ) else {
            continue;
        };
        attachments.push(Attachment {
            id: id.to_string(),
            filename: filename.to_string(),
            size: raw.get("size").and_then(Value::as_u64).unwrap_or(0),
            url: url.to_string(),
            proxy_url: raw
                .get("proxy_url")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            content_type: raw
                .get("content_type")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            width: raw
                .get("width")
                .and_then(Value::as_u64)
                .map(|width| width as u32),
            height: raw
                .get("height")
                .and_then(Value::as_u64)
                .map(|height| height as u32),
        });
    }
    attachments
}

fn parse_embeds(value: &Value) -> Vec<Embed> {
    let mut embeds = Vec::new();
    let Some(raw_embeds) = value.get("embeds").and_then(Value::as_array) else {
        return embeds;
    };
    for raw in raw_embeds {
        let mut fields = Vec::new();
        if let Some(raw_fields) = raw.get("fields").and_then(Value::as_array) {
            for field in raw_fields {
                if let (Some(name), Some(value)) = (
                    field.get("name").and_then(Value::as_str),
                    field.get("value").and_then(Value::as_str),
                ) {
                    fields.push(EmbedField {
                        name: name.to_string(),
                        value: value.to_string(),
                        inline: field
                            .get("inline")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    });
                }
            }
        }

        embeds.push(Embed {
            title: raw
                .get("title")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            description: raw
                .get("description")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            url: raw
                .get("url")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            color: raw
                .get("color")
                .and_then(Value::as_u64)
                .map(|color| color as u32),
            author: None,
            fields,
            footer: None,
            image_url: raw
                .get("image")
                .and_then(|image| image.get("url"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            thumbnail_url: raw
                .get("thumbnail")
                .and_then(|thumbnail| thumbnail.get("url"))
                .and_then(Value::as_str)
                .map(ToString::to_string),
            timestamp: raw
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_rfc3339),
        });
    }
    embeds
}

fn parse_sticker_items(value: &Value) -> Vec<MessageStickerItem> {
    let mut stickers = Vec::new();
    let Some(raw_stickers) = value.get("sticker_items").and_then(Value::as_array) else {
        return stickers;
    };
    for raw in raw_stickers {
        if let (Some(id), Some(name)) = (
            raw.get("id").and_then(Value::as_str),
            raw.get("name").and_then(Value::as_str),
        ) {
            stickers.push(MessageStickerItem::new(
                id,
                name,
                raw.get("format_type")
                    .and_then(Value::as_u64)
                    .unwrap_or(1) as u8,
            ));
        }
    }
    stickers
}

fn discord_message_url(guild_id: Option<&str>, channel_id: &str, message_id: &str) -> String {
    match guild_id {
        Some(guild_id) => {
            format!("https://discord.com/channels/{guild_id}/{channel_id}/{message_id}")
        }
        None => format!("https://discord.com/channels/@me/{channel_id}/{message_id}"),
    }
}

fn tone_color(tone: SystemMessageTone) -> Rgba {
    match tone {
        SystemMessageTone::Positive => status_online(),
        SystemMessageTone::Boost => blurple(),
        SystemMessageTone::Call => status_online(),
        SystemMessageTone::Muted => text_muted(),
        SystemMessageTone::Alert => status_dnd(),
    }
}

fn render_icon(icon: SystemMessageIcon, color: Rgba) -> Div {
    match icon {
        SystemMessageIcon::MemberJoin => icon_members(px(16.0), color),
        SystemMessageIcon::Boost => icon_sparkles(px(16.0), color),
        SystemMessageIcon::CallStarted => icon_phone(px(16.0), color),
        SystemMessageIcon::CallEnded => icon_phone_hangup(px(16.0), color),
        SystemMessageIcon::Pin => icon_pin(px(16.0), color),
        SystemMessageIcon::Unpin => icon_pin(px(16.0), color),
        SystemMessageIcon::Info => icon_info(px(16.0), color),
    }
}

pub fn render_system_message_row(message: &SystemMessage, locale: SystemLocale) -> Div {
    let view = message.view(locale);
    let color = tone_color(view.tone);

    div()
        .flex()
        .flex_row()
        .w_full()
        .min_w(px(0.0))
        .py(px(2.0))
        .pl(space_8())
        .pr(space_4())
        .gap(space_2())
        .items_start()
        .child(div().w(px(48.0)).flex_shrink_0().flex().justify_end().child(render_icon(view.icon, color)))
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(space_2())
                .min_w(px(0.0))
                .child(
                    div()
                        .text_color(color)
                        .font_weight(WEIGHT_SEMIBOLD)
                        .text_size(font_size_sm())
                        .line_height(line_height_sm())
                        .child(view.text),
                ),
        )
}

pub fn render_forwarded_card(
    forwarded: &ForwardedMessage,
    options: ForwardedRenderOptions,
) -> Div {
    let card = forwarded.card(options);
    let mut container = div()
        .flex()
        .flex_col()
        .gap(space_2())
        .max_w(px(520.0))
        .p(space_3())
        .mb(space_2())
        .border_1()
        .border_color(borders())
        .rounded(radius_md())
        .bg(bg_chat_hover());

    container = container.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(space_1())
            .text_size(font_size_xs())
            .line_height(line_height_sm())
            .font_weight(WEIGHT_BOLD)
            .text_color(text_muted())
            .child(card.label.clone()),
    );

    if let Some(notice) = &card.restriction_notice {
        container = container.child(
            div()
                .text_size(font_size_xs())
                .line_height(line_height_sm())
                .text_color(text_muted())
                .child(notice.clone()),
        );
    }

    for snapshot in &card.snapshots {
        container = container.child(render_forwarded_snapshot(snapshot));
    }

    if let Some(origin) = &card.origin {
        container = container.child(
            div()
                .text_size(font_size_xs())
                .line_height(line_height_sm())
                .text_color(blurple())
                .cursor_pointer()
                .child(origin.url.clone()),
        );
    }

    container
}

fn render_forwarded_snapshot(snapshot: &ForwardedSnapshotView) -> Div {
    let mut body = div()
        .flex()
        .flex_col()
        .gap(space_1())
        .w_full()
        .min_w(px(0.0));

    if let Some(author_name) = &snapshot.author_name {
        body = body.child(
            div()
                .text_size(font_size_sm())
                .line_height(line_height_base())
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_titles())
                .child(author_name.clone()),
        );
    }

    if !snapshot.content.trim().is_empty() {
        body = body.child(
            div()
                .text_size(font_size_base())
                .line_height(line_height_base())
                .text_color(text_primary())
                .child(snapshot.content.clone()),
        );
    }

    for attachment in &snapshot.attachments {
        body = body.child(
            div()
                .text_size(font_size_xs())
                .line_height(line_height_sm())
                .text_color(text_muted())
                .child(attachment.filename.clone()),
        );
    }

    for embed in &snapshot.embeds {
        if let Some(title) = &embed.title {
            body = body.child(
                div()
                    .text_size(font_size_sm())
                    .line_height(line_height_sm())
                    .font_weight(WEIGHT_BOLD)
                    .text_color(text_titles())
                    .child(title.clone()),
            );
        }
        if let Some(description) = &embed.description {
            body = body.child(
                div()
                    .text_size(font_size_sm())
                    .line_height(line_height_sm())
                    .text_color(text_primary())
                    .child(description.clone()),
            );
        }
    }

    for sticker in &snapshot.sticker_items {
        body = body.child(
            div()
                .text_size(font_size_xs())
                .line_height(line_height_sm())
                .text_color(text_muted())
                .rounded(radius_sm())
                .child(sticker.name.clone()),
        );
    }

    body
}

pub fn should_group_system_with_regular() -> bool {
    false
}
