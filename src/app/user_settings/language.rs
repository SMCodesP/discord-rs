use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_description, render_section_header};
use super::{UserSettingsModal, settings_text};

pub fn render_language(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let account_language = modal_state.account_language().map(str::to_string);
    let local_language = modal_state.local_language.clone();
    let languages = [
        ("pt-BR", "Português, Brasil", "Portuguese (Brazil)"),
        ("en-US", "Inglês, EUA", "English (US)"),
    ];

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_section_header("IDIOMA DA CONTA", cx))
        .child(render_section_description(
            "Sincronizado com o Discord. A alteração é aplicada somente após a confirmação da API.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .children(languages.into_iter().map(|(code, native_name, english_name)| {
                    render_language_card(
                        modal.clone(),
                        code,
                        native_name,
                        english_name,
                        account_language.as_deref() == Some(code),
                        false,
                        cx,
                    )
                })),
        )
        .child(
            div()
                .p(px(12.0))
                .rounded(px(6.0))
                .bg(cx.discord().bg_channels)
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(if account_language.is_some() {
                    ""
                } else {
                    "Locale da conta não sincronizado."
                }),
        )
        .child(render_divider(cx))
        .child(render_section_header("IDIOMA LOCAL DO DISCORD RS", cx))
        .child(render_section_description(
            "Preferência exclusiva deste cliente. É aplicada imediatamente e não substitui o locale da conta.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .children(languages.into_iter().map(|(code, native_name, english_name)| {
                    render_language_card(
                        modal.clone(),
                        code,
                        native_name,
                        english_name,
                        local_language.as_deref() == Some(code),
                        true,
                        cx,
                    )
                })),
        )
        .child(
            div()
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(settings_text(
                    "Sem preferência local: o idioma acompanha o locale sincronizado da conta.",
                    "No local preference: the language follows the synchronized account locale.",
                )),
        )
}

fn render_language_card(
    modal: Entity<UserSettingsModal>,
    code: &'static str,
    native_name: &'static str,
    english_name: &'static str,
    is_selected: bool,
    is_local: bool,
    cx: &App,
) -> Div {
    let card_bg = if is_selected {
        cx.discord().bg_channels
    } else {
        cx.discord().bg_server
    };
    let border_color = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().bg_channels
    };
    let hover_bg = cx.discord().channel_hover;

    div()
        .w(px(420.0))
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
        .hover(move |style| style.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            cx.stop_propagation();
            modal.update(cx, |this, cx| {
                if is_local {
                    this.set_local_language(code, cx);
                } else {
                    this.request_account_language(code, cx);
                }
            });
        })
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
                        .child(english_name),
                ),
        )
        .child(
            div()
                .size(px(20.0))
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
}
