use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};

/// Real Discord language list. Only the locales backed by the bundled i18n
/// dictionaries are selectable; the rest are shown as unavailable instead of
/// pretending the switch succeeded (Zero Mocks).
const LANGUAGES: &[(&str, &str, &str, &str, bool)] = &[
    ("pt-BR", "🇧🇷", "Português do Brasil", "Portuguese (Brazil)", true),
    ("en-US", "🇺🇸", "English (US)", "Inglês (Estados Unidos)", true),
    ("es-ES", "🇪🇸", "Español", "Espanhol", false),
    ("fr-FR", "🇫🇷", "Français", "Francês", false),
    ("de-DE", "🇩🇪", "Deutsch", "Alemão", false),
    ("it-IT", "🇮🇹", "Italiano", "Italiano", false),
    ("ja-JP", "🇯🇵", "日本語", "Japonês", false),
    ("ko-KR", "🇰🇷", "한국어", "Coreano", false),
    ("ru-RU", "🇷🇺", "Русский", "Russo", false),
    ("zh-CN", "🇨🇳", "简体中文", "Chinês Simplificado", false),
    ("pl-PL", "🇵🇱", "Polski", "Polonês", false),
    ("nl-NL", "🇳🇱", "Nederlands", "Holandês", false),
];

pub fn render_language(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let current_lang = app_state.current_language();

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_section_header("SELECT A LANGUAGE", cx))
        .child(render_section_description(
            "Choose the language you want Discord to display.",
            cx,
        ))
        .child(render_section_description(
            "Este cliente inclui atualmente os dicionários Português (Brasil) e English (US). Os demais idiomas oficiais do Discord aparecem desabilitados até a tradução ser adicionada.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(12.0))
                .children(LANGUAGES.iter().map(|&(code, flag, native_name, pt_name, supported)| {
                    render_language_card(
                        app_entity.clone(),
                        current_lang,
                        code,
                        flag,
                        native_name,
                        pt_name,
                        supported,
                        cx,
                    )
                })),
        )
}

fn render_language_card(
    app_entity: Entity<AppState>,
    current_lang: &str,
    code: &'static str,
    flag: &'static str,
    native_name: &'static str,
    pt_name: &'static str,
    supported: bool,
    cx: &App,
) -> Div {
    let is_selected = supported && current_lang == code;
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
    let text_color = if supported {
        cx.discord().text_titles
    } else {
        cx.discord().text_secondary
    };

    let mut card = div()
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
                                .text_color(text_color)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child(native_name),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .text_size(px(11.0))
                                .child(if supported {
                                    pt_name
                                } else {
                                    "Idioma ainda não disponível neste cliente"
                                }),
                        ),
                ),
        )
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
                .children(is_selected.then(|| {
                    div()
                        .text_color(cx.discord().white)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child("✓")
                })),
        );

    if supported {
        let modal_c = app_entity.clone();
        card = card
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                modal_c.update(cx, |state, cx| {
                    if state.set_language(code) {
                        cx.notify();
                    }
                });
            });
    }

    card
}
