use gpui::prelude::*;
use gpui::*;

use crate::app::state::{AppState, CurrentUserProfile, SettingsTab};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_info_row;

/// Renders the "Minha Conta" profile card using only current account data.
/// Email and phone are absent from `CurrentUserProfile`, so the UI says they are
/// unavailable instead of inventing values or claiming the user never added them.
pub fn render_my_account(app_entity: Entity<AppState>, user: &CurrentUserProfile, cx: &App) -> Div {
    let edit_profile_entity = app_entity.clone();
    let display_name = if user.display_name.is_empty() {
        "Nome de exibição não disponível".to_string()
    } else {
        user.display_name.clone()
    };
    let username = if user.username.is_empty() {
        "Nome de usuário não disponível".to_string()
    } else {
        format!("@{}", user.username)
    };
    let display_name_value = display_name.clone();
    let username_value = username.clone();

    div().flex().flex_col().gap(px(16.0)).child(
        div()
            .rounded(px(8.0))
            .bg(cx.discord().bg_server)
            .overflow_hidden()
            .child(div().h(px(100.0)).w_full().bg(cx.discord().bg_channels))
            .child(
                div()
                    .px(px(16.0))
                    .pb(px(16.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_end()
                            .justify_between()
                            .mt(px(-40.0))
                            .child(
                                Avatar::new(user.avatar_text.clone())
                                    .size(AvatarSize::Px80)
                                    .opt_src(user.avatar_url.clone())
                                    .opt_decoration(user.avatar_decoration_data.clone())
                                    .status(user.presence.into())
                                    .cutout_color(cx.discord().bg_server),
                            )
                            .child(
                                Button::new("btn-edit-user-profile", "Editar perfil de usuário")
                                    .variant(ButtonVariant::Blurple)
                                    .size(ButtonSize::Sm)
                                    .on_click(move |_event, _window, cx| {
                                        edit_profile_entity.update(cx, |state, cx| {
                                            state.set_settings_tab(SettingsTab::Profiles);
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .pt(px(12.0))
                            .child(
                                div()
                                    .text_color(cx.discord().text_titles)
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(18.0))
                                    .child(display_name),
                            )
                            .child(
                                div()
                                    .text_color(cx.discord().text_secondary)
                                    .text_size(px(13.0))
                                    .child(username),
                            )
                            .children(
                                user.discriminator
                                    .as_ref()
                                    .filter(|value| value.as_str() != "0")
                                    .map(|discriminator| {
                                        div()
                                            .text_color(cx.discord().text_secondary)
                                            .text_size(px(12.0))
                                            .child(format!("#{discriminator}"))
                                    }),
                            ),
                    )
                    .children(user.custom_status.as_ref().map(|status| {
                        div()
                            .pt(px(8.0))
                            .text_color(cx.discord().text_primary)
                            .text_size(px(13.0))
                            .child(status.clone())
                    }))
                    .child(
                        div()
                            .mt(px(16.0))
                            .p(px(16.0))
                            .rounded(px(8.0))
                            .bg(cx.discord().bg_channels)
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(render_info_row("NOME EXIBIDO", &display_name_value, cx))
                            .child(render_info_row("NOME DE USUÁRIO", &username_value, cx))
                            .child(render_info_row(
                                "E-MAIL",
                                "E-mail não sincronizado neste cliente.",
                                cx,
                            ))
                            .child(render_info_row(
                                "NÚMERO DE TELEFONE",
                                "Telefone não sincronizado neste cliente.",
                                cx,
                            )),
                    ),
            ),
    )
}
