use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_toggle_row};

pub fn render_accessibility(app_entity: Entity<AppState>, app_state: &AppState, cx: &App) -> Div {
    let access = &app_state.accessibility_settings;
    let reduce_motion = access.reduce_motion;
    let auto_play_gifs = access.auto_play_gifs;
    let auto_play_stickers = access.auto_play_stickers;
    let high_contrast = access.high_contrast;
    let chat_font_size = access.chat_font_size;
    let saturation = access.saturation;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_text_readability(
            app_entity.clone(),
            chat_font_size,
            cx,
        ))
        .child(render_divider(cx))
        .child(render_motion_and_media(
            app_entity.clone(),
            reduce_motion,
            auto_play_gifs,
            auto_play_stickers,
            cx,
        ))
        .child(render_divider(cx))
        .child(render_contrast(app_entity, saturation, high_contrast, cx))
}

fn render_text_readability(app_entity: Entity<AppState>, chat_font_size: f32, cx: &App) -> Div {
    let fill_pct = ((chat_font_size - 12.0) / 12.0).clamp(0.0, 1.0);
    let track_w = 480.0;
    let filled_w = track_w * fill_pct;
    let presets = [12.0, 14.0, 15.0, 16.0, 18.0, 20.0, 24.0];

    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(render_section_header("LEGIBILIDADE DO TEXTO", cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child("Tamanho do texto no chat"),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().blurple)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(13.0))
                                .child(format!(
                                    "{}px{}",
                                    chat_font_size as u32,
                                    if chat_font_size == 16.0 {
                                        " (Padrão)"
                                    } else {
                                        ""
                                    }
                                )),
                        ),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child("Ajusta o tamanho da fonte do chat."),
                )
                .child(
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
                                .size(px(16.0))
                                .rounded_full()
                                .bg(cx.discord().white)
                                .border_2()
                                .border_color(cx.discord().blurple)
                                .ml(px(-8.0)),
                        ),
                )
                .child(div().flex().flex_row().flex_wrap().gap(px(8.0)).children(
                    presets.into_iter().map(|size| {
                        let is_selected = (chat_font_size - size).abs() < 0.5;
                        let bg = if is_selected {
                            cx.discord().blurple
                        } else {
                            cx.discord().bg_channels
                        };
                        let text = if is_selected {
                            cx.discord().white
                        } else {
                            cx.discord().text_secondary
                        };
                        let entity = app_entity.clone();
                        div()
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .bg(bg)
                            .text_color(text)
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                entity.update(cx, |state, cx| {
                                    state.set_chat_font_size(size);
                                    cx.notify();
                                });
                            })
                            .child(format!("{}px", size as u32))
                    }),
                )),
        )
}

fn render_motion_and_media(
    app_entity: Entity<AppState>,
    reduce_motion: bool,
    auto_play_gifs: bool,
    auto_play_stickers: bool,
    cx: &App,
) -> Div {
    let motion_entity = app_entity.clone();
    let gifs_entity = app_entity.clone();
    let stickers_entity = app_entity;

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("MOVIMENTO E MÍDIA", cx))
        .child(render_toggle_row(
            "toggle-reduce-motion",
            "Redução de movimento",
            "Reduz animações e transições na interface.",
            reduce_motion,
            move |_checked, _window, cx| {
                motion_entity.update(cx, |state, cx| {
                    state.set_reduce_motion(!reduce_motion);
                    cx.notify();
                });
            },
            cx,
        ))
        .child(render_toggle_row(
            "toggle-auto-play-gifs",
            "Reproduzir GIFs automaticamente",
            "Reproduz GIFs animados sem passar o cursor.",
            auto_play_gifs,
            move |_checked, _window, cx| {
                gifs_entity.update(cx, |state, cx| {
                    state.set_auto_play_gifs(!auto_play_gifs);
                    cx.notify();
                });
            },
            cx,
        ))
        .child(render_toggle_row(
            "toggle-auto-play-stickers",
            "Reproduzir figurinhas automaticamente",
            "Reproduz figurinhas animadas sem passar o cursor.",
            auto_play_stickers,
            move |_checked, _window, cx| {
                stickers_entity.update(cx, |state, cx| {
                    state.set_auto_play_stickers(!auto_play_stickers);
                    cx.notify();
                });
            },
            cx,
        ))
}

fn render_contrast(
    app_entity: Entity<AppState>,
    saturation: f32,
    high_contrast: bool,
    cx: &App,
) -> Div {
    let saturation_pct = (saturation * 100.0).round() as u32;
    let fill_pct = saturation.clamp(0.0, 1.0);
    let track_w = 480.0;
    let filled_w = track_w * fill_pct;
    let contrast_entity = app_entity.clone();
    let presets = [0.0, 0.25, 0.50, 0.75, 1.0];

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("CONTRASTE E CORES", cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child("Saturação de cores"),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().blurple)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(13.0))
                                .child(format!("{}%", saturation_pct)),
                        ),
                )
                .child(
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
                                .size(px(16.0))
                                .rounded_full()
                                .bg(cx.discord().white)
                                .border_2()
                                .border_color(cx.discord().blurple)
                                .ml(px(-8.0)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .children(presets.into_iter().map(|preset| {
                            let is_selected = (saturation - preset).abs() < 0.001;
                            let bg = if is_selected {
                                cx.discord().blurple
                            } else {
                                cx.discord().bg_channels
                            };
                            let text = if is_selected {
                                cx.discord().white
                            } else {
                                cx.discord().text_secondary
                            };
                            let entity = app_entity.clone();
                            div()
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .bg(bg)
                                .text_color(text)
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    entity.update(cx, |state, cx| {
                                        state.set_color_saturation(preset);
                                        cx.notify();
                                    });
                                })
                                .child(format!("{}%", (preset * 100.0) as u32))
                        })),
                ),
        )
        .child(render_toggle_row(
            "toggle-high-contrast",
            "Contraste alto",
            "Aumenta o contraste entre textos e superfícies.",
            high_contrast,
            move |_checked, _window, cx| {
                contrast_entity.update(cx, |state, cx| {
                    state.set_enhanced_contrast(!high_contrast);
                    cx.notify();
                });
            },
            cx,
        ))
}
