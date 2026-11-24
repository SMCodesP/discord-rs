use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_toggle_row};
use super::settings::{LocalNotificationField, SettingsField};
use super::{UserSettingsModal, settings_text};

pub fn render_notifications(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_account_notifications(modal.clone(), modal_state, cx))
        .child(render_divider(cx))
        .child(render_local_notifications(modal, modal_state, cx))
}

fn render_account_notifications(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("NOTIFICAÇÕES DA CONTA", cx))
        .child(render_account_toggle_row(
            modal.clone(),
            state,
            "Pessoas que conheço começam a transmitir em pequenos servidores",
            SettingsField::StreamNotifications,
            cx,
        ))
        .child(render_account_toggle_row(
            modal.clone(),
            state,
            "Um amigo e eu chegamos a um aniversário de amizade",
            SettingsField::FriendAnniversaryNotifications,
            cx,
        ))
        .child(render_account_toggle_row(
            modal.clone(),
            state,
            "Amigos ficam online",
            SettingsField::FriendOnlineNotifications,
            cx,
        ))
        .child(render_account_toggle_row(
            modal.clone(),
            state,
            "Um servidor tem um evento futuro",
            SettingsField::UpcomingServerEventNotifications,
            cx,
        ))
        .child(render_account_toggle_row(
            modal,
            state,
            "Amigos atualizam o perfil deles",
            SettingsField::ProfileUpdatesNotifications,
            cx,
        ))
}

fn render_account_toggle_row(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    label: &'static str,
    field: SettingsField,
    cx: &App,
) -> Div {
    let value = state.account_settings.value(field);
    let is_pending = state
        .pending_setting
        .as_ref()
        .is_some_and(|pending| pending.field == field);

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py(px(10.0))
        .child(
            div()
                .flex_1()
                .pr(px(16.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.0))
                        .child(label),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child(if value.is_none() {
                            "Sincronize sua conta para alterar esta opção."
                        } else if is_pending {
                            "Aplicando alteração..."
                        } else {
                            ""
                        }),
                ),
        )
        .child(
            gpui::component::switch::Switch::new(field.wire_field())
                .checked(value.unwrap_or(false))
                .disabled(value.is_none() || is_pending)
                .on_click(move |checked, _window, cx| {
                    modal.update(cx, |this, cx| this.request_setting(field, *checked, cx));
                }),
        )
}

fn render_local_notifications(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("CONTROLES LOCAIS DO DISCORD RS", cx))
        .child(
            div()
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(settings_text(
                    "Preferências desta sessão; não alteram as notificações sincronizadas da conta.",
                    "Session preferences; these do not change synchronized account notifications.",
                )),
        )
        .child(render_local_toggle_row(
            "toggle-local-desktop-notifications",
            "Ativar notificações na área de trabalho",
            LocalNotificationField::Desktop,
            modal.clone(),
            state,
            cx,
        ))
        .child(render_local_toggle_row(
            "toggle-local-unread-badge",
            "Exibir badge de não lidas",
            LocalNotificationField::UnreadBadge,
            modal.clone(),
            state,
            cx,
        ))
        .child(render_local_toggle_row(
            "toggle-local-flash-taskbar",
            "Piscar na barra de tarefas",
            LocalNotificationField::FlashTaskbar,
            modal.clone(),
            state,
            cx,
        ))
        .child(render_local_toggle_row(
            "toggle-local-sound-message",
            "Som de nova mensagem",
            LocalNotificationField::MessageSound,
            modal.clone(),
            state,
            cx,
        ))
        .child(render_local_toggle_row(
            "toggle-local-sound-voice-join",
            "Som de entrada na chamada",
            LocalNotificationField::VoiceJoinSound,
            modal.clone(),
            state,
            cx,
        ))
        .child(render_local_toggle_row(
            "toggle-local-sound-mute",
            "Som ao ativar ou desativar o microfone",
            LocalNotificationField::MuteSound,
            modal.clone(),
            state,
            cx,
        ))
        .child(render_local_toggle_row(
            "toggle-local-sound-deafen",
            "Som ao ativar ou desativar o áudio",
            LocalNotificationField::DeafenSound,
            modal,
            state,
            cx,
        ))
}

fn render_local_toggle_row(
    modal: Entity<UserSettingsModal>,
    id: &'static str,
    label: &'static str,
    field: LocalNotificationField,
    state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let value = state.local_notifications.value(field).unwrap_or(false);
    render_toggle_row(
        id,
        label,
        "Preferência local do Discord RS.",
        value,
        move |checked, _window, cx| {
            modal.update(cx, |this, cx| {
                this.local_notifications.set_value(field, *checked);
                cx.notify();
            });
        },
        cx,
    )
}
