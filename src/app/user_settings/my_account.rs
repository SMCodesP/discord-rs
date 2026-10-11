use gpui::prelude::*;
use gpui::*;

use crate::app::state::{AppState, CurrentUserProfile, SettingsTab};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_info_row;

/// Renders the "Minha Conta" profile card and account details.
/// Purges the hardcoded fake email ("vanguard@discord-rs.native") in favor of
/// an honest empty state matching real Discord behavior.
pub fn render_my_account(
    app_entity: Entity<AppState>,
    user: &CurrentUserProfile,
    cx: &App,
) -> Div {
    let edit_profile_entity = app_entity.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        // Profile Card Container
        .child(
            div()
                .rounded(px(8.0))
                .bg(cx.discord().bg_server)
                .overflow_hidden()
                // Banner
                .child(
                    div()
                        .h(px(100.0))
                        .w_full()
                        .bg(cx.discord().blurple),
                )
                // Profile Body
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
                                // Avatar (80px) using canonical Avatar component with decoration support
                                .child(
                                    Avatar::new(user.avatar_text.clone())
                                        .size(AvatarSize::Px80)
                                        .opt_src(user.avatar_url.clone())
                                        .opt_decoration(user.avatar_decoration_data.clone())
                                        .status(user.presence.into())
                                        .cutout_color(cx.discord().bg_server)
                                )
                                // Edit Profile Button
                                .child(
                                    Button::new("btn-edit-user-profile", "Editar perfil de usuário")
                                        .variant(ButtonVariant::Blurple)
                                        .size(ButtonSize::Sm)
                                        .on_click(move |_ev, _win, cx| {
                                            edit_profile_entity.update(cx, |state, cx| {
                                                state.set_settings_tab(SettingsTab::Profiles);
                                                cx.notify();
                                            });
                                        })
                                ),
                        )
                        // Name and discriminator
                        .child(
                            div()
                                .pt(px(12.0))
                                .child(
                                    div()
                                        .text_color(cx.discord().text_titles)
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(18.0))
                                        .child(user.display_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(cx.discord().text_secondary)
                                        .text_size(px(13.0))
                                        .child(format!("@{}", user.username)),
                                ),
                        )
                        // Account details rows in dark surface
                        .child(
                            div()
                                .mt(px(16.0))
                                .p(px(16.0))
                                .rounded(px(8.0))
                                .bg(cx.discord().bg_channels)
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .child(render_info_row(
                                    "NOME DE EXIBIÇÃO",
                                    &user.display_name,
                                    "Editar",
                                    "edit-display-name",
                                    |_ev, _win, _cx| {},
                                    cx,
                                ))
                                .child(render_info_row(
                                    "NOME DE USUÁRIO",
                                    &user.username,
                                    "Editar",
                                    "edit-username",
                                    |_ev, _win, _cx| {},
                                    cx,
                                ))
                                // Honest empty state: no hardcoded email mock
                                .child(render_info_row(
                                    "E-MAIL",
                                    "Você ainda não adicionou um e-mail.",
                                    "Adicionar",
                                    "add-email",
                                    |_ev, _win, _cx| {},
                                    cx,
                                ))
                                // Honest empty state for phone number
                                .child(render_info_row(
                                    "NÚMERO DE TELEFONE",
                                    "Você ainda não adicionou um telefone.",
                                    "Adicionar",
                                    "add-phone",
                                    |_ev, _win, _cx| {},
                                    cx,
                                )),
                        ),
                ),
        )
}
