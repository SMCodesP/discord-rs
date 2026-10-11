use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::domain::settings::{MessageDisplayMode, ThemeChoice};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_section_header;

pub fn render_appearance(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let current_theme = app_state.theme_choice;
    let display_mode = app_state.message_display_mode;
    let chat_font_size = app_state.accessibility_settings.chat_font_size;
    let current_user = &app_state.current_user;

    div()
        .flex()
        .flex_col()
        .gap(px(24.0))
        // -------------------------------------------------------------
        // Theme Selector
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("TEMA", cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(14.0))
                        .child(render_theme_card(
                            app_entity.clone(),
                            ThemeChoice::Dark,
                            current_theme,
                            "Escuro",
                            "Clássico (#313338)",
                            cx.discord().bg_chat,
                            cx.discord().bg_channels,
                            cx.discord().text_titles,
                            cx,
                        ))
                        .child(render_theme_card(
                            app_entity.clone(),
                            ThemeChoice::Light,
                            current_theme,
                            "Claro",
                            "Alto contraste (#ffffff)",
                            cx.discord().white,
                            cx.discord().text_titles,
                            cx.discord().bg_chat,
                            cx,
                        ))
                        .child(render_theme_card(
                            app_entity.clone(),
                            ThemeChoice::Midnight,
                            current_theme,
                            "Midnight OLED",
                            "Preto puro (#000000)",
                            cx.discord().transparent,
                            cx.discord().bg_input,
                            cx.discord().white,
                            cx,
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
                .child(render_section_header("EXIBIÇÃO DE MENSAGENS", cx))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(render_display_mode_card(
                            app_entity.clone(),
                            MessageDisplayMode::Cozy,
                            display_mode,
                            "Aconchegante",
                            "Avatares grandes de 40px, mensagens modernas e espaçamento confortável.",
                            true,
                            chat_font_size,
                            current_user,
                            cx,
                        ))
                        .child(render_display_mode_card(
                            app_entity.clone(),
                            MessageDisplayMode::Compact,
                            display_mode,
                            "Compacto",
                            "Linhas condensadas estilo IRC clássico, timestamp à esquerda e sem avatares.",
                            false,
                            chat_font_size,
                            current_user,
                            cx,
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
                        .child(render_section_header("ESCALA DO CHAT E TAMANHO DA FONTE", cx))
                        .child(
                            div()
                                .text_color(cx.discord().blurple)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(13.0))
                                .child(format!(
                                    "{}px{}",
                                    chat_font_size as u32,
                                    if chat_font_size == 16.0 { " (Padrão)" } else { "" }
                                )),
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
                // Font Size Preset Buttons (12px to 20px)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(render_font_preset_btn(app_entity.clone(), "12px", 12.0, chat_font_size, cx))
                        .child(render_font_preset_btn(app_entity.clone(), "14px", 14.0, chat_font_size, cx))
                        .child(render_font_preset_btn(app_entity.clone(), "15px", 15.0, chat_font_size, cx))
                        .child(render_font_preset_btn(app_entity.clone(), "16px (Padrão)", 16.0, chat_font_size, cx))
                        .child(render_font_preset_btn(app_entity.clone(), "18px", 18.0, chat_font_size, cx))
                        .child(render_font_preset_btn(app_entity.clone(), "20px", 20.0, chat_font_size, cx)),
                )
                // Live Font Preview Box
                .child(
                    div()
                        .p(px(14.0))
                        .rounded(px(6.0))
                        .bg(cx.discord().bg_server)
                        .border_1()
                        .border_color(cx.discord().bg_channels)
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
                                        .text_color(cx.discord().blurple)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(13.0))
                                        .child(current_user.display_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_secondary)
                                        .text_size(px(11.0))
                                        .child("Hoje"),
                                ),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_primary)
                                .text_size(px(chat_font_size))
                                .child(format!(
                                    "Pré-visualização do chat com escala de {}px.",
                                    chat_font_size as u32
                                )),
                        ),
                ),
        )
}

fn render_theme_card(
    app_entity: Entity<AppState>,
    theme: ThemeChoice,
    current_theme: ThemeChoice,
    name: &'static str,
    subtitle: &'static str,
    bg_color: Rgba,
    card_color: Rgba,
    text_col: Rgba,
    cx: &App,
) -> Div {
    let is_selected = current_theme == theme;
    let entity = app_entity.clone();
    let border_color = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().neutral_67
    };
    let hover_bg = cx.discord().channel_hover;

    div()
        .w(px(210.0))
        .p(px(14.0))
        .rounded(px(8.0))
        .bg(cx.discord().bg_channels)
        .border_2()
        .border_color(border_color)
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            entity.update(cx, |s, cx| {
                s.set_theme_choice(theme);
                cx.notify();
            });
        })
        .child(
            div()
                .h(px(72.0))
                .rounded(px(6.0))
                .bg(bg_color)
                .border_1()
                .border_color(if is_selected { cx.discord().blurple } else { cx.discord().borders })
                .overflow_hidden()
                .flex()
                .flex_row()
                .child(
                    div()
                        .w(px(56.0))
                        .h_full()
                        .bg(card_color)
                        .p(px(6.0))
                        .flex()
                        .flex_col()
                        .gap(px(5.0))
                        .child(
                            div()
                                .h(px(6.0))
                                .w(px(36.0))
                                .rounded_full()
                                .bg(text_col),
                        )
                        .child(
                            div()
                                .h(px(5.0))
                                .w(px(28.0))
                                .rounded_full()
                                .bg(text_col),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .p(px(8.0))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .h(px(5.0))
                                        .w(px(70.0))
                                        .rounded_full()
                                        .bg(text_col),
                                )
                                .child(
                                    div()
                                        .h(px(5.0))
                                        .w(px(100.0))
                                        .rounded_full()
                                        .bg(text_col),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.0))
                        .child(name),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(11.0))
                        .child(subtitle),
                ),
        )
}

fn render_display_mode_card(
    app_entity: Entity<AppState>,
    mode: MessageDisplayMode,
    current_mode: MessageDisplayMode,
    title: &'static str,
    desc: &'static str,
    is_cozy: bool,
    font_size: f32,
    current_user: &crate::app::state::CurrentUserProfile,
    cx: &App,
) -> Div {
    let is_selected = current_mode == mode;
    let entity = app_entity.clone();
    let border_color = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().neutral_67
    };
    let hover_bg = cx.discord().channel_hover;

    div()
        .p(px(14.0))
        .rounded(px(8.0))
        .bg(cx.discord().bg_channels)
        .border_2()
        .border_color(border_color)
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .hover(move |s| s.bg(hover_bg))
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
                        .border_color(if is_selected { cx.discord().blurple } else { cx.discord().borders })
                        .bg(if is_selected { cx.discord().blurple } else { cx.discord().bg_channels })
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(if is_selected {
                            Some(
                                div()
                                    .text_color(cx.discord().white)
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
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(14.0))
                                .child(title),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
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
                .bg(cx.discord().bg_server)
                .children(if is_cozy {
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .gap(px(12.0))
                            .child(
                                Avatar::new(current_user.avatar_text.clone())
                                    .size(AvatarSize::Px40)
                                    .opt_src(current_user.avatar_url.clone())
                                    .opt_decoration(current_user.avatar_decoration_data.clone())
                                    .status(current_user.presence.into())
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
                                            .child(
                                                div()
                                                    .text_color(cx.discord().text_titles)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(px(13.0))
                                                    .child(current_user.display_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(cx.discord().text_secondary)
                                                    .text_size(px(11.0))
                                                    .child("Hoje"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.discord().text_primary)
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
                            .child(
                                div()
                                    .text_color(cx.discord().text_secondary)
                                    .text_size(px(11.0))
                                    .child("12:00"),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().text_titles)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(13.0))
                                    .child(format!("{}:", current_user.display_name)),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().text_primary)
                                    .text_size(px(font_size))
                                    .child("Visual Compacto estilo IRC denso em linha única."),
                            ),
                    )
                }),
        )
}

fn render_font_preset_btn(
    app_entity: Entity<AppState>,
    label: &'static str,
    size_val: f32,
    current_size: f32,
    cx: &App,
) -> Div {
    let is_selected = current_size == size_val;
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
