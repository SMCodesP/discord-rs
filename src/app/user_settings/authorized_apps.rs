use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};

pub fn render_authorized_apps(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let authorized_apps = &app_state.authorized_apps;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_section_description(
            "Estes aplicativos de terceiros têm acesso à sua conta através de integrações OAuth2. Você pode revogar o acesso a qualquer momento.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(render_section_header("APLICATIVOS AUTORIZADOS", cx))
                .children(if authorized_apps.is_empty() {
                    vec![
                        div()
                            .p(px(24.0))
                            .rounded(px(8.0))
                            .bg(cx.discord().bg_channels)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(cx.discord().text_secondary)
                            .text_size(px(13.0))
                            .child("Nenhum aplicativo autorizado no momento.")
                    ]
                } else {
                    authorized_apps.iter().map(|app| {
                        let app_id = app.id.clone();
                        let entity = app_entity.clone();
                        let initial = app.name.chars().next().unwrap_or('A').to_string();

                        div()
                            .p(px(16.0))
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
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .size(px(40.0))
                                                    .rounded(px(8.0))
                                                    .bg(cx.discord().blurple)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(cx.discord().white)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(18.0))
                                                    .child(initial),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_color(cx.discord().text_titles)
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(15.0))
                                                            .child(app.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(cx.discord().text_secondary)
                                                            .text_size(px(12.0))
                                                            .child("Autorizado via OAuth2 • Conexão Segura"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        Button::new(
                                            ElementId::NamedInteger("btn-revoke-app".into(), app_id.len() as u64),
                                            "Desautorizar",
                                        )
                                        .variant(ButtonVariant::Danger)
                                        .size(ButtonSize::Sm)
                                        .on_click(move |_ev, _win, cx| {
                                            let id = app_id.clone();
                                            entity.update(cx, |s, cx| {
                                                s.revoke_authorized_app(&id);
                                                cx.notify();
                                            });
                                        })
                                    ),
                            )
                            // Permissions / Scopes Chips
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap(px(6.0))
                                    .children(app.scopes.iter().map(|scope| {
                                        div()
                                            .px(px(8.0))
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(cx.discord().bg_server)
                                            .text_color(cx.discord().text_secondary)
                                            .text_size(px(11.0))
                                            .child(scope.clone())
                                    })),
                            )
                    }).collect()
                }),
        )
}
