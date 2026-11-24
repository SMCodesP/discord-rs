use gpui::prelude::*;
use gpui::*;
use gpui::component::tooltip::Tooltip;
use gpui::component::popover::Popover;
use gpui::component::scroll::ScrollableElement;
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let active_call = state.active_voice_call.clone();
        let app_entity = self.state.clone();
        let viewport = window.viewport_size();
        let stage_width = call_stage_width(viewport.width);
        let stage_height = (viewport.height - px(180.0)).max(px(180.0));

        if let Some(call) = active_call {
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(cx.discord().bg_server)
                .justify_between()
                .p(px(16.0))
                .child(render_top_bar(&call, cx))
                .child(render_stage_body(
                    app_entity.clone(),
                    &call,
                    stage_width,
                    stage_height,
                    cx,
                ))
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

/// Largura útil do palco após o trilho de servidores (72px), a sidebar de
/// canais/DMs (240px) e o padding do painel (16px em cada lado).
fn call_stage_width(viewport_width: Pixels) -> Pixels {
    px((viewport_width.as_f32() - 72.0 - 240.0 - 32.0).max(240.0))
}

fn grid_tile_size(
    available_width: Pixels,
    available_height: Pixels,
    participant_count: usize,
    focused: bool,
) -> (Pixels, Pixels) {
    if focused {
        return (px(480.0), px(300.0));
    }

    let desired_columns = match participant_count {
        0 | 1 => 1,
        2..=4 => 2,
        5..=9 => 3,
        _ => 4,
    };
    let columns = desired_columns
        .min((available_width.as_f32() / 200.0).floor() as usize)
        .max(1);
    let rows = participant_count.max(1).div_ceil(columns);
    let width = ((available_width.as_f32()
        - 16.0 * (columns.saturating_sub(1)) as f32)
        / columns as f32)
        .clamp(180.0, 480.0);
    let height = (width * 9.0 / 16.0)
        .min(
            (available_height.as_f32()
                - 16.0 * rows.saturating_sub(1) as f32)
                / rows as f32,
        )
        .clamp(80.0, 300.0);
    (px(width), px(height))
}

// ============================================================================
// Top Bar: Channel info, RTC status badge & fatal error banner
// ============================================================================

fn render_top_bar(call: &ActiveVoiceCall, cx: &App) -> AnyElement {
    let discord = cx.discord();
    let has_fatal_error = call.fatal_error.is_some();
    let call_warning = call.voice_session.as_ref().and_then(|s| s.call_warning());

    // Verificação de dispositivos de áudio reais de hardware (Zero mocks em produção)
    let device_manager = MediaDeviceManager::new();
    let input_devices = device_manager.list_input_devices();
    let output_devices = device_manager.list_output_devices();
    let device_warning = match (&input_devices, &output_devices) {
        (Err(input_error), Err(output_error)) => Some(format!(
            "Não foi possível enumerar os dispositivos de áudio: entrada: {input_error}; saída: {output_error}"
        )),
        (Err(input_error), _) => {
            Some(format!("Não foi possível enumerar os microfones: {input_error}"))
        }
        (_, Err(output_error)) => {
            Some(format!("Não foi possível enumerar as saídas de áudio: {output_error}"))
        }
        (Ok(inputs), Ok(outputs)) if inputs.is_empty() && outputs.is_empty() => {
            Some("Nenhum microfone ou saída de áudio detectados no sistema.".to_string())
        }
        (Ok(inputs), _) if inputs.is_empty() => {
            Some("Nenhum microfone de entrada detectado no sistema.".to_string())
        }
        (_, Ok(outputs)) if outputs.is_empty() => {
            Some("Nenhum dispositivo de saída de áudio detectado no sistema.".to_string())
        }
        _ => None,
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
                .overflow_hidden()
                .gap(px(8.0))
                .child(render_channel_identity(call, cx))
                .child(render_rtc_badge(call, cx)),
        )
        .children(has_fatal_error.then(|| render_fatal_error_banner(call, discord.white)))
        .children(call_warning.map(|msg| render_call_warning_banner(&msg, discord.white)))
        .children(device_warning.map(|msg| render_audio_device_warning_banner(&msg, discord.white)))
        .into_any_element()
}

fn render_call_warning_banner(msg: &str, text_col: Rgba) -> AnyElement {
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
                .child(msg.to_string()),
        )
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
        .min_w_0()
        .overflow_hidden()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .min_w_0()
                .child(icon_speaker(px(20.0), discord.text_titles))
                .child(
                    div()
                        .text_color(discord.text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(18.0))
                        .truncate()
                        .child(call.channel_name.clone()),
                ),
        )
        .child(
            div()
                .text_color(discord.text_secondary)
                .text_size(px(13.0))
                .flex_shrink_0()
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
            let label = if let Some(code) = crate::protocol::voice_gateway::extract_close_code(reason) {
                format!("Desconectado (código {})", code)
            } else if reason.contains("4006") {
                "Desconectado (código 4006)".to_string()
            } else if reason.contains("Opcode 2") || reason.contains("Timed out") || reason.contains("Tempo limite") {
                "Desconectado (tempo limite)".to_string()
            } else if reason.contains("Missing Permissions") || reason.contains("50013") {
                "Desconectado (sem permissão)".to_string()
            } else if reason.contains('{') && reason.contains('}') {
                "Desconectado (erro de conexão)".to_string()
            } else if !reason.is_empty() && reason != "Sessão inicializada" {
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
        .flex_shrink_0()
        .max_w(px(260.0))
        .overflow_hidden()
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
                .truncate()
                .child(label_text),
        )
        .into_any_element()
}

fn sanitize_user_error_message(raw: &str) -> String {
    if let Some(code) = crate::protocol::voice_gateway::extract_close_code(raw) {
        format!("Não foi possível conectar à chamada (código {})", code)
    } else if raw.contains("4006") {
        "Não foi possível conectar à chamada (código 4006)".to_string()
    } else if raw.contains("Missing Permissions") || raw.contains("50013") {
        "Permissões insuficientes no canal".to_string()
    } else if raw.contains("Timed out waiting for VoiceReady") || raw.contains("Opcode 2") {
        "Tempo limite ao aguardar resposta do servidor de voz".to_string()
    } else if raw.contains('{') && raw.contains('}') {
        "Erro na comunicação com o servidor de voz".to_string()
    } else {
        raw.to_string()
    }
}

fn render_fatal_error_banner(call: &ActiveVoiceCall, text_col: Rgba) -> AnyElement {
    let raw_desc = call
        .fatal_error
        .clone()
        .unwrap_or_else(|| "Erro desconhecido na sessão de voz".into());
    let summary = sanitize_user_error_message(&raw_desc);
    let show_details = summary != raw_desc;

    div()
        .w_full()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .bg(status_dnd())
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
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
                        .child(summary),
                ),
        )
        .children(show_details.then(|| {
            div()
                .text_color(text_col)
                .text_size(px(11.0))
                .child(raw_desc)
        }))
        .into_any_element()
}

// ============================================================================
// Stage Body: Responsive Grid or Screen Share / Focus Layout
// ============================================================================

fn render_stage_body(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    let screen_share = call
        .participants
        .iter()
        .find(|participant| participant.is_screen_sharing);
    let is_focus = call.focus_user_id.is_some() || screen_share.is_some();

    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .w_full()
        .child(if is_focus {
            render_focus_layout(
                app_entity,
                call,
                available_width,
                available_height,
                cx,
            )
        } else {
            render_grid_layout(
                app_entity,
                call,
                available_width,
                available_height,
                cx,
            )
        })
        .into_any_element()
}

fn render_grid_layout(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    let participants = &call.participants;
    let (tile_w, tile_h) = grid_tile_size(
        available_width,
        available_height,
        participants.len(),
        false,
    );

    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_center()
        .gap(px(16.0))
        .w_full()
        .h_full()
        .overflow_y_scrollbar()
        .children(participants.iter().map(|participant| {
            render_participant_tile(
                app_entity.clone(),
                participant,
                tile_w,
                tile_h,
                cx,
            )
        }))
        .into_any_element()
}

fn render_focus_layout(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    let focus_uid = call
        .focus_user_id
        .clone()
        .or_else(|| {
            call.participants
                .iter()
                .find(|participant| participant.is_screen_sharing)
                .map(|participant| participant.id.clone())
        })
        .or_else(|| call.participants.first().map(|participant| participant.id.clone()));

    let focused_p = call
        .participants
        .iter()
        .find(|participant| Some(&participant.id) == focus_uid.as_ref())
        .cloned();

    let other_participants: Vec<_> = call
        .participants
        .iter()
        .filter(|participant| Some(&participant.id) != focus_uid.as_ref())
        .cloned()
        .collect();
    let (tile_w, tile_h) = grid_tile_size(
        available_width,
        (available_height - px(102.0)).max(px(120.0)),
        1,
        true,
    );
    let tile_w = tile_w.min(available_width);
    let tile_h = tile_h.min((available_height - px(102.0)).max(px(120.0)));

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
                .child(if let Some(ref participant) = focused_p {
                    // Não existe decodificador de vídeo remoto neste cliente.
                    // O estado real de stream é mostrado no tile, nunca um
                    // quadro de tela inventado.
                    render_participant_tile(
                        app_entity.clone(),
                        participant,
                        tile_w,
                        tile_h,
                        cx,
                    )
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
                .overflow_x_scrollbar()
                .children(other_participants.into_iter().map(|participant| {
                    render_mini_tile(app_entity.clone(), &participant, cx)
                }))
        }))
        .into_any_element()
}

fn render_participant_tile(
    app_entity: Entity<AppState>,
    participant: &VoiceParticipantState,
    tile_w: Pixels,
    tile_h: Pixels,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let is_speaking = participant.is_speaking;
    let uid = participant.id.clone();
    let entity = app_entity;

    div()
        .relative()
        .w(tile_w)
        .h(tile_h)
        .flex_shrink_0()
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
                .opt_decoration(participant.avatar_decoration_data.clone())
                .cutout_color(discord.bg_channels),
        )
        .child(render_tile_name_badge(participant, discord.text_titles))
        .into_any_element()
}

fn render_mini_tile(
    app_entity: Entity<AppState>,
    participant: &VoiceParticipantState,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let is_speaking = participant.is_speaking;
    let uid = participant.id.clone();
    let entity = app_entity;

    div()
        .relative()
        .w(px(140.0))
        .h(px(90.0))
        .flex_shrink_0()
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
                .opt_decoration(participant.avatar_decoration_data.clone())
                .cutout_color(discord.bg_channels),
        )
        .child(render_tile_name_badge(participant, discord.text_titles))
        .into_any_element()
}

fn render_tile_name_badge(participant: &VoiceParticipantState, text_col: Rgba) -> AnyElement {
    div()
        .absolute()
        .bottom(px(8.0))
        .left(px(8.0))
        .max_w(px(180.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(bg_floating())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .children((!participant.name.is_empty()).then(|| {
            div()
                .text_color(text_col)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child(participant.name.clone())
        }))
        .children(participant.is_screen_sharing.then(|| {
            div()
                .px(px(4.0))
                .rounded_xs()
                .bg(status_dnd())
                .text_color(crate::tokens::colors::white())
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .child("AO VIVO")
        }))
        .children(participant.is_camera_on.then(|| {
            render_camera_glyph(px(14.0), text_col)
        }))
        .children(participant.is_muted.then(|| icon_mic_off(px(14.0), status_dnd())))
        .children(participant.is_deafened.then(|| {
            icon_headphones_off(px(14.0), status_dnd())
        }))
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
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(px(12.0))
                .p(px(8.0))
                .rounded(px(12.0))
                .bg(discord.bg_server)
                .child(render_mute_control(app_entity.clone(), call, cx))
                .child(render_deafen_control(app_entity.clone(), call, cx))
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
    pub tooltip: SharedString,
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
            .tooltip(move |window, cx| {
                Tooltip::new(self.tooltip.clone()).build(window, cx)
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
        tooltip: "Selecionar microfone".into(),
    };

    let mic_arrow = Popover::new("call-mic-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| {
                render_input_device_menu(entity.clone(), cx)
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

fn render_deafen_control(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let entity_btn = app_entity.clone();
    let is_deafened = call.is_deafened;
    let bg_col = if is_deafened {
        control_critical_primary_bg_default()
    } else {
        control_secondary_bg_default()
    };
    let hover_col = if is_deafened {
        control_critical_primary_bg_hover()
    } else {
        control_secondary_bg_hover()
    };

    let deafen_btn = div()
        .id("call-deafen-btn")
        .w(px(44.0))
        .h(px(44.0))
        .rounded(px(8.0))
        .bg(bg_col)
        .text_color(discord.white)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(hover_col))
        .tooltip(move |window, cx| {
            Tooltip::new(if is_deafened {
                "Ativar áudio"
            } else {
                "Desativar áudio"
            })
            .build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity_btn.update(cx, |state, cx| {
                state.toggle_deafen();
                cx.notify();
            });
        })
        .child(if is_deafened {
            icon_headphones_off(px(20.0), discord.white)
        } else {
            icon_headphones(px(20.0), discord.white)
        });

    let trigger = ArrowDropupTrigger {
        is_open: false,
        id: "call-output-arrow".into(),
        bg_col,
        hover_col,
        tooltip: "Selecionar saída de áudio".into(),
    };
    let output_popover = Popover::new("call-output-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| render_output_device_menu(entity.clone(), cx)
        });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(deafen_btn)
        .child(output_popover)
        .into_any_element()
}

fn render_camera_control(app_entity: Entity<AppState>, _is_camera_on: bool, cx: &App) -> AnyElement {
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

    let camera_trigger = ArrowDropupTrigger {
        is_open: false,
        id: "call-cam-arrow".into(),
        bg_col: control_secondary_bg_default(),
        hover_col: control_secondary_bg_hover(),
        tooltip: "Selecionar câmera (transmissão indisponível)".into(),
    };
    let cam_arrow = Popover::new("call-camera-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(camera_trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| render_camera_device_menu(entity.clone(), cx)
        });

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

fn render_input_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    render_audio_device_menu(
        app_entity,
        crate::media::DeviceType::Input,
        "MICROFONES",
        "Nenhum microfone encontrado",
        cx,
    )
}

fn render_output_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    render_audio_device_menu(
        app_entity,
        crate::media::DeviceType::Output,
        "SAÍDAS DE ÁUDIO",
        "Nenhum alto-falante encontrado",
        cx,
    )
}

fn render_audio_device_menu(
    app_entity: Entity<AppState>,
    device_type: crate::media::DeviceType,
    title: &str,
    empty_message: &str,
    cx: &App,
) -> AnyElement {
    let selected_device = {
        let state = app_entity.read(cx);
        let call = state.active_voice_call.as_ref();
        match device_type {
            crate::media::DeviceType::Input => call.and_then(|call| call.selected_input_device.clone()),
            crate::media::DeviceType::Output => call.and_then(|call| call.selected_output_device.clone()),
        }
    };

    let device_manager = MediaDeviceManager::new();
    let devices = match device_type {
        crate::media::DeviceType::Input => device_manager.list_input_devices(),
        crate::media::DeviceType::Output => device_manager.list_output_devices(),
    };

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
                .child(title.to_string()),
        );

    match devices {
        Err(error) => {
            menu = menu.child(
                div()
                    .text_color(status_dnd())
                    .text_size(px(12.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .child(format!("Não foi possível listar os dispositivos: {error}")),
            );
        }
        Ok(devices) if devices.is_empty() => {
            menu = menu.child(
                div()
                    .text_color(text_muted())
                    .text_size(px(12.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .child(empty_message.to_string()),
            );
        }
        Ok(devices) => {
            let default_entity = app_entity.clone();
            let default_selected = selected_device.is_none();
            menu = menu.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .hover(|style| style.bg(channel_hover()))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        default_entity.update(cx, |state, cx| {
                            match device_type {
                                crate::media::DeviceType::Input => state.set_input_device(None),
                                crate::media::DeviceType::Output => state.set_output_device(None),
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_color(if default_selected {
                                text_primary()
                            } else {
                                text_secondary()
                            })
                            .text_size(px(13.0))
                            .child("Padrão do sistema"),
                    )
                    .children(default_selected.then(|| {
                        icon_check(px(14.0), status_online()).into_any_element()
                    })),
            );

            for device in devices {
                let is_selected = selected_device.as_deref() == Some(device.id.as_str());
                let device_id = device.id.clone();
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
                        .hover(|style| style.bg(channel_hover()))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            let device_id = device_id.clone();
                            entity.update(cx, |state, cx| {
                                match device_type {
                                    crate::media::DeviceType::Input => {
                                        state.set_input_device(Some(device_id))
                                    }
                                    crate::media::DeviceType::Output => {
                                        state.set_output_device(Some(device_id))
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .text_color(if is_selected {
                                    text_primary()
                                } else {
                                    text_secondary()
                                })
                                .text_size(px(13.0))
                                .truncate()
                                .child(device.name),
                        )
                        .children(is_selected.then(|| {
                            icon_check(px(14.0), status_online()).into_any_element()
                        })),
                );
            }
        }
    }

    menu.into_any_element()
}

fn render_camera_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    let selected_camera = app_entity
        .read(cx)
        .active_voice_call
        .as_ref()
        .and_then(|call| call.selected_camera_device.clone());
    let devices = CameraManager::list_devices();

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
        .shadow_lg()
        .child(
            div()
                .text_color(text_secondary())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .child("CÂMERAS DISPONÍVEIS"),
        );

    match devices {
        Err(error) => {
            menu = menu.child(
                div()
                    .text_color(status_dnd())
                    .text_size(px(12.0))
                    .py(px(4.0))
                    .child(format!("Não foi possível listar as câmeras: {error}")),
            );
        }
        Ok(devices) if devices.is_empty() => {
            menu = menu.child(
                div()
                    .text_color(text_muted())
                    .text_size(px(12.0))
                    .py(px(4.0))
                    .child("Nenhuma câmera encontrada"),
            );
        }
        Ok(devices) => {
            for device in devices {
                let is_selected = selected_camera.as_deref() == Some(device.id.as_str());
                let device_id = device.id.clone();
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
                        .hover(|style| style.bg(channel_hover()))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            let device_id = device_id.clone();
                            entity.update(cx, |state, cx| {
                                state.set_camera_device(Some(device_id));
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .text_color(if is_selected {
                                    text_primary()
                                } else {
                                    text_secondary()
                                })
                                .text_size(px(13.0))
                                .truncate()
                                .child(device.name),
                        )
                        .children(is_selected.then(|| {
                            icon_check(px(14.0), status_online()).into_any_element()
                        })),
                );
            }
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
