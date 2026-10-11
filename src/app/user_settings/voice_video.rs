use gpui::prelude::*;
use gpui::*;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_slider_row};
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

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        // -------------------------------------------------------------
        // Voice Header & Devices (settings-voice-video.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(render_section_header("VOICE", cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(20.0))
                        // Microphone
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("MICROPHONE", cx))
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
                                        .child("Default")
                                        .child("⌵"),
                                ),
                        )
                        // Speaker
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("SPEAKER", cx))
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
                                        .child("Default")
                                        .child("⌵"),
                                ),
                        ),
                ),
        )
        // -------------------------------------------------------------
        // Volume Sliders (settings-voice-video.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child({
                    let modal_c = modal.clone();
                    render_slider_row(
                        "MICROPHONE VOLUME",
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
                        "SPEAKER VOLUME",
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
        // -------------------------------------------------------------
        // Mic Test (settings-voice-video.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(render_section_header("MIC TEST", cx))
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
                                if is_testing { "Stop Testing" } else { "Mic Test" },
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
        // -------------------------------------------------------------
        // Input Profile (settings-voice-video.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("INPUT PROFILE", cx))
                .child(render_profile_option(
                    "Voice Isolation",
                    "Just your beautiful voice: let Discord cut through the noise",
                    false,
                    cx,
                ))
                .child(render_profile_option(
                    "Studio",
                    "Pure audio: open mic with no processing",
                    false,
                    cx,
                ))
                .child(render_profile_option(
                    "Custom",
                    "Advanced controls: use all the buttons and dials",
                    true,
                    cx,
                )),
        )
}

fn render_profile_option(
    title: &'static str,
    desc: &'static str,
    is_selected: bool,
    cx: &App,
) -> Div {
    div()
        .p(px(12.0))
        .rounded(px(6.0))
        .bg(cx.discord().bg_channels)
        .border_1()
        .border_color(if is_selected { cx.discord().blurple } else { cx.discord().neutral_67 })
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .size(px(18.0))
                .rounded_full()
                .border_2()
                .border_color(if is_selected { cx.discord().blurple } else { cx.discord().borders })
                .bg(if is_selected { cx.discord().blurple } else { cx.discord().transparent })
                .flex()
                .items_center()
                .justify_center()
                .children(if is_selected {
                    Some(
                        div()
                            .size(px(6.0))
                            .rounded_full()
                            .bg(cx.discord().white),
                    )
                } else {
                    None
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.0))
                        .child(title),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child(desc),
                ),
        )
}
