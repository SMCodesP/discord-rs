use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_section_header;

pub fn render_family_center(
    _app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let links = &app_state.family_center_links;

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .children(if links.is_empty() {
            vec![
                div()
                    .w_full()
                    .p(px(16.0))
                    .rounded(px(8.0))
                    .bg(cx.discord().bg_channels)
                    .border_1()
                    .border_color(cx.discord().neutral_67)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .w(px(480.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_color(cx.discord().text_titles)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(15.0))
                                    .child("Set up Family Center"),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().text_secondary)
                                    .text_size(px(13.0))
                                    .child("Stay informed about your teen’s experience on Discord. See their activity, manage key safety settings, and start better conversations together."),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .child(
                                Button::new("btn-setup-family", "Set Up")
                                    .variant(ButtonVariant::Blurple)
                                    .size(ButtonSize::Sm)
                                    .on_click(|_ev, _win, _cx| {})
                            ),
                    ),
            ]
        } else {
            vec![
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(render_section_header("LINKED ACCOUNTS", cx))
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
                                                        "Guardian"
                                                    } else {
                                                        "Supervised Teen Account"
                                                    }),
                                            ),
                                    ),
                            )
                    }).collect::<Vec<_>>())
            ]
        })
}
