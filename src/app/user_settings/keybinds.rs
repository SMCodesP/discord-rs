use gpui::prelude::*;
use gpui::*;

use crate::tokens::icons::icon_info;
use crate::tokens::kit_theme::DiscordTheme;

use super::UserSettingsModal;
use super::components::render_section_description;

pub fn render_keybinds(
    _modal: Entity<UserSettingsModal>,
    _modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child("Atalhos do teclado personalizados"),
                )
                .child(render_section_description(
                    "Atalhos personalizados ainda não estão disponíveis neste cliente. Os atalhos abaixo são os padrões realmente implementados.",
                    cx,
                ))
                .child(
                    div()
                        .w_full()
                        .p(px(12.0))
                        .rounded(px(6.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().borders)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon_info(px(18.0), cx.discord().blurple))
                        .child(
                            div()
                                .text_color(cx.discord().text_primary)
                                .text_size(px(13.0))
                                .child("Nenhum atalho anunciado aqui depende de rebind ou de uma ação inexistente."),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Atalhos padrão"),
                )
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .child(render_keybind_row("Abrir ou fechar a busca", &["CTRL / CMD", "F"], cx))
                        .child(render_keybind_row(
                            "Fechar menu, modal ou cancelar ação",
                            &["ESC"],
                            cx,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Mensagens"),
                )
                .child(render_section_description(
                    "Este atalho funciona quando o compositor está vazio e não há seleção de mensagem.",
                    cx,
                ))
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .child(render_keybind_row(
                            "Editar a última mensagem própria",
                            &["↑"],
                            cx,
                        )),
                ),
        )
}

fn render_keybind_row(action: &'static str, keys: &[&'static str], cx: &App) -> Div {
    let hover_bg = cx.discord().channel_hover;

    div()
        .h(px(44.0))
        .px(px(16.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .hover(move |style| style.bg(hover_bg))
        .child(
            div()
                .text_color(cx.discord().text_titles)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0))
                .child(action),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(keys.iter().map(|key| {
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(cx.discord().bg_server)
                        .border_1()
                        .border_color(cx.discord().borders)
                        .text_color(cx.discord().text_primary)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child(*key)
                })),
        )
}
