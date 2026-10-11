use gpui::prelude::*;
use gpui::*;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_description, render_section_header, render_slider_row, render_toggle_row};
use super::UserSettingsModal;

pub fn render_voice_video_expanded(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let modal_read = modal_state;
    let in_vol = modal_read.input_volume;
    let out_vol = modal_read.output_volume;
    let is_testing = modal_read.is_testing_mic;
    let krisp = modal_read.krisp_enabled;
    let echo = modal_read.echo_cancellation;
    let agc = modal_read.auto_gain;
    let res = modal_read.video_resolution;
    let fps = modal_read.video_fps;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_section_description(
            "Configure seus dispositivos de áudio, níveis de volume, filtros de ruído e transmissão de vídeo.",
            cx,
        ))
        // Section 1: Devices
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(20.0))
                // Input Device Box
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(render_section_header("DISPOSITIVO DE ENTRADA", cx))
                        .child(
                            div()
                                .h(px(40.0))
                                .px(px(12.0))
                                .rounded(px(4.0))
                                .bg(cx.discord().bg_server)
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_color(cx.discord().text_primary)
                                .text_size(px(13.0))
                                .child("Microfone Padrão (PipeWire / ALSA)")
                                .child("⌵"),
                        ),
                )
                // Output Device Box
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(render_section_header("DISPOSITIVO DE SAÍDA", cx))
                        .child(
                            div()
                                .h(px(40.0))
                                .px(px(12.0))
                                .rounded(px(4.0))
                                .bg(cx.discord().bg_server)
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_color(cx.discord().text_primary)
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
                .child({
                    let modal_c = modal.clone();
                    render_slider_row(
                        "VOLUME DE ENTRADA (MICROFONE)",
                        in_vol,
                        move |v, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.input_volume = v;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_slider_row(
                        "VOLUME DE SAÍDA (ALTO-FALANTES)",
                        out_vol,
                        move |v, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.output_volume = v;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        // Section 3: Mic Test
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(render_section_header("TESTE DE MICROFONE", cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(16.0))
                        .child({
                            let modal_c = modal.clone();
                            Button::new(
                                "btn-mic-test",
                                if is_testing { "Parar Teste" } else { "Vamos Testar" },
                            )
                            .variant(if is_testing { ButtonVariant::Danger } else { ButtonVariant::Blurple })
                            .size(ButtonSize::Sm)
                            .on_click(move |_ev, _win, cx| {
                                modal_c.update(cx, |this, cx| {
                                    this.is_testing_mic = !this.is_testing_mic;
                                    cx.notify();
                                });
                            })
                        })
                        // Meter Bar
                        .child(
                            div()
                                .flex_1()
                                .h(px(8.0))
                                .rounded_full()
                                .bg(cx.discord().borders)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .h_full()
                                        .w(if is_testing { px(180.0) } else { px(0.0) })
                                        .bg(cx.discord().status_online),
                                ),
                        ),
                ),
        )
        .child(render_divider(cx))
        // Section 4: Advanced Voice Processing
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("PROCESSAMENTO AVANÇADO DE VOZ", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-krisp",
                        "Supressão de Ruído Krisp",
                        "Filtro neural avançado para eliminar ruídos de fundo e teclados mecânicos.",
                        krisp,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.krisp_enabled = !this.krisp_enabled;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-echo",
                        "Cancelamento de Eco",
                        "Impede que o áudio das caixas de som retorne ao microfone.",
                        echo,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.echo_cancellation = !this.echo_cancellation;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-agc",
                        "Controle Automático de Ganho",
                        "Equaliza volumes baixos para manter consistência vocal em chamadas de grupo.",
                        agc,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.auto_gain = !this.auto_gain;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        .child(render_divider(cx))
        // Section 5: Video Settings
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("CONFIGURAÇÕES DE TRANSMISSÃO DE VÍDEO", cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(16.0))
                        .child(render_quality_pill(modal.clone(), "720p HD", res == 720, true, 720, cx))
                        .child(render_quality_pill(modal.clone(), "1080p FHD (Nitro)", res == 1080, true, 1080, cx))
                        .child(render_quality_pill(modal.clone(), "30 FPS", fps == 30, false, 30, cx))
                        .child(render_quality_pill(modal.clone(), "60 FPS (Nitro)", fps == 60, false, 60, cx)),
                ),
        )
}

fn render_quality_pill(
    modal: Entity<UserSettingsModal>,
    label: &'static str,
    is_active: bool,
    is_res: bool,
    val: u32,
    cx: &App,
) -> Div {
    let bg_col = if is_active {
        cx.discord().blurple
    } else {
        cx.discord().bg_channels
    };
    let text_col = if is_active {
        cx.discord().white
    } else {
        cx.discord().text_secondary
    };

    div()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(4.0))
        .bg(bg_col)
        .text_color(text_col)
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            modal.update(cx, |this, cx| {
                if is_res {
                    this.video_resolution = val;
                } else {
                    this.video_fps = val;
                }
                cx.notify();
            });
        })
        .child(label)
}
