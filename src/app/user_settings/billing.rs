use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::domain::settings::NitroTier;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};

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
                                .gap(px(6.0))
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
                                        .child("Desbloqueie uma experiência sem limites"),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_titles)
                                        .text_size(px(13.0))
                                        .child("Turbine seus uploads, transmita em 4K, expresse-se com emojis globais e impulsione servidores."),
                                ),
                        )
                        .child(
                            div()
                                .size(px(56.0))
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
        // Subscription Management Card (Active Status or Promo)
        // -------------------------------------------------------------
        .child(
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
                                .child(render_section_header("STATUS DA ASSINATURA", cx))
                                .children(if current_tier != NitroTier::None {
                                    Some(
                                        div()
                                            .px(px(6.0))
                                            .py(px(1.0))
                                            .rounded(px(4.0))
                                            .bg(cx.discord().status_online)
                                            .text_color(cx.discord().white)
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(10.0))
                                            .child("ATIVA"),
                                    )
                                } else {
                                    None
                                }),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(15.0))
                                .child(if let Some(ref s) = sub {
                                    format!("Plano Atual: {}", s.plan_name)
                                } else if current_tier != NitroTier::None {
                                    format!("Plano Atual: {}", current_tier.label())
                                } else {
                                    "Você não possui nenhuma assinatura ativa do Nitro no momento.".to_string()
                                }),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .text_size(px(12.0))
                                .child(if let Some(ref s) = sub {
                                    format!(
                                        "Renovação em {} via {} • Cancele a qualquer momento sem taxas adicionais.",
                                        s.current_period_end.format("%d/%m/%Y"),
                                        s.payment_gateway
                                    )
                                } else if current_tier != NitroTier::None {
                                    "Assinatura ativa vinculada à sua conta Discord.".to_string()
                                } else {
                                    "Escolha um dos planos abaixo para desbloquear todas as vantagens especiais.".to_string()
                                }),
                        ),
                )
                .children(if current_tier != NitroTier::None {
                    let entity = app_entity.clone();
                    Some(
                        Button::new("btn-cancel-nitro", "Cancelar Assinatura")
                            .variant(ButtonVariant::Danger)
                            .size(ButtonSize::Sm)
                            .on_click(move |_ev, _win, cx| {
                                entity.update(cx, |s, cx| {
                                    s.cancel_nitro_subscription();
                                    cx.notify();
                                });
                            })
                    )
                } else {
                    None
                }),
        )
        // -------------------------------------------------------------
        // Nitro Plan Cards: Nitro Basic (R$ 8,99) & Nitro (R$ 24,99)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(16.0))
                .child(render_nitro_plan_card(
                    app_entity.clone(),
                    NitroTier::Basic,
                    current_tier,
                    "Nitro Basic",
                    "R$ 8,99",
                    "Recursos essenciais para turbinar seu chat",
                    false,
                    &[
                        "Uploads de até 50 MB",
                        "Emojis e figurinhas personalizados em qualquer lugar",
                        "Emblema especial Nitro no seu perfil",
                        "Transmissão em HD até 1080p 60 FPS",
                        "Reações personalizadas em mensagens",
                    ],
                    cx,
                ))
                .child(render_nitro_plan_card(
                    app_entity.clone(),
                    NitroTier::Full,
                    current_tier,
                    "Discord Nitro",
                    "R$ 24,99",
                    "A experiência definitiva com todas as vantagens liberadas",
                    true,
                    &[
                        "Uploads gigantescos de até 500 MB",
                        "Streaming em 4K 60 FPS Ultra HD",
                        "2 Impulsos de Servidor inclusos + 30% off em boosts",
                        "Emojis customizados em qualquer servidor & figurinhas globais",
                        "Temas especiais, perfis customizados e cores com gradiente",
                        "Emblema Nitro animado e exclusivo no perfil",
                        "Mensagens longas de até 4.000 caracteres",
                        "Sons personalizados em qualquer Soundboard",
                    ],
                    cx,
                )),
        )
        // -------------------------------------------------------------
        // Detailed Comparison Benefits Table
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("COMPARAÇÃO DE BENEFÍCIOS NITRO", cx))
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().neutral_67)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .h(px(38.0))
                                .px(px(16.0))
                                .bg(cx.discord().bg_server)
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .text_color(cx.discord().text_secondary)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(11.0))
                                .child("RECURSO")
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(40.0))
                                        .child(div().w(px(80.0)).child("PADRÃO"))
                                        .child(div().w(px(80.0)).child("BASIC"))
                                        .child(div().w(px(80.0)).child("NITRO")),
                                ),
                        )
                        .child(render_nitro_compare_row("Limite de Upload", "25 MB", "50 MB", "500 MB", cx))
                        .child(render_nitro_compare_row("Transmissão ao Vivo", "720p 30 FPS", "1080p 60 FPS", "4K 60 FPS", cx))
                        .child(render_nitro_compare_row("Emojis Globais", "Não", "Sim", "Sim", cx))
                        .child(render_nitro_compare_row("Impulsos de Servidor", "0", "0", "2 inclusos", cx))
                        .child(render_nitro_compare_row("Tamanho das Mensagens", "2.000 carac.", "2.000 carac.", "4.000 carac.", cx)),
                ),
        )
}

fn render_nitro_plan_card(
    app_entity: Entity<AppState>,
    tier: NitroTier,
    current_tier: NitroTier,
    title: &'static str,
    price: &'static str,
    description: &'static str,
    is_featured: bool,
    features: &[&'static str],
    cx: &App,
) -> Div {
    let is_current = current_tier == tier;
    let entity = app_entity.clone();

    div()
        .flex_1()
        .p(px(18.0))
        .rounded(px(8.0))
        .bg(cx.discord().bg_channels)
        .border_2()
        .border_color(if is_featured {
            cx.discord().premium_tier_2_purple
        } else {
            cx.discord().neutral_67
        })
        .flex()
        .flex_col()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_sparkles(px(18.0), cx.discord().blurple))
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(16.0))
                                .child(title),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(22.0))
                                .child(price),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .text_size(px(12.0))
                                .child("/mês"),
                        ),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child(description),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .pt(px(6.0))
                        .children(features.iter().map(|f| {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon_check(px(12.0), cx.discord().status_online))
                                .child(
                                    div()
                                        .text_color(cx.discord().text_primary)
                                        .text_size(px(12.0))
                                        .child(*f),
                                )
                        })),
                ),
        )
        .child(
            if is_current {
                Button::new(
                    ElementId::NamedInteger("btn-cur-plan".into(), tier as u64),
                    "Plano Atual",
                )
                .variant(ButtonVariant::Gray)
                .size(ButtonSize::Sm)
                .disabled(true)
            } else {
                Button::new(
                    ElementId::NamedInteger("btn-choose-plan".into(), tier as u64),
                    "Assinar",
                )
                .variant(if is_featured { ButtonVariant::Blurple } else { ButtonVariant::Gray })
                .size(ButtonSize::Sm)
                .on_click(move |_ev, _win, cx| {
                    entity.update(cx, |s, cx| {
                        s.set_nitro_tier(tier);
                        cx.notify();
                    });
                })
            }
        )
}

fn render_nitro_compare_row(
    label: &'static str,
    free: &'static str,
    basic: &'static str,
    full: &'static str,
    cx: &App,
) -> Div {
    div()
        .h(px(40.0))
        .px(px(16.0))
        .border_t_1()
        .border_color(cx.discord().neutral_67)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_color(cx.discord().text_titles)
                .text_size(px(13.0))
                .child(label),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(40.0))
                .child(div().w(px(80.0)).text_color(cx.discord().text_secondary).text_size(px(12.0)).child(free))
                .child(div().w(px(80.0)).text_color(cx.discord().text_primary).text_size(px(12.0)).child(basic))
                .child(div().w(px(80.0)).text_color(cx.discord().blurple).font_weight(FontWeight::BOLD).text_size(px(12.0)).child(full)),
        )
}

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
        .child(render_section_description(
            "Impulsione seus servidores favoritos para desbloquear slots extras de emojis, maior qualidade de áudio e banner personalizado.",
            cx,
        ))
        .child(
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
                        .child(render_section_header("SEUS IMPULSOS DE SERVIDOR", cx))
                        .child(
                            div()
                                .text_color(cx.discord().white)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(18.0))
                                .child(format!("{} Impulsos Ativos", boosts.len())),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .text_size(px(12.0))
                                .child(if boosts.is_empty() {
                                    "Você não possui nenhum Impulso de Servidor ativo no momento."
                                } else {
                                    "Impulsos aplicados aos servidores selecionados abaixo."
                                }),
                        ),
                )
                .child(
                    Button::new("btn-boost-guild", "Impulsionar Servidor")
                        .variant(ButtonVariant::Blurple)
                        .size(ButtonSize::Sm)
                        .on_click(|_ev, _win, _cx| {})
                ),
        )
        .children(if boosts.is_empty() {
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
                    .child("Nenhum servidor impulsionado no momento.")
            ]
        } else {
            vec![
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(render_section_header("SERVIDORES IMPULSIONADOS", cx))
                    .children(boosts.iter().map(|b| {
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
                                                        "Impulso ativo • Cooldown até {}",
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
                    }).collect::<Vec<_>>())
            ]
        })
}
