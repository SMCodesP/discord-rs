use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::domain::settings::NitroTier;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};

/// Nitro settings tab matching Discord reference settings-subscriptions.png & settings-nitro.png
/// Strictly zero mocks: no fabricated prices (R$ 8,99 / R$ 24,99), no invented comparison tables.
pub fn render_nitro(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
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
                .child(render_section_header("YOUR SUBSCRIPTIONS", cx))
                .child(render_section_description(
                    "These are your current subscriptions. They will be billed on the same billing cycle. You can update any subscription at any time.",
                    cx,
                ))
                .child(if let Some(ref s) = sub {
                    let entity = app_entity.clone();
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
                                                .bg(cx.discord().status_online)
                                                .text_color(cx.discord().white)
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(10.0))
                                                .child("ACTIVE"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_secondary)
                                        .text_size(px(12.0))
                                        .child(format!(
                                            "Renews on {} via {}",
                                            s.current_period_end.format("%b %-d, %Y"),
                                            s.payment_gateway
                                        )),
                                ),
                        )
                        .child(
                            Button::new("btn-cancel-nitro", "Cancel")
                                .variant(ButtonVariant::Danger)
                                .size(ButtonSize::Sm)
                                .on_click(move |_ev, _win, cx| {
                                    entity.update(cx, |st, cx| {
                                        st.cancel_nitro_subscription();
                                        cx.notify();
                                    });
                                }),
                        )
                } else if current_tier != NitroTier::None {
                    let entity = app_entity.clone();
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
                                                .child("ACTIVE"),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_secondary)
                                        .text_size(px(12.0))
                                        .child("Active subscription linked to your Discord account."),
                                ),
                        )
                        .child(
                            Button::new("btn-cancel-nitro", "Cancel")
                                .variant(ButtonVariant::Danger)
                                .size(ButtonSize::Sm)
                                .on_click(move |_ev, _win, cx| {
                                    entity.update(cx, |st, cx| {
                                        st.cancel_nitro_subscription();
                                        cx.notify();
                                    });
                                }),
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
                        .child("No active subscriptions.")
                }),
        )
}

/// Server Boost tab matching Discord reference settings-server-boost.png
/// Strictly zero mocks: real user boosts or honest empty state.
pub fn render_server_boost(
    _app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
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
                        .child("Server Boosts"),
                ),
        )
        .child(render_section_description(
            "Boosting a server unlocks perks for everyone in that server. You can manage your Boosts here (add, subtract, cancel — just beware the 7 day cooldown).",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("ACTIVE BOOSTS", cx))
                .child(render_section_description(
                    "These are your purchased Boosts, powering your favorite servers.",
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
                            .child("No active Server Boosts.")
                    ]
                } else {
                    boosts.iter().map(|b| {
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
                                            .rounded(px(6.0))
                                            .bg(cx.discord().bg_channels)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon_sparkles(px(16.0), cx.discord().role_purple))
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
                                                    .child(b.guild_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(cx.discord().text_secondary)
                                                    .text_size(px(11.0))
                                                    .child(format!(
                                                        "Cooldown until {}",
                                                        b.cooldown_ends.format("%b %-d, %Y")
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
                                    .child("ACTIVE"),
                            )
                    }).collect::<Vec<_>>()
                }),
        )
}
