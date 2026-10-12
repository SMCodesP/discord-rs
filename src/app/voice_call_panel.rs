use gpui::prelude::*;
use gpui::*;
use gpui::component::tooltip::Tooltip;
use gpui::component::popover::Popover;
use gpui::component::Selectable;

use super::state::{ActiveVoiceCall, AppState, VoiceParticipantState};
use crate::capture::camera::CameraManager;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::media::devices::MediaDeviceManager;
use crate::protocol::voice_session::VoiceSessionState;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

/// Dedicated voice call room / stage panel displaying:
/// - Connected channel status, real RTC latency (zero mocks), and connection state
/// - Responsive participant stage grid with Avatar component, green speaking ring, and screen share focus mode
/// - Media controls (Mute, Camera, Screen Share, Disconnect) with real hardware device selection
pub struct VoiceCallPanel {
    state: Entity<AppState>,
}

impl VoiceCallPanel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }
}

impl Render for VoiceCallPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let active_call = state.active_voice_call.clone();
        let app_entity = self.state.clone();

        if let Some(call) = active_call {
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(bg_floating())
                .justify_between()
                .p(px(16.0))
                .child(render_top_bar(&call, cx))
                .child(render_stage_body(app_entity.clone(), &call, cx))
                .child(render_bottom_controls(app_entity, &call, cx))
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.discord().text_secondary)
                .child("Nenhuma chamada de voz conectada.")
        }
    }
}

// ============================================================================
// Top Bar: Channel info, RTC status badge & fatal error banner
// ============================================================================

fn render_top_bar(call: &ActiveVoiceCall, cx: &App) -> AnyElement {
    let discord = cx.discord();
    let has_fatal_error = call.fatal_error.is_some();

    // Verificação de dispositivos de áudio reais de hardware (Zero mocks em produção)
    let device_manager = MediaDeviceManager::new();
    let has_input = device_manager
        .list_input_devices()
        .map(|d| !d.is_empty())
        .unwrap_or(false);
    let has_output = device_manager
        .list_output_devices()
        .map(|d| !d.is_empty())
        .unwrap_or(false);

    let device_warning = if !has_input && !has_output {
        Some("Nenhum microfone ou saída de áudio detectados no sistema.")
    } else if !has_input {
        Some("Nenhum microfone de entrada detectado no sistema.")
    } else if !has_output {
        Some("Nenhum dispositivo de saída de áudio detectado no sistema.")
    } else {
        None
    };

    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .w_full()
                .child(render_channel_identity(call, cx))
                .child(render_rtc_badge(call, cx)),
        )
        .children(has_fatal_error.then(|| render_fatal_error_banner(call, discord.white)))
        .children(device_warning.map(|msg| render_audio_device_warning_banner(msg, discord.bg_server)))
        .into_any_element()
}

fn render_audio_device_warning_banner(msg: &str, text_col: Rgba) -> AnyElement {
    div()
        .w_full()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(status_warning())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(icon_warning(px(16.0), text_col))
        .child(
            div()
                .text_color(text_col)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(12.0))
                .child(format!("Aviso de áudio: {}", msg)),
        )
        .into_any_element()
}

fn render_channel_identity(call: &ActiveVoiceCall, cx: &App) -> AnyElement {
    let discord = cx.discord();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(icon_speaker(px(20.0), discord.text_titles))
                .child(
                    div()
                        .text_color(discord.text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(18.0))
                        .child(call.channel_name.clone()),
                ),
        )
        .child(
            div()
                .text_color(discord.text_secondary)
                .text_size(px(13.0))
                .child(format!("• {}", call.guild_name)),
        )
        .into_any_element()
}

fn render_rtc_badge(call: &ActiveVoiceCall, cx: &App) -> AnyElement {
    let discord = cx.discord();
    let (icon_col, label_text) = match call.connection_state {
        VoiceSessionState::Connected => {
            let label = if let Some(ms) = call.ping_ms {
                format!("RTC {}ms", ms)
            } else {
                "Voz Conectada".to_string()
            };
            (status_online(), label)
        }
        VoiceSessionState::Connecting => (status_idle(), "Conectando ao RTC...".to_string()),
        VoiceSessionState::Reconnecting => (status_idle(), "Reconectando...".to_string()),
        VoiceSessionState::Disconnected { ref reason } => {
            let label = if !reason.is_empty() && reason != "Sessão inicializada" {
                format!("Desconectado ({})", reason)
            } else {
                "Desconectado".to_string()
            };
            (status_dnd(), label)
        }
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(discord.bg_server)
        .child(icon_wifi(px(14.0), icon_col))
        .child(
            div()
                .text_color(icon_col)
                .font_weight(FontWeight::BOLD)
                .text_size(px(12.0))
                .child(label_text),
        )
        .into_any_element()
}

fn render_fatal_error_banner(call: &ActiveVoiceCall, text_col: Rgba) -> AnyElement {
    let err_desc = call
        .fatal_error
        .clone()
        .unwrap_or_else(|| "Erro desconhecido na sessão de voz".into());

    div()
        .w_full()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(status_dnd())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(icon_warning(px(16.0), text_col))
        .child(
            div()
                .text_color(text_col)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(12.0))
                .child(format!("Falha na chamada: {}", err_desc)),
        )
        .into_any_element()
}

// ============================================================================
// Stage Body: Responsive Grid or Screen Share / Focus Layout
// ============================================================================

fn render_stage_body(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let is_focus = call.is_screen_sharing || call.focus_user_id.is_some();

    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .w_full()
        .child(if is_focus {
            render_focus_layout(app_entity, call, cx)
        } else {
            render_grid_layout(app_entity, call, cx)
        })
        .into_any_element()
}

fn render_grid_layout(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let participants = &call.participants;

    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_center()
        .gap(px(16.0))
        .w_full()
        .children(participants.iter().map(|p| {
            render_participant_tile(app_entity.clone(), p, false, cx)
        }))
        .into_any_element()
}

fn render_focus_layout(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let focus_uid = call
        .focus_user_id
        .clone()
        .or_else(|| call.participants.first().map(|p| p.id.clone()));

    let focused_p = call
        .participants
        .iter()
        .find(|p| Some(&p.id) == focus_uid.as_ref())
        .cloned();

    let other_participants: Vec<_> = call
        .participants
        .iter()
        .filter(|p| Some(&p.id) != focus_uid.as_ref())
        .cloned()
        .collect();

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .size_full()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(if call.is_screen_sharing {
                    render_screen_share_focus_tile(focused_p.as_ref(), cx)
                } else if let Some(ref p) = focused_p {
                    render_participant_tile(app_entity.clone(), p, true, cx)
                } else {
                    div().into_any_element()
                }),
        )
        .children((!other_participants.is_empty()).then(|| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .w_full()
                .overflow_x_hidden()
                .children(other_participants.into_iter().map(|p| {
                    render_mini_tile(app_entity.clone(), &p, cx)
                }))
        }))
        .into_any_element()
}

fn render_screen_share_focus_tile(
    presenter: Option<&VoiceParticipantState>,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let presenter_name = presenter
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Você".into());

    div()
        .size_full()
        .max_w(px(720.0))
        .max_h(px(440.0))
        .rounded(px(8.0))
        .bg(discord.bg_channels)
        .border_2()
        .border_color(status_online())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(16.0))
        .child(icon_screen_share(px(48.0), discord.white))
        .child(
            div()
                .text_color(discord.text_titles)
                .font_weight(FontWeight::BOLD)
                .text_size(px(16.0))
                .child("Compartilhamento de Tela"),
        )
        .child(
            div()
                .text_color(discord.text_secondary)
                .text_size(px(13.0))
                .child(format!("Apresentado por {}", presenter_name)),
        )
        .into_any_element()
}

fn render_participant_tile(
    app_entity: Entity<AppState>,
    participant: &VoiceParticipantState,
    is_focused: bool,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let is_speaking = participant.is_speaking;
    let is_muted = participant.is_muted;
    let uid = participant.id.clone();
    let entity = app_entity;

    let (tile_w, tile_h) = if is_focused {
        (px(480.0), px(300.0))
    } else {
        (px(260.0), px(170.0))
    };

    div()
        .relative()
        .w(tile_w)
        .h(tile_h)
        .rounded(px(8.0))
        .bg(discord.bg_channels)
        .border_2()
        .border_color(if is_speaking {
            status_online()
        } else {
            transparent()
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let uid_clone = uid.clone();
            entity.update(cx, |state, cx| {
                if state
                    .active_voice_call
                    .as_ref()
                    .and_then(|c| c.focus_user_id.as_ref())
                    == Some(&uid_clone)
                {
                    state.set_focus_user(None);
                } else {
                    state.set_focus_user(Some(uid_clone));
                }
                cx.notify();
            });
        })
        .child(
            Avatar::new(participant.name.clone())
                .size(AvatarSize::Px80)
                .opt_src(participant.avatar_url.clone())
                .opt_decoration(participant.avatar_decoration_data.clone()),
        )
        .child(render_tile_name_badge(
            &participant.name,
            is_muted,
            discord.text_titles,
        ))
        .into_any_element()
}

fn render_mini_tile(
    app_entity: Entity<AppState>,
    participant: &VoiceParticipantState,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let is_speaking = participant.is_speaking;
    let is_muted = participant.is_muted;
    let uid = participant.id.clone();
    let entity = app_entity;

    div()
        .relative()
        .w(px(140.0))
        .h(px(90.0))
        .rounded(px(6.0))
        .bg(discord.bg_channels)
        .border_2()
        .border_color(if is_speaking {
            status_online()
        } else {
            transparent()
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let uid_clone = uid.clone();
            entity.update(cx, |state, cx| {
                state.set_focus_user(Some(uid_clone));
                cx.notify();
            });
        })
        .child(
            Avatar::new(participant.name.clone())
                .size(AvatarSize::Px40)
                .opt_src(participant.avatar_url.clone())
                .opt_decoration(participant.avatar_decoration_data.clone()),
        )
        .child(render_tile_name_badge(
            &participant.name,
            is_muted,
            discord.text_titles,
        ))
        .into_any_element()
}

fn render_tile_name_badge(name: &str, is_muted: bool, text_col: Rgba) -> AnyElement {
    div()
        .absolute()
        .bottom(px(8.0))
        .left(px(8.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(bg_floating())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_color(text_col)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(name.to_string()),
        )
        .children(is_muted.then(|| icon_mic_off(px(14.0), status_dnd())))
        .into_any_element()
}

// ============================================================================
// Bottom Control Bar: Mute, Camera, Screen Share, Disconnect with device pickers
// ============================================================================

fn render_bottom_controls(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .p(px(8.0))
                .rounded(px(12.0))
                .bg(discord.bg_server)
                .child(render_mute_control(app_entity.clone(), call, cx))
                .child(render_camera_control(app_entity.clone(), call.is_camera_on, cx))
                .child(render_screen_share_control(
                    app_entity.clone(),
                    call.is_screen_sharing,
                    cx,
                ))
                .child(render_disconnect_control(app_entity, cx)),
        )
        .into_any_element()
}

/// Selectable trigger for popover arrow buttons
#[derive(IntoElement)]
#[allow(dead_code)]
pub struct ArrowDropupTrigger {
    pub is_open: bool,
    pub id: SharedString,
    pub bg_col: Rgba,
    pub hover_col: Rgba,
}

impl Selectable for ArrowDropupTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.is_open = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.is_open
    }
}

impl RenderOnce for ArrowDropupTrigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let discord = cx.discord();
        div()
            .id(self.id)
            .w(px(18.0))
            .h(px(44.0))
            .rounded(px(4.0))
            .bg(self.bg_col)
            .text_color(discord.white)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |s| s.bg(self.hover_col))
            .tooltip(|window, cx| {
                Tooltip::new("Dispositivos de áudio").build(window, cx)
            })
            .child(
                div()
                    .text_color(discord.white)
                    .text_size(px(10.0))
                    .child("▲"),
            )
    }
}

fn render_mute_control(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let entity_btn = app_entity.clone();
    let is_muted = call.is_muted;

    let bg_col = if is_muted {
        control_critical_primary_bg_default()
    } else {
        control_secondary_bg_default()
    };

    let hover_col = if is_muted {
        control_critical_primary_bg_hover()
    } else {
        control_secondary_bg_hover()
    };

    let mic_btn = div()
        .id("call-mic-btn")
        .w(px(44.0))
        .h(px(44.0))
        .rounded(px(8.0))
        .bg(bg_col)
        .text_color(discord.white)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_col))
        .tooltip(move |window, cx| {
            Tooltip::new(if is_muted { "Ativar microfone" } else { "Desativar microfone" }).build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity_btn.update(cx, |state, cx| {
                state.toggle_mute();
                cx.notify();
            });
        })
        .child(if is_muted {
            icon_mic_off(px(20.0), discord.white)
        } else {
            icon_mic(px(20.0), discord.white)
        });

    let trigger = ArrowDropupTrigger {
        is_open: false,
        id: "call-mic-arrow".into(),
        bg_col,
        hover_col,
    };

    let mic_arrow = Popover::new("call-mic-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| {
                render_audio_device_menu(entity.clone(), cx)
            }
        });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(mic_btn)
        .child(mic_arrow)
        .into_any_element()
}

fn render_camera_control(_app_entity: Entity<AppState>, _is_camera_on: bool, cx: &App) -> AnyElement {
    let discord = cx.discord();

    // Botão de câmera desabilitado: transmissão de vídeo ainda não suportada (Zero Mocks / Item 4)
    let cam_btn = div()
        .id("call-cam-btn")
        .w(px(44.0))
        .h(px(44.0))
        .rounded(px(8.0))
        .bg(control_secondary_bg_default())
        .opacity(0.5)
        .cursor_not_allowed()
        .flex()
        .items_center()
        .justify_center()
        .tooltip(|window, cx| {
            Tooltip::new("Transmissão de vídeo ainda não suportada nesta versão").build(window, cx)
        })
        .child(render_camera_glyph(px(20.0), discord.white));

    let cam_arrow = div()
        .id("call-cam-arrow")
        .w(px(18.0))
        .h(px(44.0))
        .rounded(px(4.0))
        .bg(control_secondary_bg_default())
        .opacity(0.5)
        .cursor_not_allowed()
        .flex()
        .items_center()
        .justify_center()
        .tooltip(|window, cx| {
            Tooltip::new("Transmissão de vídeo ainda não suportada nesta versão").build(window, cx)
        })
        .child(
            div()
                .text_color(discord.white)
                .text_size(px(10.0))
                .child("▲"),
        );

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(cam_btn)
        .child(cam_arrow)
        .into_any_element()
}

fn render_screen_share_control(
    _app_entity: Entity<AppState>,
    _is_screen_sharing: bool,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();

    // Botão de compartilhar tela desabilitado: pipeline ainda não suportado nesta versão (Item 4)
    div()
        .id("call-screen-share-btn")
        .h(px(44.0))
        .px(px(16.0))
        .rounded(px(8.0))
        .bg(control_secondary_bg_default())
        .opacity(0.5)
        .cursor_not_allowed()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .tooltip(|window, cx| {
            Tooltip::new("Compartilhamento de tela ainda não suportado nesta versão").build(window, cx)
        })
        .child(icon_screen_share(px(20.0), discord.white))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Compartilhar Tela"),
        )
        .into_any_element()
}

fn render_disconnect_control(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    let discord = cx.discord();

    div()
        .h(px(44.0))
        .px(px(18.0))
        .rounded(px(8.0))
        .bg(control_critical_primary_bg_default())
        .text_color(discord.white)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(control_critical_primary_bg_hover()))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            app_entity.update(cx, |state, cx| {
                state.disconnect_voice();
                cx.notify();
            });
        })
        .child(icon_phone_hangup(px(20.0), discord.white))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .child("Desconectar"),
        )
        .into_any_element()
}

// ============================================================================
// Real Hardware Device Menus (MediaDeviceManager & CameraManager)
// ============================================================================

fn render_audio_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    let (selected_input, selected_output) = {
        let state = app_entity.read(cx);
        let call = state.active_voice_call.as_ref();
        (
            call.and_then(|c| c.selected_input_device.clone()),
            call.and_then(|c| c.selected_output_device.clone()),
        )
    };

    let device_manager = MediaDeviceManager::new();
    let input_devices = device_manager.list_input_devices().unwrap_or_default();
    let output_devices = device_manager.list_output_devices().unwrap_or_default();

    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(8.0))
        .min_w(px(240.0))
        .rounded(px(8.0))
        .bg(bg_floating())
        .border_1()
        .border_color(borders())
        .shadow_lg()
        .child(
            div()
                .text_color(text_secondary())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .px(px(6.0))
                .child("DISPOSITIVOS DE ENTRADA"),
        );

    if input_devices.is_empty() {
        menu = menu.child(
            div()
                .text_color(text_muted())
                .text_size(px(12.0))
                .px(px(6.0))
                .py(px(4.0))
                .child("Nenhum microfone encontrado"),
        );
    } else {
        for dev in input_devices {
            let is_selected = match selected_input {
                Some(ref sel) => sel == &dev.id,
                None => dev.is_default,
            };
            let dev_id = dev.id.clone();
            let entity = app_entity.clone();
            menu = menu.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(channel_hover()))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                        let dev_id = dev_id.clone();
                        entity.update(cx, |state, cx| {
                            state.set_input_device(Some(dev_id));
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_color(if is_selected { text_primary() } else { text_secondary() })
                            .text_size(px(13.0))
                            .child(dev.name),
                    )
                    .children(is_selected.then(|| {
                        div()
                            .text_color(status_online())
                            .text_size(px(12.0))
                            .child("✓")
                    })),
            );
        }
    }

    menu = menu.child(
        div()
            .h(px(1.0))
            .my(px(4.0))
            .bg(borders()),
    );

    menu = menu.child(
        div()
            .text_color(text_secondary())
            .font_weight(FontWeight::BOLD)
            .text_size(px(11.0))
            .px(px(6.0))
            .child("DISPOSITIVOS DE SAÍDA"),
    );

    if output_devices.is_empty() {
        menu = menu.child(
            div()
                .text_color(text_muted())
                .text_size(px(12.0))
                .px(px(6.0))
                .py(px(4.0))
                .child("Nenhum alto-falante encontrado"),
        );
    } else {
        for dev in output_devices {
            let is_selected = match selected_output {
                Some(ref sel) => sel == &dev.id,
                None => dev.is_default,
            };
            let dev_id = dev.id.clone();
            let entity = app_entity.clone();
            menu = menu.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(channel_hover()))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                        let dev_id = dev_id.clone();
                        entity.update(cx, |state, cx| {
                            state.set_output_device(Some(dev_id));
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_color(if is_selected { text_primary() } else { text_secondary() })
                            .text_size(px(13.0))
                            .child(dev.name),
                    )
                    .children(is_selected.then(|| {
                        div()
                            .text_color(status_online())
                            .text_size(px(12.0))
                            .child("✓")
                    })),
            );
        }
    }

    menu.into_any_element()
}

#[allow(dead_code)]
fn render_camera_device_menu() -> AnyElement {
    let devices = CameraManager::list_devices().unwrap_or_default();

    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(8.0))
        .min_w(px(220.0))
        .rounded(px(8.0))
        .bg(bg_floating())
        .border_1()
        .border_color(borders())
        .child(
            div()
                .text_color(text_secondary())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .child("CÂMERAS DISPONÍVEIS"),
        );

    if devices.is_empty() {
        menu = menu.child(
            div()
                .text_color(text_muted())
                .text_size(px(12.0))
                .py(px(4.0))
                .child("Nenhuma câmera encontrada"),
        );
    } else {
        for dev in devices {
            menu = menu.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .hover(|s| s.bg(channel_hover()))
                    .cursor_pointer()
                    .child(
                        div()
                            .text_color(text_primary())
                            .text_size(px(13.0))
                            .child(dev.name),
                    ),
            );
        }
    }

    menu.into_any_element()
}

fn render_camera_glyph(size: Pixels, color: Rgba) -> AnyElement {
    div()
        .size(size)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(9.0))
                        .rounded(px(1.5))
                        .bg(color),
                )
                .child(
                    div()
                        .w(px(3.5))
                        .h(px(6.5))
                        .rounded(px(1.0))
                        .bg(color),
                ),
        )
        .into_any_element()
}
