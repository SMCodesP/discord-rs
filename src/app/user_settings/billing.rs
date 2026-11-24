use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::domain::settings::NitroTier;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};

/// Nitro settings tab matching Discord reference settings-subscriptions.png & settings-nitro.png
/// Uses only authoritative subscription data and never invents prices or plans.
pub fn render_nitro(_app_entity: Entity<AppState>, app_state: &AppState, cx: &App) -> Div {
    let current_tier = app_state.nitro_tier;
    let sub = app_state.active_subscription.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        // -------------------------------------------------------------
        // Official Gradient Nitro Banner
        // -------------------------------------------------------------
        .child(
            div()
                .relative()
                .w_full()
                .h(px(140.0))
                .rounded(px(10.0))
                .overflow_hidden()
                .bg(cx.discord().premium_tier_2_purple)
                // Content overlay
                .child(
                    div()
                        .size_full()
                        .p(px(20.0))
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_start()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded_full()
                                        .bg(cx.discord().bg_server)
                                        .text_color(cx.discord().white)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(10.0))
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.0))
                                        .child(icon_sparkles(px(12.0), cx.discord().white))
                                        .child("DISCORD NITRO"),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().white)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(22.0))
                                        .child("Discord Nitro"),
                                ),
                        )
                        .child(
                            div()
                                .size(px(56.0))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(cx.discord().channel_active)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_sparkles(px(32.0), cx.discord().white)),
                        ),
                ),
        )
        // -------------------------------------------------------------
        // Subscriptions Section (settings-subscriptions.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("ASSINATURAS", cx))
                .child(render_section_description(
                    "Estas são suas assinaturas atuais. Elas são cobradas no mesmo ciclo de faturamento.",
                    cx,
                ))
                .child(if let Some(ref s) = sub {
                    div()
                        .p(px(16.0))
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().neutral_67)
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
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
                                                .text_color(cx.discord().text_titles)
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(15.0))
                                                .child(s.plan_name.clone()),
                                        )
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(if s.status == "active" {
                                                    cx.discord().status_online
                                                } else {
                                                    cx.discord().text_muted
                                                })
                                                .text_color(cx.discord().white)
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(10.0))
                                                .child(s.status.to_uppercase()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_secondary)
                                        .text_size(px(12.0))
                                        .child(format!(
                                            "Renova em {} via {}",
                                            s.current_period_end.format("%d/%m/%Y"),
                                            s.payment_gateway
                                        )),
                                ),
                        )
                        .child(
                            Button::new("btn-cancel-nitro", "Cancelar")
                                .variant(ButtonVariant::Danger)
                                .size(ButtonSize::Sm)
                                .disabled(true),
                        )
                } else if current_tier != NitroTier::None {
                    div()
                        .p(px(16.0))
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().neutral_67)
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
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
                                                .text_color(cx.discord().text_titles)
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(15.0))
                                                .child(current_tier.label()),
                                        )
                                        .child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(cx.discord().status_online)
                                                .text_color(cx.discord().white)
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(10.0))
                                                .child("ATIVO"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_secondary)
                                        .text_size(px(12.0))
                                        .child("Assinatura ativa vinculada à sua conta do Discord."),
                                ),
                        )
                        .child(
                            Button::new("btn-cancel-nitro", "Cancelar")
                                .variant(ButtonVariant::Danger)
                                .size(ButtonSize::Sm)
                                .disabled(true),
                        )
                } else {
                    div()
                        .p(px(24.0))
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().neutral_67)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(13.0))
                        .child("Nenhuma assinatura ativa.")
                }),
        )
}

/// Server Boost tab matching Discord reference settings-server-boost.png
/// Strictly zero mocks: real user boosts or honest empty state.
pub fn render_server_boost(_app_entity: Entity<AppState>, app_state: &AppState, cx: &App) -> Div {
    let boosts = &app_state.server_boosts;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(icon_sparkles(px(20.0), cx.discord().role_purple))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(18.0))
                        .child("Impulso de Servidor"),
                ),
        )
        .child(render_section_description(
            "Impulsionar um servidor desbloqueia benefícios para todos. Gerencie seus Impulsos aqui.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("IMPULSOS ATIVOS", cx))
                .child(render_section_description(
                    "Estes são seus Impulsos comprados.",
                    cx,
                ))
                .children(if boosts.is_empty() {
                    vec![
                        div()
                            .p(px(24.0))
                            .rounded(px(8.0))
                            .bg(cx.discord().bg_channels)
                            .border_1()
                            .border_color(cx.discord().neutral_67)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(cx.discord().text_secondary)
                            .text_size(px(13.0))
                            .child("Nenhum Impulso de Servidor ativo.")
                    ]
                } else {
                    boosts.iter().map(|b| {
                        let boost_icon: AnyElement = if let Some(url) = &b.guild_icon {
                            img(url.clone())
                                .size(px(32.0))
                                .rounded(px(6.0))
                                .into_any_element()
                        } else {
                            div()
                                .size(px(32.0))
                                .rounded(px(6.0))
                                .bg(cx.discord().bg_channels)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_sparkles(px(16.0), cx.discord().role_purple))
                                .into_any_element()
                        };
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
                                    .child(boost_icon)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_color(cx.discord().text_titles)
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(14.0))
                                                    .child(b.guild_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(cx.discord().text_secondary)
                                                    .text_size(px(11.0))
                                                    .child(format!(
                                                        "Recarga disponível em {}",
                                                        b.cooldown_ends.format("%d/%m/%Y")
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .rounded(px(4.0))
                                    .bg(cx.discord().role_purple)
                                    .text_color(cx.discord().white)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(10.0))
                                    .child("ATIVO"),
                            )
                    }).collect::<Vec<_>>()
                }),
        )
}
