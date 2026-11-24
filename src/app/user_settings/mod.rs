use gpui::prelude::*;
use gpui::*;

use crate::app::state::{AppState, SettingsTab};
use crate::components::button::IconButton;
use crate::domain::settings::NitroTier;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

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

/// Full-screen user settings modal overlay with Discord fidelity:
/// - Left navigation sidebar with categorized tabs
/// - Right scrollable settings pane
/// - ESC close button
/// - Full interactive state for Profiles, Privacy, Voice & Video, Notifications, Keybinds, and Language
/// - Zero fake mock personas or hardcoded emails/subscriptions
pub struct UserSettingsModal {
    state: Entity<AppState>,

    // --- Profiles State ---
    pub profile_display_name: String,
    pub profile_pronouns: String,
    pub profile_bio: String,

    // --- Privacy State ---
    pub allow_dm_from_members: bool,
    pub allow_friend_requests_from_members: bool,
    pub explicit_filter: u8, // 0: Proteja-me, 1: Meus amigos são legais, 2: Não filtrar nada
    pub telemetry_improve: bool,
    pub telemetry_personalize: bool,

    // --- Voice & Video Expanded State ---
    pub input_volume: u32,  // 0 a 100
    pub output_volume: u32, // 0 a 100
    pub is_testing_mic: bool,
    pub krisp_enabled: bool,
    pub echo_cancellation: bool,
    pub auto_gain: bool,
    pub video_resolution: u32, // 720 or 1080
    pub video_fps: u32,        // 30 or 60

    // --- Real Hardware Devices (audio & camera) ---
    pub input_devices: Vec<crate::media::MediaDeviceInfo>,
    pub output_devices: Vec<crate::media::MediaDeviceInfo>,
    pub camera_devices: Vec<crate::capture::CameraInfo>,
    pub devices_error: Option<String>,
    pub input_device_menu_open: bool,
    pub output_device_menu_open: bool,
    pub camera_device_menu_open: bool,
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub selected_camera_device: Option<String>,

    // --- Connections State ---
    pub connections_feedback: Option<String>,

    // --- Privacy Expanded State ---
    pub personalize_sponsored_content: bool,
    pub allow_voice_recording_in_clips: bool,

    // --- Notifications State ---
    pub desktop_notifications: bool,
    pub unread_badge: bool,
    pub flash_taskbar: bool,
    pub notification_scope: u8, // 0: Todas, 1: @Menções, 2: Nada
    pub notify_streams: bool,
    pub notify_anniversaries: bool,
    pub notify_friends_online: bool,
    pub notify_server_events: bool,
    pub notify_profile_updates: bool,
    pub notify_reactions: u8, // 0: Todas, 1: @Menções, 2: Nada
    pub sound_message: bool,
    pub sound_voice_join: bool,
    pub sound_mute_toggle: bool,
    pub sound_deafen_toggle: bool,

    // --- Keybinds State ---
    pub is_recording_keybind: bool,

    // --- Language State ---
    pub selected_language: String,
}

impl UserSettingsModal {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        let (display_name, selected_input, selected_output, selected_camera, language) = {
            let s = state.read(cx);
            let call = s.active_voice_call.as_ref();
            (
                s.current_user.display_name.clone(),
                call.and_then(|call| call.selected_input_device.clone()),
                call.and_then(|call| call.selected_output_device.clone()),
                call.and_then(|call| call.selected_camera_device.clone()),
                s.current_language().to_string(),
            )
        };
        let (input_devices, output_devices, camera_devices, devices_error) = enumerate_hardware();

        Self {
            state,
            profile_display_name: display_name,
            profile_pronouns: String::new(),
            // A custom status is not an "About Me" bio: never reuse it as profile data.
            profile_bio: String::new(),

            allow_dm_from_members: true,
            allow_friend_requests_from_members: true,
            explicit_filter: 1, // Meus amigos são legais (padrão)
            telemetry_improve: false,
            telemetry_personalize: true,
            personalize_sponsored_content: true,
            allow_voice_recording_in_clips: true,

            input_volume: 85,
            output_volume: 100,
            is_testing_mic: false,
            krisp_enabled: true,
            echo_cancellation: true,
            auto_gain: true,
            video_resolution: 1080,
            video_fps: 60,

            input_devices,
            output_devices,
            camera_devices,
            devices_error,
            input_device_menu_open: false,
            output_device_menu_open: false,
            camera_device_menu_open: false,
            selected_input_device: selected_input,
            selected_output_device: selected_output,
            selected_camera_device: selected_camera,

            connections_feedback: None,

            desktop_notifications: true,
            unread_badge: true,
            flash_taskbar: true,
            notification_scope: 1, // @Menções
            notify_streams: true,
            notify_anniversaries: true,
            notify_friends_online: true,
            notify_server_events: true,
            notify_profile_updates: true,
            notify_reactions: 0, // Todas as mensagens
            sound_message: true,
            sound_voice_join: true,
            sound_mute_toggle: true,
            sound_deafen_toggle: true,

            is_recording_keybind: false,
            selected_language: language,
        }
    }

    /// Re-enumerates real audio and camera hardware without inventing entries.
    pub fn refresh_devices(&mut self) {
        let (input_devices, output_devices, camera_devices, devices_error) = enumerate_hardware();
        self.input_devices = input_devices;
        self.output_devices = output_devices;
        self.camera_devices = camera_devices;
        self.devices_error = devices_error;
    }
}

/// Enumerates every real audio input/output and camera exposed by the host.
/// Errors are collected as user-facing text; empty lists stay empty.
fn enumerate_hardware() -> (
    Vec<crate::media::MediaDeviceInfo>,
    Vec<crate::media::MediaDeviceInfo>,
    Vec<crate::capture::CameraInfo>,
    Option<String>,
) {
    let manager = crate::media::MediaDeviceManager::new();
    let mut errors = Vec::new();

    let input_devices = match manager.list_input_devices() {
        Ok(devices) => devices,
        Err(err) => {
            errors.push(format!("Microfones: {err}"));
            Vec::new()
        }
    };
    let output_devices = match manager.list_output_devices() {
        Ok(devices) => devices,
        Err(err) => {
            errors.push(format!("Saídas de áudio: {err}"));
            Vec::new()
        }
    };
    let camera_devices = match crate::capture::CameraManager::list_devices() {
        Ok(devices) => devices,
        Err(err) => {
            errors.push(format!("Câmeras: {err}"));
            Vec::new()
        }
    };

    let devices_error = if errors.is_empty() {
        None
    } else {
        Some(errors.join(" | "))
    };
    (input_devices, output_devices, camera_devices, devices_error)
}

impl Render for UserSettingsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let active_tab = state.user_settings_tab;
        let current_user = state.current_user.clone();
        let app_entity = self.state.clone();
        let modal_entity = cx.entity().clone();

        div()
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
                                    .child("CONFIGURAÇÕES DE USUÁRIO"),
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
                                    .child("CONFIGURAÇÕES DE COBRANÇA"),
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
                                    .child("CONFIGURAÇÕES DO APLICATIVO"),
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
                            // Logout Item
                            .child({
                                let entity = app_entity.clone();
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
                                        entity.update(cx, |state, cx| {
                                            state.close_modal();
                                            cx.notify();
                                        });
                                    })
                                    .child("Sair da Conta")
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
                                    .child(active_tab.title()),
                            )
                            .child(match active_tab {
                                SettingsTab::MyAccount => my_account::render_my_account(app_entity.clone(), &current_user, cx).into_any_element(),
                                SettingsTab::Profiles => profiles::render_profiles(modal_entity.clone(), self, &current_user, state.nitro_tier != NitroTier::None, cx).into_any_element(),
                                SettingsTab::Privacy => privacy::render_privacy(modal_entity.clone(), self, cx).into_any_element(),
                                SettingsTab::AuthorizedApps => authorized_apps::render_authorized_apps(app_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::Connections => connections::render_connections(app_entity.clone(), modal_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::FamilyCenter => family_center::render_family_center(app_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::Nitro => billing::render_nitro(app_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::ServerBoost => billing::render_server_boost(app_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::Appearance => appearance::render_appearance(app_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::Accessibility => accessibility::render_accessibility(app_entity.clone(), state, cx).into_any_element(),
                                SettingsTab::VoiceVideo => voice_video::render_voice_video_expanded(modal_entity.clone(), self, cx).into_any_element(),
                                SettingsTab::Notifications => notifications::render_notifications(modal_entity.clone(), self, cx).into_any_element(),
                                SettingsTab::Keybinds => keybinds::render_keybinds(modal_entity.clone(), self, cx).into_any_element(),
                                SettingsTab::Language => language::render_language(app_entity.clone(), state, cx).into_any_element(),
                            }),
                    )
                    // Floating ESC Close Button
                    .child({
                        let entity = app_entity.clone();
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
                                    .border_color(cx.discord().borders)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        IconButton::with_icon("btn-close-settings", |size, color| {
                                            icon_close(size, color).into_any_element()
                                        })
                                        .size(px(32.0))
                                        .icon_size(px(16.0))
                                        .rounded(px(16.0))
                                        .tooltip("ESC")
                                        .on_click(move |_event, _window, cx| {
                                            entity.update(cx, |state, cx| {
                                                state.close_modal();
                                                cx.notify();
                                            });
                                        }),
                                    ),
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

/// Renders a single sidebar tab navigation item
fn render_tab_item(
    entity: Entity<AppState>,
    active_tab: SettingsTab,
    tab: SettingsTab,
    cx: &App,
) -> Div {
    let is_selected = active_tab == tab;
    let title = tab.title();

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
