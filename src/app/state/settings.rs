use super::*;
use chrono::Utc;
use crate::domain::settings::AccountConnection;
use crate::domain::settings::MessageDisplayMode;
use crate::domain::settings::NitroTier;
use crate::domain::settings::ThemeChoice;

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

    /// Sets the Discord visual theme choice
    pub fn set_theme_choice(&mut self, theme: ThemeChoice) {
        self.theme_choice = theme;
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
