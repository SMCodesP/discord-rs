use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_section_description;
use super::UserSettingsModal;

pub fn render_language(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let current_lang = modal_state.selected_language.clone();

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
        .child(render_section_description(
            "Selecione o idioma da interface. As alterações são aplicadas instantaneamente em todos os menus do aplicativo.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(12.0))
                .children(languages.into_iter().map(|(code, flag, native_name, pt_name)| {
                    let is_selected = current_lang == code;
                    let modal_c = modal.clone();

                    let card_bg = if is_selected {
                        cx.discord().bg_channels
                    } else {
                        cx.discord().bg_server
                    };
                    let border_color = if is_selected {
                        cx.discord().status_online
                    } else {
                        cx.discord().bg_channels
                    };
                    let hover_bg = cx.discord().channel_hover;

                    div()
                        .w(px(320.0))
                        .h(px(52.0))
                        .px(px(14.0))
                        .rounded(px(6.0))
                        .bg(card_bg)
                        .border_2()
                        .border_color(border_color)
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover_bg))
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
                                                .text_color(cx.discord().text_titles)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(px(14.0))
                                                .child(native_name),
                                        )
                                        .child(
                                            div()
                                                .text_color(cx.discord().text_secondary)
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
                                .border_color(if is_selected {
                                    cx.discord().status_online
                                } else {
                                    cx.discord().borders
                                })
                                .bg(if is_selected {
                                    cx.discord().status_online
                                } else {
                                    cx.discord().bg_server
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .children(if is_selected {
                                    Some(
                                        div()
                                            .text_color(cx.discord().white)
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
