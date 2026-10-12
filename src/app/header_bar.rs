use gpui::prelude::*;
use gpui::*;

use crate::components::{Avatar, AvatarSize, IconButton};
use crate::tokens::colors::{self, border_subtle};
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;
use super::state::{ActiveModal, AppState, ChannelItemState};

/// Chat and channel top header bar (49px high: 48px content + 1px subtle bottom border)
pub struct HeaderBar {
    state: Entity<AppState>,
}

impl HeaderBar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }
}

impl Render for HeaderBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected_channel_opt = if let Some((_, ch)) = state.get_selected_channel() {
            Some(ch.clone())
        } else if let Some(ref cid) = state.selected_channel_id {
            state.dm_channels.iter().find(|d| &d.id == cid).cloned()
        } else {
            None
        };
        let is_dm = state.selected_guild_id.is_none();
        let app_entity = self.state.clone();

        div()
            .h(px(49.0))
            .w_full()
            .bg(cx.discord().bg_chat)
            .pl(px(16.0))
            .pr(px(8.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(border_subtle())
            .flex_shrink_0()
            .child(render_channel_left_side(
                selected_channel_opt.as_ref(),
                is_dm,
                state,
                cx,
            ))
            .child(render_toolbar(app_entity, state, cx))
    }
}

fn render_channel_left_side(
    selected_channel: Option<&ChannelItemState>,
    is_dm: bool,
    state: &AppState,
    cx: &App,
) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .overflow_hidden();

    if let Some(channel) = selected_channel {
        if is_dm {
            row = row.child(render_dm_avatar(channel, state, cx));
            row = row.child(render_title(&channel.name, cx));
            if let Some(topic_el) = render_topic(channel.topic.as_deref(), cx) {
                row = row.child(topic_el);
            }
        } else {
            row = row.child(render_guild_channel_icon(channel, cx));
            row = row.child(render_title(&channel.name, cx));
            if let Some(topic_el) = render_topic(channel.topic.as_deref(), cx) {
                row = row.child(topic_el);
            }
        }
    } else if is_dm {
        row = row.children(render_friends_header_items(cx));
    } else {
        row = row.children(render_no_channel_header_items(cx));
    }

    row.into_any_element()
}

fn render_dm_avatar(channel: &ChannelItemState, state: &AppState, cx: &App) -> AnyElement {
    let recipient_id = channel.recipient_ids.first();
    let recipient_friend = state.friends.iter().find(|f| {
        if let Some(rid) = recipient_id {
            f.id == *rid
        } else {
            f.username == channel.name || f.display_name == channel.name
        }
    });

    let avatar_src = channel
        .icon_url
        .as_deref()
        .or_else(|| recipient_friend.and_then(|f| f.avatar_url.as_deref()));

    let presence_status = recipient_id
        .and_then(|rid| state.get_user_presence(rid))
        .or_else(|| recipient_friend.and_then(|f| state.get_user_presence(&f.id)))
        .map(|p| p.status)
        .or_else(|| recipient_friend.map(|f| crate::domain::user::PresenceStatus::from(f.presence)));

    Avatar::new(channel.name.clone())
        .size(AvatarSize::Px24)
        .opt_src(avatar_src.map(|s| s.to_string()))
        .opt_status(presence_status)
        .opt_decoration(channel.avatar_decoration_data.clone())
        .cutout_color(cx.discord().bg_chat)
        .into_any_element()
}

fn render_guild_channel_icon(channel: &ChannelItemState, cx: &App) -> AnyElement {
    render_icon(channel.kind.discord_icon(), px(20.0), cx.discord().text_secondary).into_any_element()
}

fn render_title(name: &str, cx: &App) -> AnyElement {
    div()
        .text_color(cx.discord().text_titles)
        .font_weight(FontWeight::BOLD)
        .text_size(px(16.0))
        .child(name.to_string())
        .into_any_element()
}

fn render_topic(topic: Option<&str>, cx: &App) -> Option<AnyElement> {
    let topic = topic?;
    let discord = cx.discord();
    Some(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(1.0))
                    .h(px(16.0))
                    .bg(discord.borders)
                    .mx(px(4.0)),
            )
            .child(
                div()
                    .text_color(discord.text_secondary)
                    .text_size(px(13.0))
                    .overflow_hidden()
                    .child(topic.to_string()),
            )
            .into_any_element(),
    )
}

fn render_friends_header_items(cx: &App) -> Vec<AnyElement> {
    let discord = cx.discord();
    vec![
        icon_friends(px(20.0), discord.text_secondary).into_any_element(),
        div()
            .text_color(discord.text_titles)
            .font_weight(FontWeight::BOLD)
            .text_size(px(16.0))
            .child("Amigos")
            .into_any_element(),
        div()
            .w(px(1.0))
            .h(px(16.0))
            .bg(discord.borders)
            .mx(px(4.0))
            .into_any_element(),
        div()
            .text_color(discord.text_secondary)
            .text_size(px(13.0))
            .child("Disponível")
            .into_any_element(),
    ]
}

fn render_no_channel_header_items(cx: &App) -> Vec<AnyElement> {
    let discord = cx.discord();
    vec![
        div()
            .text_color(discord.text_secondary)
            .text_size(px(16.0))
            .child("#")
            .into_any_element(),
        div()
            .text_color(discord.text_titles)
            .font_weight(FontWeight::BOLD)
            .text_size(px(16.0))
            .child("Selecione um canal")
            .into_any_element(),
    ]
}

fn render_toolbar(app_entity: Entity<AppState>, state: &AppState, cx: &App) -> AnyElement {
    let show_members = state.show_members;
    let mut toolbar = div()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(8.0))
        .gap(px(8.0));

    // 0. Discord Connection Shortcut Button (only when disconnected)
    if !state.is_connected_to_gateway {
        toolbar = toolbar.child(render_login_button(app_entity.clone(), cx));
    }

    let is_dm_channel = state.selected_guild_id.is_none() && state.selected_channel_id.is_some();

    // Voice and Video call buttons (Only in DM / Group DM)
    if is_dm_channel {
        let channel_id = state.selected_channel_id.clone().unwrap_or_default();
        let channel_name = state
            .dm_channels
            .iter()
            .find(|d| d.id == channel_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "DM".into());
        let recipient_ids = state
            .dm_channels
            .iter()
            .find(|d| d.id == channel_id)
            .map(|d| d.recipient_ids.clone())
            .unwrap_or_default();

        let voice_entity = app_entity.clone();
        let cid_v = channel_id.clone();
        let cname_v = channel_name.clone();
        let recips_v = recipient_ids.clone();
        toolbar = toolbar.child(
            IconButton::with_icon("header-btn-voice-call", |sz, col| {
                icon_phone(sz, col).into_any_element()
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(px(4.0))
            .tooltip("Iniciar chamada de voz")
            .on_click(move |_event, _window, cx| {
                let cm_opt = voice_entity.read(cx).client_manager.clone();
                let cid = cid_v.clone();
                let cname = cname_v.clone();
                let recips = if recips_v.is_empty() {
                    None
                } else {
                    Some(recips_v.clone())
                };
                let entity = voice_entity.clone();

                cx.spawn(|cx: &mut AsyncApp| {
                    let cx = cx.clone();
                    async move {
                        if let Some(cm) = cm_opt {
                            match cm.ring_call(&cid, recips).await {
                                Ok(()) => {
                                    let _ = cx.update(|cx| {
                                        entity.update(cx, |state, cx| {
                                            state.connect_voice(&cid, &cname, "DM");
                                            cx.notify();
                                        });
                                    });
                                }
                                Err(e) => {
                                    log::error!("[HeaderBar] Falha ao iniciar chamada (ring_call recusado): {:?}", e);
                                }
                            }
                        }
                    }
                })
                .detach();
            }),
        );

        let video_entity = app_entity.clone();
        let cid_vid = channel_id.clone();
        let cname_vid = channel_name.clone();
        let recips_vid = recipient_ids;
        toolbar = toolbar.child(
            IconButton::with_icon("header-btn-video-call", |sz, col| {
                render_video_camera_icon(sz, col)
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(px(4.0))
            .tooltip("Iniciar chamada de vídeo")
            .on_click(move |_event, _window, cx| {
                let cm_opt = video_entity.read(cx).client_manager.clone();
                let cid = cid_vid.clone();
                let cname = cname_vid.clone();
                let recips = if recips_vid.is_empty() {
                    None
                } else {
                    Some(recips_vid.clone())
                };
                let entity = video_entity.clone();

                cx.spawn(|cx: &mut AsyncApp| {
                    let cx = cx.clone();
                    async move {
                        if let Some(cm) = cm_opt {
                            match cm.ring_call(&cid, recips).await {
                                Ok(()) => {
                                    let _ = cx.update(|cx| {
                                        entity.update(cx, |state, cx| {
                                            state.connect_voice(&cid, &cname, "DM");
                                            state.toggle_camera();
                                            cx.notify();
                                        });
                                    });
                                }
                                Err(e) => {
                                    log::error!("[HeaderBar] Falha ao iniciar chamada de vídeo (ring_call recusado): {:?}", e);
                                }
                            }
                        }
                    }
                })
                .detach();
            }),
        );
    } else {
        // 1. Notifications / Bell button (Guild text/voice channels)
        toolbar = toolbar.child(
            IconButton::with_icon("header-btn-notifications", |sz, col| {
                icon_bell(sz, col).into_any_element()
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(px(4.0))
            .tooltip("Notification Settings"),
        );
    }

    // 2. Pinned messages button
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-pinned", |sz, col| {
            icon_pin(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .tooltip("Pinned Messages"),
    );

    // 3. Member list toggle button
    let members_entity = app_entity.clone();
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-members", |sz, col| {
            icon_members(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .active(show_members)
        .tooltip(if show_members {
            "Hide Member List"
        } else {
            "Show Member List"
        })
        .on_click(move |_event, _window, cx| {
            members_entity.update(cx, |state, cx| {
                state.toggle_members();
                cx.notify();
            });
        }),
    );

    // 4. Search bar (Buscar)
    toolbar = toolbar.child(render_search_box(
        app_entity,
        state.is_search_open,
        state.search_query.clone(),
        cx,
    ));

    // 5. Inbox button
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-inbox", |sz, col| {
            icon_inbox(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .tooltip("Inbox"),
    );

    // 6. Help button
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-help", |sz, col| {
            icon_info(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .tooltip("Help"),
    );

    toolbar.into_any_element()
}

fn render_login_button(entity: Entity<AppState>, cx: &App) -> AnyElement {
    let discord = cx.discord();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .h(px(28.0))
        .rounded(px(4.0))
        .bg(discord.blurple)
        .hover(|s| s.bg(colors::blurple_hover()))
        .cursor_pointer()
        .text_color(discord.white)
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.open_modal(ActiveModal::Login);
                cx.notify();
            });
        })
        .child(icon_key(px(14.0), discord.white))
        .child("Entrar no Discord")
        .into_any_element()
}

fn render_search_box(
    entity: Entity<AppState>,
    is_search_open: bool,
    query: String,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    div()
        .w(px(144.0))
        .h(px(24.0))
        .px(px(6.0))
        .rounded(px(4.0))
        .bg(if is_search_open {
            discord.channel_hover
        } else {
            discord.bg_server
        })
        .border_1()
        .border_color(if is_search_open {
            discord.blurple
        } else {
            discord.bg_server
        })
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .hover(|s| s.bg(colors::channel_hover()))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.toggle_search();
                cx.notify();
            });
        })
        .child(
            div()
                .text_color(if !query.is_empty() {
                    discord.text_titles
                } else {
                    discord.text_secondary
                })
                .text_size(px(12.0))
                .child(if !query.is_empty() {
                    query
                } else {
                    "Buscar".to_string()
                }),
        )
        .child(icon_search(
            px(14.0),
            if is_search_open {
                discord.white
            } else {
                discord.text_secondary
            },
        ))
        .into_any_element()
}

fn render_video_camera_icon(size: Pixels, color: Rgba) -> AnyElement {
    div()
        .size(size)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(
                    div()
                        .w(px(10.0))
                        .h(px(8.0))
                        .rounded(px(1.5))
                        .bg(color),
                )
                .child(
                    div()
                        .w(px(3.0))
                        .h(px(6.0))
                        .rounded(px(1.0))
                        .bg(color),
                ),
        )
        .into_any_element()
}

