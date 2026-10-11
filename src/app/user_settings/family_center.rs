use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};

pub fn render_family_center(
    _app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let links = &app_state.family_center_links;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(px(28.0))
                        .rounded_full()
                        .bg(cx.discord().channel_hover)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_shield(px(18.0), cx.discord().blurple)),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Centro da Família (Family Center)"),
                ),
        )
        .child(render_section_description(
            "O Centro da Família permite que pais e responsáveis acompanhem os hábitos digitais de adolescentes com total transparência e sem invadir a privacidade das mensagens diretas.",
            cx,
        ))
        .children(if links.is_empty() {
            // Honest empty state matching Discord reference settings-family-center.png
            // Purged all fabricated metrics (fake added friends / new servers / calls counters)
            vec![
                div()
                    .p(px(20.0))
                    .rounded(px(8.0))
                    .bg(cx.discord().bg_channels)
                    .border_1()
                    .border_color(cx.discord().neutral_67)
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_color(cx.discord().text_titles)
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(15.0))
                                            .child("Configurar a Central da Família"),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.discord().text_secondary)
                                            .text_size(px(13.0))
                                            .child("Mantenha-se informado sobre a experiência do seu adolescente no Discord. Veja a atividade, gerencie configurações de segurança e inicie conversas melhores juntos."),
                                    ),
                            )
                            .child(
                                Button::new("btn-setup-family", "Configurar")
                                    .variant(ButtonVariant::Blurple)
                                    .size(ButtonSize::Sm)
                                    .on_click(|_ev, _win, _cx| {})
                            ),
                    )
            ]
        } else {
            vec![
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(render_section_header("CONTAS VINCULADAS", cx))
                    .children(links.iter().map(|link| {
                        div()
                            .p(px(12.0))
                            .rounded(px(6.0))
                            .bg(cx.discord().bg_server)
                            .border_1()
                            .border_color(cx.discord().neutral_67)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .size(px(32.0))
                                            .rounded_full()
                                            .bg(cx.discord().bg_channels)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if link.is_guardian {
                                                icon_shield(px(16.0), cx.discord().blurple)
                                            } else {
                                                icon_friends(px(16.0), cx.discord().status_online)
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
                                                    .child(link.username.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(cx.discord().text_secondary)
                                                    .text_size(px(11.0))
                                                    .child(if link.is_guardian {
                                                        "Responsável legal (Guardian)"
                                                    } else {
                                                        "Conta de adolescente supervisionada"
                                                    }),
                                            ),
                                    ),
                            )
                    }).collect::<Vec<_>>())
            ]
        })
}
