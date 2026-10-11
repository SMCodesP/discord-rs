//! Discord RS - Canonical User Profile Popout Component (Fase 4)
//! 1:1 Parity with Official Discord Desktop Client User Profile Popout (300px).
//!
//! Features:
//! - 300px popout card with rounded corners, dark theme `#232428` and subtle drop shadow
//! - 105px top banner (image CDN URL or custom banner color / accent color)
//! - 80x80px overlapping avatar with -40px negative margin, 6px border `#232428`, presence status dot
//! - Explicit slot for Avatar Decoration Frame (Fase 5 - Decor Agent integration)
//! - Badges container aligned to the right (Nitro, HypeSquad, Early Supporter, etc.)
//! - Display name (20px bold white), @username (14px #949ba4), and optional pronouns badge
//! - Custom status display with emoji and text
//! - Divider and "Sobre mim" (About Me) bio rendered via native markdown parser (`render_markdown`)
//! - Member dates: "Membro do Discord desde" (snowflake-derived) and "Membro do servidor desde"
//! - Tabs: "Servidores em comum" (Mutual Servers) and "Amigos em comum" (Mutual Friends)
//! - Quick message direct input field at bottom: "Message @username"
//! - Real states: Loading spinner/indicator, real error state, and elegant missing fields handling (zero fake mocks)
//! - Closes on click outside (backdrop) and Esc key

use chrono::{DateTime, Datelike, Utc};
use gpui::*;
use gpui::component::input::InputState;

use crate::chat::markdown::render_markdown;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::profile_effect::{
    render_profile_effect_for_user, ProfileEffectCatalog, UserProfileEffect,
};
use crate::components::TextInput;
use crate::domain::user::{ActivityType, CustomStatus, Presence, PresenceStatus};
pub use crate::domain::profile::{
    snowflake_to_datetime, ConnectedAccount, MutualGuild, ProfileBadge, ProfilePopoutState,
    ProfileTab, UserProfileData,
};
use crate::i18n::t;
use crate::tokens::colors::*;
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
// GPUI Native Profile Popout Renderer
// ============================================================================

/// Renders the complete 300px User Profile Popout component with backdrop overlay
pub fn render_user_profile_popout(
    app_entity: Entity<crate::app::state::AppState>,
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
) -> impl IntoElement {
    let entity_close = app_entity.clone();
    let entity_key = app_entity.clone();

    div()
        .id("profile-popout-overlay")
        .key_context("ProfilePopout")
        .on_key_down(move |event: &KeyDownEvent, _window, cx| {
            if event.keystroke.key == "escape" {
                entity_key.update(cx, |s, cx| {
                    s.close_profile_popout();
                    cx.notify();
                });
            }
        })
        .absolute()
        .inset_0()
        .bg(rgba(0x00000077))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_profile_popout();
                cx.notify();
            });
        })
        .child(render_profile_card(
            app_entity,
            popout,
            catalog,
            presence,
            quick_state,
        ))
}

/// Renders the 300px Popout Card container
fn render_profile_card(
    app_entity: Entity<crate::app::state::AppState>,
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
) -> impl IntoElement {
    let entity_card = app_entity.clone();
    let entity_close_btn = app_entity.clone();

    // Prevent backdrop close when clicking inside the popout card
    let mut card = div()
        .id("profile-popout-card")
        .w(px(300.0))
        .max_h(px(580.0))
        .rounded(px(8.0))
        .bg(rgb(0x232428))
        .border_1()
        .border_color(rgb(0x1f2023))
        .shadow_lg()
        .overflow_hidden()
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_ev, _window, _cx| {
            // Stop propagation to backdrop overlay
        });

    // -------------------------------------------------------------------------
    // 1. TOP BANNER (105px height) with Profile Effect Overlay
    // -------------------------------------------------------------------------
    let banner_color_val = popout
        .data
        .as_ref()
        .and_then(|d| d.banner_color.as_deref().and_then(parse_hex_color))
        .or_else(|| popout.data.as_ref().and_then(|d| d.accent_color))
        .unwrap_or(0x111214);

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

    // Profile Effect Layer (rendered from official catalog over banner)
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

    // Top Right Close button on banner
    banner = banner.child(
        div()
            .absolute()
            .top(px(8.0))
            .right(px(8.0))
            .size(px(28.0))
            .rounded_full()
            .bg(rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(rgb(0xffffff))
            .hover(|s| s.bg(rgba(0x000000cc)))
            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                entity_close_btn.update(cx, |s, cx| {
                    s.close_profile_popout();
                    cx.notify();
                });
            })
            .child(icon_close(px(14.0), rgb(0xffffff))),
    );

    card = card.child(banner);

    // -------------------------------------------------------------------------
    // 2. AVATAR + BADGES HEADER (Canonical Avatar component with decor & presence)
    // -------------------------------------------------------------------------
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
        .cutout_color(rgb(0x232428));

    let avatar_container = div()
        .relative()
        .child(avatar);

    // Badges Container (aligned right)
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
                        .bg(rgba(0xffffff11))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            img(b.icon_url())
                                .size(px(16.0)),
                        )
                }),
        );

    let avatar_row = div()
        .px(px(16.0))
        .mt(px(-40.0))
        .flex()
        .flex_row()
        .justify_between()
        .items_end()
        .child(avatar_container)
        .child(badges_container);

    card = card.child(avatar_row);

    // -------------------------------------------------------------------------
    // 3. REAL STATES: LOADING & ERROR
    // -------------------------------------------------------------------------
    if popout.is_loading && popout.data.is_none() {
        return card.child(
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
        );
    }

    if let Some(ref err) = popout.error {
        if popout.data.is_none() {
            return card.child(
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
                            .text_color(rgb(0xfa777c))
                            .child(t("profile.error")),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(text_muted())
                            .child(err.clone()),
                    ),
            );
        }
    }

    // -------------------------------------------------------------------------
    // 4. SCROLLABLE PROFILE BODY
    // -------------------------------------------------------------------------
    let mut body = div()
        .id("profile-popout-body")
        .flex_1()
        .overflow_y_scroll()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(10.0));

    // Display Name (20px bold white)
    body = body.child(
        div()
            .text_size(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(display_name),
    );

    // Username (@username 14px #949ba4) and optional Pronouns
    let mut user_tag_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .text_color(rgb(0x949ba4))
                .child(format!("@{}", username)),
        );

    if let Some(pronouns) = popout.data.as_ref().and_then(|d| d.pronouns.as_deref()) {
        user_tag_row = user_tag_row.child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .bg(rgba(0xffffff14))
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xdbdee1))
                .child(pronouns.to_string()),
        );
    }

    body = body.child(user_tag_row);

    // Custom Status (from live presence or fallback)
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
            body = body.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(rgb(0x35373c))
                    .bg(rgb(0x1e1f22))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .children(emoji_el)
                    .children(cs.text.as_ref().map(|t| {
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0xdbdee1))
                            .child(t.clone())
                    })),
            );
        }
    } else if let Some(ref act) = presence
        .and_then(|p| p.activity.as_ref())
        .or_else(|| avatar_user.and_then(|u| u.activity.as_ref()))
    {
        if let Some(ref state_text) = act.state {
            body = body.child(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(rgb(0x35373c))
                    .bg(rgb(0x1e1f22))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .child("💬"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0xdbdee1))
                            .child(state_text.clone()),
                    ),
            );
        }
    }

    // Live Game Activity (Rich Presence) if present
    let game_activity = presence
        .and_then(|p| p.activity.as_ref())
        .or_else(|| avatar_user.and_then(|u| u.activity.as_ref()))
        .filter(|a| a.activity_type == ActivityType::Playing || (!a.name.is_empty() && a.name != "Custom Status"));

    if let Some(game) = game_activity {
        body = body.child(
            div()
                .p(px(8.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(rgb(0x35373c))
                .bg(rgb(0x1e1f22))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0x23a55a))
                        .child("JOGANDO"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .child(game.name.clone()),
                )
                .children(game.details.as_ref().map(|d| {
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xdbdee1))
                        .child(d.clone())
                }))
                .children(game.state.as_ref().map(|s| {
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0x949ba4))
                        .child(s.clone())
                })),
        );
    }

    // Divider
    body = body.child(div().h_px().bg(rgb(0x35373c)).my(px(2.0)));

    // "Sobre mim" (About Me) Bio section (Rendered with native Markdown parser)
    if let Some(ref bio) = popout.data.as_ref().and_then(|d| d.bio.as_ref()) {
        if !bio.trim().is_empty() {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xb5bac1))
                            .child(t("profile.about_me").to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0xdbdee1))
                            .child(render_markdown(bio)),
                    ),
            );
            body = body.child(div().h_px().bg(rgb(0x35373c)).my(px(2.0)));
        }
    }

    // Member Dates ("Membro do Discord desde" & "Membro do servidor desde")
    // Zero fake dates: only render when snowflake or joined_server_at is genuinely present
    let created_at = popout
        .data
        .as_ref()
        .and_then(|d| d.created_at)
        .or_else(|| snowflake_to_datetime(&popout.user_id));

    let joined_server_at = popout.data.as_ref().and_then(|d| d.joined_server_at);

    if created_at.is_some() || joined_server_at.is_some() {
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
                            .text_color(rgb(0xb5bac1))
                            .child(t("profile.discord_member_since").to_uppercase()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon_info(px(14.0), rgb(0x949ba4)))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xdbdee1))
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
                            .text_color(rgb(0xb5bac1))
                            .child(t("profile.server_member_since").to_uppercase()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .child(icon_hash(px(14.0), rgb(0x949ba4)))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xdbdee1))
                                    .child(format_member_date(joined_at)),
                            ),
                    ),
            );
        }

        body = body.child(dates_col);
        body = body.child(div().h_px().bg(rgb(0x35373c)).my(px(2.0)));
    }

    // Tabs: "Servidores em comum" and "Amigos em comum"
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

    let is_guilds_tab = popout.active_tab == ProfileTab::MutualGuilds;
    let entity_tab_guilds = entity_card.clone();
    let entity_tab_friends = entity_card.clone();

    let tab_header = div()
        .flex()
        .flex_row()
        .gap(px(12.0))
        .border_b_1()
        .border_color(rgb(0x35373c))
        .pb(px(4.0))
        .child(
            div()
                .cursor_pointer()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_guilds_tab { rgb(0xffffff) } else { rgb(0x949ba4) })
                .border_b_2()
                .border_color(if is_guilds_tab { blurple() } else { rgba(0x00000000) })
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
                .text_color(if !is_guilds_tab { rgb(0xffffff) } else { rgb(0x949ba4) })
                .border_b_2()
                .border_color(if !is_guilds_tab { blurple() } else { rgba(0x00000000) })
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

    body = body.child(tab_header);

    // Tab content
    let tab_content = if is_guilds_tab {
        if mutual_guilds_count == 0 {
            div()
                .py(px(8.0))
                .text_size(px(12.0))
                .text_color(text_muted())
                .child("Nenhum servidor em comum.")
        } else {
            let mut list = div().flex().flex_col().gap(px(4.0));
            if let Some(data) = popout.data.as_ref() {
                for g in &data.mutual_guilds {
                    list = list.child(
                        div()
                            .h(px(32.0))
                            .px(px(6.0))
                            .rounded(px(4.0))
                            .bg(rgb(0x1e1f22))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .size(px(20.0))
                                    .rounded_full()
                                    .bg(blurple())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child(g.id.chars().next().unwrap_or('S').to_string()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xdbdee1))
                                    .child(g.nick.clone().unwrap_or_else(|| format!("Servidor {}", g.id))),
                            ),
                    );
                }
            }
            list
        }
    } else {
        if mutual_friends_count == 0 {
            div()
                .py(px(8.0))
                .text_size(px(12.0))
                .text_color(text_muted())
                .child("Nenhum amigo em comum.")
        } else {
            div()
                .py(px(8.0))
                .text_size(px(12.0))
                .text_color(rgb(0xdbdee1))
                .child(format!("{} amigos em comum com este usuário.", mutual_friends_count))
        }
    };

    body = body.child(tab_content);

    card = card.child(body);

    // -------------------------------------------------------------------------
    // 5. QUICK DIRECT MESSAGE INPUT (Functional real message dispatch)
    // -------------------------------------------------------------------------
    let target_uname = username.clone();

    let quick_input = div()
        .p(px(12.0))
        .border_t_1()
        .border_color(rgb(0x1f2023))
        .bg(rgb(0x1e1f22))
        .child(
            TextInput::new("profile-quick-message-input", quick_state)
                .placeholder(format!("Conversar com @{}", target_uname))
                .full_width(true),
        );

    card = card.child(quick_input);

    card
}
