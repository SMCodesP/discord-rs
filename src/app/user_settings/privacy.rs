use gpui::prelude::*;
use gpui::*;

use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_description, render_section_header, render_toggle_row};
use super::UserSettingsModal;

pub fn render_privacy(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let modal_read = modal_state;
    let allow_dm = modal_read.allow_dm_from_members;
    let allow_req = modal_read.allow_friend_requests_from_members;
    let explicit = modal_read.explicit_filter;
    let tel_improve = modal_read.telemetry_improve;
    let tel_person = modal_read.telemetry_personalize;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_section_description(
            "Controle como você interage com outros usuários e gerencie suas permissões de dados.",
            cx,
        ))
        // Section 1: Explicit Content Filter Cards
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("FILTRO DE MENSAGENS COM CONTEÚDO EXPLÍCITO", cx))
                .child(render_safety_card(
                    modal.clone(),
                    0,
                    icon_shield(px(20.0), cx.discord().status_online).into_any_element(),
                    "Proteja-me",
                    "Analisa e filtra automaticamente mensagens diretas de todos os usuários.",
                    explicit == 0,
                    cx,
                ))
                .child(render_safety_card(
                    modal.clone(),
                    1,
                    icon_friends(px(20.0), cx.discord().blurple).into_any_element(),
                    "Meus amigos são legais",
                    "Analisa mensagens diretas de todos, exceto de seus amigos adicionados.",
                    explicit == 1,
                    cx,
                ))
                .child(render_safety_card(
                    modal.clone(),
                    2,
                    icon_warning(px(20.0), cx.discord().status_idle).into_any_element(),
                    "Não filtrar nada",
                    "Você vive perigosamente. Nenhuma mensagem direta será analisada.",
                    explicit == 2,
                    cx,
                )),
        )
        .child(render_divider(cx))
        // Section 2: Server Privacy Defaults
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("PADRÕES DE PRIVACIDADE DO SERVIDOR", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-allow-dm",
                        "Permitir mensagens diretas de membros do servidor",
                        "Essa configuração é aplicada quando você entra em um novo servidor. Não afeta servidores existentes.",
                        allow_dm,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.allow_dm_from_members = !this.allow_dm_from_members;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-allow-req",
                        "Permitir solicitações de amizade de membros do servidor",
                        "Membros que compartilham servidores com você poderão enviar solicitações de amizade.",
                        allow_req,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.allow_friend_requests_from_members = !this.allow_friend_requests_from_members;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        .child(render_divider(cx))
        // Section 3: Data & Telemetry
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("COMO USAMOS SEUS DADOS", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-tel-improve",
                        "Usar dados para melhorar o Discord RS",
                        "Coleta métricas anônimas de taxa de quadros e latência para otimizar o motor GPUI nativo.",
                        tel_improve,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.telemetry_improve = !this.telemetry_improve;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-tel-person",
                        "Usar dados para personalizar minha experiência",
                        "Permite sugerir comunidades, amigos em comum e integrações com base em seu uso.",
                        tel_person,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.telemetry_personalize = !this.telemetry_personalize;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
}

fn render_safety_card(
    modal: Entity<UserSettingsModal>,
    filter_index: u8,
    icon: AnyElement,
    title: &'static str,
    description: &'static str,
    is_selected: bool,
    cx: &App,
) -> Div {
    let hover_bg = cx.discord().channel_hover;
    let card_bg = cx.discord().bg_channels;
    let border_color = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().neutral_67
    };

    div()
        .p(px(14.0))
        .rounded(px(8.0))
        .bg(card_bg)
        .border_2()
        .border_color(border_color)
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.0))
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            modal.update(cx, |this, cx| {
                this.explicit_filter = filter_index;
                cx.notify();
            });
        })
        .child(
            div()
                .size(px(36.0))
                .rounded(px(6.0))
                .bg(cx.discord().bg_server)
                .flex()
                .items_center()
                .justify_center()
                .child(icon),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.0))
                        .child(title),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child(description),
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
                    cx.discord().transparent
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
