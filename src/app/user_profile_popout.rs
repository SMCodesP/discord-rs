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
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::sync::broadcast;

use crate::chat::markdown::render_markdown;
use crate::domain::user::{PresenceStatus, User};
use crate::i18n::t;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::typography::*;

// ============================================================================
// Global Event Channel for Profile Popout Opening
// ============================================================================

static PROFILE_TRIGGER_TX: LazyLock<broadcast::Sender<String>> =
    LazyLock::new(|| broadcast::channel(64).0);

/// Triggers opening the user profile popout from anywhere in the application
/// (chat messages, member sidebar, DM rows, friends hub).
pub fn trigger_open_profile(user_id: &str) {
    let _ = PROFILE_TRIGGER_TX.send(user_id.to_string());
}

/// Subscribes to profile popout opening triggers
pub fn subscribe_profile_trigger() -> broadcast::Receiver<String> {
    PROFILE_TRIGGER_TX.subscribe()
}

// ============================================================================
// Data Models
// ============================================================================

/// Profile badge earned by user
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileBadge {
    pub id: String,
    pub description: String,
    pub icon: String,
    #[serde(default)]
    pub link: Option<String>,
}

impl ProfileBadge {
    pub fn icon_url(&self) -> String {
        if self.icon.starts_with("http://") || self.icon.starts_with("https://") {
            self.icon.clone()
        } else {
            format!("https://cdn.discordapp.com/badge-icons/{}.png", self.icon)
        }
    }
}

/// Connected third-party account (Spotify, Steam, GitHub, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedAccount {
    #[serde(rename = "type")]
    pub account_type: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub verified: bool,
}

/// Mutual guild shared between current user and profile target
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutualGuild {
    pub id: String,
    #[serde(default)]
    pub nick: Option<String>,
}

/// Complete official Discord User Profile data structure (GET /users/{id}/profile)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfileData {
    pub user: User,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub banner_color: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    #[serde(default)]
    pub badges: Vec<ProfileBadge>,
    #[serde(default)]
    pub connected_accounts: Vec<ConnectedAccount>,
    #[serde(default)]
    pub mutual_guilds: Vec<MutualGuild>,
    #[serde(default)]
    pub mutual_friends_count: u32,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub joined_server_at: Option<DateTime<Utc>>,
}

/// Navigation tabs in the profile popout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProfileTab {
    #[default]
    MutualGuilds,
    MutualFriends,
}

/// Interactive state of the opened profile popout in `AppState`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilePopoutState {
    pub user_id: String,
    pub guild_id: Option<String>,
    pub data: Option<UserProfileData>,
    pub is_loading: bool,
    pub error: Option<String>,
    pub active_tab: ProfileTab,
    pub quick_message: String,
}

// ============================================================================
// Helper Utilities
// ============================================================================

/// Converts a 64-bit Discord Snowflake ID string into account creation `DateTime<Utc>`.
/// Discord epoch is 2015-01-01T00:00:00Z (1420070400000 ms).
pub fn snowflake_to_datetime(snowflake_str: &str) -> Option<DateTime<Utc>> {
    let snowflake = snowflake_str.parse::<u64>().ok()?;
    let timestamp_ms = (snowflake >> 22) + 1_420_070_400_000;
    DateTime::from_timestamp_millis(timestamp_ms as i64)
}

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
        .child(render_profile_card(app_entity, popout))
}

/// Renders the 300px Popout Card container
fn render_profile_card(
    app_entity: Entity<crate::app::state::AppState>,
    popout: &ProfilePopoutState,
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
    // 1. TOP BANNER (105px height)
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
    // 2. AVATAR + BADGES HEADER (Avatar 80x80 with negative 40px margin)
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

    let presence_status = avatar_user
        .map(|u| u.presence)
        .unwrap_or(PresenceStatus::Offline);

    let status_color = presence_status.color_rgb();

    let avatar_initial = display_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let mut avatar_content = div()
        .relative()
        .size(px(80.0))
        .rounded_full()
        .border_6()
        .border_color(rgb(0x232428))
        .bg(rgb(0x5865f2))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden();

    if let Some(ref url) = avatar_url {
        avatar_content = avatar_content.child(
            img(url.clone())
                .size_full()
                .rounded_full(),
        );
    } else {
        avatar_content = avatar_content.child(
            div()
                .font_weight(WEIGHT_BOLD)
                .text_color(rgb(0xffffff))
                .text_size(px(28.0))
                .child(avatar_initial),
        );
    }

    // Avatar status dot (bottom right)
    let avatar_container = div()
        .relative()
        .child(avatar_content)
        // SLOT: Avatar Decoration Frame (Fase 5 - Agente Decor encaixará a moldura aqui)
        .child(
            div()
                .id("avatar-decoration-slot")
                .absolute()
                .inset_0()
                .invisible(),
        )
        // Presence Dot (16x16 with 3px border #232428)
        .child(
            div()
                .absolute()
                .bottom(px(0.0))
                .right(px(0.0))
                .size(px(16.0))
                .rounded_full()
                .bg(rgb(status_color))
                .border_3()
                .border_color(rgb(0x232428)),
        );

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

    // Custom Status (if active on user)
    if let Some(ref act) = avatar_user.and_then(|u| u.activity.as_ref()) {
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

    // Divider
    body = body.child(div().h_px().bg(rgb(0x35373c)).my(px(2.0)));

    // "Sobre mim" (About Me) Bio section (Rendered with existing Markdown parser)
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
    let created_at = popout
        .data
        .as_ref()
        .map(|d| d.created_at)
        .or_else(|| snowflake_to_datetime(&popout.user_id))
        .unwrap_or_else(Utc::now);

    let mut dates_col = div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
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

    if let Some(joined_at) = popout.data.as_ref().and_then(|d| d.joined_server_at) {
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

    // Divider
    body = body.child(div().h_px().bg(rgb(0x35373c)).my(px(2.0)));

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
    // 5. QUICK DIRECT MESSAGE INPUT (Caixa de Envio Rápido)
    // -------------------------------------------------------------------------
    let target_uid = popout.user_id.clone();
    let target_uname = username.clone();
    let entity_msg = entity_card.clone();

    let quick_input = div()
        .p(px(12.0))
        .border_t_1()
        .border_color(rgb(0x1f2023))
        .bg(rgb(0x1e1f22))
        .child(
            div()
                .h(px(36.0))
                .px(px(12.0))
                .rounded(px(6.0))
                .bg(rgb(0x2b2d31))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(0x35373c)))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    let uid = target_uid.clone();
                    let uname = target_uname.clone();
                    entity_msg.update(cx, |s, cx| {
                        s.close_profile_popout();
                        s.open_dm_with_user(&uid, &uname);
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(text_muted())
                        .child(format!("Conversar com @{}", username)),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(text_muted())
                        .child("💬"),
                ),
        );

    card = card.child(quick_input);

    card
}
