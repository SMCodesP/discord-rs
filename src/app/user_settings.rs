use gpui::*;
use super::state::{AppState, CurrentUserProfile, SettingsTab};
use crate::domain::settings::{MessageDisplayMode, NitroTier, ThemeChoice};
use crate::tokens::colors::with_alpha;
use crate::tokens::icons::*;

/// Full-screen user settings modal overlay with Discord fidelity:
/// - Left navigation sidebar with categorized tabs
/// - Right scrollable settings pane
/// - ESC close button
/// - Full interactive state for Profiles, Privacy, Voice & Video, Notifications, Keybinds, and Language
pub struct UserSettingsModal {
    state: Entity<AppState>,

    // --- Profiles State ---
    pub profile_display_name: String,
    pub profile_pronouns: String,
    pub profile_bio: String,
    pub profile_banner_color: u32,
    pub profile_badge_nitro: bool,
    pub profile_badge_dev: bool,
    pub profile_badge_hypesquad: bool,

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

    // --- Notifications State ---
    pub desktop_notifications: bool,
    pub unread_badge: bool,
    pub flash_taskbar: bool,
    pub notification_scope: u8, // 0: Todas, 1: @Menções, 2: Nada
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

        Self {
            state,
            profile_display_name: "Ferran Vanguard".to_string(),
            profile_pronouns: "ele/dele".to_string(),
            profile_bio: "Engenheiro de Sistemas & Rustacean 🦀\nConstruindo o Discord RS com GPUI nativo a 120 FPS!".to_string(),
            profile_banner_color: 0x5865f2,
            profile_badge_nitro: true,
            profile_badge_dev: true,
            profile_badge_hypesquad: true,

            allow_dm_from_members: true,
            allow_friend_requests_from_members: true,
            explicit_filter: 1, // Meus amigos são legais (padrão)
            telemetry_improve: false,
            telemetry_personalize: true,

            input_volume: 85,
            output_volume: 100,
            is_testing_mic: false,
            krisp_enabled: true,
            echo_cancellation: true,
            auto_gain: true,
            video_resolution: 1080,
            video_fps: 60,

            desktop_notifications: true,
            unread_badge: true,
            flash_taskbar: true,
            notification_scope: 1, // @Menções
            sound_message: true,
            sound_voice_join: true,
            sound_mute_toggle: true,
            sound_deafen_toggle: true,

            is_recording_keybind: false,
            selected_language: "pt-BR".to_string(),
        }
    }
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
            .bg(rgb(0x313338))
            // =========================================================================
            // LEFT SIDEBAR: Navigation Tabs (width 240px, bg #2b2d31)
            // =========================================================================
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .bg(rgb(0x2b2d31))
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
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("CONFIGURAÇÕES DE USUÁRIO"),
                            )
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::MyAccount,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Profiles,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Privacy,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::AuthorizedApps,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Connections,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::FamilyCenter,
                            ))
                            // Divider
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .my(px(8.0))
                                    .bg(rgb(0x35363c)),
                            )
                            // Category: CONFIGURAÇÕES DE COBRANÇA
                            .child(
                                div()
                                    .pb(px(6.0))
                                    .px(px(10.0))
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("CONFIGURAÇÕES DE COBRANÇA"),
                            )
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Nitro,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::ServerBoost,
                            ))
                            // Divider
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .my(px(8.0))
                                    .bg(rgb(0x35363c)),
                            )
                            // Category: CONFIGURAÇÕES DO APLICATIVO
                            .child(
                                div()
                                    .pb(px(6.0))
                                    .px(px(10.0))
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("CONFIGURAÇÕES DO APLICATIVO"),
                            )
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Appearance,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Accessibility,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::VoiceVideo,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Notifications,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Keybinds,
                            ))
                            .child(render_tab_item(
                                app_entity.clone(),
                                active_tab,
                                SettingsTab::Language,
                            ))
                            // Divider
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .my(px(8.0))
                                    .bg(rgb(0x35363c)),
                            )
                            // Logout Item
                            .child({
                                let entity = app_entity.clone();
                                div()
                                    .h(px(32.0))
                                    .px(px(10.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_color(rgb(0xf23f43))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.0))
                                    .hover(|s| s.bg(rgb(0x35373c)))
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
                    .bg(rgb(0x313338))
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
                                    .text_color(rgb(0xf2f3f5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(20.0))
                                    .child(active_tab.title()),
                            )
                            .child(match active_tab {
                                SettingsTab::MyAccount => render_my_account(&current_user).into_any_element(),
                                SettingsTab::Profiles => self.render_profiles(modal_entity.clone(), &current_user).into_any_element(),
                                SettingsTab::Privacy => self.render_privacy(modal_entity.clone()).into_any_element(),
                                SettingsTab::AuthorizedApps => self.render_authorized_apps(app_entity.clone(), state).into_any_element(),
                                SettingsTab::Connections => self.render_connections(app_entity.clone(), state).into_any_element(),
                                SettingsTab::FamilyCenter => self.render_family_center(app_entity.clone(), state).into_any_element(),
                                SettingsTab::Nitro => self.render_nitro(app_entity.clone(), state).into_any_element(),
                                SettingsTab::ServerBoost => self.render_server_boost(app_entity.clone(), state).into_any_element(),
                                SettingsTab::Appearance => self.render_appearance(app_entity.clone(), state).into_any_element(),
                                SettingsTab::Accessibility => self.render_accessibility(app_entity.clone(), state).into_any_element(),
                                SettingsTab::VoiceVideo => self.render_voice_video_expanded(modal_entity.clone()).into_any_element(),
                                SettingsTab::Notifications => self.render_notifications(modal_entity.clone()).into_any_element(),
                                SettingsTab::Keybinds => self.render_keybinds(modal_entity.clone()).into_any_element(),
                                SettingsTab::Language => self.render_language(modal_entity.clone()).into_any_element(),
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
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(rgb(0x80848e))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(rgb(0xdbdee1))
                                    .hover(|s| {
                                        s.border_color(rgb(0xf2f3f5))
                                            .bg(rgb(0x35373c))
                                            .text_color(rgb(0xffffff))
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.close_modal();
                                            cx.notify();
                                        });
                                    })
                                    .child(icon_close(px(16.0), rgb(0xdbdee1))),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
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
) -> impl IntoElement {
    let is_selected = active_tab == tab;
    let title = tab.title();

    div()
        .h(px(32.0))
        .px(px(10.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .bg(if is_selected {
            rgb(0x3f4248)
        } else {
            rgb(0x2b2d31)
        })
        .text_color(if is_selected {
            rgb(0xffffff)
        } else {
            rgb(0x80848e)
        })
        .font_weight(if is_selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_size(px(13.0))
        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xdbdee1)))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_settings_tab(tab);
                cx.notify();
            });
        })
        .child(title)
}

/// Renders the "Minha Conta" profile card and account details
fn render_my_account(user: &CurrentUserProfile) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        // Profile Card Container
        .child(
            div()
                .rounded(px(8.0))
                .bg(rgb(0x1e1f22))
                .overflow_hidden()
                // Banner
                .child(
                    div()
                        .h(px(100.0))
                        .w_full()
                        .bg(rgb(0x5865f2)),
                )
                // Profile Body
                .child(
                    div()
                        .px(px(16.0))
                        .pb(px(16.0))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_end()
                                .justify_between()
                                .mt(px(-40.0))
                                // Avatar (80px)
                                .child(
                                    div()
                                        .relative()
                                        .w(px(80.0))
                                        .h(px(80.0))
                                        .rounded_full()
                                        .border_4()
                                        .border_color(rgb(0x1e1f22))
                                        .bg(rgb(0x5865f2))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(32.0))
                                        .child(user.avatar_text.clone()),
                                )
                                // Edit Profile Button
                                .child(
                                    div()
                                        .h(px(32.0))
                                        .px(px(16.0))
                                        .rounded(px(4.0))
                                        .bg(rgb(0x5865f2))
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_size(px(13.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0x4752c4)))
                                        .child("Editar perfil de usuário"),
                                ),
                        )
                        // Name and discriminator
                        .child(
                            div()
                                .pt(px(12.0))
                                .child(
                                    div()
                                        .text_color(rgb(0xf2f3f5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(18.0))
                                        .child(user.display_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x80848e))
                                        .text_size(px(13.0))
                                        .child(format!("@{}", user.username)),
                                ),
                        )
                        // Account details rows
                        .child(
                            div()
                                .mt(px(16.0))
                                .p(px(16.0))
                                .rounded(px(8.0))
                                .bg(rgb(0x2b2d31))
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .child(render_info_row("NOME DE EXIBIÇÃO", &user.display_name, "Editar"))
                                .child(render_info_row("NOME DE USUÁRIO", &user.username, "Editar"))
                                .child(render_info_row("E-MAIL", "vanguard@discord-rs.native", "Revelar"))
                                .child(render_info_row("NÚMERO DE TELEFONE", "Você ainda não adicionou um telefone.", "Adicionar")),
                        ),
                ),
        )
}

fn render_info_row(label: &'static str, value: &str, action: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_color(rgb(0x80848e))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child(label),
                )
                .child(
                    div()
                        .text_color(rgb(0xdbdee1))
                        .text_size(px(13.0))
                        .child(value.to_string()),
                ),
        )
        .child(
            div()
                .h(px(32.0))
                .px(px(16.0))
                .rounded(px(4.0))
                .bg(rgb(0x3f4248))
                .text_color(rgb(0xffffff))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(13.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(0x4e5058)))
                .child(action),
        )
}

/// Standard settings toggle row for AppState
fn render_app_toggle_row<F>(
    app_entity: Entity<AppState>,
    title: &'static str,
    description: &'static str,
    is_on: bool,
    action: F,
) -> impl IntoElement
where
    F: Fn(&mut AppState) + 'static + Send + Sync + Clone,
{
    let action_clone = action.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py(px(10.0))
        .child(
            div()
                .flex_1()
                .pr(px(16.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(rgb(0xf2f3f5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.0))
                        .child(title),
                )
                .child(
                    div()
                        .text_color(rgb(0x949ba4))
                        .text_size(px(12.0))
                        .child(description),
                ),
        )
        .child(
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                    let act = action_clone.clone();
                    app_entity.update(cx, |this, cx| {
                        act(this);
                        cx.notify();
                    });
                })
                .child(render_toggle_switch(is_on)),
        )
}

// ============================================================================
// UI Helper Primitives for Discord Settings Fidelity
// ============================================================================

/// Canonical Discord toggle switch (pill with white sliding circle)
fn render_toggle_switch(is_on: bool) -> impl IntoElement {
    div()
        .w(px(40.0))
        .h(px(24.0))
        .rounded_full()
        .p(px(2.0))
        .bg(if is_on { rgb(0x23a55a) } else { rgb(0x4e5058) })
        .flex()
        .items_center()
        .child(
            div()
                .size(px(20.0))
                .rounded_full()
                .bg(rgb(0xffffff))
                .ml(if is_on { px(16.0) } else { px(0.0) }),
        )
}

/// Standard settings toggle row with title, description and clickable switch
fn render_toggle_row<F>(
    modal: Entity<UserSettingsModal>,
    title: &'static str,
    description: &'static str,
    is_on: bool,
    action: F,
) -> impl IntoElement
where
    F: Fn(&mut UserSettingsModal) + 'static + Send + Sync + Clone,
{
    let action_clone = action.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py(px(10.0))
        .child(
            div()
                .flex_1()
                .pr(px(16.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(rgb(0xf2f3f5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.0))
                        .child(title),
                )
                .child(
                    div()
                        .text_color(rgb(0x949ba4))
                        .text_size(px(12.0))
                        .child(description),
                ),
        )
        .child(
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                    let act = action_clone.clone();
                    modal.update(cx, |this, cx| {
                        act(this);
                        cx.notify();
                    });
                })
                .child(render_toggle_switch(is_on)),
        )
}

/// Volume slider with visual percentage track and interactive presets
fn render_slider_control<F>(
    modal: Entity<UserSettingsModal>,
    label: &'static str,
    value: u32,
    set_action: F,
) -> impl IntoElement
where
    F: Fn(&mut UserSettingsModal, u32) + 'static + Send + Sync + Clone,
{
    let fill_ratio = (value as f32 / 100.0).clamp(0.0, 1.0);
    let track_width_px = 440.0;
    let filled_px = track_width_px * fill_ratio;

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_color(rgb(0x80848e))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child(label),
                )
                .child(
                    div()
                        .text_color(rgb(0x5865f2))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child(format!("{}%", value)),
                ),
        )
        // Slider Track
        .child(
            div()
                .w(px(track_width_px))
                .h(px(8.0))
                .rounded_full()
                .bg(rgb(0x4e5058))
                .relative()
                .flex()
                .items_center()
                .child(
                    div()
                        .h(px(8.0))
                        .w(px(filled_px))
                        .rounded_full()
                        .bg(rgb(0x5865f2)),
                )
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_full()
                        .bg(rgb(0xffffff))
                        .border_2()
                        .border_color(rgb(0x5865f2))
                        .ml(px(-8.0)),
                ),
        )
        // Preset Buttons
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .pt(px(2.0))
                .child(render_slider_preset(modal.clone(), "0%", 0, set_action.clone()))
                .child(render_slider_preset(modal.clone(), "25%", 25, set_action.clone()))
                .child(render_slider_preset(modal.clone(), "50%", 50, set_action.clone()))
                .child(render_slider_preset(modal.clone(), "75%", 75, set_action.clone()))
                .child(render_slider_preset(modal.clone(), "100%", 100, set_action)),
        )
}

fn render_slider_preset<F>(
    modal: Entity<UserSettingsModal>,
    text: &'static str,
    val: u32,
    action: F,
) -> impl IntoElement
where
    F: Fn(&mut UserSettingsModal, u32) + 'static + Send + Sync + Clone,
{
    div()
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(rgb(0x2b2d31))
        .text_color(rgb(0x949ba4))
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xf2f3f5)))
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            let act = action.clone();
            modal.update(cx, |this, cx| {
                act(this, val);
                cx.notify();
            });
        })
        .child(text)
}

// ============================================================================
// Implementation of Tab 1: render_profiles (Discord Live Preview Profile Editor)
// ============================================================================

impl UserSettingsModal {
    pub fn render_profiles(
        &self,
        modal: Entity<UserSettingsModal>,
        user: &CurrentUserProfile,
    ) -> impl IntoElement {
        let display_name = self.profile_display_name.clone();
        let username = user.username.clone();
        let pronouns = self.profile_pronouns.clone();
        let bio = self.profile_bio.clone();
        let banner_color = self.profile_banner_color;
        let badge_nitro = self.profile_badge_nitro;
        let badge_dev = self.profile_badge_dev;
        let badge_hypesquad = self.profile_badge_hypesquad;

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Personalize a forma como você é visto em todos os servidores do Discord RS."),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(28.0))
                    // ---------------------------------------------------------
                    // LEFT COLUMN: Profile Form Fields (w = 340px)
                    // ---------------------------------------------------------
                    .child(
                        div()
                            .w(px(340.0))
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            // Display Name Box
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child("NOME DE EXIBIÇÃO"),
                                    )
                                    .child(
                                        div()
                                            .h(px(38.0))
                                            .px(px(12.0))
                                            .rounded(px(4.0))
                                            .bg(rgb(0x1e1f22))
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .text_color(rgb(0xf2f3f5))
                                            .text_size(px(13.0))
                                            .child(display_name.clone())
                                            .child({
                                                let modal_c = modal.clone();
                                                div()
                                                    .text_color(rgb(0x5865f2))
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .hover(|s| s.text_color(rgb(0x4752c4)))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        modal_c.update(cx, |this, cx| {
                                                            if this.profile_display_name.contains("Vanguard") {
                                                                this.profile_display_name = "Ferran (smcodes)".to_string();
                                                            } else {
                                                                this.profile_display_name = "Ferran Vanguard".to_string();
                                                            }
                                                            cx.notify();
                                                        });
                                                    })
                                                    .child("Alternar Nome")
                                            }),
                                    ),
                            )
                            // Pronouns Row
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child("PRONOMES"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(6.0))
                                            .child(self.render_pronoun_pill(modal.clone(), "ele/dele"))
                                            .child(self.render_pronoun_pill(modal.clone(), "ela/dela"))
                                            .child(self.render_pronoun_pill(modal.clone(), "they/them"))
                                            .child(self.render_pronoun_pill(modal.clone(), "elu/delu")),
                                    ),
                            )
                            // Bio (About Me)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child("SOBRE MIM"),
                                    )
                                    .child(
                                        div()
                                            .p(px(10.0))
                                            .rounded(px(4.0))
                                            .bg(rgb(0x1e1f22))
                                            .text_color(rgb(0xdbdee1))
                                            .text_size(px(12.0))
                                            .child(bio.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(6.0))
                                            .child({
                                                let modal_c = modal.clone();
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(px(4.0))
                                                    .bg(rgb(0x2b2d31))
                                                    .text_color(rgb(0x949ba4))
                                                    .text_size(px(11.0))
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xf2f3f5)))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        modal_c.update(cx, |this, cx| {
                                                            this.profile_bio = "Engenheiro de Sistemas & Rustacean 🦀\nConstruindo o Discord RS com GPUI nativo a 120 FPS!".to_string();
                                                            cx.notify();
                                                        });
                                                    })
                                                    .child("🦀 Bio Dev")
                                            })
                                            .child({
                                                let modal_c = modal.clone();
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(px(4.0))
                                                    .bg(rgb(0x2b2d31))
                                                    .text_color(rgb(0x949ba4))
                                                    .text_size(px(11.0))
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xf2f3f5)))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        modal_c.update(cx, |this, cx| {
                                                            this.profile_bio = "Focado em desempenho zero-latency ✨\nLinux Wayland & Vulkan Enthusiast.".to_string();
                                                            cx.notify();
                                                        });
                                                    })
                                                    .child("✨ Bio Minimal")
                                            }),
                                    ),
                            )
                            // Banner Accent Color Swatches
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child("COR DO BANNER / DESTAQUE"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(8.0))
                                            .child(self.render_color_swatch(modal.clone(), 0x5865f2, banner_color))
                                            .child(self.render_color_swatch(modal.clone(), 0x23a55a, banner_color))
                                            .child(self.render_color_swatch(modal.clone(), 0xf23f43, banner_color))
                                            .child(self.render_color_swatch(modal.clone(), 0xf0b232, banner_color))
                                            .child(self.render_color_swatch(modal.clone(), 0x9b59b6, banner_color))
                                            .child(self.render_color_swatch(modal.clone(), 0x1e1f22, banner_color)),
                                    ),
                            )
                            // Badges toggles
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child("DISTINTIVOS ATIVOS"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(8.0))
                                            .child({
                                                let modal_c = modal.clone();
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(px(4.0))
                                                    .bg(if badge_nitro { rgb(0x5865f2) } else { rgb(0x2b2d31) })
                                                    .text_color(rgb(0xffffff))
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        modal_c.update(cx, |this, cx| {
                                                            this.profile_badge_nitro = !this.profile_badge_nitro;
                                                            cx.notify();
                                                        });
                                                    })
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(icon_sparkles(px(13.0), rgb(0xffffff)))
                                                    .child("Nitro")
                                            })
                                            .child({
                                                let modal_c = modal.clone();
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(px(4.0))
                                                    .bg(if badge_dev { rgb(0x23a55a) } else { rgb(0x2b2d31) })
                                                    .text_color(rgb(0xffffff))
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        modal_c.update(cx, |this, cx| {
                                                            this.profile_badge_dev = !this.profile_badge_dev;
                                                            cx.notify();
                                                        });
                                                    })
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(icon_bot(px(13.0), rgb(0xffffff)))
                                                    .child("Dev Ativo")
                                            })
                                            .child({
                                                let modal_c = modal.clone();
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(4.0))
                                                    .rounded(px(4.0))
                                                    .bg(if badge_hypesquad { rgb(0x9b59b6) } else { rgb(0x2b2d31) })
                                                    .text_color(rgb(0xffffff))
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                        modal_c.update(cx, |this, cx| {
                                                            this.profile_badge_hypesquad = !this.profile_badge_hypesquad;
                                                            cx.notify();
                                                        });
                                                    })
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(icon_shield(px(13.0), rgb(0xffffff)))
                                                    .child("HypeSquad")
                                            }),
                                    ),
                            ),
                    )
                    // ---------------------------------------------------------
                    // RIGHT COLUMN: Discord Live Preview Profile Card (w = 300px)
                    // ---------------------------------------------------------
                    .child(
                        div()
                            .w(px(300.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(12.0))
                                    .child("PRÉ-VISUALIZAÇÃO EM TEMPO REAL"),
                            )
                            // Floating Card
                            .child(
                                div()
                                    .rounded(px(8.0))
                                    .bg(rgb(0x1e1f22))
                                    .border_1()
                                    .border_color(rgb(0x2b2d31))
                                    .overflow_hidden()
                                    .flex()
                                    .flex_col()
                                    // Live Banner
                                    .child(
                                        div()
                                            .h(px(90.0))
                                            .w_full()
                                            .bg(rgb(banner_color)),
                                    )
                                    // Profile Body
                                    .child(
                                        div()
                                            .px(px(14.0))
                                            .pb(px(14.0))
                                            .flex()
                                            .flex_col()
                                            // Avatar + Edit Badge + Badges Row
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .justify_between()
                                                    .items_end()
                                                    .mt(px(-36.0))
                                                    .child(
                                                        div()
                                                            .relative()
                                                            .size(px(72.0))
                                                            .rounded_full()
                                                            .border_4()
                                                            .border_color(rgb(0x1e1f22))
                                                            .bg(rgb(0x5865f2))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_color(rgb(0xffffff))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(28.0))
                                                            .child(user.avatar_text.clone())
                                                            // Avatar Edit Pencil Badge
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .top(px(-4.0))
                                                                    .right(px(-4.0))
                                                                    .size(px(22.0))
                                                                    .rounded_full()
                                                                    .bg(rgb(0x2b2d31))
                                                                    .border_1()
                                                                    .border_color(rgb(0x4e5058))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .text_size(px(11.0))
                                                                    .child(icon_pin(px(11.0), rgb(0xdbdee1))),
                                                            ),
                                                    )
                                                    // Badges pill container
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .gap(px(4.0))
                                                            .pb(px(4.0))
                                                            .children(if badge_nitro {
                                                                Some(
                                                                    div()
                                                                        .px(px(6.0))
                                                                        .py(px(2.0))
                                                                        .rounded(px(4.0))
                                                                        .bg(rgb(0x2b2d31))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .child(icon_sparkles(px(13.0), rgb(0xf47fff))),
                                                                )
                                                            } else {
                                                                None
                                                            })
                                                            .children(if badge_dev {
                                                                Some(
                                                                    div()
                                                                        .px(px(6.0))
                                                                        .py(px(2.0))
                                                                        .rounded(px(4.0))
                                                                        .bg(rgb(0x2b2d31))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .child(icon_bot(px(13.0), rgb(0x5865f2))),
                                                                )
                                                            } else {
                                                                None
                                                            })
                                                            .children(if badge_hypesquad {
                                                                Some(
                                                                    div()
                                                                        .px(px(6.0))
                                                                        .py(px(2.0))
                                                                        .rounded(px(4.0))
                                                                        .bg(rgb(0x2b2d31))
                                                                        .flex()
                                                                        .items_center()
                                                                        .justify_center()
                                                                        .child(icon_shield(px(13.0), rgb(0xeb459e))),
                                                                )
                                                            } else {
                                                                None
                                                            }),
                                                    ),
                                            )
                                            // Names
                                            .child(
                                                div()
                                                    .pt(px(10.0))
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0xf2f3f5))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(17.0))
                                                            .child(display_name),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0x949ba4))
                                                            .text_size(px(12.0))
                                                            .child(format!("@{}", username)),
                                                    )
                                                    .child(
                                                        div()
                                                            .pt(px(2.0))
                                                            .text_color(rgb(0xb5bac1))
                                                            .text_size(px(11.0))
                                                            .child(pronouns),
                                                    ),
                                            )
                                            // Divider
                                            .child(
                                                div()
                                                    .w_full()
                                                    .h(px(1.0))
                                                    .my(px(10.0))
                                                    .bg(rgb(0x2b2d31)),
                                            )
                                            // About Me
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(4.0))
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0x80848e))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(10.0))
                                                            .child("SOBRE MIM"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0xdbdee1))
                                                            .text_size(px(12.0))
                                                            .child(bio),
                                                    ),
                                            )
                                            // Divider
                                            .child(
                                                div()
                                                    .w_full()
                                                    .h(px(1.0))
                                                    .my(px(10.0))
                                                    .bg(rgb(0x2b2d31)),
                                            )
                                            // Member Since
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0x80848e))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(10.0))
                                                            .child("MEMBRO DO DISCORD DESDE"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0xdbdee1))
                                                            .text_size(px(11.0))
                                                            .child("14 de jan. de 2020"),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
    }

    fn render_pronoun_pill(&self, modal: Entity<UserSettingsModal>, pronoun: &'static str) -> impl IntoElement {
        let is_selected = self.profile_pronouns == pronoun;
        div()
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .bg(if is_selected { rgb(0x5865f2) } else { rgb(0x1e1f22) })
            .text_color(if is_selected { rgb(0xffffff) } else { rgb(0x949ba4) })
            .text_size(px(11.0))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x35373c)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                modal.update(cx, |this, cx| {
                    this.profile_pronouns = pronoun.to_string();
                    cx.notify();
                });
            })
            .child(pronoun)
    }

    fn render_color_swatch(
        &self,
        modal: Entity<UserSettingsModal>,
        color_hex: u32,
        current_hex: u32,
    ) -> impl IntoElement {
        let is_selected = color_hex == current_hex;
        div()
            .size(px(28.0))
            .rounded_full()
            .bg(rgb(color_hex))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .border_2()
            .border_color(if is_selected { rgb(0xffffff) } else { rgb(0x2b2d31) })
            .hover(|s| s.border_color(rgb(0xf2f3f5)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                modal.update(cx, |this, cx| {
                    this.profile_banner_color = color_hex;
                    cx.notify();
                });
            })
            .children(if is_selected {
                Some(
                    div()
                        .text_color(rgb(0xffffff))
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .child("✓"),
                )
            } else {
                None
            })
    }
}

// ============================================================================
// Implementation of Tab 2: render_privacy (Safety Cards & Data Telemetry)
// ============================================================================

impl UserSettingsModal {
    pub fn render_privacy(&self, modal: Entity<UserSettingsModal>) -> impl IntoElement {
        let allow_dm = self.allow_dm_from_members;
        let allow_req = self.allow_friend_requests_from_members;
        let explicit = self.explicit_filter;
        let tel_improve = self.telemetry_improve;
        let tel_person = self.telemetry_personalize;

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Controle como você interage com outros usuários e gerencie suas permissões de dados."),
            )
            // Section 1: Explicit Content Filter Cards
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("FILTRO DE MENSAGENS COM CONTEÚDO EXPLÍCITO"),
                    )
                    .child(self.render_safety_card(
                        modal.clone(),
                        0,
                        icon_shield(px(20.0), rgb(0x23a55a)).into_any_element(),
                        "Proteja-me",
                        "Analisa e filtra automaticamente mensagens diretas de todos os usuários.",
                        explicit == 0,
                    ))
                    .child(self.render_safety_card(
                        modal.clone(),
                        1,
                        icon_friends(px(20.0), rgb(0x5865f2)).into_any_element(),
                        "Meus amigos são legais",
                        "Analisa mensagens diretas de todos, exceto de seus amigos adicionados.",
                        explicit == 1,
                    ))
                    .child(self.render_safety_card(
                        modal.clone(),
                        2,
                        icon_warning(px(20.0), rgb(0xf0b232)).into_any_element(),
                        "Não filtrar nada",
                        "Você vive perigosamente. Nenhuma mensagem direta será analisada.",
                        explicit == 2,
                    )),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // Section 2: Server Privacy Defaults
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("PADRÕES DE PRIVACIDADE DO SERVIDOR"),
                    )
                    .child(render_toggle_row(
                        modal.clone(),
                        "Permitir mensagens diretas de membros do servidor",
                        "Essa configuração é aplicada quando você entra em um novo servidor. Não afeta servidores existentes.",
                        allow_dm,
                        |this| this.allow_dm_from_members = !this.allow_dm_from_members,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Permitir solicitações de amizade de membros do servidor",
                        "Membros que compartilham servidores com você poderão enviar solicitações de amizade.",
                        allow_req,
                        |this| this.allow_friend_requests_from_members = !this.allow_friend_requests_from_members,
                    )),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // Section 3: Data & Telemetry
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("COMO USAMOS SEUS DADOS"),
                    )
                    .child(render_toggle_row(
                        modal.clone(),
                        "Usar dados para melhorar o Discord RS",
                        "Coleta métricas anônimas de taxa de quadros e latência para otimizar o motor GPUI nativo.",
                        tel_improve,
                        |this| this.telemetry_improve = !this.telemetry_improve,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Usar dados para personalizar minha experiência",
                        "Permite sugerir comunidades, amigos em comum e integrações com base em seu uso.",
                        tel_person,
                        |this| this.telemetry_personalize = !this.telemetry_personalize,
                    ))
                    .child(
                        div()
                            .pt(px(8.0))
                            .child(
                                div()
                                    .h(px(36.0))
                                    .w(px(220.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x3f4248))
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x4e5058)))
                                    .child("Solicitar todos os meus dados"),
                            ),
                    ),
            )
    }

    fn render_safety_card(
        &self,
        modal: Entity<UserSettingsModal>,
        filter_index: u8,
        icon: AnyElement,
        title: &'static str,
        desc: &'static str,
        is_selected: bool,
    ) -> impl IntoElement {
        div()
            .p(px(12.0))
            .rounded(px(8.0))
            .bg(if is_selected { rgb(0x2b2d31) } else { rgb(0x1e1f22) })
            .border_2()
            .border_color(if is_selected { rgb(0x23a55a) } else { rgb(0x2b2d31) })
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .hover(|s| s.bg(rgb(0x2b2d31)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                modal.update(cx, |this, cx| {
                    this.explicit_filter = filter_index;
                    cx.notify();
                });
            })
            // Radio Indicator
            .child(
                div()
                    .size(px(20.0))
                    .rounded_full()
                    .border_2()
                    .border_color(if is_selected { rgb(0x23a55a) } else { rgb(0x80848e) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(if is_selected {
                        Some(
                            div()
                                .size(px(10.0))
                                .rounded_full()
                                .bg(rgb(0x23a55a)),
                        )
                    } else {
                        None
                    }),
            )
            // Icon
            .child(
                div()
                    .size(px(32.0))
                    .rounded_full()
                    .bg(if is_selected { with_alpha(0x23a55a, 0.15) } else { rgb(0x2b2d31) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon),
            )
            // Text Block
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_color(rgb(0xf2f3f5))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(14.0))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x949ba4))
                            .text_size(px(12.0))
                            .child(desc),
                    ),
            )
    }
}

// ============================================================================
// Implementation of Tab 3: render_voice_video_expanded (Audio Sliders, Mic Meter, Krisp, Resolution)
// ============================================================================

impl UserSettingsModal {
    pub fn render_voice_video_expanded(&self, modal: Entity<UserSettingsModal>) -> impl IntoElement {
        let in_vol = self.input_volume;
        let out_vol = self.output_volume;
        let is_testing = self.is_testing_mic;
        let krisp = self.krisp_enabled;
        let echo = self.echo_cancellation;
        let agc = self.auto_gain;
        let res = self.video_resolution;
        let fps = self.video_fps;

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Configure seus dispositivos de áudio, níveis de volume, filtros neurais de ruído e transmissão de tela."),
            )
            // Section 1: Devices
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(20.0))
                    // Input Box
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("DISPOSITIVO DE ENTRADA"),
                            )
                            .child(
                                div()
                                    .h(px(40.0))
                                    .px(px(12.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x1e1f22))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_color(rgb(0xdbdee1))
                                    .text_size(px(13.0))
                                    .child("Microfone Padrão (PipeWire / ALSA)")
                                    .child("⌵"),
                            ),
                    )
                    // Output Box
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("DISPOSITIVO DE SAÍDA"),
                            )
                            .child(
                                div()
                                    .h(px(40.0))
                                    .px(px(12.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x1e1f22))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_color(rgb(0xdbdee1))
                                    .text_size(px(13.0))
                                    .child("Alto-falantes / Fones (Padrão)")
                                    .child("⌵"),
                            ),
                    ),
            )
            // Section 2: Volume Sliders
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .child(render_slider_control(
                        modal.clone(),
                        "VOLUME DE ENTRADA (MICROFONE)",
                        in_vol,
                        |this, v| this.input_volume = v,
                    ))
                    .child(render_slider_control(
                        modal.clone(),
                        "VOLUME DE SAÍDA (ALTO-FALANTES)",
                        out_vol,
                        |this, v| this.output_volume = v,
                    )),
            )
            // Section 3: Mic Test with Live Sound Meter
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("TESTE DE MICROFONE"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(16.0))
                            // Test Button
                            .child({
                                let modal_c = modal.clone();
                                div()
                                    .h(px(38.0))
                                    .px(px(16.0))
                                    .rounded(px(4.0))
                                    .bg(if is_testing { rgb(0xda373c) } else { rgb(0x5865f2) })
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(if is_testing { rgb(0xf23f43) } else { rgb(0x4752c4) }))
                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                        modal_c.update(cx, |this, cx| {
                                            this.is_testing_mic = !this.is_testing_mic;
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(if is_testing {
                                                icon_close(px(14.0), rgb(0xffffff)).into_any_element()
                                            } else {
                                                icon_mic(px(14.0), rgb(0xffffff)).into_any_element()
                                            })
                                            .child(if is_testing { "Parar teste" } else { "Vamos verificar" }),
                                    )
                            })
                            // Equalizer / Audio Meter Container
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(38.0))
                                    .px(px(12.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x1e1f22))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.0))
                                    .children((0..24).map(|i| {
                                        let h = if is_testing {
                                            let pattern = [12.0, 18.0, 26.0, 16.0, 22.0, 30.0, 14.0, 24.0];
                                            pattern[i % 8]
                                        } else {
                                            8.0
                                        };
                                        let col = if is_testing {
                                            if i > 20 { rgb(0xf23f43) } else if i > 15 { rgb(0xf0b232) } else { rgb(0x23a55a) }
                                        } else {
                                            rgb(0x35363c)
                                        };
                                        div()
                                            .w(px(6.0))
                                            .h(px(h))
                                            .rounded(px(2.0))
                                            .bg(col)
                                    }))
                                    .child(
                                        div()
                                            .pl(px(8.0))
                                            .text_color(if is_testing { rgb(0x23a55a) } else { rgb(0x80848e) })
                                            .text_size(px(11.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(if is_testing { "Modulação ativa: -14 dB" } else { "Microfone em repouso" }),
                                    ),
                            ),
                    ),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // Section 4: Advanced Audio Processing (Krisp, Echo, AGC)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("PROCESSAMENTO AVANÇADO DE ÁUDIO"),
                    )
                    .child(render_toggle_row(
                        modal.clone(),
                        "Supressão de Ruído Krisp (Machine Learning)",
                        "Remove ruídos de fundo estáticos, cliques de teclado e ventiladores usando rede neural em tempo real.",
                        krisp,
                        |this| this.krisp_enabled = !this.krisp_enabled,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Cancelamento de Eco Acústico (AEC3)",
                        "Impede que o som dos seus alto-falantes vaze de volta para o microfone durante chamadas.",
                        echo,
                        |this| this.echo_cancellation = !this.echo_cancellation,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Controle Automático de Ganho (AGC)",
                        "Ajusta dinamicamente a sensibilidade do microfone para garantir volume uniforme de fala.",
                        agc,
                        |this| this.auto_gain = !this.auto_gain,
                    )),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // Section 5: Video & Screenshare Resolution/FPS
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("TRANSMISSÃO DE TELA & VÍDEO (WAYLAND / PIPEWIRE)"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(24.0))
                            // Resolution Selector
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x949ba4))
                                            .text_size(px(12.0))
                                            .child("Resolução de Transmissão:"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(8.0))
                                            .child(self.render_option_btn(modal.clone(), "720p (HD)", res == 720, |this| this.video_resolution = 720))
                                            .child(self.render_option_btn(modal.clone(), "1080p (FHD)", res == 1080, |this| this.video_resolution = 1080)),
                                    ),
                            )
                            // FPS Selector
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x949ba4))
                                            .text_size(px(12.0))
                                            .child("Taxa de Quadros (FPS):"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(8.0))
                                            .child(self.render_option_btn(modal.clone(), "30 FPS", fps == 30, |this| this.video_fps = 30))
                                            .child(self.render_option_btn(modal.clone(), "60 FPS (Fluido)", fps == 60, |this| this.video_fps = 60)),
                                    ),
                            ),
                    ),
            )
    }

    fn render_option_btn<F>(
        &self,
        modal: Entity<UserSettingsModal>,
        text: &'static str,
        is_selected: bool,
        action: F,
    ) -> impl IntoElement
    where
        F: Fn(&mut UserSettingsModal) + 'static + Send + Sync + Clone,
    {
        div()
            .h(px(34.0))
            .px(px(14.0))
            .rounded(px(4.0))
            .bg(if is_selected { rgb(0x3f4248) } else { rgb(0x1e1f22) })
            .border_2()
            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x2b2d31) })
            .text_color(if is_selected { rgb(0xffffff) } else { rgb(0x949ba4) })
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(12.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xf2f3f5)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                let act = action.clone();
                modal.update(cx, |this, cx| {
                    act(this);
                    cx.notify();
                });
            })
            .child(text)
    }
}

// ============================================================================
// Implementation of Tab 4: render_notifications (Desktop Popups, Pills, Event Sounds)
// ============================================================================

impl UserSettingsModal {
    pub fn render_notifications(&self, modal: Entity<UserSettingsModal>) -> impl IntoElement {
        let desk = self.desktop_notifications;
        let badge = self.unread_badge;
        let flash = self.flash_taskbar;
        let scope = self.notification_scope;
        let s_msg = self.sound_message;
        let s_join = self.sound_voice_join;
        let s_mute = self.sound_mute_toggle;
        let s_deaf = self.sound_deafen_toggle;

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Gerencie notificações na área de trabalho, selos não lidos e alertas sonoros de eventos."),
            )
            // Section 1: Desktop Toggles
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("NOTIFICAÇÕES NA ÁREA DE TRABALHO"),
                    )
                    .child(render_toggle_row(
                        modal.clone(),
                        "Ativar notificações na área de trabalho",
                        "Exibe popups nativos do sistema operacional quando novas mensagens forem recebidas.",
                        desk,
                        |this| this.desktop_notifications = !this.desktop_notifications,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Ativar selo não lido na barra de tarefas / dock",
                        "Exibe contador numérico vermelho de mensagens não lidas sobre o ícone do aplicativo.",
                        badge,
                        |this| this.unread_badge = !this.unread_badge,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Piscar barra de tarefas ao receber menções",
                        "Pisca suavemente a janela na barra de tarefas ao receber mensagens direcionadas com @você.",
                        flash,
                        |this| this.flash_taskbar = !this.flash_taskbar,
                    )),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // Section 2: Default Notification Scope Pills
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("CONFIGURAÇÕES PADRÃO DE NOTIFICAÇÃO DA CONTA"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(self.render_scope_pill(modal.clone(), 0, "Todas as mensagens", scope == 0))
                            .child(self.render_scope_pill(modal.clone(), 1, "Apenas @menções (Recomendado)", scope == 1))
                            .child(self.render_scope_pill(modal.clone(), 2, "Nada", scope == 2)),
                    ),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // Section 3: Event Sounds List
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("SONS DE EVENTOS DO DISCORD"),
                    )
                    .child(render_toggle_row(
                        modal.clone(),
                        "Mensagem enviada",
                        "Reproduz o som característico de confirmação ao enviar mensagens no chat.",
                        s_msg,
                        |this| this.sound_message = !this.sound_message,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Usuário entrou na chamada",
                        "Alerta sonoro ao conectar em salas de voz ou quando outros usuários entram.",
                        s_join,
                        |this| this.sound_voice_join = !this.sound_voice_join,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Microfone mutado / desmutado",
                        "Bip audível ao ativar ou silenciar o microfone localmente.",
                        s_mute,
                        |this| this.sound_mute_toggle = !this.sound_mute_toggle,
                    ))
                    .child(render_toggle_row(
                        modal.clone(),
                        "Ensurdecer / Desativar ensurdecer",
                        "Som característico ao desligar totalmente o áudio de entrada e saída.",
                        s_deaf,
                        |this| this.sound_deafen_toggle = !this.sound_deafen_toggle,
                    )),
            )
    }

    fn render_scope_pill(
        &self,
        modal: Entity<UserSettingsModal>,
        index: u8,
        label: &'static str,
        is_selected: bool,
    ) -> impl IntoElement {
        div()
            .px(px(12.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(if is_selected { rgb(0x5865f2) } else { rgb(0x1e1f22) })
            .border_1()
            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x2b2d31) })
            .text_color(if is_selected { rgb(0xffffff) } else { rgb(0x949ba4) })
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(12.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x35373c)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                modal.update(cx, |this, cx| {
                    this.notification_scope = index;
                    cx.notify();
                });
            })
            .child(label)
    }
}

// ============================================================================
// Implementation of Tab 5: render_keybinds (Discord Canonical Keybinds Table)
// ============================================================================

impl UserSettingsModal {
    pub fn render_keybinds(&self, modal: Entity<UserSettingsModal>) -> impl IntoElement {
        let is_recording = self.is_recording_keybind;

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Atalhos rápidos para navegação veloz, controle de chamadas de voz e gerenciamento de microfone."),
            )
            // Top Bar: Header + Record Button
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("ATALHOS PADRÃO DO DISCORD RS"),
                    )
                    .child({
                        let modal_c = modal.clone();
                        div()
                            .h(px(32.0))
                            .px(px(14.0))
                            .rounded(px(4.0))
                            .bg(if is_recording { rgb(0xda373c) } else { rgb(0x5865f2) })
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(if is_recording { rgb(0xf23f43) } else { rgb(0x4752c4) }))
                            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                modal_c.update(cx, |this, cx| {
                                    this.is_recording_keybind = !this.is_recording_keybind;
                                    cx.notify();
                                });
                            })
                            .child(if is_recording { "⏹️ Cancelar Gravação" } else { "⌨️ Gravar Novo Atalho" })
                    }),
            )
            // Optional Recording Glow Banner
            .children(if is_recording {
                Some(
                    div()
                        .p(px(12.0))
                        .rounded(px(6.0))
                        .bg(rgb(0x1e1f22))
                        .border_2()
                        .border_color(rgb(0x5865f2))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .size(px(8.0))
                                        .rounded_full()
                                        .bg(rgb(0xda373c)),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x5865f2))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(13.0))
                                        .child("Modo de Gravação Ativo: Pressione qualquer combinação de teclas..."),
                                ),
                        )
                        .child(
                            div()
                                .text_color(rgb(0x949ba4))
                                .text_size(px(12.0))
                                .child("Exemplo: Ctrl + Shift + P ou Alt + F. Clique em 'Cancelar Gravação' para fechar."),
                        ),
                )
            } else {
                None
            })
            // Styled Keybinds Table
            .child(
                div()
                    .rounded(px(8.0))
                    .bg(rgb(0x1e1f22))
                    .border_1()
                    .border_color(rgb(0x2b2d31))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Table Header
                    .child(
                        div()
                            .h(px(36.0))
                            .px(px(16.0))
                            .bg(rgb(0x2b2d31))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("AÇÃO")
                            .child("ATALHO DE TECLADO")
                            .child("CATEGORIA"),
                    )
                    // Rows
                    .child(render_keybind_row("Busca Rápida de Canais e DMs", &["Ctrl", "K"], "Navegação"))
                    .child(render_keybind_row("Navegar para Canal Acima", &["Alt", "Up"], "Navegação"))
                    .child(render_keybind_row("Navegar para Canal Abaixo", &["Alt", "Down"], "Navegação"))
                    .child(render_keybind_row("Alternar Mudo do Microfone", &["Ctrl", "Shift", "M"], "Voz"))
                    .child(render_keybind_row("Alternar Ensurdecer", &["Ctrl", "Shift", "D"], "Voz"))
                    .child(render_keybind_row("Push-To-Talk (PTT)", &["Mouse 4", "Caps Lock"], "Voz / Global"))
                    .child(render_keybind_row("Alternar Painel de Configurações", &["Ctrl", ","], "Sistema"))
                    .child(render_keybind_row("Marcar Canal como Lido", &["Esc"], "Chat"))
                    .child(render_keybind_row("Pular para a Mensagem Mais Recente", &["Shift", "PageDown"], "Chat")),
            )
    }
}

fn render_keybind_row(action: &'static str, keys: &[&'static str], category: &'static str) -> impl IntoElement {
    div()
        .h(px(44.0))
        .px(px(16.0))
        .border_t_1()
        .border_color(rgb(0x2b2d31))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .hover(|s| s.bg(rgb(0x2b2d31)))
        // Action Label
        .child(
            div()
                .w(px(240.0))
                .text_color(rgb(0xf2f3f5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(13.0))
                .child(action),
        )
        // Key Pills
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(keys.iter().enumerate().map(|(idx, key)| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .children(if idx > 0 {
                            Some(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .text_size(px(11.0))
                                    .child("+"),
                            )
                        } else {
                            None
                        })
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded(px(4.0))
                                .bg(rgb(0x2b2d31))
                                .border_1()
                                .border_color(rgb(0x4e5058))
                                .text_color(rgb(0xdbdee1))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(11.0))
                                .child(*key),
                        )
                })),
        )
        // Category Label
        .child(
            div()
                .w(px(100.0))
                .flex()
                .justify_end()
                .text_color(rgb(0x80848e))
                .text_size(px(12.0))
                .child(category),
        )
}

// ============================================================================
// Implementation of Tab 6: render_language (Supported Languages Grid with Checkmarks)
// ============================================================================

impl UserSettingsModal {
    pub fn render_language(&self, modal: Entity<UserSettingsModal>) -> impl IntoElement {
        let current_lang = self.selected_language.clone();

        let languages = [
            ("pt-BR", "🇧🇷", "Português do Brasil", "Portuguese (Brazil)"),
            ("en-US", "🇺🇸", "English (US)", "Inglês (Estados Unidos)"),
            ("es-ES", "🇪🇸", "Español", "Espanhol"),
            ("fr-FR", "🇫🇷", "Français", "Francês"),
            ("de-DE", "🇩🇪", "Deutsch", "Alemão"),
            ("it-IT", "🇮🇹", "Italiano", "Italiano"),
            ("ja-JP", "🇯🇵", "日本語", "Japonês"),
            ("ko-KR", "🇰🇷", "한국어", "Coreano"),
            ("ru-RU", "🇷🇺", "Русский", "Russo"),
            ("zh-CN", "🇨🇳", "简体中文", "Chinês Simplificado"),
            ("pl-PL", "🇵🇱", "Polski", "Polonês"),
            ("nl-NL", "🇳🇱", "Nederlands", "Holandês"),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Selecione o idioma da interface. As alterações são aplicadas instantaneamente em todos os menus do aplicativo."),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(12.0))
                    .children(languages.into_iter().map(|(code, flag, native_name, pt_name)| {
                        let is_selected = current_lang == code;
                        let modal_c = modal.clone();

                        div()
                            .w(px(320.0))
                            .h(px(52.0))
                            .px(px(14.0))
                            .rounded(px(6.0))
                            .bg(if is_selected { rgb(0x2b2d31) } else { rgb(0x1e1f22) })
                            .border_2()
                            .border_color(if is_selected { rgb(0x23a55a) } else { rgb(0x2b2d31) })
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x2b2d31)))
                            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                modal_c.update(cx, |this, cx| {
                                    this.selected_language = code.to_string();
                                    cx.notify();
                                });
                            })
                            // Left: Flag + Names
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .text_size(px(22.0))
                                            .child(flag),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_color(rgb(0xf2f3f5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(px(14.0))
                                                    .child(native_name),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x80848e))
                                                    .text_size(px(11.0))
                                                    .child(pt_name),
                                            ),
                                    ),
                            )
                            // Right: Checkmark Circle
                            .child(
                                div()
                                    .size(px(20.0))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(if is_selected { rgb(0x23a55a) } else { rgb(0x4e5058) })
                                    .bg(if is_selected { rgb(0x23a55a) } else { rgb(0x1e1f22) })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .children(if is_selected {
                                        Some(
                                            div()
                                                .text_color(rgb(0xffffff))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(11.0))
                                                .child("✓"),
                                        )
                                    } else {
                                        None
                                    }),
                            )
                    })),
            )
    }
}

// ============================================================================
// Implementation of Tab: render_nitro (Nitro e Assinaturas)
// ============================================================================

impl UserSettingsModal {
    pub fn render_nitro(&self, app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let current_tier = app_state.nitro_tier;
        let sub = app_state.active_subscription.clone();

        // Official Discord Nitro gradient color stops (#8547c6 -> #b45be6)
        let gradient_stops = [
            0x8547c6, 0x8848c9, 0x8c4acc, 0x904bce, 0x944dd1, 0x984ed4,
            0x9c50d7, 0xa052da, 0xa453dd, 0xa855e0, 0xac56e2, 0xb058e4, 0xb45be6,
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            // -------------------------------------------------------------
            // Official Gradient Nitro Banner (#8547c6 -> #b45be6)
            // -------------------------------------------------------------
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(140.0))
                    .rounded(px(10.0))
                    .overflow_hidden()
                    // Gradient background layer
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_row()
                            .children(gradient_stops.iter().map(|&c| {
                                div()
                                    .flex_1()
                                    .h_full()
                                    .bg(rgb(c))
                            })),
                    )
                    // Content overlay
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .p(px(20.0))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .px(px(8.0))
                                                    .py(px(3.0))
                                                    .rounded_full()
                                                    .bg(rgba(0x00000044))
                                                    .text_color(rgb(0xffffff))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(10.0))
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(icon_sparkles(px(12.0), rgb(0xffffff)))
                                                    .child("DISCORD NITRO"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0xffffff))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(22.0))
                                            .child("Desbloqueie uma experiência sem limites"),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0xf2f3f5))
                                            .text_size(px(13.0))
                                            .child("Turbine seus uploads, transmita em 4K, expresse-se com emojis globais e impulsione servidores."),
                                    ),
                            )
                            .child(
                                div()
                                    .size(px(56.0))
                                    .rounded_full()
                                    .bg(rgba(0xffffff22))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon_sparkles(px(32.0), rgb(0xffffff))),
                            ),
                    ),
            )
            // -------------------------------------------------------------
            // Subscription Management Card (Active Status or Promo)
            // -------------------------------------------------------------
            .child(
                div()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(rgb(0x2b2d31))
                    .border_1()
                    .border_color(rgb(0x35363c))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(11.0))
                                            .child("STATUS DA ASSINATURA"),
                                    )
                                    .children(if current_tier != NitroTier::None {
                                        Some(
                                            div()
                                                .px(px(6.0))
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(rgb(0x23a55a))
                                                .text_color(rgb(0xffffff))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(10.0))
                                                .child("ATIVA"),
                                        )
                                    } else {
                                        None
                                    }),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xf2f3f5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(15.0))
                                    .child(if let Some(ref s) = sub {
                                        format!("Plano Atual: {}", s.plan_name)
                                    } else {
                                        "Você não possui nenhuma assinatura ativa do Nitro no momento.".to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x949ba4))
                                    .text_size(px(12.0))
                                    .child(if current_tier != NitroTier::None {
                                        "Próxima renovação automática em 28 dias via Stripe • Cancele a qualquer momento sem taxas adicionais."
                                    } else {
                                        "Escolha um dos planos abaixo para desbloquear todas as vantagens especiais."
                                    }),
                            ),
                    )
                    .children(if current_tier != NitroTier::None {
                        let entity = app_entity.clone();
                        Some(
                            div()
                                .h(px(34.0))
                                .px(px(14.0))
                                .rounded(px(4.0))
                                .bg(rgb(0xda373c))
                                .text_color(rgb(0xffffff))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(12.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0xf23f43)))
                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.cancel_nitro_subscription();
                                        cx.notify();
                                    });
                                })
                                .child("Cancelar Assinatura"),
                        )
                    } else {
                        None
                    }),
            )
            // -------------------------------------------------------------
            // Nitro Plan Cards: Nitro Basic (R$ 8,99) & Nitro (R$ 24,99)
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(16.0))
                    // Card 1: Nitro Basic
                    .child(self.render_nitro_plan_card(
                        app_entity.clone(),
                        NitroTier::Basic,
                        current_tier,
                        "Nitro Basic",
                        "R$ 8,99",
                        "Recursos essenciais para turbinar seu chat",
                        false,
                        &[
                            "Uploads de até 50 MB",
                            "Emojis e figurinhas personalizados em qualquer lugar",
                            "Emblema especial Nitro no seu perfil",
                            "Transmissão em HD até 1080p 60 FPS",
                            "Reações personalizadas em mensagens",
                        ],
                    ))
                    // Card 2: Nitro Full (Most Popular)
                    .child(self.render_nitro_plan_card(
                        app_entity.clone(),
                        NitroTier::Full,
                        current_tier,
                        "Discord Nitro",
                        "R$ 24,99",
                        "A experiência definitiva com todas as vantagens liberadas",
                        true,
                        &[
                            "Uploads gigantescos de até 500 MB",
                            "Streaming em 4K 60 FPS Ultra HD",
                            "2 Impulsos de Servidor inclusos + 30% off em boosts",
                            "Emojis customizados em qualquer servidor & figurinhas globais",
                            "Temas especiais, perfis customizados e cores com gradiente",
                            "Emblema Nitro animado e exclusivo no perfil",
                            "Mensagens longas de até 4.000 caracteres",
                            "Sons personalizados em qualquer Soundboard",
                        ],
                    )),
            )
            // -------------------------------------------------------------
            // Detailed Comparison Benefits Table
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(12.0))
                            .child("COMPARAÇÃO DE BENEFÍCIOS NITRO"),
                    )
                    .child(
                        div()
                            .rounded(px(8.0))
                            .bg(rgb(0x1e1f22))
                            .border_1()
                            .border_color(rgb(0x2b2d31))
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .h(px(36.0))
                                    .px(px(16.0))
                                    .bg(rgb(0x2b2d31))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("RECURSO")
                                    .child("GRATUITO")
                                    .child("NITRO BASIC (R$ 8,99)")
                                    .child("NITRO (R$ 24,99)"),
                            )
                            .child(render_nitro_compare_row("Upload máximo de arquivos", "25 MB", "50 MB", "500 MB"))
                            .child(render_nitro_compare_row("Qualidade de transmissão de vídeo", "720p 30 FPS", "1080p 60 FPS", "4K 60 FPS (Ultra)"))
                            .child(render_nitro_compare_row("Emojis personalizados em qualquer servidor", "Não", "Sim", "Sim (Animados)"))
                            .child(render_nitro_compare_row("Impulsos de Servidor (Server Boosts)", "Nenhum", "Nenhum", "2 Inclusos (+30% off)"))
                            .child(render_nitro_compare_row("Temas visuais e cores especiais", "Padrão", "Padrão", "Perfis com Gradiente"))
                            .child(render_nitro_compare_row("Distintivo Nitro no Perfil", "Não", "Emblema Basic", "Emblema Especial Animado")),
                    ),
            )
    }

    fn render_nitro_plan_card(
        &self,
        app_entity: Entity<AppState>,
        tier: NitroTier,
        current_tier: NitroTier,
        title: &'static str,
        price: &'static str,
        subtitle: &'static str,
        is_popular: bool,
        perks: &[&'static str],
    ) -> impl IntoElement {
        let is_current = current_tier == tier;
        let entity = app_entity.clone();

        div()
            .flex_1()
            .p(px(18.0))
            .rounded(px(10.0))
            .bg(rgb(0x2b2d31))
            .border_2()
            .border_color(if is_popular {
                rgb(0x8547c6)
            } else if is_current {
                rgb(0x23a55a)
            } else {
                rgb(0x35363c)
            })
            .flex()
            .flex_col()
            .justify_between()
            .gap(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    // Header with optional "MAIS POPULAR" badge
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_color(rgb(0xf2f3f5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(18.0))
                                    .child(title),
                            )
                            .children(if is_popular {
                                Some(
                                    div()
                                        .px(px(8.0))
                                        .py(px(2.0))
                                        .rounded_full()
                                        .bg(rgb(0x8547c6))
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(10.0))
                                        .child("⭐ MAIS POPULAR"),
                                )
                            } else {
                                None
                            }),
                    )
                    // Price
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(24.0))
                                    .child(price),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .text_size(px(13.0))
                                    .child("/ mês"),
                            ),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x949ba4))
                            .text_size(px(12.0))
                            .child(subtitle),
                    )
                    // Perks
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .pt(px(6.0))
                            .children(perks.iter().map(|perk| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(if is_popular {
                                        icon_sparkles(px(14.0), rgb(0xf47fff)).into_any_element()
                                    } else {
                                        icon_check(px(14.0), rgb(0x23a55a)).into_any_element()
                                    })
                                    .child(
                                        div()
                                            .text_color(rgb(0xdbdee1))
                                            .text_size(px(12.0))
                                            .child(*perk),
                                    )
                            })),
                    ),
            )
            // Action button
            .child({
                let mut btn = div()
                    .h(px(38.0))
                    .rounded(px(4.0))
                    .bg(if is_current {
                        rgb(0x35373c)
                    } else if is_popular {
                        rgb(0x8547c6)
                    } else {
                        rgb(0x4e5058)
                    })
                    .text_color(if is_current {
                        rgb(0x80848e)
                    } else {
                        rgb(0xffffff)
                    })
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(13.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer();

                if !is_current {
                    if is_popular {
                        btn = btn.hover(|s| s.bg(rgb(0x9b54d4)));
                    } else {
                        btn = btn.hover(|s| s.bg(rgb(0x5865f2)));
                    }
                }

                btn.on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                    if !is_current {
                        entity.update(cx, |s, cx| {
                            s.set_nitro_tier(tier);
                            cx.notify();
                        });
                    }
                })
                .child(if is_current {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_check(px(14.0), rgb(0x80848e)))
                        .child("Plano Atual")
                        .into_any_element()
                } else if is_popular {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_sparkles(px(14.0), rgb(0xffffff)))
                        .child("Assinar Nitro")
                        .into_any_element()
                } else {
                    div().child("Assinar Nitro Basic").into_any_element()
                })
            })
    }
}

fn render_nitro_compare_row(
    feature: &'static str,
    free: &'static str,
    basic: &'static str,
    nitro: &'static str,
) -> impl IntoElement {
    div()
        .h(px(38.0))
        .px(px(16.0))
        .border_t_1()
        .border_color(rgb(0x2b2d31))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .text_size(px(12.0))
        .hover(|s| s.bg(rgb(0x2b2d31)))
        .child(
            div()
                .w(px(260.0))
                .text_color(rgb(0xf2f3f5))
                .font_weight(FontWeight::SEMIBOLD)
                .child(feature),
        )
        .child(
            div()
                .w(px(100.0))
                .text_color(rgb(0x80848e))
                .child(free),
        )
        .child(
            div()
                .w(px(140.0))
                .text_color(rgb(0xdbdee1))
                .child(basic),
        )
        .child(
            div()
                .w(px(140.0))
                .text_color(rgb(0xb45be6))
                .font_weight(FontWeight::BOLD)
                .child(nitro),
        )
}

// ============================================================================
// Implementation of Tab: render_connections (Conexões de Contas)
// ============================================================================

impl UserSettingsModal {
    pub fn render_connection_badge(platform: &str, size: Pixels) -> Div {
        let (bg, icon_div) = match platform {
            "github" => (
                rgb(0x24292e),
                icon_github(size * 0.6, rgb(0xffffff)),
            ),
            "spotify" => (
                rgb(0x1db954),
                icon_music(size * 0.6, rgb(0xffffff)),
            ),
            "steam" => (
                rgb(0x171a21),
                icon_gamepad(size * 0.6, rgb(0xffffff)),
            ),
            "twitch" => (
                rgb(0x9146ff),
                icon_twitch(size * 0.6, rgb(0xffffff)),
            ),
            "youtube" => (
                rgb(0xff0000),
                icon_youtube(size * 0.6, rgb(0xffffff)),
            ),
            "playstation" => (
                rgb(0x003791),
                icon_gamepad(size * 0.6, rgb(0xffffff)),
            ),
            "xbox" => (
                rgb(0x107c10),
                icon_gamepad(size * 0.6, rgb(0xffffff)),
            ),
            "battlenet" => (
                rgb(0x00aeef),
                icon_gamepad(size * 0.6, rgb(0xffffff)),
            ),
            "reddit" => (
                rgb(0xff4500),
                icon_bot(size * 0.6, rgb(0xffffff)),
            ),
            "tiktok" => (
                rgb(0x010101),
                icon_music(size * 0.6, rgb(0xfe2c55)),
            ),
            _ => (
                rgb(0x5865f2),
                icon_link(size * 0.6, rgb(0xffffff)),
            ),
        };

        div()
            .size(size)
            .rounded(px(6.0))
            .bg(bg)
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .child(icon_div)
    }

    pub fn render_connections(&self, app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let connections = &app_state.connections;

        // 9 official third-party providers specified in the scope
        let providers = [
            ("github", "GitHub", "smcodes"),
            ("spotify", "Spotify", "Ferran Vanguard"),
            ("steam", "Steam", "vanguard_steam"),
            ("twitch", "Twitch", "ferran_live"),
            ("youtube", "YouTube", "Ferran Codes"),
            ("playstation", "PlayStation Network", "PSN_Ferran"),
            ("xbox", "Xbox Live", "Xbox_Vanguard"),
            ("battlenet", "Battle.net", "Ferran#1234"),
            ("reddit", "Reddit", "u/ferran_rust"),
        ];

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Vincule suas contas de terceiros para exibir distintivos no seu perfil e compartilhar suas atividades de jogos e músicas em tempo real."),
            )
            // -------------------------------------------------------------
            // Grid of 9 Providers (Conectar Contas)
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("CONECTAR UMA CONTA"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(10.0))
                            .children(providers.into_iter().map(|(platform, display_name, default_user)| {
                                let is_connected = connections.iter().any(|c| c.platform == platform);
                                let entity = app_entity.clone();

                                div()
                                    .w(px(210.0))
                                    .h(px(52.0))
                                    .px(px(12.0))
                                    .rounded(px(6.0))
                                    .bg(rgb(0x2b2d31))
                                    .border_1()
                                    .border_color(if is_connected { rgb(0x23a55a) } else { rgb(0x35363c) })
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x35373c)))
                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                        entity.update(cx, |s, cx| {
                                            if is_connected {
                                                s.disconnect_platform(platform);
                                            } else {
                                                s.connect_platform(platform, default_user);
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(Self::render_connection_badge(platform, px(28.0)))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0xf2f3f5))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_size(px(13.0))
                                                            .child(display_name),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(if is_connected { rgb(0x23a55a) } else { rgb(0x80848e) })
                                                            .text_size(px(11.0))
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap(px(3.0))
                                                            .children(if is_connected {
                                                                Some(icon_check(px(10.0), rgb(0x23a55a)))
                                                            } else {
                                                                None
                                                            })
                                                            .child(if is_connected { "Conectado" } else { "Clique para conectar" }),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .size(px(18.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if is_connected {
                                                icon_close(px(12.0), rgb(0x80848e)).into_any_element()
                                            } else {
                                                icon_plus(px(12.0), rgb(0x80848e)).into_any_element()
                                            }),
                                    )
                            })),
                    ),
            )
            // -------------------------------------------------------------
            // Active Linked Accounts List
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("CONTAS CONECTADAS"),
                    )
                    .children(if connections.is_empty() {
                        vec![
                            div()
                                .p(px(24.0))
                                .rounded(px(8.0))
                                .bg(rgb(0x2b2d31))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0x80848e))
                                .text_size(px(13.0))
                                .child("Nenhuma conta de terceiro vinculada no momento. Selecione um provedor acima para conectar.")
                        ]
                    } else {
                        connections.iter().map(|conn| {
                            let platform = conn.platform.clone();
                            let plat_for_prof = platform.clone();
                            let plat_for_act = platform.clone();
                            let plat_for_disc = platform.clone();
                            let entity_prof = app_entity.clone();
                            let entity_act = app_entity.clone();
                            let entity_disc = app_entity.clone();
                            let is_spotify = conn.platform == "spotify";

                            div()
                                .p(px(16.0))
                                .rounded(px(8.0))
                                .bg(rgb(0x2b2d31))
                                .border_1()
                                .border_color(rgb(0x35363c))
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                // Card Top Bar
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(10.0))
                                                .child(Self::render_connection_badge(&conn.platform, px(36.0)))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_row()
                                                                .items_center()
                                                                .gap(px(6.0))
                                                                .child(
                                                                    div()
                                                                        .text_color(rgb(0xf2f3f5))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_size(px(15.0))
                                                                        .child(conn.name.clone()),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .px(px(6.0))
                                                                        .py(px(2.0))
                                                                        .rounded(px(4.0))
                                                                        .bg(rgb(0x5865f2))
                                                                        .text_color(rgb(0xffffff))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_size(px(10.0))
                                                                        .flex()
                                                                        .flex_row()
                                                                        .items_center()
                                                                        .gap(px(4.0))
                                                                        .child(icon_check(px(10.0), rgb(0xffffff)))
                                                                        .child("Verificado"),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_color(rgb(0x80848e))
                                                                .text_size(px(12.0))
                                                                .child(conn.platform_display_name()),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .h(px(30.0))
                                                .px(px(12.0))
                                                .rounded(px(4.0))
                                                .bg(rgb(0x1e1f22))
                                                .text_color(rgb(0xf23f43))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(px(12.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .hover(|s| s.bg(rgb(0xda373c)).text_color(rgb(0xffffff)))
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    let p = plat_for_disc.clone();
                                                    entity_disc.update(cx, |s, cx| {
                                                        s.disconnect_platform(&p);
                                                        cx.notify();
                                                    });
                                                })
                                                .child("Desconectar"),
                                        ),
                                )
                                // Divider
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(1.0))
                                        .bg(rgb(0x35363c)),
                                )
                                // Toggle 1: Exibir no perfil
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xf2f3f5))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_size(px(13.0))
                                                        .child("Exibir no perfil"),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(rgb(0x949ba4))
                                                        .text_size(px(12.0))
                                                        .child("Exibe este vínculo e o nome da conta diretamente no seu perfil público do Discord."),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    let p = plat_for_prof.clone();
                                                    entity_prof.update(cx, |s, cx| {
                                                        s.toggle_connection_profile(&p);
                                                        cx.notify();
                                                    });
                                                })
                                                .child(render_toggle_switch(conn.show_on_profile)),
                                        ),
                                )
                                // Toggle 2: Exibir atividade como status (Spotify ao vivo, etc.)
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(2.0))
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xf2f3f5))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_size(px(13.0))
                                                        .child(if is_spotify {
                                                            "Exibir atividade do Spotify ao vivo"
                                                        } else {
                                                            "Exibir atividade como status"
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(rgb(0x949ba4))
                                                        .text_size(px(12.0))
                                                        .child(if is_spotify {
                                                            "Transmite em tempo real a música que você está ouvindo com botão 'Ouvir junto' para seus amigos."
                                                        } else {
                                                            "Transmite em tempo real suas atividades e jogos nesta plataforma para seus amigos."
                                                        }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    let p = plat_for_act.clone();
                                                    entity_act.update(cx, |s, cx| {
                                                        s.toggle_connection_activity(&p);
                                                        cx.notify();
                                                    });
                                                })
                                                .child(render_toggle_switch(conn.show_activity)),
                                        ),
                                )
                        }).collect()
                    }),
            )
    }
}

// ============================================================================
// Implementation of Tab: render_authorized_apps & Family Center
// ============================================================================

impl UserSettingsModal {
    pub fn render_authorized_apps(&self, app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let authorized_apps = &app_state.authorized_apps;

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Estes aplicativos de terceiros têm acesso à sua conta através de integrações OAuth2. Você pode revogar o acesso a qualquer momento."),
            )
            // Authorized apps list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .children(if authorized_apps.is_empty() {
                        vec![
                            div()
                                .p(px(24.0))
                                .rounded(px(8.0))
                                .bg(rgb(0x2b2d31))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0x80848e))
                                .text_size(px(13.0))
                                .child("Nenhum aplicativo autorizado no momento.")
                        ]
                    } else {
                        authorized_apps.iter().map(|app| {
                            let app_id = app.id.clone();
                            let entity = app_entity.clone();
                            let initial = app.name.chars().next().unwrap_or('A').to_string();

                            div()
                                .p(px(16.0))
                                .rounded(px(8.0))
                                .bg(rgb(0x2b2d31))
                                .border_1()
                                .border_color(rgb(0x35363c))
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                // Top row
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(12.0))
                                                .child(
                                                    div()
                                                        .size(px(40.0))
                                                        .rounded(px(8.0))
                                                        .bg(rgb(0x5865f2))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_color(rgb(0xffffff))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_size(px(18.0))
                                                        .child(initial),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .child(
                                                            div()
                                                                .text_color(rgb(0xf2f3f5))
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_size(px(15.0))
                                                                .child(app.name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_color(rgb(0x80848e))
                                                                .text_size(px(12.0))
                                                                .child("Autorizado via OAuth2 • Conexão Segura"),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .h(px(32.0))
                                                .px(px(14.0))
                                                .rounded(px(4.0))
                                                .bg(rgb(0xda373c))
                                                .text_color(rgb(0xffffff))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(px(12.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .hover(|s| s.bg(rgb(0xf23f43)))
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    let aid = app_id.clone();
                                                    entity.update(cx, |s, cx| {
                                                        s.revoke_authorized_app(&aid);
                                                        cx.notify();
                                                    });
                                                })
                                                .child("Revogar Acesso"),
                                        ),
                                )
                                // Scopes list / Permissões concedidas
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .text_color(rgb(0x80848e))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(11.0))
                                                .child("PERMISSÕES CONCEDIDAS"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .flex_wrap()
                                                .gap(px(6.0))
                                                .children(app.scopes.iter().map(|scope| {
                                                    let (icon_el, label): (AnyElement, String) = match scope.as_str() {
                                                        "identify" => (icon_user(px(12.0), rgb(0x5865f2)).into_any_element(), "Identificação básica (usuário e avatar)".to_string()),
                                                        "email" => (icon_inbox(px(12.0), rgb(0x23a55a)).into_any_element(), "Acessar endereço de e-mail".to_string()),
                                                        "guilds" => (icon_folder(px(12.0), rgb(0xf0b232)).into_any_element(), "Visualizar servidores dos quais você faz parte".to_string()),
                                                        "connections" => (icon_link(px(12.0), rgb(0x00a8fc)).into_any_element(), "Visualizar contas externas conectadas".to_string()),
                                                        "rpc" => (icon_sparkles(px(12.0), rgb(0xb45be6)).into_any_element(), "Controle local via Discord RPC".to_string()),
                                                        "bot" => (icon_bot(px(12.0), rgb(0x5865f2)).into_any_element(), "Adicionar e autorizar bot".to_string()),
                                                        _ => (icon_key(px(12.0), rgb(0x80848e)).into_any_element(), scope.clone()),
                                                    };
                                                    div()
                                                        .px(px(8.0))
                                                        .py(px(4.0))
                                                        .rounded(px(4.0))
                                                        .bg(rgb(0x1e1f22))
                                                        .border_1()
                                                        .border_color(rgb(0x35363c))
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap(px(6.0))
                                                        .child(icon_el)
                                                        .child(
                                                            div()
                                                                .text_color(rgb(0xdbdee1))
                                                                .text_size(px(11.0))
                                                                .child(label),
                                                        )
                                                })),
                                        ),
                                )
                        }).collect()
                    }),
            )
            // Centro da Família integration inside this tab
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .my(px(8.0))
                    .bg(rgb(0x35363c)),
            )
            .child(self.render_family_center(app_entity, app_state))
    }

    pub fn render_family_center(&self, _app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let links = &app_state.family_center_links;

        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .size(px(28.0))
                            .rounded_full()
                            .bg(with_alpha(0x5865f2, 0.15))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon_shield(px(18.0), rgb(0x5865f2))),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xf2f3f5))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.0))
                            .child("Centro da Família (Family Center)"),
                    ),
            )
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("O Centro da Família permite que pais e responsáveis acompanhem os hábitos digitais de adolescentes com total transparência e sem invadir a privacidade das mensagens diretas."),
            )
            // Activity summary cards
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x2b2d31))
                            .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child("AMIGOS ADICIONADOS (30d)"))
                            .child(div().text_color(rgb(0x23a55a)).font_weight(FontWeight::BOLD).text_size(px(20.0)).child("12")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x2b2d31))
                            .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child("SERVIDORES NOVOS"))
                            .child(div().text_color(rgb(0x5865f2)).font_weight(FontWeight::BOLD).text_size(px(20.0)).child("3")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x2b2d31))
                            .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child("CHAMADAS COM DESCONHECIDOS"))
                            .child(div().text_color(rgb(0xf23f43)).font_weight(FontWeight::BOLD).text_size(px(20.0)).child("0")),
                    ),
            )
            // Linked accounts
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .children(links.iter().map(|link| {
                        div()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x1e1f22))
                            .border_1()
                            .border_color(rgb(0x35363c))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .size(px(32.0))
                                            .rounded_full()
                                            .bg(if link.is_guardian { with_alpha(0x5865f2, 0.15) } else { with_alpha(0x23a55a, 0.15) })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if link.is_guardian {
                                                icon_shield(px(16.0), rgb(0x5865f2))
                                            } else {
                                                icon_user(px(16.0), rgb(0x23a55a))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(div().text_color(rgb(0xf2f3f5)).font_weight(FontWeight::BOLD).text_size(px(14.0)).child(link.username.clone()))
                                            .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child(if link.is_guardian { "Vínculo de Responsável" } else { "Vínculo de Adolescente Supervisionado" })),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x23a55a))
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(10.0))
                                    .child("ATIVO"),
                            )
                    })),
            )
    }

    pub fn render_server_boost(&self, _app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let boosts = &app_state.server_boosts;

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Impulsione seus servidores favoritos para desbloquear slots extras de emojis, maior qualidade de áudio e banner personalizado."),
            )
            .child(
                div()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(rgb(0x2b2d31))
                    .border_1()
                    .border_color(rgb(0x35363c))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(div().text_color(rgb(0x80848e)).font_weight(FontWeight::BOLD).text_size(px(11.0)).child("SEUS IMPULSOS DE SERVIDOR"))
                            .child(div().text_color(rgb(0xffffff)).font_weight(FontWeight::BOLD).text_size(px(18.0)).child(format!("{} Impulsos Ativos", boosts.len())))
                            .child(div().text_color(rgb(0x949ba4)).text_size(px(12.0)).child("2 impulsos inclusos na sua assinatura ativa do Discord Nitro.")),
                    )
                    .child(
                        div()
                            .h(px(34.0))
                            .px(px(14.0))
                            .rounded(px(4.0))
                            .bg(rgb(0xb45be6))
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x8547c6)))
                            .child(icon_sparkles(px(16.0), rgb(0xffffff)))
                            .child("Impulsionar Servidor"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("SERVIDORES IMPULSIONADOS"),
                    )
                    .children(boosts.iter().map(|b| {
                        div()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x1e1f22))
                            .border_1()
                            .border_color(rgb(0x35363c))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .size(px(32.0))
                                            .rounded(px(6.0))
                                            .bg(with_alpha(0xb45be6, 0.15))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon_sparkles(px(16.0), rgb(0xb45be6)))
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(div().text_color(rgb(0xf2f3f5)).font_weight(FontWeight::BOLD).text_size(px(14.0)).child(b.guild_name.clone()))
                                            .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child("Impulso aplicado • Cooldown de 7 dias para transferência")),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0xb45be6))
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(10.0))
                                    .child("NÍVEL 2"),
                            )
                    })),
            )
    }
}

// ============================================================================
// Implementation of Tab: render_appearance (Aparência Avançada)
// ============================================================================

impl UserSettingsModal {
    pub fn render_appearance(&self, app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let current_theme = app_state.theme_choice;
        let display_mode = app_state.message_display_mode;
        let chat_font_size = app_state.accessibility_settings.chat_font_size;

        div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            // -------------------------------------------------------------
            // Theme Selector: Escuro (#313338), Claro (#ffffff), Midnight OLED (#000000)
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(12.0))
                            .child("TEMA"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(14.0))
                            // 1. Escuro (#313338)
                            .child(self.render_theme_card(
                                app_entity.clone(),
                                ThemeChoice::Dark,
                                current_theme,
                                "Escuro",
                                "Clássico (#313338)",
                                0x313338,
                                0x2b2d31,
                                0xf2f3f5,
                            ))
                            // 2. Claro (#ffffff)
                            .child(self.render_theme_card(
                                app_entity.clone(),
                                ThemeChoice::Light,
                                current_theme,
                                "Claro",
                                "Alto contraste (#ffffff)",
                                0xffffff,
                                0xf2f3f5,
                                0x313338,
                            ))
                            // 3. Midnight OLED (#000000)
                            .child(self.render_theme_card(
                                app_entity.clone(),
                                ThemeChoice::Midnight,
                                current_theme,
                                "Midnight OLED",
                                "Preto puro (#000000)",
                                0x000000,
                                0x111214,
                                0xffffff,
                            )),
                    ),
            )
            // -------------------------------------------------------------
            // Message Display Mode: Aconchegante vs Compacto
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(12.0))
                            .child("EXIBIÇÃO DE MENSAGENS"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            // Aconchegante (Cozy)
                            .child(self.render_display_mode_card(
                                app_entity.clone(),
                                MessageDisplayMode::Cozy,
                                display_mode,
                                "Aconchegante",
                                "Avatares grandes de 40px, mensagens modernas e espaçamento confortável.",
                                true,
                                chat_font_size,
                            ))
                            // Compacto (Compact)
                            .child(self.render_display_mode_card(
                                app_entity.clone(),
                                MessageDisplayMode::Compact,
                                display_mode,
                                "Compacto",
                                "Linhas condensadas estilo IRC clássico, timestamp à esquerda e sem avatares.",
                                false,
                                chat_font_size,
                            )),
                    ),
            )
            // -------------------------------------------------------------
            // Chat Scale Slider: 12px a 20px
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(12.0))
                                    .child("ESCALA DO CHAT E TAMANHO DA FONTE"),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x5865f2))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(13.0))
                                    .child(format!("{}px{}", chat_font_size as u32, if chat_font_size == 16.0 { " (Padrão)" } else { "" })),
                            ),
                    )
                    // Visual Track
                    .child({
                        let fill_pct = ((chat_font_size - 12.0) / 8.0).clamp(0.0, 1.0);
                        let track_w = 480.0;
                        let filled_w = track_w * fill_pct;

                        div()
                            .w(px(track_w))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgb(0x4e5058))
                            .relative()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .h(px(8.0))
                                    .w(px(filled_w))
                                    .rounded_full()
                                    .bg(rgb(0x5865f2)),
                            )
                            .child(
                                div()
                                    .size(px(18.0))
                                    .rounded_full()
                                    .bg(rgb(0xffffff))
                                    .border_2()
                                    .border_color(rgb(0x5865f2))
                                    .ml(px(-9.0)),
                            )
                    })
                    // Font Size Preset Buttons (12px to 20px)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(self.render_font_preset(app_entity.clone(), "12px", 12.0, chat_font_size))
                            .child(self.render_font_preset(app_entity.clone(), "14px", 14.0, chat_font_size))
                            .child(self.render_font_preset(app_entity.clone(), "15px", 15.0, chat_font_size))
                            .child(self.render_font_preset(app_entity.clone(), "16px (Padrão)", 16.0, chat_font_size))
                            .child(self.render_font_preset(app_entity.clone(), "18px", 18.0, chat_font_size))
                            .child(self.render_font_preset(app_entity.clone(), "20px", 20.0, chat_font_size)),
                    )
                    // Live Font Preview Box
                    .child(
                        div()
                            .p(px(14.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x1e1f22))
                            .border_1()
                            .border_color(rgb(0x2b2d31))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x5865f2))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(13.0))
                                            .child("Vanguard"),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .text_size(px(11.0))
                                            .child("Hoje às 19:50"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xdbdee1))
                                    .text_size(px(chat_font_size))
                                    .child(format!("Pré-visualização do chat com escala de {}px: Rustacean construindo Discord nativo com GPUI a 120 FPS!", chat_font_size as u32)),
                            ),
                    ),
            )
    }

    fn render_theme_card(
        &self,
        app_entity: Entity<AppState>,
        theme: ThemeChoice,
        current_theme: ThemeChoice,
        name: &'static str,
        subtitle: &'static str,
        bg_color: u32,
        card_color: u32,
        text_col: u32,
    ) -> impl IntoElement {
        let is_selected = current_theme == theme;
        let entity = app_entity.clone();

        div()
            .w(px(210.0))
            .p(px(14.0))
            .rounded(px(8.0))
            .bg(rgb(0x2b2d31))
            .border_2()
            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x35363c) })
            .cursor_pointer()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .hover(|s| s.bg(rgb(0x35373c)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                entity.update(cx, |s, cx| {
                    s.set_theme_choice(theme);
                    cx.notify();
                });
            })
            // Mini Preview Box: Authentic Discord Desktop layout
            .child(
                div()
                    .h(px(72.0))
                    .rounded(px(6.0))
                    .bg(rgb(bg_color))
                    .border_1()
                    .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x4e5058) })
                    .overflow_hidden()
                    .flex()
                    .flex_row()
                    // Mini sidebar / channel list
                    .child(
                        div()
                            .w(px(56.0))
                            .h_full()
                            .bg(rgb(card_color))
                            .p(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .h(px(6.0))
                                    .w(px(36.0))
                                    .rounded_full()
                                    .bg(with_alpha(text_col, 0.4)),
                            )
                            .child(
                                div()
                                    .h(px(5.0))
                                    .w(px(28.0))
                                    .rounded_full()
                                    .bg(with_alpha(text_col, 0.2)),
                            )
                            .child(
                                div()
                                    .h(px(5.0))
                                    .w(px(40.0))
                                    .rounded_full()
                                    .bg(with_alpha(text_col, 0.2)),
                            )
                            .child(
                                div()
                                    .h(px(5.0))
                                    .w(px(24.0))
                                    .rounded_full()
                                    .bg(with_alpha(text_col, 0.2)),
                            ),
                    )
                    // Mini chat conversation area
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .p(px(6.0))
                            .flex()
                            .flex_col()
                            .justify_between()
                            // Channel header pill
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(3.0))
                                    .child(icon_hash(px(10.0), with_alpha(text_col, 0.7)))
                                    .child(
                                        div()
                                            .text_color(rgb(text_col))
                                            .text_size(px(10.0))
                                            .font_weight(FontWeight::BOLD)
                                            .child("geral"),
                                    ),
                            )
                            // Message preview with avatar and bubble
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .size(px(12.0))
                                            .rounded_full()
                                            .bg(rgb(0x5865f2))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon_user(px(8.0), rgb(0xffffff))),
                                    )
                                    .child(
                                        div()
                                            .h(px(6.0))
                                            .flex_1()
                                            .rounded_full()
                                            .bg(with_alpha(text_col, 0.35)),
                                    ),
                            )
                            // Second message text line
                            .child(
                                div()
                                    .pl(px(16.0))
                                    .child(
                                        div()
                                            .h(px(5.0))
                                            .w(px(70.0))
                                            .rounded_full()
                                            .bg(with_alpha(text_col, 0.2)),
                                    ),
                            ),
                    ),
            )
            // Label row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .size(px(16.0))
                            .rounded_full()
                            .border_2()
                            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x80848e) })
                            .bg(if is_selected { rgb(0x5865f2) } else { rgb(0x2b2d31) })
                            .flex()
                            .items_center()
                            .justify_center()
                            .children(if is_selected {
                                Some(icon_check(px(10.0), rgb(0xffffff)))
                            } else {
                                None
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_color(rgb(0xf2f3f5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(13.0))
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .text_size(px(10.0))
                                    .child(subtitle),
                            ),
                    ),
            )
    }

    fn render_display_mode_card(
        &self,
        app_entity: Entity<AppState>,
        mode: MessageDisplayMode,
        current_mode: MessageDisplayMode,
        title: &'static str,
        desc: &'static str,
        is_cozy: bool,
        font_size: f32,
    ) -> impl IntoElement {
        let is_selected = current_mode == mode;
        let entity = app_entity.clone();

        div()
            .p(px(14.0))
            .rounded(px(8.0))
            .bg(rgb(0x2b2d31))
            .border_2()
            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x35363c) })
            .cursor_pointer()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .hover(|s| s.bg(rgb(0x35373c)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                entity.update(cx, |s, cx| {
                    s.set_message_display_mode(mode);
                    cx.notify();
                });
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(18.0))
                            .rounded_full()
                            .border_2()
                            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x80848e) })
                            .bg(if is_selected { rgb(0x5865f2) } else { rgb(0x2b2d31) })
                            .flex()
                            .items_center()
                            .justify_center()
                            .children(if is_selected {
                                Some(
                                    div()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(10.0))
                                        .child("✓"),
                                )
                            } else {
                                None
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_color(rgb(0xf2f3f5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(14.0))
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x949ba4))
                                    .text_size(px(12.0))
                                    .child(desc),
                            ),
                    ),
            )
            // Visual Preview Row
            .child(
                div()
                    .p(px(10.0))
                    .rounded(px(6.0))
                    .bg(rgb(0x1e1f22))
                    .children(if is_cozy {
                        Some(
                            div()
                                .flex()
                                .flex_row()
                                .items_start()
                                .gap(px(12.0))
                                .child(
                                    div()
                                        .size(px(36.0))
                                        .rounded_full()
                                        .bg(rgb(0x5865f2))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(0xffffff))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(14.0))
                                        .child("V"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_baseline()
                                                .gap(px(6.0))
                                                .child(div().text_color(rgb(0xf2f3f5)).font_weight(FontWeight::SEMIBOLD).text_size(px(13.0)).child("Vanguard"))
                                                .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child("Hoje às 19:42")),
                                        )
                                        .child(
                                            div()
                                                .text_color(rgb(0xdbdee1))
                                                .text_size(px(font_size))
                                                .child("Visual Aconchegante com avatar e espaçamento completo."),
                                        ),
                                ),
                        )
                    } else {
                        Some(
                            div()
                                .flex()
                                .flex_row()
                                .items_baseline()
                                .gap(px(8.0))
                                .child(div().text_color(rgb(0x80848e)).text_size(px(11.0)).child("19:42"))
                                .child(div().text_color(rgb(0xf2f3f5)).font_weight(FontWeight::BOLD).text_size(px(13.0)).child("Vanguard:"))
                                .child(div().text_color(rgb(0xdbdee1)).text_size(px(font_size)).child("Visual Compacto estilo IRC denso em linha única.")),
                        )
                    }),
            )
    }

    fn render_font_preset(
        &self,
        app_entity: Entity<AppState>,
        label: &'static str,
        size: f32,
        current_size: f32,
    ) -> impl IntoElement {
        let is_selected = (current_size - size).abs() < 0.1;
        let entity = app_entity.clone();

        div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .bg(if is_selected { rgb(0x5865f2) } else { rgb(0x2b2d31) })
            .border_1()
            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x35363c) })
            .text_color(if is_selected { rgb(0xffffff) } else { rgb(0x949ba4) })
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(11.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xf2f3f5)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                entity.update(cx, |s, cx| {
                    s.set_chat_font_size(size);
                    cx.notify();
                });
            })
            .child(label)
    }
}

// ============================================================================
// Implementation of Tab: render_accessibility (Acessibilidade)
// ============================================================================

impl UserSettingsModal {
    pub fn render_accessibility(&self, app_entity: Entity<AppState>, app_state: &AppState) -> impl IntoElement {
        let access = &app_state.accessibility_settings;
        let reduce_motion = access.reduce_motion;
        let auto_play_stickers = access.auto_play_stickers;
        let saturation = access.saturation;
        let high_contrast = access.high_contrast;

        div()
            .flex()
            .flex_col()
            .gap(px(20.0))
            .child(
                div()
                    .text_color(rgb(0x949ba4))
                    .text_size(px(13.0))
                    .child("Personalize os efeitos visuais, contraste e saturação para aprimorar sua experiência de uso e navegação."),
            )
            // -------------------------------------------------------------
            // Section 1: Motion Controls (Redução de Movimento)
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("CONTROLE DE MOVIMENTO E ANIMAÇÕES"),
                    )
                    .child(render_app_toggle_row(
                        app_entity.clone(),
                        "Redução de Movimento (Reduce Motion)",
                        "Desativa transições elásticas, animações de morph e efeitos de aceleração gráfica em toda a interface nativa do GPUI.",
                        reduce_motion,
                        move |s| s.set_reduce_motion(!reduce_motion),
                    ))
                    .child(render_app_toggle_row(
                        app_entity.clone(),
                        "Reproduzir figurinhas e emojis automaticamente",
                        "Quando ativado, figurinhas e emojis animados são reproduzidos continuamente. Quando desativado, passe o mouse para animar.",
                        auto_play_stickers,
                        move |s| s.accessibility_settings.auto_play_stickers = !auto_play_stickers,
                    )),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // -------------------------------------------------------------
            // Section 2: Color Saturation Slider (0% a 100%)
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child("CONTROLE DE SATURAÇÃO DE CORES"),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x5865f2))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(12.0))
                                    .child(format!("{}%{}", (saturation * 100.0) as u32, if saturation == 1.0 { " (Padrão)" } else if saturation == 0.0 { " (P&B)" } else { "" })),
                            ),
                    )
                    // Visual Track
                    .child({
                        let fill_pct = saturation.clamp(0.0, 1.0);
                        let track_w = 480.0;
                        let filled_w = track_w * fill_pct;

                        div()
                            .w(px(track_w))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgb(0x4e5058))
                            .relative()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .h(px(8.0))
                                    .w(px(filled_w))
                                    .rounded_full()
                                    .bg(rgb(0x5865f2)),
                            )
                            .child(
                                div()
                                    .size(px(18.0))
                                    .rounded_full()
                                    .bg(rgb(0xffffff))
                                    .border_2()
                                    .border_color(rgb(0x5865f2))
                                    .ml(px(-9.0)),
                            )
                    })
                    // Presets
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(self.render_saturation_preset(app_entity.clone(), "0% (Escala de Cinza)", 0.0, saturation))
                            .child(self.render_saturation_preset(app_entity.clone(), "25%", 0.25, saturation))
                            .child(self.render_saturation_preset(app_entity.clone(), "50%", 0.50, saturation))
                            .child(self.render_saturation_preset(app_entity.clone(), "75%", 0.75, saturation))
                            .child(self.render_saturation_preset(app_entity.clone(), "100% (Padrão)", 1.0, saturation)),
                    )
                    // Live Palette Preview
                    .child(
                        div()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(rgb(0x1e1f22))
                            .border_1()
                            .border_color(rgb(0x2b2d31))
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(10.0))
                                    .child("PALETA DE CORES DA INTERFACE"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(8.0))
                                    .child(render_color_chip(0x5865f2, "Blurple"))
                                    .child(render_color_chip(0x23a55a, "Online"))
                                    .child(render_color_chip(0xf0b232, "Ausente"))
                                    .child(render_color_chip(0xf23f43, "Ocupado"))
                                    .child(render_color_chip(0x9b59b6, "Nitro"))
                                    .child(render_color_chip(0xeb459e, "Boost")),
                            ),
                    ),
            )
            // Divider
            .child(
                div()
                    .w_full()
                    .h(px(1.0))
                    .bg(rgb(0x35363c)),
            )
            // -------------------------------------------------------------
            // Section 3: High Contrast & System Sync (Contraste Aprimorado)
            // -------------------------------------------------------------
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_color(rgb(0x80848e))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(11.0))
                            .child("CONTRASTE E ACESSIBILIDADE VISUAL"),
                    )
                    .child(render_app_toggle_row(
                        app_entity.clone(),
                        "Contraste Aprimorado (High Contrast Mode)",
                        "Aumenta a nitidez e o contraste de bordas, caixas de diálogo, divisores e fundos de mensagens para máxima legibilidade.",
                        high_contrast,
                        move |s| s.set_enhanced_contrast(!high_contrast),
                    ))
                    .child(render_app_toggle_row(
                        app_entity.clone(),
                        "Sincronizar com Contraste do Sistema",
                        "Detecta preferências de alto contraste e esquema de cores do sistema operacional (Linux DBus / FreeDesktop).",
                        true,
                        |_s| {},
                    )),
            )
    }

    fn render_saturation_preset(
        &self,
        app_entity: Entity<AppState>,
        label: &'static str,
        value: f32,
        current: f32,
    ) -> impl IntoElement {
        let is_selected = (current - value).abs() < 0.05;
        let entity = app_entity.clone();

        div()
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .bg(if is_selected { rgb(0x5865f2) } else { rgb(0x2b2d31) })
            .border_1()
            .border_color(if is_selected { rgb(0x5865f2) } else { rgb(0x35363c) })
            .text_color(if is_selected { rgb(0xffffff) } else { rgb(0x949ba4) })
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(11.0))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xf2f3f5)))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                entity.update(cx, |s, cx| {
                    s.set_color_saturation(value);
                    cx.notify();
                });
            })
            .child(label)
    }
}

fn render_color_chip(color: u32, label: &'static str) -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(rgb(color))
        .text_color(rgb(0xffffff))
        .font_weight(FontWeight::BOLD)
        .text_size(px(11.0))
        .child(label)
}
