//! Desktop notification routing and unread badge accounting.
//!
//! The Linux backend talks to the freedesktop notification service through
//! `notify-rust` (D-Bus `org.freedesktop.Notifications`). All policy decisions
//! live in [`NotificationService`] so tests can inject recording backends and
//! validate mention/DM/mute/DND/own-message handling without touching the host.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::domain::message::Message;
use crate::media::{AudioError, ClientSoundService, CpalSoundPlaybackBackend, SoundEffect, SoundPlaybackBackend};

/// Messages from the same channel within this window are grouped into a single
/// desktop notification. The freedesktop backend reuses the notification id so
/// the notification is replaced instead of stacking.
pub const NOTIFICATION_GROUP_WINDOW: Duration = Duration::from_secs(5);

/// Maximum number of message characters carried by a desktop notification.
pub const NOTIFICATION_BODY_MAX_CHARS: usize = 180;

/// Scope of desktop notifications, matching the Settings labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationScope {
    All,
    Mentions,
    None,
}

impl NotificationScope {
    pub fn from_setting(value: u8) -> Self {
        match value {
            0 => Self::All,
            2 => Self::None,
            _ => Self::Mentions,
        }
    }

    pub fn as_setting(self) -> u8 {
        match self {
            Self::All => 0,
            Self::Mentions => 1,
            Self::None => 2,
        }
    }
}

/// User-facing notification and sound preferences persisted in `AppState`.
///
/// These values are read by the gateway notification path and edited by the
/// Notifications settings tab. Defaults mirror the existing Discord UI defaults
/// already shipped by the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationPreferences {
    pub desktop_notifications: bool,
    pub unread_badge: bool,
    pub flash_taskbar: bool,
    pub notification_scope: u8,
    pub notify_streams: bool,
    pub notify_anniversaries: bool,
    pub notify_friends_online: bool,
    pub notify_server_events: bool,
    pub notify_profile_updates: bool,
    pub notify_reactions: u8,
    pub sound_message: bool,
    pub sound_voice_join: bool,
    pub sound_mute_toggle: bool,
    pub sound_deafen_toggle: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            desktop_notifications: true,
            unread_badge: true,
            flash_taskbar: true,
            notification_scope: NotificationScope::Mentions.as_setting(),
            notify_streams: true,
            notify_anniversaries: true,
            notify_friends_online: true,
            notify_server_events: true,
            notify_profile_updates: true,
            notify_reactions: 0,
            sound_message: true,
            sound_voice_join: true,
            sound_mute_toggle: true,
            sound_deafen_toggle: true,
        }
    }
}

impl NotificationPreferences {
    pub fn scope(&self) -> NotificationScope {
        NotificationScope::from_setting(self.notification_scope)
    }

    /// Whether a message passes the configured scope. Direct messages always
    /// pass; guild messages depend on the selected scope.
    pub fn allows(&self, is_direct_message: bool, is_mention: bool) -> bool {
        if is_direct_message {
            return true;
        }
        match self.scope() {
            NotificationScope::All => true,
            NotificationScope::Mentions => is_mention,
            NotificationScope::None => false,
        }
    }
}

/// Payload sent to the desktop notification backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopNotification {
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub title: String,
    pub body: String,
    pub is_mention: bool,
    pub is_direct_message: bool,
    pub grouped_count: u32,
}

impl DesktopNotification {
    /// Stable per-channel id so a new message replaces the previous desktop
    /// notification for that channel instead of stacking duplicates.
    pub fn notification_id(&self) -> u32 {
        let mut hash: u32 = 2_166_136_261;
        for byte in self.channel_id.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(16_777_619);
        }
        hash.max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationError {
    Unavailable(String),
    Failed(String),
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => write!(f, "notification service unavailable: {message}"),
            Self::Failed(message) => write!(f, "notification failed: {message}"),
        }
    }
}

impl std::error::Error for NotificationError {}

/// Injectable desktop notification sink.
pub trait NotificationBackend: fmt::Debug + Send + Sync {
    fn show(&self, notification: &DesktopNotification) -> Result<(), NotificationError>;

    /// Updates the taskbar/badge integration when the platform exposes one.
    fn set_badge(&self, _unread: u32) {}

    /// Removes grouped state for a channel after it is read.
    fn clear_channel(&self, _channel_id: &str) {}
}

/// Freedesktop/D-Bus notification backend implemented by `notify-rust`.
#[derive(Debug, Clone)]
pub struct FreedesktopNotificationBackend {
    click_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
}

impl FreedesktopNotificationBackend {
    pub fn new(click_tx: tokio::sync::mpsc::UnboundedSender<String>) -> Self {
        Self {
            click_tx: Some(click_tx),
        }
    }

    pub fn without_click_events() -> Self {
        Self { click_tx: None }
    }
}

impl Default for FreedesktopNotificationBackend {
    fn default() -> Self {
        let (tx, rx) = notification_click_channel();
        // Keep the receiver alive for the process lifetime when nobody wires it
        // (for example in tests that only inspect the Decision).
        notification_click_receiver_slot().lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(rx);
        Self::new(tx)
    }
}

impl NotificationBackend for FreedesktopNotificationBackend {
    fn show(&self, notification: &DesktopNotification) -> Result<(), NotificationError> {
        use notify_rust::{Notification, NotificationResponse};

        let mut native = Notification::new();
        native
            .appname("Discord RS")
            .summary(&notification.title)
            .body(&notification.body)
            .id(notification.notification_id())
            .timeout(notify_rust::Timeout::Milliseconds(8_000))
            .action("default", "Abrir");

        let handle = native
            .show()
            .map_err(|err| NotificationError::Unavailable(err.to_string()))?;

        if let Some(tx) = self.click_tx.clone() {
            let channel_id = notification.channel_id.clone();
            std::thread::Builder::new()
                .name("discord-notification-click".into())
                .spawn(move || {
                    handle.wait_for_response(move |response| {
                        let opened = match response {
                            NotificationResponse::Default => true,
                            NotificationResponse::Action(action) => {
                                action == "default" || action == "open"
                            }
                            _ => false,
                        };
                        if opened {
                            let _ = tx.send(channel_id.clone());
                        }
                    });
                })
                .map_err(|err| NotificationError::Failed(err.to_string()))?;
        }

        Ok(())
    }
}

static NOTIFICATION_CLICK_TX: OnceLock<tokio::sync::mpsc::UnboundedSender<String>> = OnceLock::new();
static NOTIFICATION_CLICK_RX: Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>> =
    Mutex::new(None);

fn notification_click_channel() -> (
    tokio::sync::mpsc::UnboundedSender<String>,
    tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    tokio::sync::mpsc::unbounded_channel()
}

fn notification_click_receiver_slot(
) -> &'static Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>> {
    &NOTIFICATION_CLICK_RX
}

fn notification_click_sender() -> tokio::sync::mpsc::UnboundedSender<String> {
    NOTIFICATION_CLICK_TX
        .get_or_init(|| {
            let (tx, rx) = notification_click_channel();
            notification_click_receiver_slot()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert(rx);
            tx
        })
        .clone()
}

/// Takes the receiver used by the UI event loop to open the channel that the
/// user clicked in a desktop notification.
pub fn take_notification_click_receiver(
) -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
    notification_click_receiver_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSuppression {
    OwnMessage,
    SelectedChannel,
    MutedChannel,
    DoNotDisturb,
    Scope,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationDecision {
    pub desktop_shown: bool,
    pub sound_played: bool,
    pub suppression: Option<NotificationSuppression>,
}

impl NotificationDecision {
    fn suppressed(reason: NotificationSuppression) -> Self {
        Self {
            desktop_shown: false,
            sound_played: false,
            suppression: Some(reason),
        }
    }
}

#[derive(Debug, Clone)]
struct NotificationGroup {
    last_at: Instant,
    count: u32,
}

/// Notification policy, grouping and sound playback.
#[derive(Debug, Clone)]
pub struct NotificationService {
    backend: Arc<dyn NotificationBackend>,
    sound_player: Arc<dyn SoundPlaybackBackend>,
    sound: ClientSoundService,
    groups: HashMap<String, NotificationGroup>,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self::with_backend(Arc::new(FreedesktopNotificationBackend::new(
            notification_click_sender(),
        )))
    }

    pub fn with_backend(backend: Arc<dyn NotificationBackend>) -> Self {
        Self::with_backends(backend, Arc::new(CpalSoundPlaybackBackend))
    }

    pub fn with_backends(
        backend: Arc<dyn NotificationBackend>,
        sound_player: Arc<dyn SoundPlaybackBackend>,
    ) -> Self {
        Self {
            backend,
            sound_player,
            sound: ClientSoundService::new(),
            groups: HashMap::new(),
        }
    }

    pub fn backend(&self) -> &Arc<dyn NotificationBackend> {
        &self.backend
    }

    pub fn set_backend(&mut self, backend: Arc<dyn NotificationBackend>) {
        self.backend = backend;
    }

    pub fn set_sound_player(&mut self, sound_player: Arc<dyn SoundPlaybackBackend>) {
        self.sound_player = sound_player;
    }

    pub fn set_unread_badge(&self, unread: u32) {
        self.backend.set_badge(unread);
    }

    pub fn clear_channel(&mut self, channel_id: &str) {
        self.groups.remove(channel_id);
        self.backend.clear_channel(channel_id);
    }

    /// Applies notification policy for an incoming message. Badge/unread state
    /// is owned by `AppState`; this method only handles desktop notifications,
    /// grouping and sounds.
    pub fn notify_message(
        &mut self,
        message: &Message,
        channel_name: &str,
        guild_name: Option<&str>,
        current_user_id: &str,
        is_direct_message: bool,
        is_mention: bool,
        is_muted: bool,
        dnd: bool,
        channel_selected: bool,
        preferences: &NotificationPreferences,
        now: Instant,
    ) -> NotificationDecision {
        if message.is_pending || message.author.id == current_user_id {
            return NotificationDecision::suppressed(NotificationSuppression::OwnMessage);
        }
        if channel_selected {
            return NotificationDecision::suppressed(NotificationSuppression::SelectedChannel);
        }
        if is_muted {
            return NotificationDecision::suppressed(NotificationSuppression::MutedChannel);
        }
        if dnd {
            return NotificationDecision::suppressed(NotificationSuppression::DoNotDisturb);
        }
        if !preferences.allows(is_direct_message, is_mention) {
            return NotificationDecision::suppressed(NotificationSuppression::Scope);
        }
        if !preferences.desktop_notifications && !preferences.sound_message {
            return NotificationDecision::suppressed(NotificationSuppression::Disabled);
        }

        let group = self
            .groups
            .entry(message.channel_id.clone())
            .or_insert_with(|| NotificationGroup {
                last_at: now,
                count: 0,
            });
        if now.saturating_duration_since(group.last_at) <= NOTIFICATION_GROUP_WINDOW {
            group.count = group.count.saturating_add(1);
        } else {
            group.count = 1;
        }
        group.last_at = now;
        let grouped_count = group.count;

        let author = message.author.display_name();
        let channel_label = if let Some(guild) = guild_name {
            format!("#{} · {}", channel_name, guild)
        } else {
            channel_name.to_string()
        };
        let title = if is_direct_message {
            author.to_string()
        } else if !preferences.desktop_notifications {
            String::new()
        } else if grouped_count > 1 {
            format!("{} novas mensagens em {}", grouped_count, channel_label)
        } else if is_mention {
            format!("{} mencionou você em {}", author, channel_label)
        } else {
            channel_label
        };
        let body = notification_body(message);
        let notification = DesktopNotification {
            channel_id: message.channel_id.clone(),
            guild_id: message.guild_id.clone(),
            title,
            body,
            is_mention,
            is_direct_message,
            grouped_count,
        };

        let sound_played = if preferences.sound_message {
            self.sound.policy_mut().set_dnd_suppressed(false);
            let effect = if is_mention {
                SoundEffect::Mention
            } else {
                SoundEffect::Message
            };
            match self.sound.play_at(effect, now) {
                Some(pcm) => match self.sound_player.play(pcm) {
                    Ok(()) => true,
                    Err(err) => {
                        log::warn!("Falha ao tocar som de notificação: {err}");
                        false
                    }
                },
                None => false,
            }
        } else {
            false
        };

        let desktop_shown = if preferences.desktop_notifications {
            match self.backend.show(&notification) {
                Ok(()) => true,
                Err(err) => {
                    log::warn!("Falha ao exibir notificação desktop: {err}");
                    false
                }
            }
        } else {
            false
        };

        NotificationDecision {
            desktop_shown,
            sound_played,
            suppression: None,
        }
    }
}

fn notification_body(message: &Message) -> String {
    let content = message.content.trim();
    let base = if !content.is_empty() {
        content.to_string()
    } else if !message.attachments.is_empty() {
        "Enviou um anexo".to_string()
    } else if !message.sticker_items.is_empty() {
        "Enviou uma figurinha".to_string()
    } else {
        "Nova mensagem".to_string()
    };
    truncate_chars(&base, NOTIFICATION_BODY_MAX_CHARS)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

/// Whether `content` mentions the given user id (or `@everyone`/`@here`).
pub fn message_mentions_user(content: &str, user_id: &str) -> bool {
    if !user_id.is_empty() {
        let plain = format!("<@{user_id}>");
        let nickname = format!("<@!{user_id}>");
        if content.contains(&plain) || content.contains(&nickname) {
            return true;
        }
    }
    content.contains("@everyone") || content.contains("@here")
}

impl crate::app::state::AppState {
    /// Updates unread/mention badges, emits desktop notifications and plays the
    /// configured sound for a real incoming `MESSAGE_CREATE`.
    pub fn notify_incoming_message(&mut self, message: &Message) -> NotificationDecision {
        let own_message = message.is_pending || message.author.id == self.current_user.id;
        let is_guild_message = message.guild_id.is_some()
            || self.find_channel(&message.channel_id).is_some();
        let is_direct_message = !is_guild_message
            || self
                .dm_channels
                .iter()
                .any(|dm| dm.id == message.channel_id);

        let (channel_name, guild_name) = if let Some((guild, channel)) =
            self.find_channel(&message.channel_id)
        {
            (channel.name.clone(), Some(guild.name.clone()))
        } else if let Some(dm) = self
            .dm_channels
            .iter()
            .find(|dm| dm.id == message.channel_id)
        {
            (dm.name.clone(), None)
        } else {
            (message.author.display_name().to_string(), None)
        };

        let is_mention = is_direct_message
            || message_mentions_user(&message.content, &self.current_user.id);
        let selected = self.selected_channel_id.as_deref() == Some(message.channel_id.as_str());
        let muted = self.muted_channels.contains(&message.channel_id);
        let dnd = self.current_user.presence == crate::app::state::PresenceStatus::DoNotDisturb;

        self.apply_message_badges(message, is_guild_message, own_message, selected, is_mention);
        self.notifications
            .set_unread_badge(self.unread_badge_count());

        self.notifications.notify_message(
            message,
            &channel_name,
            guild_name.as_deref(),
            &self.current_user.id,
            is_direct_message,
            is_mention,
            muted,
            dnd,
            selected,
            &self.notification_settings,
            Instant::now(),
        )
    }

    fn apply_message_badges(
        &mut self,
        message: &Message,
        is_guild_message: bool,
        own_message: bool,
        selected: bool,
        is_mention: bool,
    ) {
        if is_guild_message {
            for guild in &mut self.guilds {
                let mut touched = false;
                for category in &mut guild.categories {
                    for channel in &mut category.channels {
                        if channel.id != message.channel_id {
                            continue;
                        }
                        touched = true;
                        channel.last_message_id = Some(message.id.clone());
                        if own_message || selected {
                            channel.unread = false;
                            channel.mention_count = 0;
                        } else {
                            channel.unread = true;
                            if is_mention {
                                channel.mention_count = channel.mention_count.saturating_add(1);
                            }
                        }
                    }
                }
                if touched {
                    guild.mention_count = guild
                        .categories
                        .iter()
                        .flat_map(|category| category.channels.iter())
                        .map(|channel| channel.mention_count)
                        .sum();
                    guild.unread = guild
                        .categories
                        .iter()
                        .flat_map(|category| category.channels.iter())
                        .any(|channel| channel.unread);
                }
            }
        } else if let Some(dm) = self
            .dm_channels
            .iter_mut()
            .find(|dm| dm.id == message.channel_id)
        {
            dm.last_message_id = Some(message.id.clone());
            if own_message || selected {
                dm.unread = false;
                dm.mention_count = 0;
            } else {
                dm.unread = true;
                if is_mention {
                    dm.mention_count = dm.mention_count.saturating_add(1);
                }
            }
        }
    }

    /// Number of channels with unread activity, used for the taskbar/in-app
    /// unread badge.
    pub fn unread_badge_count(&self) -> u32 {
        let guild_unread = self
            .guilds
            .iter()
            .flat_map(|guild| guild.categories.iter())
            .flat_map(|category| category.channels.iter())
            .filter(|channel| channel.unread || channel.mention_count > 0)
            .count();
        let dm_unread = self
            .dm_channels
            .iter()
            .filter(|dm| dm.unread || dm.mention_count > 0)
            .count();
        (guild_unread + dm_unread) as u32
    }

    /// Total unread mentions across guilds and DMs.
    pub fn unread_mention_count(&self) -> u32 {
        let mut total: u32 = self
            .guilds
            .iter()
            .flat_map(|guild| guild.categories.iter())
            .flat_map(|category| category.channels.iter())
            .map(|channel| channel.mention_count)
            .sum();
        total = total.saturating_add(
            self.dm_channels
                .iter()
                .map(|dm| dm.mention_count)
                .sum::<u32>(),
        );
        total
    }

    /// Clears unread/mention badges for a channel after it is read.
    pub fn mark_channel_read(&mut self, channel_id: &str) -> bool {
        let mut changed = false;
        for guild in &mut self.guilds {
            let mut touched = false;
            for category in &mut guild.categories {
                for channel in &mut category.channels {
                    if channel.id == channel_id {
                        changed |= channel.unread || channel.mention_count > 0;
                        channel.unread = false;
                        channel.mention_count = 0;
                        touched = true;
                    }
                }
            }
            if touched {
                guild.mention_count = guild
                    .categories
                    .iter()
                    .flat_map(|category| category.channels.iter())
                    .map(|channel| channel.mention_count)
                    .sum();
                guild.unread = guild
                    .categories
                    .iter()
                    .flat_map(|category| category.channels.iter())
                    .any(|channel| channel.unread);
            }
        }
        if let Some(dm) = self.dm_channels.iter_mut().find(|dm| dm.id == channel_id) {
            changed |= dm.unread || dm.mention_count > 0;
            dm.unread = false;
            dm.mention_count = 0;
        }
        if changed {
            self.notifications.clear_channel(channel_id);
            self.notifications
                .set_unread_badge(self.unread_badge_count());
        }
        changed
    }

    /// Opens the guild/DM and selects the channel referenced by a clicked
    /// desktop notification, then clears its unread badge.
    pub fn handle_notification_click(&mut self, channel_id: &str) -> bool {
        let guild_id = self
            .find_channel(channel_id)
            .map(|(guild, _)| guild.id.clone());
        if let Some(guild_id) = guild_id {
            if self.selected_guild_id.as_deref() != Some(guild_id.as_str()) {
                self.select_guild(Some(guild_id));
            }
        } else if self.dm_channels.iter().any(|dm| dm.id == channel_id) {
            if self.selected_guild_id.is_some() {
                self.select_guild(None);
            }
        } else {
            return false;
        }

        self.select_channel(channel_id.to_string());
        self.mark_channel_read(channel_id);
        true
    }
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::OpusEncodeError(message) => write!(f, "opus encode error: {message}"),
            AudioError::OpusDecodeError(message) => write!(f, "opus decode error: {message}"),
            AudioError::DeviceNotFound(message) => write!(f, "audio device not found: {message}"),
            AudioError::NoDefaultDevice(message) => {
                write!(f, "no default audio device: {message}")
            }
            AudioError::DeviceUnavailable(message) => {
                write!(f, "audio device unavailable: {message}")
            }
            AudioError::StreamError(message) => write!(f, "audio stream error: {message}"),
            AudioError::UnsupportedFormat(message) => {
                write!(f, "unsupported audio format: {message}")
            }
            AudioError::InvalidFrameSize { expected, actual } => {
                write!(f, "invalid frame size: expected {expected}, got {actual}")
            }
            AudioError::InvalidSampleRate(rate) => write!(f, "invalid sample rate: {rate} Hz"),
            AudioError::InvalidChannels(channels) => {
                write!(f, "invalid channel count: {channels}")
            }
        }
    }
}

impl std::error::Error for AudioError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mention_detection_matches_user_and_everyone() {
        assert!(message_mentions_user("oi <@123>", "123"));
        assert!(message_mentions_user("oi <@!123>", "123"));
        assert!(message_mentions_user("oi @everyone", "123"));
        assert!(message_mentions_user("oi @here", "123"));
        assert!(!message_mentions_user("oi <@1234>", "123"));
        assert!(!message_mentions_user("oi", ""));
    }

    #[test]
    fn scope_defaults_to_mentions_and_allows_dms() {
        let prefs = NotificationPreferences::default();
        assert_eq!(prefs.scope(), NotificationScope::Mentions);
        assert!(!prefs.allows(false, false));
        assert!(prefs.allows(false, true));
        assert!(prefs.allows(true, false));
    }

    #[test]
    fn notification_ids_are_stable_and_channel_specific() {
        let a = DesktopNotification {
            channel_id: "chan-a".into(),
            guild_id: None,
            title: String::new(),
            body: String::new(),
            is_mention: false,
            is_direct_message: false,
            grouped_count: 1,
        };
        let mut b = a.clone();
        assert_eq!(a.notification_id(), b.notification_id());
        b.channel_id = "chan-b".into();
        assert_ne!(a.notification_id(), b.notification_id());
    }
}
