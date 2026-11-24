use gpui::prelude::*;
use gpui::*;

use crate::app::state::{AppState, CurrentUserProfile};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::domain::settings::{MessageDisplayMode, ThemeChoice};
use crate::tokens::colors::DiscordPalette;
use crate::tokens::kit_theme::{DiscordTheme, install_with_palette};

use super::components::render_section_header;

pub fn render_appearance(app_entity: Entity<AppState>, app_state: &AppState, cx: &App) -> Div {
    let current_theme = app_state.theme_choice;
    let display_mode = app_state.message_display_mode;
    let chat_font_size = app_state.accessibility_settings.chat_font_size;
    let zoom = app_state.accessibility_settings.zoom_level;
    let current_user = &app_state.current_user;

    div()
        .flex()
        .flex_col()
        .gap(px(24.0))
        .child(render_theme_section(app_entity.clone(), current_theme, cx))
        .child(render_display_mode_section(
            app_entity.clone(),
            display_mode,
            chat_font_size,
            current_user,
            cx,
        ))
        .child(render_font_scale_section(
            app_entity.clone(),
            chat_font_size,
            current_user,
            cx,
        ))
        .child(render_zoom_section(app_entity, zoom, cx))
}

fn render_theme_section(app_entity: Entity<AppState>, current_theme: ThemeChoice, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(render_section_header("TEMA", cx))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(14.0))
                .child(render_theme_card(
                    app_entity.clone(),
                    ThemeChoice::Dark,
                    current_theme,
                    "Escuro",
                    "Tema clássico",
                    rgb(0x313338),
                    rgb(0x2b2d31),
                    rgb(0xf2f3f5),
                    cx,
                ))
                .child(render_theme_card(
                    app_entity.clone(),
                    ThemeChoice::Light,
                    current_theme,
                    "Claro",
                    "Tema claro",
                    rgb(0xffffff),
                    rgb(0xf3f3f4),
                    rgb(0x2e2e34),
                    cx,
                ))
                .child(render_theme_card(
                    app_entity.clone(),
                    ThemeChoice::Midnight,
                    current_theme,
                    "Meia-noite",
                    "OLED",
                    rgb(0x000000),
                    rgb(0x0a0a0c),
                    rgb(0xefeff1),
                    cx,
                ))
                .child(render_theme_card(
                    app_entity,
                    ThemeChoice::SyncWithSystem,
                    current_theme,
                    "Sincronizar com o computador",
                    "Segue o tema do sistema",
                    rgb(0xffffff),
                    rgb(0xf3f3f4),
                    rgb(0x2e2e34),
                    cx,
                )),
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
    text_color: Rgba,
    cx: &App,
) -> Div {
    let is_selected = current_theme == theme;
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
        .hover(move |style| style.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            cx.stop_propagation();
            app_entity.update(cx, |state, cx| {
                state.set_theme_choice(theme);
                cx.notify();
            });
            apply_theme(theme, window, cx);
        })
        .child(
            div()
                .h(px(72.0))
                .rounded(px(6.0))
                .bg(bg_color)
                .border_1()
                .border_color(cx.discord().borders)
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
                        .child(div().h(px(6.0)).w(px(36.0)).rounded_full().bg(text_color))
                        .child(div().h(px(5.0)).w(px(28.0)).rounded_full().bg(text_color)),
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
                                .child(div().h(px(5.0)).w(px(70.0)).rounded_full().bg(text_color))
                                .child(div().h(px(5.0)).w(px(100.0)).rounded_full().bg(text_color)),
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

fn render_display_mode_section(
    app_entity: Entity<AppState>,
    current_mode: MessageDisplayMode,
    font_size: f32,
    current_user: &CurrentUserProfile,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(render_section_header("EXIBIÇÃO DAS MENSAGENS", cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_display_mode_card(
                    app_entity.clone(),
                    MessageDisplayMode::Cozy,
                    current_mode,
                    "Aconchegante",
                    true,
                    font_size,
                    current_user,
                    cx,
                ))
                .child(render_display_mode_card(
                    app_entity,
                    MessageDisplayMode::Compact,
                    current_mode,
                    "Compacto",
                    false,
                    font_size,
                    current_user,
                    cx,
                )),
        )
}

fn render_display_mode_card(
    app_entity: Entity<AppState>,
    mode: MessageDisplayMode,
    current_mode: MessageDisplayMode,
    title: &'static str,
    is_cozy: bool,
    font_size: f32,
    current_user: &CurrentUserProfile,
    cx: &App,
) -> Div {
    let is_selected = current_mode == mode;
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
        .hover(move |style| style.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            cx.stop_propagation();
            app_entity.update(cx, |state, cx| {
                state.set_message_display_mode(mode);
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
                        .border_color(if is_selected {
                            cx.discord().blurple
                        } else {
                            cx.discord().borders
                        })
                        .bg(if is_selected {
                            cx.discord().blurple
                        } else {
                            cx.discord().bg_channels
                        })
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
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.0))
                        .child(title),
                ),
        )
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
                                    .status(current_user.presence.into()),
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
                                            .text_size(px(13.0))
                                            .child(if current_user.display_name.is_empty() {
                                                "Sem nome de exibição".to_string()
                                            } else {
                                                current_user.display_name.clone()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.discord().text_primary)
                                            .text_size(px(font_size))
                                            .child("Prévia do estilo de mensagem"),
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
                                    .text_color(cx.discord().text_titles)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(13.0))
                                    .child(if current_user.display_name.is_empty() {
                                        "Sem nome:".to_string()
                                    } else {
                                        format!("{}:", current_user.display_name)
                                    }),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().text_primary)
                                    .text_size(px(font_size))
                                    .child("Prévia do estilo de mensagem"),
                            ),
                    )
                }),
        )
}

fn render_font_scale_section(
    app_entity: Entity<AppState>,
    chat_font_size: f32,
    current_user: &CurrentUserProfile,
    cx: &App,
) -> Div {
    let fill_pct = ((chat_font_size - 12.0) / 12.0).clamp(0.0, 1.0);
    let track_w = 480.0;
    let filled_w = track_w * fill_pct;

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
                .child(render_section_header(
                    "ESCALA DO CHAT E TAMANHO DA FONTE",
                    cx,
                ))
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
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(8.0))
                .child(render_font_preset_btn(
                    app_entity.clone(),
                    "12px",
                    12.0,
                    chat_font_size,
                    cx,
                ))
                .child(render_font_preset_btn(
                    app_entity.clone(),
                    "14px",
                    14.0,
                    chat_font_size,
                    cx,
                ))
                .child(render_font_preset_btn(
                    app_entity.clone(),
                    "15px",
                    15.0,
                    chat_font_size,
                    cx,
                ))
                .child(render_font_preset_btn(
                    app_entity.clone(),
                    "16px (Padrão)",
                    16.0,
                    chat_font_size,
                    cx,
                ))
                .child(render_font_preset_btn(
                    app_entity.clone(),
                    "18px",
                    18.0,
                    chat_font_size,
                    cx,
                ))
                .child(render_font_preset_btn(
                    app_entity.clone(),
                    "20px",
                    20.0,
                    chat_font_size,
                    cx,
                ))
                .child(render_font_preset_btn(
                    app_entity,
                    "24px",
                    24.0,
                    chat_font_size,
                    cx,
                )),
        )
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
                        .text_color(cx.discord().blurple)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(13.0))
                        .child(if current_user.display_name.is_empty() {
                            "Sem nome de exibição".to_string()
                        } else {
                            current_user.display_name.clone()
                        }),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_primary)
                        .text_size(px(chat_font_size))
                        .child(format!(
                            "Prévia do chat com escala de {}px.",
                            chat_font_size as u32
                        )),
                ),
        )
}

fn render_zoom_section(app_entity: Entity<AppState>, zoom: f32, cx: &App) -> Div {
    let presets = [0.70, 0.80, 0.90, 1.0, 1.10, 1.25];

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(render_section_header("ZOOM DA INTERFACE", cx))
        .child(
            div()
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child("Ajusta a escala da interface entre 70% e 125%."),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(8.0))
                .children(presets.into_iter().map(|preset| {
                    let is_selected = (zoom - preset).abs() < 0.001;
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
                                state.set_zoom_level(preset);
                                cx.notify();
                            });
                        })
                        .child(format!("{}%", (preset * 100.0).round() as u32))
                })),
        )
}

fn render_font_preset_btn(
    app_entity: Entity<AppState>,
    label: &'static str,
    size_val: f32,
    current_size: f32,
    cx: &App,
) -> Div {
    let is_selected = (current_size - size_val).abs() < 0.5;
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
            app_entity.update(cx, |state, cx| {
                state.set_chat_font_size(size_val);
                cx.notify();
            });
        })
        .child(label)
}

pub(super) fn apply_theme(theme: ThemeChoice, window: &Window, cx: &mut App) {
    let palette = match theme {
        ThemeChoice::Dark => DiscordPalette::dark(),
        ThemeChoice::Light => light_palette(),
        ThemeChoice::Midnight => midnight_palette(),
        ThemeChoice::SyncWithSystem => {
            if matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ) {
                DiscordPalette::dark()
            } else {
                light_palette()
            }
        }
    };
    install_with_palette(cx, &palette);
}

/// Values come from the official Discord web client CSS asset
/// `https://discord.com/assets/952007.8c2302e28f050501.css`, `.theme-light`
/// (`background-base-*`, `background-surface-*`, `text-*`, `border-subtle`).
/// The neutral HSL primitives were converted to RGB with saturation factor 1.
fn light_palette() -> DiscordPalette {
    DiscordPalette {
        bg_server: rgb(0xf3f3f4),
        bg_channels: rgb(0xfbfbfb),
        bg_chat: rgb(0xffffff),
        bg_chat_hover: rgb(0xfbfbfb),
        bg_user_bar: rgb(0xfbfbfb),
        bg_input: rgba(0x00000005),
        bg_floating: rgb(0xffffff),
        text_primary: rgb(0x2e2e34),
        text_secondary: rgb(0x595a63),
        text_titles: rgb(0x28282d),
        text_muted: rgb(0x666770),
        borders: rgba(0x00000047),
        border_subtle: rgba(0x00000047),
        channel_hover: rgba(0x00000029),
        channel_active: rgba(0x0000003d),
        channels_default: rgb(0x666770),
        interactive_normal: rgb(0x2e2e34),
        scrollbar_thumb: rgb(0xc5c6ca),
        neutral_67: rgba(0x00000047),
        opacity_white_8: rgba(0x00000014),
        opacity_white_20: rgba(0x00000033),
        control_primary_border: rgba(0x00000014),
        control_secondary_bg_default: rgba(0x0000001f),
        control_secondary_bg_hover: rgba(0x00000029),
        control_secondary_bg_active: rgba(0x0000003d),
        control_icon_only_icon_default: rgb(0x2e2e34),
        ..DiscordPalette::dark()
    }
}

/// Values come from the same official CSS asset, `.theme-midnight` and its
/// neutral primitives. The main canvas remains pure OLED black.
fn midnight_palette() -> DiscordPalette {
    DiscordPalette {
        bg_server: rgb(0x000000),
        bg_channels: rgb(0x000000),
        bg_chat: rgb(0x000000),
        bg_chat_hover: rgb(0x0a0a0c),
        bg_user_bar: rgb(0x000000),
        bg_input: rgb(0x0a0a0c),
        bg_floating: rgb(0x0a0a0c),
        text_primary: rgb(0xd4d5d8),
        text_secondary: rgb(0x96979e),
        text_titles: rgb(0xefeff1),
        text_muted: rgb(0x7a7b83),
        borders: rgba(0xffffff33),
        border_subtle: rgba(0xffffff33),
        channel_hover: rgba(0xffffff14),
        channel_active: rgba(0xffffff24),
        channels_default: rgb(0x7a7b83),
        interactive_normal: rgb(0xd4d5d8),
        scrollbar_thumb: rgb(0x242429),
        neutral_67: rgba(0xffffff33),
        control_icon_only_icon_default: rgb(0xd4d5d8),
        ..DiscordPalette::dark()
    }
}
