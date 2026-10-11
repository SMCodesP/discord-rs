use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_description, render_section_header, render_toggle_row};
use super::UserSettingsModal;

pub fn render_notifications(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let modal_read = modal_state;
    let desk = modal_read.desktop_notifications;
    let badge = modal_read.unread_badge;
    let flash = modal_read.flash_taskbar;
    let scope = modal_read.notification_scope;
    let s_msg = modal_read.sound_message;
    let s_join = modal_read.sound_voice_join;
    let s_mute = modal_read.sound_mute_toggle;
    let s_deaf = modal_read.sound_deafen_toggle;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_section_description(
            "Gerencie notificações na área de trabalho, selos não lidos e alertas sonoros de eventos.",
            cx,
        ))
        // Section 1: Desktop Toggles
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("NOTIFICAÇÕES NA ÁREA DE TRABALHO", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-desk-notif",
                        "Ativar notificações na área de trabalho",
                        "Exibe popups nativos do sistema operacional quando novas mensagens forem recebidas.",
                        desk,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.desktop_notifications = !this.desktop_notifications;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-unread-badge",
                        "Ativar selo não lido na barra de tarefas / dock",
                        "Exibe contador numérico de mensagens não lidas sobre o ícone do aplicativo.",
                        badge,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.unread_badge = !this.unread_badge;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-flash-taskbar",
                        "Piscar barra de tarefas ao receber menções",
                        "Pisca suavemente a janela na barra de tarefas ao receber mensagens direcionadas com @você.",
                        flash,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.flash_taskbar = !this.flash_taskbar;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        .child(render_divider(cx))
        // Section 2: Default Notification Scope Pills
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("CONFIGURAÇÕES PADRÃO DE NOTIFICAÇÃO DA CONTA", cx))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(render_scope_pill(modal.clone(), 0, "Todas as mensagens", scope == 0, cx))
                        .child(render_scope_pill(modal.clone(), 1, "Apenas @menções (Recomendado)", scope == 1, cx))
                        .child(render_scope_pill(modal.clone(), 2, "Nada", scope == 2, cx)),
                ),
        )
        .child(render_divider(cx))
        // Section 3: Event Sounds List
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("SONS DE EVENTOS DO DISCORD", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-snd-msg",
                        "Mensagem enviada",
                        "Reproduz o som característico de confirmação ao enviar mensagens no chat.",
                        s_msg,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_message = !this.sound_message;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-snd-join",
                        "Entrar em canal de voz",
                        "Reproduz sinal sonoro ao conectar com sucesso em um canal de áudio.",
                        s_join,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_voice_join = !this.sound_voice_join;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-snd-mute",
                        "Silenciar microfone",
                        "Alerta sonoro ao ativar ou desativar o mudo via tecla de atalho.",
                        s_mute,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_mute_toggle = !this.sound_mute_toggle;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-snd-deaf",
                        "Desativar áudio (Ensurdecer)",
                        "Confirmação sonora ao ensurdecer a saída geral de voz.",
                        s_deaf,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_deafen_toggle = !this.sound_deafen_toggle;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
}

fn render_scope_pill(
    modal: Entity<UserSettingsModal>,
    scope_idx: u8,
    label: &'static str,
    is_active: bool,
    cx: &App,
) -> Div {
    let bg_col = if is_active {
        cx.discord().blurple
    } else {
        cx.discord().bg_channels
    };
    let text_col = if is_active {
        cx.discord().white
    } else {
        cx.discord().text_secondary
    };

    div()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(4.0))
        .bg(bg_col)
        .text_color(text_col)
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            modal.update(cx, |this, cx| {
                this.notification_scope = scope_idx;
                cx.notify();
            });
        })
        .child(label)
}
