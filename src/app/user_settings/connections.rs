use gpui::prelude::*;
use gpui::*;
use gpui::component::switch::Switch;

use crate::app::state::AppState;
use crate::components::button::IconButton;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_section_description;

pub fn render_connections(
    app_entity: Entity<AppState>,
    app_state: &AppState,
    cx: &App,
) -> Div {
    let connections = &app_state.connections;
    let current_username = app_state.current_user.username.clone();

    // 10 official third-party providers from Discord
    let providers = [
        ("github", "GitHub"),
        ("spotify", "Spotify"),
        ("steam", "Steam"),
        ("twitch", "Twitch"),
        ("youtube", "YouTube"),
        ("playstation", "PlayStation Network"),
        ("xbox", "Xbox Live"),
        ("battlenet", "Battle.net"),
        ("reddit", "Reddit"),
        ("tiktok", "TikTok"),
    ];

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_section_description(
            "These are accounts from other platforms. Connecting them here can unlock new features on Discord, like the ability to share what you're currently listening to, get paid safely and securely, and more.",
            cx,
        ))
        // -------------------------------------------------------------
        // Grid of Providers (Add a new connection)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.0))
                        .child("Add a new connection"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap(px(10.0))
                        .children(providers.into_iter().map(|(platform, display_name)| {
                            let is_connected = connections.iter().any(|c| c.platform == platform);
                            let entity = app_entity.clone();
                            let default_user = current_username.clone();

                            let card_bg = cx.discord().bg_channels;
                            let hover_bg = cx.discord().channel_hover;
                            let border_color = if is_connected {
                                cx.discord().status_online
                            } else {
                                cx.discord().neutral_67
                            };

                            div()
                                .w(px(210.0))
                                .h(px(52.0))
                                .px(px(12.0))
                                .rounded(px(6.0))
                                .bg(card_bg)
                                .border_1()
                                .border_color(border_color)
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover_bg))
                                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                    let def = default_user.clone();
                                    entity.update(cx, |s, cx| {
                                        if is_connected {
                                            s.disconnect_platform(platform);
                                        } else {
                                            s.connect_platform(platform, &def);
                                        }
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(10.0))
                                        .child(render_connection_badge(platform, px(28.0), cx))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_color(cx.discord().text_titles)
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_size(px(13.0))
                                                        .child(display_name),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(if is_connected {
                                                            cx.discord().status_online
                                                        } else {
                                                            cx.discord().text_secondary
                                                        })
                                                        .text_size(px(11.0))
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap(px(3.0))
                                                        .children(if is_connected {
                                                            Some(icon_check(px(10.0), cx.discord().status_online))
                                                        } else {
                                                            None
                                                        })
                                                        .child(if is_connected { "Connected" } else { "Connect" }),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .size(px(18.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(if is_connected {
                                            icon_close(px(12.0), cx.discord().text_secondary).into_any_element()
                                        } else {
                                            icon_plus(px(12.0), cx.discord().text_secondary).into_any_element()
                                        }),
                                )
                        })),
                ),
        )
        // -------------------------------------------------------------
        // Active Linked Accounts List
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.0))
                        .child(format!("{} connections", connections.len())),
                )
                .children(if connections.is_empty() {
                    vec![
                        div()
                            .p(px(24.0))
                            .rounded(px(8.0))
                            .bg(cx.discord().bg_channels)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(cx.discord().text_secondary)
                            .text_size(px(13.0))
                            .child("No accounts connected.")
                    ]
                } else {
                    connections.iter().map(|conn| {
                        let platform = conn.platform.clone();
                        let plat_for_prof = platform.clone();
                        let plat_for_act = platform.clone();
                        let plat_for_disc = platform.clone();
                        let entity_prof = app_entity.clone();
                        let entity_act = app_entity.clone();
                        let entity_disc = app_entity.clone();
                        let is_spotify = conn.platform == "spotify";

                        div()
                            .p(px(16.0))
                            .rounded(px(8.0))
                            .bg(cx.discord().bg_channels)
                            .border_1()
                            .border_color(cx.discord().neutral_67)
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            // Card Top Bar
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(render_connection_badge(&conn.platform, px(36.0), cx))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .text_color(cx.discord().text_titles)
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(15.0))
                                                            .child(conn.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(cx.discord().text_secondary)
                                                            .text_size(px(12.0))
                                                            .child(conn.platform_display_name()),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        IconButton::with_icon(
                                            ElementId::NamedInteger("btn-disc-conn".into(), conn.id.len() as u64),
                                            |size, color| icon_close(size, color).into_any_element(),
                                        )
                                        .tooltip("Disconnect")
                                        .on_click(move |_ev, _win, cx| {
                                            let plat = plat_for_disc.clone();
                                            entity_disc.update(cx, |s, cx| {
                                                s.disconnect_platform(&plat);
                                                cx.notify();
                                            });
                                        })
                                    ),
                            )
                            // Toggle 1: Display on profile
                            .child(
                                div()
                                    .pt(px(4.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_color(cx.discord().text_primary)
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(px(13.0))
                                            .child("Display on profile"),
                                    )
                                    .child(
                                        Switch::new(ElementId::NamedInteger("sw-prof".into(), conn.id.len() as u64))
                                            .checked(conn.show_on_profile)
                                            .on_click(move |_checked, _win, cx| {
                                                let plat = plat_for_prof.clone();
                                                entity_prof.update(cx, |s, cx| {
                                                    s.toggle_connection_profile(&plat);
                                                    cx.notify();
                                                });
                                            })
                                    ),
                            )
                            // Toggle 2: Display Spotify as your status (apenas Spotify)
                            .children(if is_spotify {
                                Some(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_color(cx.discord().text_primary)
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(px(13.0))
                                                .child("Display Spotify as your status"),
                                        )
                                        .child(
                                            Switch::new("sw-spotify-act")
                                                .checked(conn.show_activity)
                                                .on_click(move |_checked, _win, cx| {
                                                    let plat = plat_for_act.clone();
                                                    entity_act.update(cx, |s, cx| {
                                                        s.toggle_connection_activity(&plat);
                                                        cx.notify();
                                                    });
                                                })
                                        ),
                                )
                            } else {
                                None
                            })
                    }).collect()
                }),
        )
}

pub fn render_connection_badge(platform: &str, size: Pixels, cx: &App) -> Div {
    let (bg, label) = match platform {
        "github" => (rgb(0x24292e).into(), "GH"), /* COR ANTIGA SEM ORIGEM */
        "spotify" => (cx.discord().platform_spotify, "SP"),
        "steam" => (rgb(0x171a21).into(), "ST"), /* COR ANTIGA SEM ORIGEM */
        "twitch" => (rgb(0x9146ff).into(), "TW"), /* COR ANTIGA SEM ORIGEM */
        "youtube" => (rgb(0xff0000).into(), "YT"), /* COR ANTIGA SEM ORIGEM */
        "playstation" => (rgb(0x003791).into(), "PS"), /* COR ANTIGA SEM ORIGEM */
        "xbox" => (cx.discord().platform_xbox, "XB"),
        "battlenet" => (rgb(0x00aeef).into(), "BN"), /* COR ANTIGA SEM ORIGEM */
        "reddit" => (cx.discord().platform_reddit, "RD"),
        "tiktok" => (rgb(0xfe2c55).into(), "TT"), /* COR ANTIGA SEM ORIGEM */
        _ => (cx.discord().blurple, "??"),
    };

    div()
        .size(size)
        .rounded(px(6.0))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .text_color(cx.discord().white)
        .font_weight(FontWeight::BOLD)
        .text_size(px(11.0))
        .child(label)
}
