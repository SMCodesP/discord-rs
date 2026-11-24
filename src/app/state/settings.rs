use super::*;
use crate::domain::settings::MessageDisplayMode;
use crate::domain::settings::NitroTier;
use crate::domain::settings::ThemeChoice;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// Loading state for account-backed Settings data.
///
/// `Idle` is deliberately different from an empty account: no response has been
/// observed yet, so the UI must not display invented defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsLoadState {
    Idle,
    Loading,
    Loaded,
    Failed(String),
}

impl Default for SettingsLoadState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsSaveState {
    Idle,
    Saving,
    Saved,
    Failed(String),
}

impl Default for SettingsSaveState {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StickerAnimation {
    Always = 0,
    OnInteraction = 1,
    Never = 2,
}

impl StickerAnimation {
    pub fn from_wire(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Always),
            1 => Some(Self::OnInteraction),
            2 => Some(Self::Never),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExplicitContentFilter {
    Disabled = 0,
    NonFriends = 1,
    Everyone = 2,
}

impl ExplicitContentFilter {
    pub fn from_wire(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Disabled),
            1 => Some(Self::NonFriends),
            2 => Some(Self::Everyone),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReactionNotificationMode {
    Enabled = 0,
    DmsOnly = 1,
    Disabled = 2,
}

impl ReactionNotificationMode {
    pub fn from_wire(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Enabled),
            1 => Some(Self::DmsOnly),
            2 => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// Legacy `GET/PATCH /users/@me/settings` fields used by this client.
///
/// Every field is optional. Missing fields stay absent instead of becoming a
/// fabricated `false`, theme, locale, or filter value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct LegacyUserSettings {
    pub locale: Option<String>,
    pub theme: Option<String>,
    pub message_display_compact: Option<bool>,
    pub gif_auto_play: Option<bool>,
    pub animate_stickers: Option<u32>,
    pub explicit_content_filter: Option<u32>,
    pub friend_source_flags: Option<u32>,
    pub default_guilds_restricted: Option<bool>,
    pub stream_notifications_enabled: Option<bool>,
}

/// Normalized view of the real `PreloadedUserSettings` fields relevant to
/// Settings. Net-Gateway owns protobuf decoding and populates this DTO.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoUserSettings {
    pub notifications: Option<ProtoNotificationSettings>,
    pub clips: Option<ProtoClipsSettings>,
    pub localization: Option<ProtoLocalizationSettings>,
    pub text_and_images: Option<ProtoTextAndImagesSettings>,
    pub privacy: Option<ProtoPrivacySettings>,
    pub voice_and_video: Option<ProtoVoiceAndVideoSettings>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoNotificationSettings {
    pub reaction_notifications: Option<u32>,
    pub enable_friend_online_notifications: Option<bool>,
    pub enable_friend_anniversary_notifications: Option<bool>,
    pub enable_profile_updates_notifications: Option<bool>,
    pub enable_upcoming_server_event_notifications: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoClipsSettings {
    pub allow_voice_recording: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoLocalizationSettings {
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoTextAndImagesSettings {
    pub gif_auto_play: Option<bool>,
    pub animate_stickers: Option<u32>,
    pub message_display_compact: Option<bool>,
    pub explicit_content_filter: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoPrivacySettings {
    pub friend_source_flags: Option<u32>,
    pub default_guilds_restricted: Option<bool>,
    pub non_spam_retraining_opt_in: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProtoVoiceAndVideoSettings {
    pub stream_notifications_enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ConsentStatus {
    pub consented: bool,
}

/// Real `/users/@me/consent` payload. Sponsored-content personalization uses
/// `personalization`; it must never reuse DM or friend-request fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ConsentSettings {
    pub personalization: Option<ConsentStatus>,
    pub usage_statistics: Option<ConsentStatus>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProfileSnapshot {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub pronouns: Option<String>,
    pub custom_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct ProfilePatch {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub pronouns: Option<String>,
    pub custom_status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileDirty {
    pub display_name: bool,
    pub bio: bool,
    pub pronouns: bool,
    pub custom_status: bool,
}

impl ProfileDirty {
    pub fn is_empty(&self) -> bool {
        !self.display_name && !self.bio && !self.pronouns && !self.custom_status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileValidationError {
    EmptyDisplayName,
    DisplayNameTooLong,
    BioTooLong,
    PronounsTooLong,
    CustomStatusTooLong,
}

impl ProfileValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::EmptyDisplayName => "O nome de exibição não pode ficar vazio.",
            Self::DisplayNameTooLong => "O nome de exibição deve ter no máximo 32 caracteres.",
            Self::BioTooLong => "A biografia deve ter no máximo 190 caracteres.",
            Self::PronounsTooLong => "Os pronomes devem ter no máximo 40 caracteres.",
            Self::CustomStatusTooLong => {
                "O status personalizado deve ter no máximo 128 caracteres."
            }
        }
    }
}

/// Profile editor state. Draft changes are local until the API adapter returns
/// an authoritative snapshot; closing Settings never confirms a save.
#[derive(Debug, Clone)]
pub struct ProfileEditState {
    pub saved: Option<ProfileSnapshot>,
    pub draft: ProfileSnapshot,
    pub dirty: ProfileDirty,
    pub save_state: SettingsSaveState,
}

impl Default for ProfileEditState {
    fn default() -> Self {
        Self {
            saved: None,
            draft: ProfileSnapshot::default(),
            dirty: ProfileDirty::default(),
            save_state: SettingsSaveState::Idle,
        }
    }
}

impl ProfileEditState {
    pub fn hydrate(&mut self, snapshot: ProfileSnapshot) {
        self.saved = Some(snapshot.clone());
        self.draft = snapshot;
        self.dirty = ProfileDirty::default();
        self.save_state = SettingsSaveState::Idle;
    }

    pub fn set_display_name(&mut self, value: String) {
        self.dirty.display_name = self.draft.display_name.as_deref() != Some(value.as_str());
        self.draft.display_name = Some(value);
        self.mark_editing();
    }

    pub fn set_bio(&mut self, value: String) {
        self.dirty.bio = self.draft.bio.as_deref() != Some(value.as_str());
        self.draft.bio = Some(value);
        self.mark_editing();
    }

    pub fn set_pronouns(&mut self, value: String) {
        self.dirty.pronouns = self.draft.pronouns.as_deref() != Some(value.as_str());
        self.draft.pronouns = Some(value);
        self.mark_editing();
    }

    fn mark_editing(&mut self) {
        if !self.dirty.is_empty() && !matches!(self.save_state, SettingsSaveState::Saving) {
            self.save_state = SettingsSaveState::Idle;
        }
    }

    pub fn patch(&self) -> ProfilePatch {
        ProfilePatch {
            display_name: self
                .dirty
                .display_name
                .then(|| self.draft.display_name.clone())
                .flatten(),
            bio: self.dirty.bio.then(|| self.draft.bio.clone()).flatten(),
            pronouns: self
                .dirty
                .pronouns
                .then(|| self.draft.pronouns.clone())
                .flatten(),
            custom_status: self
                .dirty
                .custom_status
                .then(|| self.draft.custom_status.clone())
                .flatten(),
        }
    }

    pub fn validate(&self) -> Result<(), ProfileValidationError> {
        let patch = self.patch();
        if let Some(value) = patch.display_name {
            let value = value.trim();
            if value.is_empty() {
                return Err(ProfileValidationError::EmptyDisplayName);
            }
            if value.chars().count() > 32 {
                return Err(ProfileValidationError::DisplayNameTooLong);
            }
        }
        if patch.bio.is_some_and(|value| value.chars().count() > 190) {
            return Err(ProfileValidationError::BioTooLong);
        }
        if patch
            .pronouns
            .is_some_and(|value| value.chars().count() > 40)
        {
            return Err(ProfileValidationError::PronounsTooLong);
        }
        if patch
            .custom_status
            .is_some_and(|value| value.chars().count() > 128)
        {
            return Err(ProfileValidationError::CustomStatusTooLong);
        }
        Ok(())
    }

    pub fn begin_save(&mut self) -> Result<ProfilePatch, ProfileValidationError> {
        self.validate()?;
        self.save_state = SettingsSaveState::Saving;
        Ok(self.patch())
    }

    pub fn confirm_saved(&mut self, snapshot: ProfileSnapshot) {
        self.saved = Some(snapshot.clone());
        self.draft = snapshot;
        self.dirty = ProfileDirty::default();
        self.save_state = SettingsSaveState::Saved;
    }

    pub fn fail_save(&mut self, error: String) {
        self.save_state = SettingsSaveState::Failed(error);
    }

    pub fn discard(&mut self) {
        if let Some(saved) = &self.saved {
            self.draft = saved.clone();
        }
        self.dirty = ProfileDirty::default();
        self.save_state = SettingsSaveState::Idle;
    }
}

/// Settings controls backed by one real account/protobuf/consent field each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    UsageStatisticsConsent,
    PersonalizationConsent,
    ClipsVoiceRecording,
    DefaultGuildsRestricted,
    FriendRequestsFromGuildMembers,
    StreamNotifications,
    FriendAnniversaryNotifications,
    FriendOnlineNotifications,
    UpcomingServerEventNotifications,
    ProfileUpdatesNotifications,
    ReactionNotifications,
}

impl SettingsField {
    pub fn wire_field(self) -> &'static str {
        match self {
            Self::UsageStatisticsConsent => "consents.usage_statistics.consented",
            Self::PersonalizationConsent => "consents.personalization.consented",
            Self::ClipsVoiceRecording => "clips.allow_voice_recording",
            Self::DefaultGuildsRestricted => "default_guilds_restricted",
            Self::FriendRequestsFromGuildMembers => "friend_source_flags.mutual_guilds",
            Self::StreamNotifications => "stream_notifications_enabled",
            Self::FriendAnniversaryNotifications => {
                "notifications.enable_friend_anniversary_notifications"
            }
            Self::FriendOnlineNotifications => "notifications.enable_friend_online_notifications",
            Self::UpcomingServerEventNotifications => {
                "notifications.enable_upcoming_server_event_notifications"
            }
            Self::ProfileUpdatesNotifications => {
                "notifications.enable_profile_updates_notifications"
            }
            Self::ReactionNotifications => "notifications.reaction_notifications",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSetting {
    pub field: SettingsField,
    pub value: bool,
}

/// Single normalized source of truth for account-backed Settings values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsHydration {
    pub legacy: LegacyUserSettings,
    pub proto: ProtoUserSettings,
    pub consents: ConsentSettings,
    pub profile: Option<ProfileSnapshot>,
}

impl SettingsHydration {
    pub fn from_legacy_json(value: &Value) -> Result<Self, String> {
        let legacy = serde_json::from_value::<LegacyUserSettings>(value.clone())
            .map_err(|_| "Resposta inválida de configurações da conta.".to_string())?;
        Ok(Self {
            legacy,
            ..Self::default()
        })
    }

    pub fn from_proto_json(value: &Value) -> Result<Self, String> {
        let proto = serde_json::from_value::<ProtoUserSettings>(value.clone())
            .map_err(|_| "Resposta inválida das configurações protobuf.".to_string())?;
        Ok(Self {
            proto,
            ..Self::default()
        })
    }

    pub fn from_consents_json(value: &Value) -> Result<Self, String> {
        let consents = serde_json::from_value::<ConsentSettings>(value.clone())
            .map_err(|_| "Resposta inválida dos consentimentos de privacidade.".to_string())?;
        Ok(Self {
            consents,
            ..Self::default()
        })
    }

    pub fn from_profile_json(value: &Value) -> Result<Self, String> {
        let profile = serde_json::from_value::<ProfileSnapshot>(value.clone())
            .map_err(|_| "Resposta inválida do perfil.".to_string())?;
        Ok(Self {
            profile: Some(profile),
            ..Self::default()
        })
    }

    /// Merges only fields present in `newer`. Missing fields preserve the last
    /// authoritative value instead of resetting it to a default.
    pub fn merge(&mut self, newer: Self) {
        macro_rules! merge_fields {
            ($target:expr, $source:expr, $($field:ident),+ $(,)?) => {
                $(if $source.$field.is_some() {
                    $target.$field = $source.$field.clone();
                })+
            };
        }

        merge_fields!(
            self.legacy,
            newer.legacy,
            locale,
            theme,
            message_display_compact,
            gif_auto_play,
            animate_stickers,
            explicit_content_filter,
            friend_source_flags,
            default_guilds_restricted,
            stream_notifications_enabled,
        );

        if let Some(source) = newer.proto.notifications {
            let target = self
                .proto
                .notifications
                .get_or_insert_with(Default::default);
            merge_fields!(
                target,
                source,
                reaction_notifications,
                enable_friend_online_notifications,
                enable_friend_anniversary_notifications,
                enable_profile_updates_notifications,
                enable_upcoming_server_event_notifications,
            );
        }
        if let Some(source) = newer.proto.clips {
            let target = self.proto.clips.get_or_insert_with(Default::default);
            merge_fields!(target, source, allow_voice_recording);
        }
        if let Some(source) = newer.proto.localization {
            let target = self.proto.localization.get_or_insert_with(Default::default);
            merge_fields!(target, source, locale);
        }
        if let Some(source) = newer.proto.text_and_images {
            let target = self
                .proto
                .text_and_images
                .get_or_insert_with(Default::default);
            merge_fields!(
                target,
                source,
                gif_auto_play,
                animate_stickers,
                message_display_compact,
                explicit_content_filter,
            );
        }
        if let Some(source) = newer.proto.privacy {
            let target = self.proto.privacy.get_or_insert_with(Default::default);
            merge_fields!(
                target,
                source,
                friend_source_flags,
                default_guilds_restricted,
                non_spam_retraining_opt_in,
            );
        }
        if let Some(source) = newer.proto.voice_and_video {
            let target = self
                .proto
                .voice_and_video
                .get_or_insert_with(Default::default);
            merge_fields!(target, source, stream_notifications_enabled);
        }
        if newer.consents.personalization.is_some() {
            self.consents.personalization = newer.consents.personalization;
        }
        if newer.consents.usage_statistics.is_some() {
            self.consents.usage_statistics = newer.consents.usage_statistics;
        }
        if newer.profile.is_some() {
            self.profile = newer.profile;
        }
    }

    pub fn locale(&self) -> Option<&str> {
        self.proto
            .localization
            .as_ref()
            .and_then(|settings| settings.locale.as_deref())
            .or_else(|| self.legacy.locale.as_deref())
    }

    pub fn value(&self, field: SettingsField) -> Option<bool> {
        match field {
            SettingsField::UsageStatisticsConsent => self
                .consents
                .usage_statistics
                .as_ref()
                .map(|status| status.consented),
            SettingsField::PersonalizationConsent => self
                .consents
                .personalization
                .as_ref()
                .map(|status| status.consented),
            SettingsField::ClipsVoiceRecording => self
                .proto
                .clips
                .as_ref()
                .and_then(|settings| settings.allow_voice_recording),
            SettingsField::DefaultGuildsRestricted => self
                .proto
                .privacy
                .as_ref()
                .and_then(|settings| settings.default_guilds_restricted)
                .or(self.legacy.default_guilds_restricted),
            SettingsField::FriendRequestsFromGuildMembers => self
                .proto
                .privacy
                .as_ref()
                .and_then(|settings| settings.friend_source_flags)
                .or(self.legacy.friend_source_flags)
                .map(|flags| flags & (1 << 2) != 0),
            SettingsField::StreamNotifications => self
                .proto
                .voice_and_video
                .as_ref()
                .and_then(|settings| settings.stream_notifications_enabled)
                .or(self.legacy.stream_notifications_enabled),
            SettingsField::FriendAnniversaryNotifications => self
                .proto
                .notifications
                .as_ref()
                .and_then(|settings| settings.enable_friend_anniversary_notifications),
            SettingsField::FriendOnlineNotifications => self
                .proto
                .notifications
                .as_ref()
                .and_then(|settings| settings.enable_friend_online_notifications),
            SettingsField::UpcomingServerEventNotifications => self
                .proto
                .notifications
                .as_ref()
                .and_then(|settings| settings.enable_upcoming_server_event_notifications),
            SettingsField::ProfileUpdatesNotifications => self
                .proto
                .notifications
                .as_ref()
                .and_then(|settings| settings.enable_profile_updates_notifications),
            SettingsField::ReactionNotifications => self
                .proto
                .notifications
                .as_ref()
                .and_then(|settings| settings.reaction_notifications)
                .and_then(ReactionNotificationMode::from_wire)
                .map(|mode| mode != ReactionNotificationMode::Disabled),
        }
    }

    /// Applies a value only after the owning API adapter confirmed the write.
    pub fn set_value(&mut self, field: SettingsField, value: bool) {
        match field {
            SettingsField::UsageStatisticsConsent => {
                self.consents.usage_statistics = Some(ConsentStatus { consented: value });
            }
            SettingsField::PersonalizationConsent => {
                self.consents.personalization = Some(ConsentStatus { consented: value });
            }
            SettingsField::ClipsVoiceRecording => {
                self.proto
                    .clips
                    .get_or_insert_with(Default::default)
                    .allow_voice_recording = Some(value);
            }
            SettingsField::DefaultGuildsRestricted => {
                if let Some(settings) = self.proto.privacy.as_mut() {
                    settings.default_guilds_restricted = Some(value);
                } else {
                    self.legacy.default_guilds_restricted = Some(value);
                }
            }
            SettingsField::FriendRequestsFromGuildMembers => {
                let flags = self
                    .proto
                    .privacy
                    .as_ref()
                    .and_then(|settings| settings.friend_source_flags)
                    .or(self.legacy.friend_source_flags)
                    .unwrap_or(0);
                let flags = if value {
                    flags | (1 << 2)
                } else {
                    flags & !(1 << 2)
                };
                if let Some(settings) = self.proto.privacy.as_mut() {
                    settings.friend_source_flags = Some(flags);
                } else {
                    self.legacy.friend_source_flags = Some(flags);
                }
            }
            SettingsField::StreamNotifications => {
                if let Some(settings) = self.proto.voice_and_video.as_mut() {
                    settings.stream_notifications_enabled = Some(value);
                } else {
                    self.legacy.stream_notifications_enabled = Some(value);
                }
            }
            SettingsField::FriendAnniversaryNotifications => {
                self.proto
                    .notifications
                    .get_or_insert_with(Default::default)
                    .enable_friend_anniversary_notifications = Some(value);
            }
            SettingsField::FriendOnlineNotifications => {
                self.proto
                    .notifications
                    .get_or_insert_with(Default::default)
                    .enable_friend_online_notifications = Some(value);
            }
            SettingsField::UpcomingServerEventNotifications => {
                self.proto
                    .notifications
                    .get_or_insert_with(Default::default)
                    .enable_upcoming_server_event_notifications = Some(value);
            }
            SettingsField::ProfileUpdatesNotifications => {
                self.proto
                    .notifications
                    .get_or_insert_with(Default::default)
                    .enable_profile_updates_notifications = Some(value);
            }
            SettingsField::ReactionNotifications => {
                self.proto
                    .notifications
                    .get_or_insert_with(Default::default)
                    .reaction_notifications = Some(if value { 0 } else { 2 });
            }
        }
    }
}

/// Local RS notification controls. They are intentionally separate from the
/// synchronized account fields above.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalNotificationSettings {
    pub desktop_notifications: Option<bool>,
    pub unread_badge: Option<bool>,
    pub flash_taskbar: Option<bool>,
    pub sound_message: Option<bool>,
    pub sound_voice_join: Option<bool>,
    pub sound_mute_toggle: Option<bool>,
    pub sound_deafen_toggle: Option<bool>,
}

impl LocalNotificationSettings {
    pub fn value(&self, field: LocalNotificationField) -> Option<bool> {
        match field {
            LocalNotificationField::Desktop => self.desktop_notifications,
            LocalNotificationField::UnreadBadge => self.unread_badge,
            LocalNotificationField::FlashTaskbar => self.flash_taskbar,
            LocalNotificationField::MessageSound => self.sound_message,
            LocalNotificationField::VoiceJoinSound => self.sound_voice_join,
            LocalNotificationField::MuteSound => self.sound_mute_toggle,
            LocalNotificationField::DeafenSound => self.sound_deafen_toggle,
        }
    }

    pub fn set_value(&mut self, field: LocalNotificationField, value: bool) {
        match field {
            LocalNotificationField::Desktop => self.desktop_notifications = Some(value),
            LocalNotificationField::UnreadBadge => self.unread_badge = Some(value),
            LocalNotificationField::FlashTaskbar => self.flash_taskbar = Some(value),
            LocalNotificationField::MessageSound => self.sound_message = Some(value),
            LocalNotificationField::VoiceJoinSound => self.sound_voice_join = Some(value),
            LocalNotificationField::MuteSound => self.sound_mute_toggle = Some(value),
            LocalNotificationField::DeafenSound => self.sound_deafen_toggle = Some(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalNotificationField {
    Desktop,
    UnreadBadge,
    FlashTaskbar,
    MessageSound,
    VoiceJoinSound,
    MuteSound,
    DeafenSound,
}

/// Integration boundary owned by Net-Gateway. The Settings UI never performs
/// network I/O directly and never confirms a change before these methods return.
#[async_trait]
pub trait UserSettingsStore: Send + Sync {
    async fn load_settings(&self) -> Result<SettingsHydration, String> {
        Err("Carregamento de configurações ainda não está conectado.".to_string())
    }

    async fn save_setting(&self, field: SettingsField, value: bool) -> Result<(), String> {
        Err(format!(
            "Gravação de {} ainda não está conectada.",
            field.wire_field()
        ))
    }

    async fn save_locale(&self, locale: &str) -> Result<(), String> {
        Err(format!(
            "Gravação do locale {locale} ainda não está conectada."
        ))
    }

    async fn save_profile(&self, patch: &ProfilePatch) -> Result<ProfileSnapshot, String> {
        Err("Salvamento de perfil ainda não está conectado.".to_string())
    }

    async fn logout(&self) -> Result<(), String> {
        Err("Logout ainda não está conectado.".to_string())
    }
}

/// Integration boundary owned by Voice-Audio. Returning `Err` preserves the
/// previous volume; the UI must not display a fabricated success.
pub trait AudioSettingsSink: Send + Sync {
    fn set_input_volume(&self, percent: u32) -> Result<(), String>;
    fn set_output_volume(&self, percent: u32) -> Result<(), String>;
}

pub fn clamp_volume_percent(percent: u32) -> u32 {
    percent.min(100)
}

pub fn supported_locale(code: &str) -> Option<crate::i18n::Locale> {
    match code {
        "pt-BR" => Some(crate::i18n::Locale::PtBr),
        "en-US" => Some(crate::i18n::Locale::EnUs),
        _ => None,
    }
}

pub fn apply_locale(code: &str) -> bool {
    let Some(locale) = supported_locale(code) else {
        return false;
    };
    crate::i18n::set_locale(locale);
    true
}

pub fn theme_from_account(value: &str) -> Option<ThemeChoice> {
    match value {
        "dark" => Some(ThemeChoice::Dark),
        "light" => Some(ThemeChoice::Light),
        "darker" | "midnight" => Some(ThemeChoice::Midnight),
        "system" => Some(ThemeChoice::SyncWithSystem),
        _ => None,
    }
}

impl AppState {
    /// Sets the active settings tab.
    pub fn set_settings_tab(&mut self, tab: SettingsTab) {
        if let Some(ref tel) = self.telemetry {
            tel.track_tab_change(tab.title(), 0);
            let mut meta = std::collections::HashMap::new();
            meta.insert("tab".into(), tab.title().into());
            let guard = tel.start_screen_transition(
                crate::telemetry::ScreenKind::UserSettingsModal {
                    tab: tab.title().into(),
                },
                meta,
            );
            guard.complete(None);
        }
        self.user_settings_tab = tab;
    }

    /// Sets the Nitro subscription tier from an authoritative account payload.
    /// This method never fabricates a subscription or billing metadata.
    pub fn set_nitro_tier(&mut self, tier: NitroTier) {
        self.nitro_tier = tier;
        if tier == NitroTier::None {
            self.active_subscription = None;
        }
    }

    /// Cancels the locally cached active Nitro subscription.
    ///
    /// The API owner must call this only after `PATCH /users/@me/billing/subscriptions`
    /// succeeds. Until then the UI must keep the previous state and show the error.
    pub fn cancel_nitro_subscription(&mut self) {
        self.nitro_tier = NitroTier::None;
        if let Some(ref mut sub) = self.active_subscription {
            sub.status = "canceled".into();
        }
    }

    /// Toggles showing a connected platform on public profile.
    pub fn toggle_connection_profile(&mut self, platform: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.platform == platform) {
            conn.show_on_profile = !conn.show_on_profile;
        }
    }

    /// Toggles broadcasting activity (e.g. Spotify listening along, Steam gaming).
    pub fn toggle_connection_activity(&mut self, platform: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.platform == platform) {
            conn.show_activity = !conn.show_activity;
        }
    }

    /// Disconnects a linked platform from the local cache.
    ///
    /// Production callers must wait for the corresponding Discord API deletion to
    /// succeed before invoking this method.
    pub fn disconnect_platform(&mut self, platform: &str) {
        self.connections.retain(|c| c.platform != platform);
    }

    /// Revokes an authorized app from the local cache.
    ///
    /// Production callers must wait for `DELETE /oauth2/tokens/{token_id}` to
    /// succeed before invoking this method.
    pub fn revoke_authorized_app(&mut self, app_id: &str) {
        self.authorized_apps.retain(|a| a.id != app_id);
    }

    /// Sets the local Discord visual theme choice.
    pub fn set_theme_choice(&mut self, theme: ThemeChoice) {
        self.theme_choice = theme;
    }

    /// Maps the authoritative account appearance theme to the local UI choice.
    /// `Unset` remains absent instead of being converted into an invented theme.
    pub fn account_theme_choice(&self) -> Option<ThemeChoice> {
        use crate::protocol::user_settings_proto::Theme;

        self.account_theme.and_then(|theme| match theme {
            Theme::Dark => Some(ThemeChoice::Dark),
            Theme::Light => Some(ThemeChoice::Light),
            Theme::Darker | Theme::Midnight => Some(ThemeChoice::Midnight),
            Theme::Unset => None,
        })
    }

    /// Sets chat message display mode (Cozy vs Compact).
    pub fn set_message_display_mode(&mut self, mode: MessageDisplayMode) {
        self.message_display_mode = mode;
    }

    /// Sets chat font size, clamped to the supported 12px..=24px range.
    pub fn set_chat_font_size(&mut self, size: f32) {
        self.accessibility_settings.chat_font_size = size.clamp(12.0, 24.0);
    }

    /// Sets motion reduction preference.
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.accessibility_settings.reduce_motion = reduce;
    }

    /// Sets color saturation (0.0 to 1.0).
    pub fn set_color_saturation(&mut self, saturation: f32) {
        self.accessibility_settings.saturation = saturation.clamp(0.0, 1.0);
    }

    /// Sets enhanced contrast preference.
    pub fn set_enhanced_contrast(&mut self, contrast: bool) {
        self.accessibility_settings.high_contrast = contrast;
    }

    /// Sets automatic GIF playback.
    pub fn set_auto_play_gifs(&mut self, auto_play: bool) {
        self.accessibility_settings.auto_play_gifs = auto_play;
    }

    /// Sets automatic sticker playback.
    pub fn set_auto_play_stickers(&mut self, auto_play: bool) {
        self.accessibility_settings.auto_play_stickers = auto_play;
    }

    /// Sets interface zoom, clamped to the supported 70%..=125% range.
    pub fn set_zoom_level(&mut self, zoom: f32) {
        self.accessibility_settings.zoom_level = zoom.clamp(0.70, 1.25);
    }

    /// Stores the selected camera for the active call after the caller confirms it.
    /// This does not claim that video transmission is active.
    pub fn set_camera_device(&mut self, device_id: Option<String>) {
        if let Some(ref mut call) = self.active_voice_call {
            call.selected_camera_device = device_id;
        }
    }
}
