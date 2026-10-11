use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_description, render_section_header, render_toggle_row};

pub fn render_accessibility(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let access = &app_state.accessibility_settings;
    let reduce_motion = access.reduce_motion;
    let auto_play_stickers = access.auto_play_stickers;
    let saturation = access.saturation;
    let high_contrast = access.high_contrast;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_section_description(
            "Personalize os efeitos visuais, contraste e saturação para aprimorar sua experiência de uso e navegação.",
            cx,
        ))
        // -------------------------------------------------------------
        // Section 1: Motion Controls (Redução de Movimento)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("CONTROLE DE MOVIMENTO E ANIMAÇÕES", cx))
                .child({
                    let entity = app_entity.clone();
                    render_toggle_row(
                        "toggle-reduce-motion",
                        "Redução de Movimento (Reduce Motion)",
                        "Desativa transições elásticas, animações e efeitos em toda a interface gráfica.",
                        reduce_motion,
                        move |_checked, _win, cx| {
                            entity.update(cx, |s, cx| {
                                s.set_reduce_motion(!reduce_motion);
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let entity = app_entity.clone();
                    render_toggle_row(
                        "toggle-autoplay-stickers",
                        "Reproduzir figurinhas e emojis automaticamente",
                        "Quando ativado, figurinhas e emojis animados são reproduzidos continuamente.",
                        auto_play_stickers,
                        move |_checked, _win, cx| {
                            entity.update(cx, |s, cx| {
                                s.accessibility_settings.auto_play_stickers = !auto_play_stickers;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        .child(render_divider(cx))
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
                        .child(render_section_header("CONTROLE DE SATURAÇÃO DE CORES", cx))
                        .child(
                            div()
                                .text_color(cx.discord().blurple)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(12.0))
                                .child(format!(
                                    "{}%{}",
                                    (saturation * 100.0) as u32,
                                    if saturation == 1.0 { " (Padrão)" } else if saturation == 0.0 { " (P&B)" } else { "" }
                                )),
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
                        .bg(cx.discord().borders)
                        .relative()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .h(px(8.0))
                                .w(px(filled_w))
                                .rounded_full()
                                .bg(cx.discord().blurple),
                        )
                        .child(
                            div()
                                .size(px(18.0))
                                .rounded_full()
                                .bg(cx.discord().white)
                                .border_2()
                                .border_color(cx.discord().blurple)
                                .ml(px(-9.0)),
                        )
                })
                // Presets
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(render_saturation_preset_btn(app_entity.clone(), "0% (Escala de Cinza)", 0.0, saturation, cx))
                        .child(render_saturation_preset_btn(app_entity.clone(), "25%", 0.25, saturation, cx))
                        .child(render_saturation_preset_btn(app_entity.clone(), "50%", 0.50, saturation, cx))
                        .child(render_saturation_preset_btn(app_entity.clone(), "75%", 0.75, saturation, cx))
                        .child(render_saturation_preset_btn(app_entity.clone(), "100% (Padrão)", 1.0, saturation, cx)),
                )
                // Live Palette Preview
                .child(
                    div()
                        .p(px(12.0))
                        .rounded(px(6.0))
                        .bg(cx.discord().bg_server)
                        .border_1()
                        .border_color(cx.discord().bg_channels)
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(10.0))
                                .child("PALETA DE CORES DA INTERFACE"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(render_color_chip(cx.discord().blurple, "Blurple", cx))
                                .child(render_color_chip(cx.discord().status_online, "Online", cx))
                                .child(render_color_chip(cx.discord().status_idle, "Ausente", cx))
                                .child(render_color_chip(cx.discord().status_dnd, "Ocupado", cx))
                                .child(render_color_chip(cx.discord().role_purple, "Purple", cx)),
                        ),
                ),
        )
        .child(render_divider(cx))
        // -------------------------------------------------------------
        // Section 3: High Contrast
        // -------------------------------------------------------------
        .child({
            let entity = app_entity.clone();
            render_toggle_row(
                "toggle-high-contrast",
                "Alto Contraste",
                "Aumenta o contraste das bordas e divisores para melhor legibilidade.",
                high_contrast,
                move |_checked, _win, cx| {
                    entity.update(cx, |s, cx| {
                        s.accessibility_settings.high_contrast = !high_contrast;
                        cx.notify();
                    });
                },
                cx,
            )
        })
}

fn render_saturation_preset_btn(
    app_entity: Entity<AppState>,
    label: &'static str,
    val: f32,
    current_val: f32,
    cx: &App,
) -> Div {
    let is_selected = (current_val - val).abs() < 0.05;
    let bg_col = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().bg_channels
    };
    let text_col = if is_selected {
        cx.discord().white
    } else {
        cx.discord().text_secondary
    };

    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(bg_col)
        .text_color(text_col)
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            app_entity.update(cx, |s, cx| {
                s.set_color_saturation(val);
                cx.notify();
            });
        })
        .child(label)
}

fn render_color_chip(color: Rgba, label: &'static str, cx: &App) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .size(px(14.0))
                .rounded(px(3.0))
                .bg(color),
        )
        .child(
            div()
                .text_color(cx.discord().text_primary)
                .text_size(px(11.0))
                .child(label),
        )
}
