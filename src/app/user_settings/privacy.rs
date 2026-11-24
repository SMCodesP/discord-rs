use gpui::component::switch::Switch;
use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::UserSettingsModal;
use super::components::{render_divider, render_section_header};
use super::settings::SettingsField;

pub fn render_privacy(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("COMO O DISCORD USA MEUS DADOS", cx))
                .child(render_account_toggle_row(
                    modal.clone(),
                    modal_state,
                    "Utilizar dados para melhorar o Discord",
                    SettingsField::UsageStatisticsConsent,
                    cx,
                ))
                .child(render_account_toggle_row(
                    modal.clone(),
                    modal_state,
                    "Usar meus dados para personalizar minha experiência e conteúdo patrocinado",
                    SettingsField::PersonalizationConsent,
                    cx,
                ))
                .child(render_account_toggle_row(
                    modal,
                    "Permitir que minha voz seja gravada nos clipes",
                    SettingsField::ClipsVoiceRecording,
                    cx,
                )),
        )
        .child(render_divider(cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("PRIVACIDADE E SEGURANÇA", cx))
                .child(render_account_toggle_row(
                    modal.clone(),
                    modal_state,
                    "Restringir mensagens diretas de membros de novos servidores",
                    SettingsField::DefaultGuildsRestricted,
                    cx,
                ))
                .child(render_account_toggle_row(
                    modal,
                    "Permitir pedidos de amizade de membros do servidor",
                    SettingsField::FriendRequestsFromGuildMembers,
                    cx,
                )),
        )
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
            Switch::new(field.wire_field())
                .checked(value.unwrap_or(false))
                .disabled(value.is_none() || is_pending)
                .on_click(move |checked, _window, cx| {
                    modal.update(cx, |this, cx| this.request_setting(field, *checked, cx));
                }),
        )
}
