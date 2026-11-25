use super::*;
use chrono::Utc;
use crate::domain::settings::AccountConnection;
use crate::domain::settings::MessageDisplayMode;
use crate::domain::settings::NitroTier;
use crate::domain::settings::ThemeChoice;
use crate::i18n::{self, Locale};
use crate::media::{AudioError, MediaDeviceInfo, MediaDeviceManager};

impl AppState {
    /// Sets the active settings tab
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

    /// Sets the Nitro subscription tier (Zero Mocks: does not fabricate subscriptions)
    pub fn set_nitro_tier(&mut self, tier: NitroTier) {
        self.nitro_tier = tier;
        if tier == NitroTier::None {
            self.active_subscription = None;
        }
    }

    /// Cancels active Nitro subscription
    pub fn cancel_nitro_subscription(&mut self) {
        self.nitro_tier = NitroTier::None;
        if let Some(ref mut sub) = self.active_subscription {
            sub.status = "canceled".into();
        }
    }

    /// Toggles showing a connected platform on public profile
    pub fn toggle_connection_profile(&mut self, platform: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.platform == platform) {
            conn.show_on_profile = !conn.show_on_profile;
        }
    }

    /// Toggles broadcasting activity (e.g. Spotify listening along, Steam gaming)
    pub fn toggle_connection_activity(&mut self, platform: &str) {
        if let Some(conn) = self.connections.iter_mut().find(|c| c.platform == platform) {
            conn.show_activity = !conn.show_activity;
        }
    }

    /// Connects an account platform or updates existing
    pub fn connect_platform(&mut self, platform: &str, username: &str) {
        if let Some(existing) = self.connections.iter_mut().find(|c| c.platform == platform) {
            existing.name = username.to_string();
        } else {
            self.connections.push(AccountConnection {
                platform: platform.to_string(),
                id: format!("conn-{}-{}", platform, Utc::now().timestamp()),
                name: username.to_string(),
                verified: true,
                show_on_profile: true,
                show_activity: platform == "spotify" || platform == "steam",
            });
        }
    }

    /// Disconnects a linked platform
    pub fn disconnect_platform(&mut self, platform: &str) {
        self.connections.retain(|c| c.platform != platform);
    }

    /// Revokes authorization for an OAuth2 application
    pub fn revoke_authorized_app(&mut self, app_id: &str) {
        self.authorized_apps.retain(|a| a.id != app_id);
    }

    /// Account-provided custom theme colors (gradient stops), if any.
    pub fn custom_theme_colors(&self) -> &[String] {
        self.account_client_theme_settings
            .as_ref()
            .and_then(|settings| settings.custom_user_theme_settings.as_ref())
            .map(|custom| custom.colors.as_slice())
            .unwrap_or(&[])
    }

    /// Applies the current theme choice and account custom colors to the live
    /// GPUI theme. Re-registers colors in place; no restart or window reopen.
    pub fn apply_active_theme(&self, cx: &mut gpui::Context<Self>) {
        crate::tokens::kit_theme::apply_theme(cx, self.theme_choice, self.custom_theme_colors());
    }

    /// Sets the Discord visual theme choice and applies it immediately.
    pub fn set_theme_choice(&mut self, theme: ThemeChoice, cx: &mut gpui::Context<Self>) {
        self.theme_choice = theme;
        self.apply_active_theme(cx);
    }

    /// Applies a UI language understood by the bundled i18n dictionaries.
    ///
    /// Returns `false` (and leaves the current locale untouched) for languages
    /// that this client does not ship yet, so the UI never claims a switch that
    /// did not happen (Zero Mocks).
    pub fn set_language(&mut self, code: &str) -> bool {
        let locale = match code {
            "pt-BR" => Locale::PtBr,
            "en-US" => Locale::EnUs,
            _ => return false,
        };
        i18n::set_locale(locale);
        true
    }

    /// Current UI language code, derived from the real i18n locale.
    pub fn current_language(&self) -> &'static str {
        match i18n::current_locale() {
            Locale::PtBr => "pt-BR",
            Locale::EnUs => "en-US",
        }
    }

    /// Enumerates the real system audio input devices (microphones).
    ///
    /// Returns an empty list when the host exposes no device; it never invents
    /// hardware options.
    pub fn list_audio_input_devices(&self) -> Result<Vec<MediaDeviceInfo>, AudioError> {
        MediaDeviceManager::new().list_input_devices()
    }

    /// Enumerates the real system audio output devices (speakers/headphones).
    pub fn list_audio_output_devices(&self) -> Result<Vec<MediaDeviceInfo>, AudioError> {
        MediaDeviceManager::new().list_output_devices()
    }

    /// Enumerates the real system cameras exposed through V4L2.
    pub fn list_camera_devices(&self) -> Result<Vec<crate::capture::CameraInfo>, crate::capture::CaptureError> {
        crate::capture::CameraManager::list_devices()
    }

    /// Sets chat message display mode (Cozy vs Compact)
    pub fn set_message_display_mode(&mut self, mode: MessageDisplayMode) {
        self.message_display_mode = mode;
    }

    /// Sets chat font size (12px to 24px)
    pub fn set_chat_font_size(&mut self, size: f32) {
        self.accessibility_settings.chat_font_size = size;
    }

    /// Sets motion reduction preference
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.accessibility_settings.reduce_motion = reduce;
    }

    /// Sets color saturation (0.0 to 1.0)
    pub fn set_color_saturation(&mut self, saturation: f32) {
        self.accessibility_settings.saturation = saturation;
    }

    /// Sets enhanced contrast preference
    pub fn set_enhanced_contrast(&mut self, contrast: bool) {
        self.accessibility_settings.high_contrast = contrast;
    }
}
