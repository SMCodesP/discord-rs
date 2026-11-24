use gpui::prelude::*;
use gpui::*;

use crate::app::state::CurrentUserProfile;
use crate::components::TextInput;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::UserSettingsModal;
use super::components::{render_section_description, render_section_header};
use super::settings::SettingsSaveState;

pub fn render_profiles(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    user: &CurrentUserProfile,
    has_nitro: bool,
    cx: &App,
) -> Div {
    let draft = &modal_state.profile_edit.draft;
    let display_name = draft.display_name.clone().unwrap_or_default();
    let pronouns = draft.pronouns.clone().unwrap_or_default();
    let bio = draft.bio.clone().unwrap_or_default();
    let save_modal = modal.clone();
    let discard_modal = modal;
    let is_saving = matches!(
        modal_state.profile_edit.save_state,
        SettingsSaveState::Saving
    );
    let has_changes = !modal_state.profile_edit.dirty.is_empty();

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_section_description(
            "Personalize a forma como você é visto em todos os servidores do Discord.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(28.0))
                .child(
                    div()
                        .w(px(340.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("NOME DE EXIBIÇÃO", cx))
                                .child(
                                    modal_state
                                        .profile_display_input
                                        .as_ref()
                                        .map(|input| {
                                            TextInput::new("settings-profile-display-name", input)
                                                .placeholder("Nome de exibição")
                                                .show_clear(false)
                                                .show_paste(false)
                                                .into_any_element()
                                        })
                                        .unwrap_or_else(|| {
                                            div()
                                                .h(px(40.0))
                                                .child(display_name.clone())
                                                .into_any_element()
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("PRONOMES", cx))
                                .child(
                                    modal_state
                                        .profile_pronouns_input
                                        .as_ref()
                                        .map(|input| {
                                            TextInput::new("settings-profile-pronouns", input)
                                                .placeholder("Adicione seus pronomes")
                                                .show_clear(false)
                                                .show_paste(false)
                                                .into_any_element()
                                        })
                                        .unwrap_or_else(|| {
                                            div()
                                                .h(px(40.0))
                                                .child(pronouns.clone())
                                                .into_any_element()
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("SOBRE MIM", cx))
                                .child(
                                    modal_state
                                        .profile_bio_input
                                        .as_ref()
                                        .map(|input| {
                                            div()
                                                .min_h(px(96.0))
                                                .p(px(10.0))
                                                .rounded(px(4.0))
                                                .bg(cx.discord().bg_server)
                                                .border_1()
                                                .border_color(cx.discord().borders)
                                                .child(
                                                    gpui::component::input::Textarea::new(input)
                                                        .text_size(px(13.0))
                                                        .text_color(cx.discord().text_primary)
                                                        .placeholder("Escreva algo sobre você..."),
                                                )
                                        })
                                        .unwrap_or_else(|| {
                                            div()
                                                .min_h(px(96.0))
                                                .p(px(10.0))
                                                .rounded(px(4.0))
                                                .bg(cx.discord().bg_server)
                                                .child(if bio.is_empty() {
                                                    "Biografia não sincronizada."
                                                } else {
                                                    bio.clone()
                                                })
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(
                                    Button::new("settings-profile-save", "Salvar alterações")
                                        .variant(ButtonVariant::Blurple)
                                        .size(ButtonSize::Sm)
                                        .disabled(!has_changes || is_saving)
                                        .on_click(move |_event, _window, cx| {
                                            save_modal.update(cx, |this, cx| this.save_profile(cx));
                                        }),
                                )
                                .child(
                                    Button::new("settings-profile-discard", "Descartar")
                                        .variant(ButtonVariant::Gray)
                                        .size(ButtonSize::Sm)
                                        .disabled(!has_changes || is_saving)
                                        .on_click(move |_event, _window, cx| {
                                            discard_modal
                                                .update(cx, |this, cx| this.discard_profile(cx));
                                        }),
                                ),
                        )
                        .children(match &modal_state.profile_edit.save_state {
                            SettingsSaveState::Saving => Some(
                                div()
                                    .text_color(cx.discord().text_secondary)
                                    .text_size(px(12.0))
                                    .child("Salvando na conta..."),
                            ),
                            SettingsSaveState::Saved => Some(
                                div()
                                    .text_color(cx.discord().status_online)
                                    .text_size(px(12.0))
                                    .child("Alterações salvas."),
                            ),
                            SettingsSaveState::Failed(error) => Some(
                                div()
                                    .text_color(cx.discord().status_dnd)
                                    .text_size(px(12.0))
                                    .child(error.clone()),
                            ),
                            SettingsSaveState::Idle => None,
                        }),
                )
                .child(
                    div()
                        .w(px(300.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(render_section_header("PRÉ-VISUALIZAÇÃO", cx))
                        .child(
                            div()
                                .rounded(px(8.0))
                                .bg(cx.discord().bg_server)
                                .border_1()
                                .border_color(cx.discord().bg_channels)
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .h(px(90.0))
                                        .w_full()
                                        .bg(cx.discord().bg_channels)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(cx.discord().text_muted)
                                        .text_size(px(11.0))
                                        .child("Banner não disponível neste cliente"),
                                )
                                .child(
                                    div()
                                        .px(px(14.0))
                                        .pb(px(14.0))
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .justify_between()
                                                .items_end()
                                                .mt(px(-36.0))
                                                .child(
                                                    Avatar::new(user.avatar_text.clone())
                                                        .size(AvatarSize::Px80)
                                                        .opt_src(user.avatar_url.clone())
                                                        .opt_decoration(
                                                            user.avatar_decoration_data.clone(),
                                                        )
                                                        .status(user.presence.into())
                                                        .cutout_color(cx.discord().bg_server),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .gap(px(4.0))
                                                        .pb(px(4.0))
                                                        .children(if has_nitro {
                                                            Some(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(2.0))
                                                                    .rounded(px(4.0))
                                                                    .bg(cx.discord().bg_channels)
                                                                    .child(icon_sparkles(
                                                                        px(13.0),
                                                                        cx.discord().role_purple,
                                                                    )),
                                                            )
                                                        } else {
                                                            None
                                                        }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .pt(px(10.0))
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_titles)
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_size(px(16.0))
                                                        .child(if display_name.is_empty() {
                                                            "Sem nome de exibição".to_string()
                                                        } else {
                                                            display_name
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_secondary)
                                                        .text_size(px(12.0))
                                                        .child(format!("@{}", user.username)),
                                                ),
                                        )
                                        .children(if !pronouns.is_empty() {
                                            Some(
                                                div()
                                                    .pt(px(4.0))
                                                    .text_color(cx.discord().text_secondary)
                                                    .text_size(px(11.0))
                                                    .child(pronouns),
                                            )
                                        } else {
                                            None
                                        })
                                        .children(user.custom_status.as_ref().map(|status| {
                                            div()
                                                .pt(px(6.0))
                                                .text_color(cx.discord().text_primary)
                                                .text_size(px(12.0))
                                                .child(status.clone())
                                        }))
                                        .child(
                                            div()
                                                .w_full()
                                                .h(px(1.0))
                                                .my(px(10.0))
                                                .bg(cx.discord().neutral_67),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(4.0))
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_secondary)
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_size(px(10.0))
                                                        .child("SOBRE MIM"),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_primary)
                                                        .text_size(px(12.0))
                                                        .child(if bio.is_empty() {
                                                            "Nenhuma biografia adicionada."
                                                                .to_string()
                                                        } else {
                                                            bio
                                                        }),
                                                ),
                                        ),
                                ),
                        ),
                ),
        )
}
