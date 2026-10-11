use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_toggle_row};

pub fn render_accessibility(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let access = &app_state.accessibility_settings;
    let reduce_motion = access.reduce_motion;
    let auto_play_stickers = access.auto_play_stickers;
    let chat_font_size = access.chat_font_size;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        // -------------------------------------------------------------
        // Text Readability (settings-accessibility.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .child(render_section_header("TEXT READABILITY", cx))
                // Text size in chat
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
                                        .child("Text size in chat"),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().blurple)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(13.0))
                                        .child(format!("{}px{}", chat_font_size as u32, if chat_font_size == 16.0 { " (Default)" } else { "" })),
                                ),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .text_size(px(12.0))
                                .child("Adjust the size of the chat font."),
                        )
                        .child({
                            let fill_pct = ((chat_font_size - 12.0) / 12.0).clamp(0.0, 1.0);
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
                                        .size(px(16.0))
                                        .rounded_full()
                                        .bg(cx.discord().white)
                                        .border_2()
                                        .border_color(cx.discord().blurple)
                                        .ml(px(-8.0)),
                                )
                        })
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .pt(px(2.0))
                                .child(render_font_preset_btn(app_entity.clone(), "12px", 12.0, chat_font_size, cx))
                                .child(render_font_preset_btn(app_entity.clone(), "14px", 14.0, chat_font_size, cx))
                                .child(render_font_preset_btn(app_entity.clone(), "15px", 15.0, chat_font_size, cx))
                                .child(render_font_preset_btn(app_entity.clone(), "16px (Default)", 16.0, chat_font_size, cx))
                                .child(render_font_preset_btn(app_entity.clone(), "18px", 18.0, chat_font_size, cx))
                                .child(render_font_preset_btn(app_entity.clone(), "20px", 20.0, chat_font_size, cx))
                                .child(render_font_preset_btn(app_entity.clone(), "24px", 24.0, chat_font_size, cx)),
                        ),
                )
                // Always underline links
                .child({
                    let entity = app_entity.clone();
                    render_toggle_row(
                        "toggle-underline-links",
                        "Always underline links",
                        "Make links stand out more.",
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
                // Display Name Styles
                .child({
                    let entity = app_entity.clone();
                    render_toggle_row(
                        "toggle-display-name-styles",
                        "Display Name Styles",
                        "Enable display name styles — including font, effect, and color — across Discord.",
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
}

fn render_font_preset_btn(
    app_entity: Entity<AppState>,
    label: &'static str,
    size_val: f32,
    current_size: f32,
    cx: &App,
) -> Div {
    let is_selected = (current_size - size_val).abs() < 0.5;
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
                s.set_chat_font_size(size_val);
                cx.notify();
            });
        })
        .child(label)
}
