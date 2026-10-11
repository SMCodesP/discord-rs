use gpui::prelude::*;
use gpui::*;

use crate::app::state::CurrentUserProfile;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_section_description, render_section_header};
use super::UserSettingsModal;

pub fn render_profiles(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    user: &CurrentUserProfile,
    cx: &App,
) -> Div {
    let modal_read = modal_state;
    let display_name = modal_read.profile_display_name.clone();
    let username = user.username.clone();
    let pronouns = modal_read.profile_pronouns.clone();
    let bio = modal_read.profile_bio.clone();
    let banner_color = modal_read.profile_banner_color;
    let badge_nitro = modal_read.profile_badge_nitro;
    let badge_dev = modal_read.profile_badge_dev;
    let badge_hypesquad = modal_read.profile_badge_hypesquad;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(render_section_description(
            "Personalize a forma como você é visto em todos os servidores do Discord RS.",
            cx,
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(28.0))
                // ---------------------------------------------------------
                // LEFT COLUMN: Profile Form Fields (w = 340px)
                // ---------------------------------------------------------
                .child(
                    div()
                        .w(px(340.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        // Display Name Box
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("NOME DE EXIBIÇÃO", cx))
                                .child(
                                    div()
                                        .h(px(38.0))
                                        .px(px(12.0))
                                        .rounded(px(4.0))
                                        .bg(cx.discord().bg_server)
                                        .flex()
                                        .items_center()
                                        .text_color(cx.discord().text_titles)
                                        .text_size(px(13.0))
                                        .child(display_name.clone()),
                                ),
                        )
                        // Pronouns Row
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("PRONOMES", cx))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(6.0))
                                        .child(render_pronoun_pill(modal.clone(), "ele/dele", &pronouns, cx))
                                        .child(render_pronoun_pill(modal.clone(), "ela/dela", &pronouns, cx))
                                        .child(render_pronoun_pill(modal.clone(), "they/them", &pronouns, cx))
                                        .child(render_pronoun_pill(modal.clone(), "elu/delu", &pronouns, cx)),
                                ),
                        )
                        // Bio (About Me)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("SOBRE MIM", cx))
                                .child(
                                    div()
                                        .min_h(px(60.0))
                                        .p(px(10.0))
                                        .rounded(px(4.0))
                                        .bg(cx.discord().bg_server)
                                        .text_color(cx.discord().text_primary)
                                        .text_size(px(12.0))
                                        .child(if bio.is_empty() {
                                            "Escreva algo sobre você...".to_string()
                                        } else {
                                            bio.clone()
                                        }),
                                ),
                        )
                        // Banner Accent Color Swatches
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("COR DO BANNER / DESTAQUE", cx))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(8.0))
                                        .child(render_color_swatch(modal.clone(), HEX_BLURPLE, banner_color, cx))
                                        .child(render_color_swatch(modal.clone(), HEX_STATUS_ONLINE, banner_color, cx))
                                        .child(render_color_swatch(modal.clone(), HEX_STATUS_DND, banner_color, cx))
                                        .child(render_color_swatch(modal.clone(), HEX_STATUS_IDLE, banner_color, cx))
                                        .child(render_color_swatch(modal.clone(), HEX_ROLE_PURPLE, banner_color, cx))
                                        .child(render_color_swatch(modal.clone(), HEX_BG_SERVER, banner_color, cx)),
                                ),
                        )
                        // Badges toggles
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(render_section_header("DISTINTIVOS ATIVOS", cx))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(8.0))
                                        .child({
                                            let modal_c = modal.clone();
                                            div()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .rounded(px(4.0))
                                                .bg(if badge_nitro { cx.discord().blurple } else { cx.discord().bg_channels })
                                                .text_color(cx.discord().white)
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    modal_c.update(cx, |this, cx| {
                                                        this.profile_badge_nitro = !this.profile_badge_nitro;
                                                        cx.notify();
                                                    });
                                                })
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(4.0))
                                                .child(icon_sparkles(px(13.0), cx.discord().white))
                                                .child("Nitro")
                                        })
                                        .child({
                                            let modal_c = modal.clone();
                                            div()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .rounded(px(4.0))
                                                .bg(if badge_dev { cx.discord().status_online } else { cx.discord().bg_channels })
                                                .text_color(cx.discord().white)
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    modal_c.update(cx, |this, cx| {
                                                        this.profile_badge_dev = !this.profile_badge_dev;
                                                        cx.notify();
                                                    });
                                                })
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(4.0))
                                                .child(icon_bot(px(13.0), cx.discord().white))
                                                .child("Dev Ativo")
                                        })
                                        .child({
                                            let modal_c = modal.clone();
                                            div()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .rounded(px(4.0))
                                                .bg(if badge_hypesquad { cx.discord().role_purple } else { cx.discord().bg_channels })
                                                .text_color(cx.discord().white)
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                    modal_c.update(cx, |this, cx| {
                                                        this.profile_badge_hypesquad = !this.profile_badge_hypesquad;
                                                        cx.notify();
                                                    });
                                                })
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(4.0))
                                                .child(icon_shield(px(13.0), cx.discord().white))
                                                .child("HypeSquad")
                                        }),
                                ),
                        ),
                )
                // ---------------------------------------------------------
                // RIGHT COLUMN: Discord Live Preview Profile Card (w = 300px)
                // ---------------------------------------------------------
                .child(
                    div()
                        .w(px(300.0))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(render_section_header("PRÉ-VISUALIZAÇÃO EM TEMPO REAL", cx))
                        .child(
                            div()
                                .rounded(px(8.0))
                                .bg(cx.discord().bg_server)
                                .border_1()
                                .border_color(cx.discord().bg_channels)
                                .overflow_hidden()
                                .flex()
                                .flex_col()
                                // Live Banner
                                .child(
                                    div()
                                        .h(px(90.0))
                                        .w_full()
                                        .bg(rgb(banner_color)),
                                )
                                // Profile Body
                                .child(
                                    div()
                                        .px(px(14.0))
                                        .pb(px(14.0))
                                        .flex()
                                        .flex_col()
                                        // Avatar using crate::components::Avatar
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
                                                        .opt_decoration(user.avatar_decoration_data.clone())
                                                        .status(user.presence.into())
                                                        .cutout_color(cx.discord().bg_server)
                                                )
                                                // Badges pill container
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .gap(px(4.0))
                                                        .pb(px(4.0))
                                                        .children(if badge_nitro {
                                                            Some(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(2.0))
                                                                    .rounded(px(4.0))
                                                                    .bg(cx.discord().bg_channels)
                                                                    .child(icon_sparkles(px(13.0), cx.discord().role_purple)),
                                                            )
                                                        } else {
                                                            None
                                                        })
                                                        .children(if badge_dev {
                                                            Some(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(2.0))
                                                                    .rounded(px(4.0))
                                                                    .bg(cx.discord().bg_channels)
                                                                    .child(icon_bot(px(13.0), cx.discord().blurple)),
                                                            )
                                                        } else {
                                                            None
                                                        })
                                                        .children(if badge_hypesquad {
                                                            Some(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(2.0))
                                                                    .rounded(px(4.0))
                                                                    .bg(cx.discord().bg_channels)
                                                                    .child(icon_shield(px(13.0), cx.discord().bg_animated_gradient_pink_1)),
                                                            )
                                                        } else {
                                                            None
                                                        }),
                                                ),
                                        )
                                        // Names
                                        .child(
                                            div()
                                                .pt(px(10.0))
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_titles)
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_size(px(16.0))
                                                        .child(if display_name.is_empty() {
                                                            user.display_name.clone()
                                                        } else {
                                                            display_name
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_secondary)
                                                        .text_size(px(12.0))
                                                        .child(format!("@{}", username)),
                                                ),
                                        )
                                        // Pronouns Tag (if present)
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
                                        // Divider
                                        .child(
                                            div()
                                                .w_full()
                                                .h(px(1.0))
                                                .my(px(10.0))
                                                .bg(cx.discord().neutral_67),
                                        )
                                        // About Me Section
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
                                                            "Nenhuma biografia adicionada.".to_string()
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

fn render_pronoun_pill(
    modal: Entity<UserSettingsModal>,
    pronoun: &'static str,
    current_pronouns: &str,
    cx: &App,
) -> Div {
    let is_selected = current_pronouns == pronoun;
    let bg_col = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().bg_channels
    };
    let text_col = if is_selected {
        cx.discord().white
    } else {
        cx.discord().text_secondary
    };

    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(bg_col)
        .text_color(text_col)
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            modal.update(cx, |this, cx| {
                if this.profile_pronouns == pronoun {
                    this.profile_pronouns.clear();
                } else {
                    this.profile_pronouns = pronoun.to_string();
                }
                cx.notify();
            });
        })
        .child(pronoun)
}

fn render_color_swatch(
    modal: Entity<UserSettingsModal>,
    color_hex: u32,
    active_hex: u32,
    cx: &App,
) -> Div {
    let is_active = active_hex == color_hex;

    div()
        .size(px(24.0))
        .rounded_full()
        .bg(rgb(color_hex))
        .border_2()
        .border_color(if is_active {
            cx.discord().white
        } else {
            cx.discord().transparent
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            modal.update(cx, |this, cx| {
                this.profile_banner_color = color_hex;
                cx.notify();
            });
        })
}
