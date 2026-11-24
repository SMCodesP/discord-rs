use gpui::component::input::{InputEvent, InputState, TextareaState};
use gpui::component::slider::SliderState;
use gpui::prelude::*;
use gpui::*;

use crate::app::state::{AppState, SettingsTab};
use crate::capture::CameraInfo;
use crate::components::TextInput;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::domain::settings::{MessageDisplayMode, NitroTier, ThemeChoice};
use crate::media::devices::MediaDeviceInfo;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

pub use super::settings::{
    AudioSettingsSink, LocalNotificationField, LocalNotificationSettings, PendingSetting,
    ProfileEditState, ProfilePatch, ProfileSnapshot, ProfileValidationError, SettingsField,
    SettingsHydration, SettingsLoadState, SettingsSaveState, UserSettingsStore, apply_locale,
};
use voice_video::MicTestSession;

mod accessibility;
mod appearance;
mod authorized_apps;
mod billing;
pub mod components;
mod connections;
mod family_center;
mod keybinds;
mod language;
mod my_account;
mod notifications;
mod privacy;
mod profiles;
mod voice_video;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeviceMenu {
    Input,
    Output,
    Camera,
}

/// Full-screen user settings modal overlay with Discord fidelity:
/// - Left navigation sidebar with categorized tabs
/// - Right scrollable settings pane
/// - ESC close button
/// - Full interactive state for Profiles, Privacy, Voice & Video, Notifications, Keybinds, and Language
/// - Zero fake mock personas or hardcoded emails/subscriptions
pub struct UserSettingsModal {
    state: Entity<AppState>,
    focus_handle: FocusHandle,
    appearance_subscription: Option<Subscription>,
    applied_theme: Option<ThemeChoice>,

    // --- Profiles State ---
    pub profile_edit: ProfileEditState,
    pub profile_display_input: Option<Entity<InputState>>,
    pub profile_pronouns_input: Option<Entity<InputState>>,
    pub profile_bio_input: Option<Entity<TextareaState>>,
    pub profile_input_subscriptions: Vec<Subscription>,

    // --- Account-backed Settings State ---
    pub account_settings: SettingsHydration,
    pub settings_load_state: SettingsLoadState,
    pub settings_error: Option<String>,
    pub pending_setting: Option<PendingSetting>,
    pub settings_store: Option<Arc<dyn UserSettingsStore>>,
    pub pending_logout: bool,

    // --- Voice & Video State ---
    // Volumes remain absent until a real local/audio preference is observed.
    pub input_volume: Option<u32>,
    pub output_volume: Option<u32>,
    pub input_volume_slider: Entity<SliderState>,
    pub output_volume_slider: Entity<SliderState>,
    pub input_devices: Vec<MediaDeviceInfo>,
    pub output_devices: Vec<MediaDeviceInfo>,
    pub cameras: Vec<CameraInfo>,
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub selected_camera_device: Option<String>,
    pub(super) device_menu: Option<DeviceMenu>,
    pub(super) pending_device: Option<DeviceMenu>,
    pub(super) device_task: Option<Task<()>>,
    pub devices_loaded: bool,
    pub device_error: Option<String>,
    pub audio_sink: Option<Arc<dyn AudioSettingsSink>>,
    pub volume_error: Option<String>,
    pub(super) mic_test: Option<MicTestSession>,
    pub mic_test_task: Option<Task<()>>,
    pub mic_level: f32,
    pub mic_test_error: Option<String>,

    // --- Notifications State ---
    // Local RS controls are separate from account-backed notification toggles.
    pub local_notifications: LocalNotificationSettings,

    // --- Language State ---
    /// Explicit RS-only override. It is applied immediately through `set_locale`
    /// and never masquerades as the synchronized account locale.
    pub local_language: Option<String>,
}

impl UserSettingsModal {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        if let Some(theme) = state.read(cx).account_theme_choice() {
            state.update(cx, |state, cx| {
                state.set_theme_choice(theme);
                cx.notify();
            });
        }

        let profile_snapshot = {
            let s = state.read(cx);
            let call = s.active_voice_call.as_ref();
            let snapshot = ProfileSnapshot {
                display_name: (!s.current_user.display_name.is_empty())
                    .then(|| s.current_user.display_name.clone()),
                bio: None,
                pronouns: None,
                custom_status: s.current_user.custom_status.clone(),
            };
            (
                snapshot,
                call.and_then(|c| c.selected_input_device.clone()),
                call.and_then(|c| c.selected_output_device.clone()),
                call.and_then(|c| c.selected_camera_device.clone()),
            )
        };
        let (profile_snapshot, selected_input, selected_output, selected_camera) = profile_snapshot;
        let mut profile_edit = ProfileEditState::default();
        profile_edit.hydrate(profile_snapshot);

        let input_volume_slider = cx.new(|_| SliderState::new().default_value(0.0));
        let output_volume_slider = cx.new(|_| SliderState::new().default_value(0.0));

        let input_slider_observer = input_volume_slider.clone();
        cx.observe(&input_slider_observer, move |this, slider, cx| {
            let value = slider.read(cx).value().end().round().clamp(0.0, 100.0) as u32;
            if this.input_volume != Some(value) {
                let _ = this.set_input_volume(value, cx);
                cx.notify();
            }
        })
        .detach();

        let output_slider_observer = output_volume_slider.clone();
        cx.observe(&output_slider_observer, move |this, slider, cx| {
            let value = slider.read(cx).value().end().round().clamp(0.0, 100.0) as u32;
            if this.output_volume != Some(value) {
                let _ = this.set_output_volume(value, cx);
                cx.notify();
            }
        })
        .detach();

        Self {
            state,
            focus_handle: cx.focus_handle(),
            appearance_subscription: None,
            applied_theme: None,
            profile_edit,
            profile_display_input: None,
            profile_pronouns_input: None,
            profile_bio_input: None,
            profile_input_subscriptions: Vec::new(),
            account_settings: SettingsHydration::default(),
            settings_load_state: SettingsLoadState::Idle,
            settings_error: None,
            pending_setting: None,
            settings_store: None,
            pending_logout: false,
            input_volume: None,
            output_volume: None,
            input_volume_slider,
            output_volume_slider,
            input_devices: Vec::new(),
            output_devices: Vec::new(),
            cameras: Vec::new(),
            selected_input_device: selected_input,
            selected_output_device: selected_output,
            selected_camera_device: selected_camera,
            device_menu: None,
            pending_device: None,
            device_task: None,
            devices_loaded: false,
            device_error: None,
            audio_sink: None,
            volume_error: None,
            mic_test: None,
            mic_test_task: None,
            mic_level: 0.0,
            mic_test_error: None,
            local_notifications: LocalNotificationSettings::default(),
            local_language: None,
        }
    }

    /// Installs the Net-Gateway adapter and starts an explicit load. Until it is
    /// installed, account fields remain absent and writes fail without mutation.
    pub fn install_settings_store(
        &mut self,
        store: Arc<dyn UserSettingsStore>,
        cx: &mut Context<Self>,
    ) {
        self.settings_store = Some(store);
        self.reload_settings(cx);
    }

    pub fn install_audio_sink(&mut self, sink: Arc<dyn AudioSettingsSink>) {
        self.audio_sink = Some(sink);
    }

    pub fn reload_settings(&mut self, cx: &mut Context<Self>) {
        let Some(store) = self.settings_store.clone() else {
            self.settings_load_state =
                SettingsLoadState::Failed("Carregamento de configurações não conectado.".into());
            cx.notify();
            return;
        };
        self.settings_load_state = SettingsLoadState::Loading;
        self.settings_error = None;
        let entity = cx.entity();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = store.load_settings().await;
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| match result {
                        Ok(hydration) => this.apply_hydration(hydration, cx),
                        Err(error) => {
                            this.settings_load_state = SettingsLoadState::Failed(error.clone());
                            this.settings_error = Some(error);
                            cx.notify();
                        }
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub fn apply_hydration(&mut self, hydration: SettingsHydration, cx: &mut Context<Self>) {
        let account_theme = hydration
            .legacy
            .theme
            .as_deref()
            .and_then(super::settings::theme_from_account);
        let compact = hydration
            .proto
            .text_and_images
            .as_ref()
            .and_then(|settings| settings.message_display_compact)
            .or(hydration.legacy.message_display_compact);
        let gif_auto_play = hydration
            .proto
            .text_and_images
            .as_ref()
            .and_then(|settings| settings.gif_auto_play)
            .or(hydration.legacy.gif_auto_play);
        let sticker_animation = hydration
            .proto
            .text_and_images
            .as_ref()
            .and_then(|settings| settings.animate_stickers)
            .or(hydration.legacy.animate_stickers)
            .and_then(super::settings::StickerAnimation::from_wire);
        let locale = hydration.locale().map(str::to_string);
        let profile = hydration.profile.clone();

        self.account_settings.merge(hydration);
        if let Some(profile) = profile {
            self.profile_edit.hydrate(profile.clone());
            self.state.update(cx, |state, cx| {
                if let Some(display_name) = profile.display_name {
                    state.current_user.display_name = display_name;
                }
                if let Some(custom_status) = profile.custom_status {
                    state.current_user.custom_status = Some(custom_status);
                }
                cx.notify();
            });
        }

        self.state.update(cx, |state, cx| {
            if let Some(theme) = account_theme.or_else(|| state.account_theme_choice()) {
                state.set_theme_choice(theme);
            }
            if let Some(compact) = compact {
                state.set_message_display_mode(if compact {
                    MessageDisplayMode::Compact
                } else {
                    MessageDisplayMode::Cozy
                });
            }
            if let Some(auto_play) = gif_auto_play {
                state.set_auto_play_gifs(auto_play);
            }
            if let Some(animation) = sticker_animation {
                match animation {
                    super::settings::StickerAnimation::Always => state.set_auto_play_stickers(true),
                    super::settings::StickerAnimation::Never => state.set_auto_play_stickers(false),
                    // The current AppState model cannot represent interaction-only playback.
                    super::settings::StickerAnimation::OnInteraction => {}
                }
            }
            cx.notify();
        });

        if self.local_language.is_none() {
            if let Some(locale) = locale {
                let _ = apply_locale(&locale);
            }
        }
        self.settings_load_state = SettingsLoadState::Loaded;
        self.settings_error = None;
        cx.notify();
    }

    pub fn request_setting(&mut self, field: SettingsField, value: bool, cx: &mut Context<Self>) {
        if self.pending_setting.is_some() {
            return;
        }
        let Some(current) = self.account_settings.value(field) else {
            self.settings_error = Some("Este campo ainda não foi hidratado da conta.".into());
            cx.notify();
            return;
        };
        if current == value {
            return;
        }
        let Some(store) = self.settings_store.clone() else {
            self.settings_error =
                Some("Gravação de configurações ainda não está conectada.".into());
            cx.notify();
            return;
        };

        self.settings_error = None;
        self.pending_setting = Some(PendingSetting { field, value });
        let entity = cx.entity();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = store.save_setting(field, value).await;
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.pending_setting = None;
                        match result {
                            Ok(()) => {
                                this.account_settings.set_value(field, value);
                                this.settings_error = None;
                            }
                            Err(error) => this.settings_error = Some(error),
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub fn account_language(&self) -> Option<&str> {
        self.account_settings.locale()
    }

    pub fn effective_language(&self) -> Option<&str> {
        self.local_language
            .as_deref()
            .or_else(|| self.account_language())
    }

    pub fn set_local_language(&mut self, locale: &str, cx: &mut Context<Self>) {
        if !apply_locale(locale) {
            self.settings_error = Some("Idioma local não suportado.".into());
            cx.notify();
            return;
        }
        self.local_language = Some(locale.to_string());
        self.settings_error = None;
        cx.notify();
    }

    pub fn request_account_language(&mut self, locale: &str, cx: &mut Context<Self>) {
        if super::settings::supported_locale(locale).is_none() {
            self.settings_error = Some("Locale da conta não suportado.".into());
            cx.notify();
            return;
        }
        let Some(store) = self.settings_store.clone() else {
            self.settings_error = Some("Gravação do locale ainda não está conectada.".into());
            cx.notify();
            return;
        };
        self.settings_error = None;
        let entity = cx.entity();
        let locale = locale.to_string();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = store.save_locale(&locale).await;
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(()) => {
                                this.account_settings
                                    .proto
                                    .localization
                                    .get_or_insert_with(Default::default)
                                    .locale = Some(locale.clone());
                                if this.local_language.is_none() {
                                    let _ = apply_locale(&locale);
                                }
                                this.settings_error = None;
                            }
                            Err(error) => this.settings_error = Some(error),
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub fn save_profile(&mut self, cx: &mut Context<Self>) {
        if matches!(self.profile_edit.save_state, SettingsSaveState::Saving) {
            return;
        }
        let patch = match self.profile_edit.begin_save() {
            Ok(patch) => patch,
            Err(error) => {
                self.profile_edit.fail_save(error.message().to_string());
                cx.notify();
                return;
            }
        };
        let Some(store) = self.settings_store.clone() else {
            self.profile_edit
                .fail_save("Salvamento de perfil ainda não está conectado.".into());
            cx.notify();
            return;
        };
        let entity = cx.entity();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = store.save_profile(&patch).await;
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(profile) => {
                                this.profile_edit.confirm_saved(profile.clone());
                                this.state.update(cx, |state, cx| {
                                    if let Some(display_name) = profile.display_name {
                                        state.current_user.display_name = display_name;
                                    }
                                    if let Some(custom_status) = profile.custom_status {
                                        state.current_user.custom_status = Some(custom_status);
                                    }
                                    cx.notify();
                                });
                            }
                            Err(error) => this.profile_edit.fail_save(error),
                        }
                        cx.notify();
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    pub fn discard_profile(&mut self, cx: &mut Context<Self>) {
        self.profile_edit.discard();
        cx.notify();
    }

    fn request_logout(&mut self, cx: &mut Context<Self>) {
        if self.pending_logout {
            return;
        }
        let Some(store) = self.settings_store.clone() else {
            self.settings_error = Some("Logout ainda não está conectado.".into());
            cx.notify();
            return;
        };
        self.pending_logout = true;
        self.settings_error = None;
        let entity = cx.entity();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let result = store.logout().await;
                let _ = cx.update(|cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.pending_logout = false;
                        match result {
                            Ok(()) => this.close_settings(cx),
                            Err(error) => {
                                this.settings_error = Some(error);
                                cx.notify();
                            }
                        }
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Voice-Audio owns the real gain path. A missing/failed sink preserves the
    /// previous value and surfaces a recoverable error.
    pub fn set_input_volume(&mut self, value: u32, cx: &mut Context<Self>) -> Result<(), String> {
        let value = super::settings::clamp_volume_percent(value);
        let result = self
            .audio_sink
            .as_ref()
            .ok_or_else(|| "Ganho de entrada ainda não está conectado ao motor.".to_string())
            .and_then(|sink| sink.set_input_volume(value));
        match result {
            Ok(()) => {
                self.input_volume = Some(value);
                self.volume_error = None;
                cx.notify();
                Ok(())
            }
            Err(error) => {
                self.volume_error = Some(error.clone());
                cx.notify();
                Err(error)
            }
        }
    }

    pub fn set_output_volume(&mut self, value: u32, cx: &mut Context<Self>) -> Result<(), String> {
        let value = super::settings::clamp_volume_percent(value);
        let result = self
            .audio_sink
            .as_ref()
            .ok_or_else(|| "Ganho de saída ainda não está conectado ao motor.".to_string())
            .and_then(|sink| sink.set_output_volume(value));
        match result {
            Ok(()) => {
                self.output_volume = Some(value);
                self.volume_error = None;
                cx.notify();
                Ok(())
            }
            Err(error) => {
                self.volume_error = Some(error.clone());
                cx.notify();
                Err(error)
            }
        }
    }

    pub fn hydrate_audio_preferences(
        &mut self,
        input_volume: Option<u32>,
        output_volume: Option<u32>,
    ) {
        self.input_volume = input_volume.map(super::settings::clamp_volume_percent);
        self.output_volume = output_volume.map(super::settings::clamp_volume_percent);
    }

    fn ensure_profile_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile_display_input.is_some()
            && self.profile_pronouns_input.is_some()
            && self.profile_bio_input.is_some()
        {
            return;
        }

        let display_value = self
            .profile_edit
            .draft
            .display_name
            .clone()
            .unwrap_or_default();
        let pronouns_value = self.profile_edit.draft.pronouns.clone().unwrap_or_default();
        let bio_value = self.profile_edit.draft.bio.clone().unwrap_or_default();

        let display = cx.new(|cx| InputState::new(window, cx).default_value(display_value));
        let pronouns = cx.new(|cx| InputState::new(window, cx).default_value(pronouns_value));
        let bio = cx.new(|cx| TextareaState::new(window, cx).default_value(bio_value));

        let display_source = display.clone();
        self.profile_input_subscriptions.push(cx.subscribe_in(
            &display,
            window,
            move |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = display_source.read(cx).value().to_string();
                    this.profile_edit.set_display_name(value);
                    cx.notify();
                }
            },
        ));
        let pronouns_source = pronouns.clone();
        self.profile_input_subscriptions.push(cx.subscribe_in(
            &pronouns,
            window,
            move |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = pronouns_source.read(cx).value().to_string();
                    this.profile_edit.set_pronouns(value);
                    cx.notify();
                }
            },
        ));
        let bio_source = bio.clone();
        self.profile_input_subscriptions.push(cx.subscribe_in(
            &bio,
            window,
            move |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = bio_source.read(cx).value().to_string();
                    this.profile_edit.set_bio(value);
                    cx.notify();
                }
            },
        ));

        self.profile_display_input = Some(display);
        self.profile_pronouns_input = Some(pronouns);
        self.profile_bio_input = Some(bio);
    }

    fn sync_profile_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let display = self
            .profile_edit
            .draft
            .display_name
            .clone()
            .unwrap_or_default();
        let pronouns = self.profile_edit.draft.pronouns.clone().unwrap_or_default();
        let bio = self.profile_edit.draft.bio.clone().unwrap_or_default();
        if let Some(input) = &self.profile_display_input {
            if input.read(cx).value() != display.as_str() {
                input.update(cx, |input, cx| input.set_value(display.into(), window, cx));
            }
        }
        if let Some(input) = &self.profile_pronouns_input {
            if input.read(cx).value() != pronouns.as_str() {
                input.update(cx, |input, cx| input.set_value(pronouns.into(), window, cx));
            }
        }
        if let Some(input) = &self.profile_bio_input {
            if input.read(cx).value() != bio.as_str() {
                input.update(cx, |input, cx| input.set_value(bio.into(), window, cx));
            }
        }
    }

    pub(super) fn stop_mic_test(&mut self) {
        self.mic_test = None;
        self.mic_test_task = None;
        self.mic_level = 0.0;
        self.mic_test_error = None;
    }

    fn close_settings(&mut self, cx: &mut Context<Self>) {
        self.stop_mic_test();
        self.state.update(cx, |state, cx| {
            state.close_modal();
            cx.notify();
        });
    }
}

fn render_settings_status(state: &UserSettingsModal, cx: &App) -> Option<Div> {
    let message = match &state.settings_load_state {
        SettingsLoadState::Idle if state.settings_store.is_none() => {
            Some("As configurações sincronizadas da conta ainda não estão conectadas.")
        }
        SettingsLoadState::Idle | SettingsLoadState::Loading => Some("Carregando configurações..."),
        SettingsLoadState::Loaded => state.settings_error.as_deref(),
        SettingsLoadState::Failed(error) => Some(error.as_str()),
    }?;

    let is_error = !matches!(state.settings_load_state, SettingsLoadState::Loading)
        && (matches!(state.settings_load_state, SettingsLoadState::Failed(_))
            || state.settings_error.is_some());

    Some(
        div()
            .p(px(10.0))
            .rounded(px(6.0))
            .bg(cx.discord().bg_channels)
            .text_color(if is_error {
                cx.discord().status_dnd
            } else {
                cx.discord().text_secondary
            })
            .text_size(px(12.0))
            .child(message),
    )
}

impl Render for UserSettingsModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus_handle.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }

        if self.appearance_subscription.is_none() {
            self.appearance_subscription =
                Some(cx.observe_window_appearance(window, |this, window, cx| {
                    let theme = this.state.read(cx).theme_choice;
                    if theme == ThemeChoice::SyncWithSystem {
                        appearance::apply_theme(theme, window, cx);
                        this.applied_theme = Some(theme);
                        cx.notify();
                    }
                }));
        }

        let theme_choice = self.state.read(cx).theme_choice;
        if self.applied_theme != Some(theme_choice) {
            appearance::apply_theme(theme_choice, window, cx);
            self.applied_theme = Some(theme_choice);
        }

        let active_tab = self.state.read(cx).user_settings_tab;
        if active_tab == SettingsTab::VoiceVideo && !self.devices_loaded {
            self.refresh_devices();
        }
        if self.mic_test.is_some() && self.state.read(cx).active_voice_call.is_some() {
            self.stop_mic_test();
        }
        self.sync_active_call_devices(cx);

        let locale = self.effective_language().map(str::to_string);
        if let Some(locale) = locale {
            let _ = apply_locale(&locale);
        }
        if self.state.read(cx).user_settings_tab == SettingsTab::Profiles {
            self.ensure_profile_inputs(window, cx);
            self.sync_profile_inputs(window, cx);
        }
        let input_volume = self.input_volume.unwrap_or(0) as f32;
        let output_volume = self.output_volume.unwrap_or(0) as f32;
        if self.input_volume_slider.read(cx).value().end != input_volume {
            self.input_volume_slider.update(cx, |slider, cx| {
                slider.set_value(input_volume, window, cx);
            });
        }
        if self.output_volume_slider.read(cx).value().end != output_volume {
            self.output_volume_slider.update(cx, |slider, cx| {
                slider.set_value(output_volume, window, cx);
            });
        }

        let state = self.state.read(cx);
        let current_user = state.current_user.clone();
        let app_entity = self.state.clone();
        let modal_entity = cx.entity().clone();
        let key_modal = modal_entity.clone();

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    key_modal.update(cx, |this, cx| this.close_settings(cx));
                }
            })
            .size_full()
            .flex()
            .flex_row()
            .bg(cx.discord().bg_chat)
            // =========================================================================
            // LEFT SIDEBAR: Navigation Tabs (width 240px, bg #2b2d31)
            // =========================================================================
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .bg(cx.discord().bg_channels)
                    .flex()
                    .flex_col()
                    .items_end()
                    .pt(px(60.0))
                    .pb(px(20.0))
                    .px(px(16.0))
                    .overflow_hidden()
                    .flex_shrink_0()
                    .child(
                        div()
                            .w(px(192.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            // Category: CONFIGURAÇÕES DE USUÁRIO
                            .child(
                                div()
                                    .pb(px(6.0))
                                    .px(px(10.0))
                                    .text_color(cx.discord().text_secondary)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child(settings_text(
                                        "CONFIGURAÇÕES DE USUÁRIO",
                                        "USER SETTINGS",
                                    )),
                            )
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::MyAccount,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Profiles,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Privacy,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::AuthorizedApps,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Connections,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::FamilyCenter,
                                cx,
                            ))
                            // Divider
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .my(px(8.0))
                                    .bg(cx.discord().neutral_67),
                            )
                            // Category: CONFIGURAÇÕES DE COBRANÇA
                            .child(
                                div()
                                    .pb(px(6.0))
                                    .px(px(10.0))
                                    .text_color(cx.discord().text_secondary)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child(settings_text(
                                        "CONFIGURAÇÕES DE COBRANÇA",
                                        "BILLING SETTINGS",
                                    )),
                            )
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Nitro,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::ServerBoost,
                                cx,
                            ))
                            // Divider
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .my(px(8.0))
                                    .bg(cx.discord().neutral_67),
                            )
                            // Category: CONFIGURAÇÕES DO APLICATIVO
                            .child(
                                div()
                                    .pb(px(6.0))
                                    .px(px(10.0))
                                    .text_color(cx.discord().text_secondary)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child(settings_text(
                                        "CONFIGURAÇÕES DO APLICATIVO",
                                        "APP SETTINGS",
                                    )),
                            )
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Appearance,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Accessibility,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::VoiceVideo,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Notifications,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Keybinds,
                                cx,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Language,
                                cx,
                            ))
                            // Divider
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .my(px(8.0))
                                    .bg(cx.discord().neutral_67),
                            )
                            // Logout is performed by the Net-Gateway adapter. The modal
                            // closes only after the adapter confirms server/session cleanup.
                            .child({
                                let modal = modal_entity.clone();
                                let hover_bg = cx.discord().channel_hover;
                                div()
                                    .h(px(32.0))
                                    .px(px(10.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_color(cx.discord().status_dnd)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.0))
                                    .hover(move |s| s.bg(hover_bg))
                                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                        cx.stop_propagation();
                                        modal.update(cx, |this, cx| this.request_logout(cx));
                                    })
                                    .child(if self.pending_logout {
                                        settings_text("Saindo...", "Logging out...")
                                    } else {
                                        settings_text("Sair da Conta", "Log Out")
                                    })
                            }),
                    ),
            )
            // =========================================================================
            // RIGHT CONTENT PANE: Settings Body + Close Button
            // =========================================================================
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .bg(cx.discord().bg_chat)
                    .flex()
                    .flex_row()
                    .justify_between()
                    .pt(px(60.0))
                    .px(px(40.0))
                    .overflow_hidden()
                    // Main settings scrollable content pane
                    .child(
                        div()
                            .id("settings-scroll-pane")
                            .w(px(700.0))
                            .h_full()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(20.0))
                            .pr(px(24.0))
                            .pb(px(80.0))
                            .child(
                                div()
                                    .text_color(cx.discord().text_titles)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(20.0))
                                    .child(localized_settings_title(active_tab)),
                            )
                            .child(render_settings_status(self, cx))
                            .child(match active_tab {
                                SettingsTab::MyAccount => my_account::render_my_account(
                                    app_entity.clone(),
                                    &current_user,
                                    cx,
                                )
                                .into_any_element(),
                                SettingsTab::Profiles => profiles::render_profiles(
                                    modal_entity.clone(),
                                    self,
                                    &current_user,
                                    state.nitro_tier != NitroTier::None,
                                    cx,
                                )
                                .into_any_element(),
                                SettingsTab::Privacy => {
                                    privacy::render_privacy(modal_entity.clone(), self, cx)
                                        .into_any_element()
                                }
                                SettingsTab::AuthorizedApps => {
                                    authorized_apps::render_authorized_apps(
                                        app_entity.clone(),
                                        state,
                                        cx,
                                    )
                                    .into_any_element()
                                }
                                SettingsTab::Connections => {
                                    connections::render_connections(app_entity.clone(), state, cx)
                                        .into_any_element()
                                }
                                SettingsTab::FamilyCenter => family_center::render_family_center(
                                    app_entity.clone(),
                                    state,
                                    cx,
                                )
                                .into_any_element(),
                                SettingsTab::Nitro => {
                                    billing::render_nitro(app_entity.clone(), state, cx)
                                        .into_any_element()
                                }
                                SettingsTab::ServerBoost => {
                                    billing::render_server_boost(app_entity.clone(), state, cx)
                                        .into_any_element()
                                }
                                SettingsTab::Appearance => {
                                    appearance::render_appearance(app_entity.clone(), state, cx)
                                        .into_any_element()
                                }
                                SettingsTab::Accessibility => accessibility::render_accessibility(
                                    app_entity.clone(),
                                    state,
                                    cx,
                                )
                                .into_any_element(),
                                SettingsTab::VoiceVideo => {
                                    voice_video::render_voice_video_expanded(
                                        modal_entity.clone(),
                                        self,
                                        cx,
                                    )
                                    .into_any_element()
                                }
                                SettingsTab::Notifications => notifications::render_notifications(
                                    modal_entity.clone(),
                                    self,
                                    cx,
                                )
                                .into_any_element(),
                                SettingsTab::Keybinds => {
                                    keybinds::render_keybinds(modal_entity.clone(), self, cx)
                                        .into_any_element()
                                }
                                SettingsTab::Language => {
                                    language::render_language(modal_entity.clone(), self, cx)
                                        .into_any_element()
                                }
                            }),
                    )
                    // Floating ESC Close Button
                    .child({
                        let modal = modal_entity.clone();
                        let border = cx.discord().borders;
                        let hover = cx.discord().channel_hover;
                        let hover_border = cx.discord().text_secondary;
                        let icon = cx.discord().text_secondary;
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(4.0))
                            .pt(px(4.0))
                            .child(
                                div()
                                    .size(px(36.0))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(border)
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(icon)
                                    .hover(move |s| s.bg(hover).border_color(hover_border))
                                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                        cx.stop_propagation();
                                        modal.update(cx, |this, cx| this.close_settings(cx));
                                    })
                                    .child(icon_close(px(16.0), icon)),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().text_secondary)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("ESC"),
                            )
                    }),
            )
    }
}

fn settings_text(pt_br: &'static str, en_us: &'static str) -> &'static str {
    match crate::i18n::current_locale() {
        crate::i18n::Locale::EnUs => en_us,
        crate::i18n::Locale::PtBr => pt_br,
    }
}

fn localized_settings_title(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::MyAccount => settings_text("Minha Conta", "My Account"),
        SettingsTab::Profiles => settings_text("Perfis", "Profiles"),
        SettingsTab::Privacy => settings_text("Privacidade & Segurança", "Privacy & Safety"),
        SettingsTab::AuthorizedApps => settings_text("Aplicativos Autorizados", "Authorized Apps"),
        SettingsTab::Connections => settings_text("Conexões", "Connections"),
        SettingsTab::Nitro => settings_text("Nitro", "Nitro"),
        SettingsTab::ServerBoost => settings_text("Impulso de Servidor", "Server Boost"),
        SettingsTab::FamilyCenter => settings_text("Centro da Família", "Family Center"),
        SettingsTab::Appearance => settings_text("Aparência", "Appearance"),
        SettingsTab::Accessibility => settings_text("Acessibilidade", "Accessibility"),
        SettingsTab::VoiceVideo => settings_text("Voz & Vídeo", "Voice & Video"),
        SettingsTab::Notifications => settings_text("Notificações", "Notifications"),
        SettingsTab::Keybinds => settings_text("Atalhos de Teclado", "Keybinds"),
        SettingsTab::Language => settings_text("Idioma", "Language"),
    }
}

/// Renders a single sidebar tab navigation item
fn render_tab_item(
    entity: Entity<AppState>,
    active_tab: SettingsTab,
    tab: SettingsTab,
    cx: &App,
) -> Div {
    let is_selected = active_tab == tab;
    let title = localized_settings_title(tab);

    let bg_color = if is_selected {
        cx.discord().channel_active
    } else {
        cx.discord().transparent
    };
    let text_color = if is_selected {
        cx.discord().white
    } else {
        cx.discord().text_secondary
    };
    let hover_bg = cx.discord().channel_hover;
    let hover_text = cx.discord().text_primary;

    div()
        .h(px(32.0))
        .px(px(10.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .bg(bg_color)
        .text_color(text_color)
        .font_weight(if is_selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_size(px(13.0))
        .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_settings_tab(tab);
                cx.notify();
            });
        })
        .child(title)
}
