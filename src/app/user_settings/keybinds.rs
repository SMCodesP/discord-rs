use gpui::prelude::*;
use gpui::*;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};
use super::UserSettingsModal;

pub fn render_keybinds(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let is_recording = modal_state.is_recording_keybind;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_section_description(
            "Atalhos rápidos para navegação veloz, controle de chamadas de voz e gerenciamento de microfone.",
            cx,
        ))
        // Top Bar: Header + Record Button
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(render_section_header("ATALHOS PADRÃO DO DISCORD RS", cx))
                .child({
                    let modal_c = modal.clone();
                    Button::new(
                        "btn-rec-keybind",
                        if is_recording { "Cancelar Gravação" } else { "Gravar Novo Atalho" },
                    )
                    .variant(if is_recording { ButtonVariant::Danger } else { ButtonVariant::Blurple })
                    .size(ButtonSize::Sm)
                    .on_click(move |_ev, _win, cx| {
                        modal_c.update(cx, |this, cx| {
                            this.is_recording_keybind = !this.is_recording_keybind;
                            cx.notify();
                        });
                    })
                }),
        )
        // Optional Recording Glow Banner
        .children(if is_recording {
            Some(
                div()
                    .p(px(12.0))
                    .rounded(px(6.0))
                    .bg(cx.discord().bg_server)
                    .border_2()
                    .border_color(cx.discord().blurple)
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .bg(cx.discord().status_dnd),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().blurple)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(13.0))
                                    .child("Modo de Gravação Ativo: Pressione qualquer combinação de teclas..."),
                            ),
                    )
                    .child(
                        div()
                            .text_color(cx.discord().text_secondary)
                            .text_size(px(12.0))
                            .child("Exemplo: Ctrl + Shift + P ou Alt + F. Clique em 'Cancelar Gravação' para fechar."),
                    ),
            )
        } else {
            None
        })
        // Styled Keybinds Table
        .child(
            div()
                .rounded(px(8.0))
                .bg(cx.discord().bg_server)
                .border_1()
                .border_color(cx.discord().neutral_67)
                .overflow_hidden()
                .flex()
                .flex_col()
                // Table Header
                .child(
                    div()
                        .h(px(36.0))
                        .px(px(16.0))
                        .bg(cx.discord().bg_channels)
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .text_color(cx.discord().text_secondary)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child("AÇÃO")
                        .child("ATALHO DE TECLADO")
                        .child("CATEGORIA"),
                )
                // Rows
                .child(render_keybind_row("Busca Rápida de Canais e DMs", &["Ctrl", "K"], "Navegação", cx))
                .child(render_keybind_row("Navegar para Canal Acima", &["Alt", "Up"], "Navegação", cx))
                .child(render_keybind_row("Navegar para Canal Abaixo", &["Alt", "Down"], "Navegação", cx))
                .child(render_keybind_row("Alternar Mudo do Microfone", &["Ctrl", "Shift", "M"], "Voz", cx))
                .child(render_keybind_row("Alternar Ensurdecer", &["Ctrl", "Shift", "D"], "Voz", cx))
                .child(render_keybind_row("Push-To-Talk (PTT)", &["Mouse 4", "Caps Lock"], "Voz / Global", cx))
                .child(render_keybind_row("Alternar Painel de Configurações", &["Ctrl", ","], "Sistema", cx))
                .child(render_keybind_row("Marcar Canal como Lido", &["Esc"], "Chat", cx))
                .child(render_keybind_row("Pular para a Mensagem Mais Recente", &["Shift", "PageDown"], "Chat", cx)),
        )
}

fn render_keybind_row(
    action: &'static str,
    keys: &[&'static str],
    category: &'static str,
    cx: &App,
) -> Div {
    let hover_bg = cx.discord().bg_channels;

    div()
        .h(px(44.0))
        .px(px(16.0))
        .border_t_1()
        .border_color(cx.discord().neutral_67)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .hover(move |s| s.bg(hover_bg))
        // Action Label
        .child(
            div()
                .w(px(240.0))
                .text_color(cx.discord().text_titles)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(13.0))
                .child(action),
        )
        // Key Pills
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(keys.iter().enumerate().map(|(idx, key)| {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .children(if idx > 0 {
                            Some(
                                div()
                                    .text_color(cx.discord().text_secondary)
                                    .text_size(px(11.0))
                                    .child("+"),
                            )
                        } else {
                            None
                        })
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(3.0))
                                .rounded(px(4.0))
                                .bg(cx.discord().bg_channels)
                                .border_1()
                                .border_color(cx.discord().borders)
                                .text_color(cx.discord().text_primary)
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(11.0))
                                .child(*key),
                        )
                })),
        )
        // Category Label
        .child(
            div()
                .w(px(100.0))
                .flex()
                .justify_end()
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(category),
        )
}
