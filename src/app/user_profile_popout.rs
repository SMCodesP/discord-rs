//! Discord RS - Canonical User Profile Components (Fase 4 & Modal Parity)
//! 1:1 Parity with Official Discord Desktop Client:
//! - UserProfilePopout: 300px anchored popout card, no backdrop, dismissible on click-outside or Esc
//! - UserProfileModal: 1184x704px centered Dialog with backdrop, 3 tabs (Sobre mim, Servidores em comum, Amigos em comum)
//!
//! Features:
//! - Popout opens anchored to the clicked element/position and snaps to window edges
//! - Popout has NO close button (closes on click outside / Esc key)
//! - Opens full modal when clicking avatar/name inside popout or "Ver perfil completo"
//! - Modal is a single 1184x704 r12 box with a single close button (native DialogClose)
//! - Bio text breaks lines smoothly within 300px width
//! - Quick message input has show_paste(false) and ellipsis placeholder
//! - Canonical Avatar component with decor & presence support throughout
//! - Official Button and IconButton with icons
//! - Legacy colors marked with // COR ANTIGA SEM ORIGEM per Adendo A
//! - Zero fake mocks (empty when real data is absent, no fake fallback names)

use chrono::{DateTime, Datelike, Utc};
use gpui::component::dialog::Dialog;
use gpui::component::input::InputState;
use gpui::*;

use crate::app::state::{AppState, ProfileModalTab};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::{Button, ButtonSize};
use crate::components::profile_effect::{
    render_profile_effect_for_user, ProfileEffectCatalog, UserProfileEffect,
};
use crate::components::TextInput;
pub use crate::domain::profile::{
    snowflake_to_datetime, ConnectedAccount, MutualGuild, ProfileBadge, ProfilePopoutState,
    ProfileTab, UserProfileData,
};
use crate::domain::user::{ActivityType, CustomStatus, Presence, PresenceStatus};
use crate::i18n::t;
use crate::tokens::colors::*;
use crate::tokens::colors::white;
use crate::tokens::icons::*;

// ============================================================================
// Helper Utilities
// ============================================================================

/// Parses a hex color string (e.g. "#110fbf" or "110fbf") into a `u32` RGB value
pub fn parse_hex_color(hex_str: &str) -> Option<u32> {
    let clean = hex_str.trim().trim_start_matches('#');
    u32::from_str_radix(clean, 16).ok()
}

/// Formats a month name in pt-BR
fn pt_month_short(month: u32) -> &'static str {
    match month {
        1 => "jan.",
        2 => "fev.",
        3 => "mar.",
        4 => "abr.",
        5 => "mai.",
        6 => "jun.",
        7 => "jul.",
        8 => "ago.",
        9 => "set.",
        10 => "out.",
        11 => "nov.",
        12 => "dez.",
        _ => "",
    }
}

/// Formats a DateTime into canonical Discord display ("20 de out. de 2023")
pub fn format_member_date(dt: DateTime<Utc>) -> String {
    format!("{} de {} de {}", dt.day(), pt_month_short(dt.month()), dt.year())
}

// ============================================================================
// Shared Content Renderers (Popout & Modal Deduplication)
// ============================================================================

/// Renders bio text splitting by paragraphs and wrapping cleanly at word boundaries
fn render_bio_content(bio: &str) -> AnyElement {
    let paragraphs: Vec<_> = bio.split('\n').collect();
    let mut bio_container = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(4.0));

    for p in paragraphs {
        if !p.trim().is_empty() {
            bio_container = bio_container.child(
                div()
                    .w_full()
                    .text_size(px(13.0))
                    .text_color(text_primary())
                    .line_height(px(18.0))
                    .child(p.to_string()),
            );
        }
    }

    bio_container.into_any_element()
}

/// Renders mutual guilds list shared between popout and modal tabs
fn render_mutual_guilds_list(guilds: &[MutualGuild], is_modal: bool) -> AnyElement {
    if guilds.is_empty() {
        return div()
            .py(if is_modal { px(0.0) } else { px(8.0) })
            .text_size(if is_modal { px(14.0) } else { px(12.0) })
            .text_color(legacy_949ba4())
            .child("Nenhum servidor em comum.")
            .into_any_element();
    }

    let item_h = if is_modal { px(40.0) } else { px(32.0) };
    let icon_sz = if is_modal { px(24.0) } else { px(20.0) };
    let text_sz = if is_modal { px(13.0) } else { px(12.0) };

    let mut list = div().flex().flex_col().gap(px(4.0));
    for g in guilds {
        // Zero dados inventados: sem nome real mostra vazio (zero fallback "Servidor {id}")
        let display_nick = g.nick.clone().unwrap_or_default();
        list = list.child(
            div()
                .h(item_h)
                .px(px(6.0))
                .rounded(px(4.0))
                .bg(bg_server())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(icon_sz)
                        .rounded(px(4.0))
                        .bg(blurple())
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(white())
                        .child(g.id.chars().next().unwrap_or('S').to_string()),
                )
                .child(
                    div()
                        .text_size(text_sz)
                        .text_color(text_primary())
                        .child(display_nick),
                ),
        );
    }

    list.into_any_element()
}

/// Renders mutual friends count summary shared between popout and modal tabs
fn render_mutual_friends_content(count: u32, is_modal: bool) -> AnyElement {
    if count == 0 {
        return div()
            .py(if is_modal { px(0.0) } else { px(8.0) })
            .text_size(if is_modal { px(14.0) } else { px(12.0) })
            .text_color(legacy_949ba4())
            .child("Nenhum amigo em comum.")
            .into_any_element();
    }

    div()
        .py(if is_modal { px(0.0) } else { px(8.0) })
        .text_size(if is_modal { px(14.0) } else { px(12.0) })
        .text_color(text_primary())
        .child(format!("{} amigos em comum com este usuário.", count))
        .into_any_element()
}

/// Renders member dates: snowflake-derived account creation date and joined server date
fn render_member_dates_content(
    created_at: Option<DateTime<Utc>>,
    joined_server_at: Option<DateTime<Utc>>,
) -> Option<AnyElement> {
    if created_at.is_none() && joined_server_at.is_none() {
        return None;
    }

    let mut dates_col = div().flex().flex_col().gap(px(8.0));

    if let Some(created_at) = created_at {
        dates_col = dates_col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xb5bac1)) // COR ANTIGA SEM ORIGEM
                        .child(t("profile.discord_member_since").to_uppercase()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_info(px(14.0), legacy_949ba4()))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_primary())
                                .child(format_member_date(created_at)),
                        ),
                ),
        );
    }

    if let Some(joined_at) = joined_server_at {
        dates_col = dates_col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xb5bac1)) // COR ANTIGA SEM ORIGEM
                        .child(t("profile.server_member_since").to_uppercase()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_hash(px(14.0), legacy_949ba4()))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_primary())
                                .child(format_member_date(joined_at)),
                        ),
                ),
        );
    }

    Some(dates_col.into_any_element())
}

// ============================================================================
// Component 1: Anchored User Profile Popout (300px)
// ============================================================================

/// Renders the 300px User Profile Popout anchored to the click position, with no backdrop
pub fn render_user_profile_popout(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    anchor: Option<Point<Pixels>>,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
) -> impl IntoElement {
    let entity_dismiss = app_entity.clone();
    let entity_key = app_entity.clone();

    // Responsive anchoring logic:
    // If click position is near the right edge of the window (> 900px), open to the left (Anchor::TopRight).
    // Otherwise open to the right (Anchor::TopLeft).
    let (anchor_pos, anchor_corner) = if let Some(pt) = anchor {
        let is_near_right = pt.x > px(900.0);
        if is_near_right {
            (point(pt.x - px(8.0), pt.y), Anchor::TopRight)
        } else {
            (point(pt.x + px(8.0), pt.y), Anchor::TopLeft)
        }
    } else {
        (point(px(1040.0) - px(8.0), px(60.0)), Anchor::TopRight)
    };

    div()
        .id("profile-popout-layer")
        .key_context("ProfilePopout")
        .on_key_down(move |event: &KeyDownEvent, _window, cx| {
            if event.keystroke.key == "escape" {
                entity_key.update(cx, |s, cx| {
                    s.handle_escape();
                    cx.notify();
                });
            }
        })
        .absolute()
        .inset_0()
        // Transparent dismiss backdrop (sem backdrop escurecido, zero opacidade)
        // Nota: O literal antigo rgba(0x00000077) foi removido pois o popout do Discord não tem backdrop escurecido
        .child(
            div()
                .id("profile-popout-dismiss-backdrop")
                .absolute()
                .inset_0()
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity_dismiss.update(cx, |s, cx| {
                        s.close_profile_popout();
                        cx.notify();
                    });
                }),
        )
        // Popout card container anchored with high priority
        .child(
            deferred(
                anchored()
                    .position(anchor_pos)
                    .anchor(anchor_corner)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_ev, _window, cx| {
                                cx.stop_propagation();
                            })
                            .child(render_profile_card(
                                app_entity,
                                popout,
                                catalog,
                                presence,
                                quick_state,
                            )),
                    ),
            ),
        )
}

/// Renders the 300px Popout Card container
fn render_profile_card(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
) -> AnyElement {
    let mut card = div()
        .id("profile-popout-card")
        .w(px(300.0))
        .max_h(px(580.0))
        .rounded(px(8.0))
        .bg(bg_user_bar())
        .border_1()
        .border_color(rgb(0x1f2023)) // COR ANTIGA SEM ORIGEM
        .shadow_lg()
        .overflow_hidden()
        .flex()
        .flex_col();

    card = card.child(render_popout_banner(popout, catalog));
    card = card.child(render_popout_avatar_row(app_entity.clone(), popout, presence));

    if popout.is_loading && popout.data.is_none() {
        return card
            .child(
                div()
                    .p(px(24.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(text_muted())
                            .child(t("profile.loading")),
                    ),
            )
            .into_any_element();
    }

    if let Some(ref err) = popout.error {
        if popout.data.is_none() {
            return card
                .child(
                    div()
                        .p(px(24.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(legacy_fa777c())
                                .child(t("profile.error")),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_muted())
                                .child(err.clone()),
                        ),
                )
                .into_any_element();
        }
    }

    card = card.child(render_popout_body(app_entity.clone(), popout, presence));
    card = card.child(render_popout_quick_input(popout, quick_state));

    card.into_any_element()
}

/// Renders the 105px top banner with profile effect (sem botão X no popout)
/// Nota: O literal antigo rgba(0x00000088) do botão fechar foi removido pois o popout do Discord não tem botão X
fn render_popout_banner(
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
) -> AnyElement {
    let banner_color_val = popout
        .data
        .as_ref()
        .and_then(|d| d.banner_color.as_deref().and_then(parse_hex_color))
        .or_else(|| popout.data.as_ref().and_then(|d| d.accent_color))
        .unwrap_or(HEX_BG_FLOATING);

    let banner_image_url = popout.data.as_ref().and_then(|d| {
        d.banner.as_ref().map(|b| {
            if b.starts_with("http://") || b.starts_with("https://") {
                b.clone()
            } else {
                format!("https://cdn.discordapp.com/banners/{}/{}.png?size=300", d.user.id, b)
            }
        })
    });

    let mut banner = div()
        .relative()
        .w(px(300.0))
        .h(px(105.0))
        .bg(rgb(banner_color_val))
        .flex_shrink_0();

    if let Some(ref img_url) = banner_image_url {
        banner = banner.child(
            img(img_url.clone())
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Cover),
        );
    }

    let profile_effect_id = popout
        .data
        .as_ref()
        .and_then(|d| d.profile_effect_id.as_deref());
    let user_effect = profile_effect_id.map(UserProfileEffect::new);
    let effect_overlay = render_profile_effect_for_user(
        user_effect.as_ref(),
        catalog,
        px(105.0),
        Some(px(300.0)),
    );
    banner = banner.child(effect_overlay);

    banner.into_any_element()
}

/// Renders the overlapping avatar (clickable to open full modal) and badges container
fn render_popout_avatar_row(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> AnyElement {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let avatar_url = avatar_user.and_then(|u| u.avatar_url());
    let display_name = popout
        .data
        .as_ref()
        .map(|d| d.user.display_name().to_string())
        .unwrap_or_else(|| popout.user_id.clone());

    let presence_status = presence
        .map(|p| p.status)
        .or_else(|| avatar_user.map(|u| u.presence))
        .unwrap_or(PresenceStatus::Offline);

    let avatar_initial = display_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let avatar = Avatar::new(avatar_initial)
        .size(AvatarSize::Px80)
        .opt_src(avatar_url)
        .opt_status(Some(presence_status))
        .opt_decoration(avatar_user.and_then(|u| u.avatar_decoration_data.clone()))
        .cutout_color(bg_user_bar());

    let entity_av_modal = app_entity;
    let avatar_container = div()
        .relative()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            entity_av_modal.update(cx, |s, cx| {
                s.open_user_profile_modal();
                cx.notify();
            });
        })
        .child(avatar);

    let badges_container = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(4.0))
        .items_center()
        .pb(px(4.0))
        .children(
            popout
                .data
                .as_ref()
                .map(|d| d.badges.clone())
                .unwrap_or_default()
                .into_iter()
                .map(|b| {
                    div()
                        .size(px(20.0))
                        .rounded(px(4.0))
                        .bg(rgba(0xffffff11)) // COR ANTIGA SEM ORIGEM
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(b.icon_url()).size(px(16.0)))
                }),
        );

    div()
        .px(px(16.0))
        .mt(px(-40.0))
        .flex()
        .flex_row()
        .justify_between()
        .items_end()
        .child(avatar_container)
        .child(badges_container)
        .into_any_element()
}

/// Renders the scrollable popout body
fn render_popout_body(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> AnyElement {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let display_name = popout
        .data
        .as_ref()
        .map(|d| d.user.display_name().to_string())
        .unwrap_or_else(|| popout.user_id.clone());
    let username = avatar_user
        .map(|u| u.username.clone())
        .unwrap_or_else(|| popout.user_id.clone());

    let mut body = div()
        .id("profile-popout-body")
        .flex_1()
        .overflow_y_scroll()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(10.0));

    // Display Name (clickable to open full modal)
    let entity_name_modal = app_entity.clone();
    body = body.child(
        div()
            .text_size(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(white())
            .cursor_pointer()
            .hover(|s| s.underline())
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                entity_name_modal.update(cx, |s, cx| {
                    s.open_user_profile_modal();
                    cx.notify();
                });
            })
            .child(display_name),
    );

    // @username and optional pronouns
    let mut tag_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .text_color(legacy_949ba4())
                .child(format!("@{}", username)),
        );

    if let Some(pronouns) = popout.data.as_ref().and_then(|d| d.pronouns.as_deref()) {
        tag_row = tag_row.child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .bg(opacity_white_8())
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_primary())
                .child(pronouns.to_string()),
        );
    }
    body = body.child(tag_row);

    if let Some(status_el) = render_popout_status(popout, presence) {
        body = body.child(status_el);
    }
    if let Some(game_el) = render_popout_activity(popout, presence) {
        body = body.child(game_el);
    }

    body = body.child(div().h_px().bg(channel_hover()).my(px(2.0)));

    if let Some(bio_el) = render_popout_bio(popout) {
        body = body.child(bio_el);
        body = body.child(div().h_px().bg(channel_hover()).my(px(2.0)));
    }

    let created_at = popout
        .data
        .as_ref()
        .and_then(|d| d.created_at)
        .or_else(|| snowflake_to_datetime(&popout.user_id));
    let joined_server_at = popout.data.as_ref().and_then(|d| d.joined_server_at);

    if let Some(dates_el) = render_member_dates_content(created_at, joined_server_at) {
        body = body.child(dates_el);
        body = body.child(div().h_px().bg(channel_hover()).my(px(2.0)));
    }

    body = body.child(render_popout_tabs(app_entity.clone(), popout));

    // "Ver perfil completo" action button
    let entity_full = app_entity;
    body = body.child(
        Button::ghost("popout-btn-view-full", "Ver perfil completo")
            .size(ButtonSize::Sm)
            .full_width()
            .on_click(move |_ev, _win, cx| {
                entity_full.update(cx, |s, cx| {
                    s.open_user_profile_modal();
                    cx.notify();
                });
            }),
    );

    body.into_any_element()
}

/// Renders custom status text/emoji if present
fn render_popout_status(
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> Option<AnyElement> {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let custom_status: Option<CustomStatus> = presence
        .and_then(|p| p.custom_status.clone())
        .or_else(|| {
            avatar_user.and_then(|u| u.activity.as_ref()).and_then(|act| {
                act.state.as_ref().map(|s| CustomStatus::from_text(s))
            })
        });

    if let Some(cs) = custom_status {
        let emoji_el: Option<AnyElement> = if let Some(ref em) = cs.emoji {
            if let Some(url) = em.image_url() {
                Some(img(url).size(px(16.0)).into_any_element())
            } else if !em.name.is_empty() {
                Some(div().text_size(px(14.0)).child(em.name.clone()).into_any_element())
            } else {
                Some(div().text_size(px(14.0)).child("💬").into_any_element())
            }
        } else {
            Some(div().text_size(px(14.0)).child("💬").into_any_element())
        };

        if cs.text.is_some() || cs.emoji.is_some() {
            return Some(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(channel_hover())
                    .bg(bg_server())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .children(emoji_el)
                    .children(cs.text.as_ref().map(|t| {
                        div()
                            .text_size(px(13.0))
                            .text_color(text_primary())
                            .child(t.clone())
                    }))
                    .into_any_element(),
            );
        }
    }
    None
}

/// Renders live game activity if present
fn render_popout_activity(
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> Option<AnyElement> {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let game = presence
        .and_then(|p| p.activity.as_ref())
        .or_else(|| avatar_user.and_then(|u| u.activity.as_ref()))
        .filter(|a| a.activity_type == ActivityType::Playing || (!a.name.is_empty() && a.name != "Custom Status"))?;

    Some(
        div()
            .p(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(channel_hover())
            .bg(bg_server())
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(status_online())
                    .child("JOGANDO"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white())
                    .child(game.name.clone()),
            )
            .children(game.details.as_ref().map(|d| {
                div()
                    .text_size(px(12.0))
                    .text_color(text_primary())
                    .child(d.clone())
            }))
            .children(game.state.as_ref().map(|s| {
                div()
                    .text_size(px(12.0))
                    .text_color(legacy_949ba4())
                    .child(s.clone())
            }))
            .into_any_element(),
    )
}

/// Renders "Sobre mim" bio section with automatic line wrapping within 300px popout
fn render_popout_bio(popout: &ProfilePopoutState) -> Option<AnyElement> {
    let bio = popout.data.as_ref().and_then(|d| d.bio.as_ref())?;
    if bio.trim().is_empty() {
        return None;
    }

    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xb5bac1)) // COR ANTIGA SEM ORIGEM
                    .child(t("profile.about_me").to_uppercase()),
            )
            .child(render_bio_content(bio))
            .into_any_element(),
    )
}

/// Renders tabs and tab content in the popout
fn render_popout_tabs(app_entity: Entity<AppState>, popout: &ProfilePopoutState) -> AnyElement {
    let mutual_guilds = popout
        .data
        .as_ref()
        .map(|d| d.mutual_guilds.as_slice())
        .unwrap_or(&[]);
    let mutual_guilds_count = mutual_guilds.len();
    let mutual_friends_count = popout
        .data
        .as_ref()
        .map(|d| d.mutual_friends_count)
        .unwrap_or(0);

    let is_guilds_tab = popout.active_tab == ProfileTab::MutualGuilds;
    let entity_tab_guilds = app_entity.clone();
    let entity_tab_friends = app_entity;

    let tab_header = div()
        .flex()
        .flex_row()
        .gap(px(12.0))
        .border_b_1()
        .border_color(channel_hover())
        .pb(px(4.0))
        .child(
            div()
                .cursor_pointer()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_guilds_tab { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_guilds_tab { blurple() } else { transparent() })
                .pb(px(4.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity_tab_guilds.update(cx, |s, cx| {
                        if let Some(ref mut p) = s.profile_popout {
                            p.active_tab = ProfileTab::MutualGuilds;
                        }
                        cx.notify();
                    });
                })
                .child(format!("Servidores ({})", mutual_guilds_count)),
        )
        .child(
            div()
                .cursor_pointer()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if !is_guilds_tab { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if !is_guilds_tab { blurple() } else { transparent() })
                .pb(px(4.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity_tab_friends.update(cx, |s, cx| {
                        if let Some(ref mut p) = s.profile_popout {
                            p.active_tab = ProfileTab::MutualFriends;
                        }
                        cx.notify();
                    });
                })
                .child(format!("Amigos ({})", mutual_friends_count)),
        );

    let tab_content = if is_guilds_tab {
        render_mutual_guilds_list(mutual_guilds, false)
    } else {
        render_mutual_friends_content(mutual_friends_count, false)
    };

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(tab_header)
        .child(tab_content)
        .into_any_element()
}

/// Renders quick direct message input field at the bottom of the popout (show_paste(false) per Parity)
fn render_popout_quick_input(
    popout: &ProfilePopoutState,
    quick_state: &Entity<InputState>,
) -> AnyElement {
    let target_uname = popout
        .data
        .as_ref()
        .map(|d| d.user.username.clone())
        .unwrap_or_else(|| popout.user_id.clone());

    div()
        .p(px(12.0))
        .border_t_1()
        .border_color(rgb(0x1f2023)) // COR ANTIGA SEM ORIGEM
        .bg(bg_server())
        .child(
            TextInput::new("profile-quick-message-input", quick_state)
                .placeholder(format!("Conversar com @{}...", target_uname))
                .show_paste(false)
                .full_width(true),
        )
        .into_any_element()
}

// ============================================================================
// Component 2: Full User Profile Modal (1184x704px Dialog, 3 Tabs)
// ============================================================================

/// Builds the Dialog for the full User Profile Modal (opened exclusively from the popout)
/// UMA caixa só (1184x704 r12 com backdrop e sem moldura dupla) e UM botão de fechar (DialogClose do kit)
pub fn build_user_profile_modal_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    vh: Pixels,
    cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let state = entity.read(cx);

    let popout = match state.profile_popout {
        Some(ref p) => p.clone(),
        None => return dialog,
    };

    let catalog = state.profile_effects_catalog.clone();
    let presence = state.presences_by_user.get(&popout.user_id).cloned();
    let active_tab = state.profile_modal_tab;

    let margin_top = (vh - px(704.0)).max(px(20.0)) / 2.0;

    let content = div()
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(render_modal_banner(&popout, &catalog))
        .child(render_modal_header(&popout, presence.as_ref()))
        .child(render_modal_tabs_bar(entity.clone(), &popout, active_tab))
        .child(render_modal_tab_content(&popout, active_tab));

    dialog
        .w(px(1184.0))
        .h(px(704.0))
        .margin_top(margin_top)
        .p_0()
        .bg(bg_user_bar())
        .rounded(px(12.0))
        .border_1()
        .border_color(rgb(0x1f2023)) // COR ANTIGA SEM ORIGEM
        .overflow_hidden()
        .close_button(true)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_user_profile_modal();
                cx.notify();
            });
        })
        .child(content)
}

/// Renders the top modal banner with effect overlay (sem botão X duplicado; o X pertence ao Dialog)
fn render_modal_banner(
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
) -> AnyElement {
    let banner_color_val = popout
        .data
        .as_ref()
        .and_then(|d| d.banner_color.as_deref().and_then(parse_hex_color))
        .or_else(|| popout.data.as_ref().and_then(|d| d.accent_color))
        .unwrap_or(HEX_BG_FLOATING);

    let banner_image_url = popout.data.as_ref().and_then(|d| {
        d.banner.as_ref().map(|b| {
            if b.starts_with("http://") || b.starts_with("https://") {
                b.clone()
            } else {
                format!("https://cdn.discordapp.com/banners/{}/{}.png?size=600", d.user.id, b)
            }
        })
    });

    let mut banner = div()
        .relative()
        .w_full()
        .h(px(210.0))
        .bg(rgb(banner_color_val))
        .flex_shrink_0();

    if let Some(ref img_url) = banner_image_url {
        banner = banner.child(
            img(img_url.clone())
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Cover),
        );
    }

    let profile_effect_id = popout
        .data
        .as_ref()
        .and_then(|d| d.profile_effect_id.as_deref());
    let user_effect = profile_effect_id.map(UserProfileEffect::new);
    let effect_overlay = render_profile_effect_for_user(
        user_effect.as_ref(),
        catalog,
        px(210.0),
        Some(px(1184.0)),
    );
    banner = banner.child(effect_overlay);

    banner.into_any_element()
}

/// Renders the profile header row inside the modal
fn render_modal_header(popout: &ProfilePopoutState, presence: Option<&Presence>) -> AnyElement {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let avatar_url = avatar_user.and_then(|u| u.avatar_url());
    let display_name = popout
        .data
        .as_ref()
        .map(|d| d.user.display_name().to_string())
        .unwrap_or_else(|| popout.user_id.clone());
    let username = avatar_user
        .map(|u| u.username.clone())
        .unwrap_or_else(|| popout.user_id.clone());

    let presence_status = presence
        .map(|p| p.status)
        .or_else(|| avatar_user.map(|u| u.presence))
        .unwrap_or(PresenceStatus::Offline);

    let avatar_initial = display_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let avatar = Avatar::new(avatar_initial)
        .size(AvatarSize::Px80)
        .opt_src(avatar_url)
        .opt_status(Some(presence_status))
        .opt_decoration(avatar_user.and_then(|u| u.avatar_decoration_data.clone()))
        .cutout_color(bg_user_bar());

    let badges = popout
        .data
        .as_ref()
        .map(|d| d.badges.clone())
        .unwrap_or_default();

    div()
        .px(px(24.0))
        .mt(px(-40.0))
        .flex()
        .flex_row()
        .justify_between()
        .items_end()
        .child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(16.0))
                .child(avatar)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(24.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(white())
                                .child(display_name),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(legacy_949ba4())
                                .child(format!("@{}", username)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(6.0))
                .items_center()
                .children(badges.into_iter().map(|b| {
                    div()
                        .size(px(24.0))
                        .rounded(px(4.0))
                        .bg(rgba(0xffffff11)) // COR ANTIGA SEM ORIGEM
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(b.icon_url()).size(px(18.0)))
                })),
        )
        .into_any_element()
}

/// Renders the 3 canonical profile modal tabs bar
fn render_modal_tabs_bar(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    active_tab: ProfileModalTab,
) -> AnyElement {
    let mutual_guilds_count = popout
        .data
        .as_ref()
        .map(|d| d.mutual_guilds.len())
        .unwrap_or(0);
    let mutual_friends_count = popout
        .data
        .as_ref()
        .map(|d| d.mutual_friends_count)
        .unwrap_or(0);

    let e_tab1 = app_entity.clone();
    let e_tab2 = app_entity.clone();
    let e_tab3 = app_entity;

    let is_about = active_tab == ProfileModalTab::AboutMe;
    let is_guilds = active_tab == ProfileModalTab::MutualGuilds;
    let is_friends = active_tab == ProfileModalTab::MutualFriends;

    div()
        .mt(px(16.0))
        .px(px(24.0))
        .flex()
        .flex_row()
        .gap(px(24.0))
        .border_b_1()
        .border_color(channel_hover())
        .child(
            div()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_about { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_about { blurple() } else { transparent() })
                .pb(px(8.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    e_tab1.update(cx, |s, cx| {
                        s.set_profile_modal_tab(ProfileModalTab::AboutMe);
                        cx.notify();
                    });
                })
                .child("Sobre mim"),
        )
        .child(
            div()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_guilds { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_guilds { blurple() } else { transparent() })
                .pb(px(8.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    e_tab2.update(cx, |s, cx| {
                        s.set_profile_modal_tab(ProfileModalTab::MutualGuilds);
                        cx.notify();
                    });
                })
                .child(format!("Servidores em comum ({})", mutual_guilds_count)),
        )
        .child(
            div()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_friends { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_friends { blurple() } else { transparent() })
                .pb(px(8.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    e_tab3.update(cx, |s, cx| {
                        s.set_profile_modal_tab(ProfileModalTab::MutualFriends);
                        cx.notify();
                    });
                })
                .child(format!("Amigos em comum ({})", mutual_friends_count)),
        )
        .into_any_element()
}

/// Renders the modal content corresponding to the selected tab
fn render_modal_tab_content(
    popout: &ProfilePopoutState,
    active_tab: ProfileModalTab,
) -> AnyElement {
    let mut container = div()
        .id("modal-tab-content-container")
        .flex_1()
        .overflow_y_scroll()
        .p(px(24.0));

    match active_tab {
        ProfileModalTab::AboutMe => {
            container = container.child(render_modal_about_me(popout));
        }
        ProfileModalTab::MutualGuilds => {
            let guilds = popout
                .data
                .as_ref()
                .map(|d| d.mutual_guilds.as_slice())
                .unwrap_or(&[]);
            container = container.child(render_mutual_guilds_list(guilds, true));
        }
        ProfileModalTab::MutualFriends => {
            let count = popout
                .data
                .as_ref()
                .map(|d| d.mutual_friends_count)
                .unwrap_or(0);
            container = container.child(render_mutual_friends_content(count, true));
        }
    }

    container.into_any_element()
}

/// Renders "Sobre mim" tab content (Bio, Connected accounts, Member dates)
fn render_modal_about_me(popout: &ProfilePopoutState) -> AnyElement {
    let mut col = div().flex().flex_col().gap(px(16.0));

    if let Some(ref bio) = popout.data.as_ref().and_then(|d| d.bio.as_ref()) {
        if !bio.trim().is_empty() {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xb5bac1)) // COR ANTIGA SEM ORIGEM
                            .child("BIOGRAFIA"),
                    )
                    .child(render_bio_content(bio)),
            );
        }
    }

    let created_at = popout
        .data
        .as_ref()
        .and_then(|d| d.created_at)
        .or_else(|| snowflake_to_datetime(&popout.user_id));
    let joined_server_at = popout.data.as_ref().and_then(|d| d.joined_server_at);

    if let Some(dates_el) = render_member_dates_content(created_at, joined_server_at) {
        col = col.child(dates_el);
    }

    if let Some(data) = popout.data.as_ref() {
        if !data.connected_accounts.is_empty() {
            let mut acc_col = div().flex().flex_col().gap(px(8.0)).child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xb5bac1)) // COR ANTIGA SEM ORIGEM
                    .child("CONTAS CONECTADAS"),
            );
            let mut acc_grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
            for acc in &data.connected_accounts {
                acc_grid = acc_grid.child(
                    div()
                        .px(px(12.0))
                        .py(px(8.0))
                        .rounded(px(6.0))
                        .bg(bg_server())
                        .border_1()
                        .border_color(channel_hover())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text_primary())
                                .child(acc.name.clone()),
                        ),
                );
            }
            acc_col = acc_col.child(acc_grid);
            col = col.child(acc_col);
        }
    }

    col.into_any_element()
}
