use gpui::*;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::domain::user::{ActivityType, PresenceStatus};
use crate::tokens::colors::text_muted;
use crate::tokens::icons::*;
use super::state::{ActiveModal, AppState, FriendsTab};

/// Channel sidebar (240px wide) containing:
/// - Server header
/// - Collapsible categories & channel lists
/// - Active voice status bar (when connected)
/// - Bottom user profile & audio controls panel
pub struct ChannelSidebar {
    state: Entity<AppState>,
}

impl ChannelSidebar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }
}

impl Render for ChannelSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected_guild = state.get_selected_guild().cloned();
        let selected_channel_id = state.selected_channel_id.clone();
        let active_voice = state.active_voice_call.clone();
        let current_user = state.current_user.clone();
        let is_dm = state.selected_guild_id.is_none();
        let app_entity = self.state.clone();

        div()
            .flex()
            .flex_col()
            .w(px(240.0))
            .h_full()
            .min_h(px(0.0))
            .bg(rgb(0x2b2d31))
            .justify_between()
            .flex_shrink_0()
            // =========================================================================
            // TOP SECTION: Header + Scrollable Channels
            // =========================================================================
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    // 1. Server Header (48px)
                    .child(if let Some(guild) = selected_guild.as_ref() {
                        div()
                            .h(px(48.0))
                            .px(px(16.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(rgb(0x1f2023))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x35373c)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .text_color(rgb(0xf2f3f5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(15.0))
                                            .child(guild.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .child(icon_check(px(14.0), rgb(0x5865f2))),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xdbdee1))
                                    .text_size(px(12.0))
                                    .child("⌵"),
                            )
                    } else {
                        // Direct Messages Header
                        div()
                            .h(px(48.0))
                            .px(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0x1f2023))
                            .child(
                                div()
                                    .w_full()
                                    .h(px(28.0))
                                    .px(px(8.0))
                                    .rounded(px(4.0))
                                    .bg(rgb(0x1e1f22))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .child(
                                        div()
                                            .text_color(rgb(0x80848e))
                                            .text_size(px(12.0))
                                            .child("Encontre ou comece uma conversa"),
                                    ),
                            )
                    })
                    // 2. Channels List / DM List
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .min_h(px(0.0))
                            .id("channels-scroll-area")
                            .overflow_y_scroll()
                            .p(px(8.0))
                            .gap(px(2.0))
                            .children(if is_dm {
                                let is_amigos_active = selected_channel_id.is_none() && state.friends_tab != FriendsTab::MessageRequests;
                                let is_msg_requests_active = selected_channel_id.is_none() && state.friends_tab == FriendsTab::MessageRequests;
                                let pending_req_count = state.message_requests.iter().filter(|r| !r.is_spam).count();

                                let mut dm_items = vec![
                                    div()
                                        .h(px(36.0))
                                        .px(px(12.0))
                                        .rounded(px(4.0))
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .cursor_pointer()
                                        .bg(if is_amigos_active { rgb(0x3f4248) } else { rgba(0x00000000) })
                                        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xdbdee1)))
                                        .text_color(if is_amigos_active { rgb(0xffffff) } else { rgb(0x80848e) })
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .on_mouse_down(MouseButton::Left, {
                                            let entity = app_entity.clone();
                                            move |_ev, _window, cx| {
                                                entity.update(cx, |s, cx| {
                                                    s.selected_channel_id = None;
                                                    s.friends_tab = FriendsTab::Online;
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(12.0))
                                                .child(icon_friends(px(20.0), if is_amigos_active { rgb(0xffffff) } else { rgb(0x80848e) }))
                                                .child("Amigos"),
                                        ),
                                    div()
                                        .h(px(36.0))
                                        .px(px(12.0))
                                        .rounded(px(4.0))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .cursor_pointer()
                                        .bg(if is_msg_requests_active { rgb(0x3f4248) } else { rgba(0x00000000) })
                                        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xdbdee1)))
                                        .text_color(if is_msg_requests_active { rgb(0xffffff) } else { rgb(0x80848e) })
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .on_mouse_down(MouseButton::Left, {
                                            let entity = app_entity.clone();
                                            move |_ev, _window, cx| {
                                                entity.update(cx, |s, cx| {
                                                    s.selected_channel_id = None;
                                                    s.friends_tab = FriendsTab::MessageRequests;
                                                    cx.notify();
                                                });
                                            }
                                        })
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(12.0))
                                                .child(icon_inbox(px(20.0), if is_msg_requests_active { rgb(0xffffff) } else { rgb(0x80848e) }))
                                                .child("Solicitações de mensagens"),
                                        )
                                        .children(if pending_req_count > 0 {
                                            Some(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.0))
                                                    .rounded_full()
                                                    .bg(rgb(0x5865f2))
                                                    .text_size(px(11.0))
                                                    .text_color(rgb(0xffffff))
                                                    .child(pending_req_count.to_string())
                                            )
                                        } else {
                                            None
                                        }),
                                    div()
                                        .h(px(36.0))
                                        .px(px(12.0))
                                        .rounded(px(4.0))
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .cursor_pointer()
                                        .text_color(rgb(0x80848e))
                                        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xdbdee1)))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(12.0))
                                                .child(icon_sparkles(px(20.0), rgb(0x80848e)))
                                                .child("Nitro"),
                                        ),
                                    div()
                                        .h(px(36.0))
                                        .px(px(12.0))
                                        .rounded(px(4.0))
                                        .flex()
                                        .items_center()
                                        .gap(px(12.0))
                                        .cursor_pointer()
                                        .text_color(rgb(0x80848e))
                                        .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xdbdee1)))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(12.0))
                                                .child(icon_gift(px(20.0), rgb(0x80848e)))
                                                .child("Loja"),
                                        ),
                                    div()
                                        .pt(px(16.0))
                                        .pb(px(4.0))
                                        .px(px(8.0))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_color(rgb(0x80848e))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(11.0))
                                                .child("MENSAGENS DIRETAS"),
                                        )
                                        .child({
                                            let entity = app_entity.clone();
                                            div()
                                                .size(px(18.0))
                                                .rounded(px(3.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .text_color(rgb(0x80848e))
                                                .hover(|s| s.text_color(rgb(0xffffff)).bg(rgb(0x35373c)))
                                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                    entity.update(cx, |s, cx| {
                                                        s.open_modal(ActiveModal::CreateGroupDm);
                                                        cx.notify();
                                                    });
                                                })
                                                .child(icon_plus(px(12.0), rgb(0x80848e)))
                                        }),
                                ];

                                if state.dm_channels.is_empty() {
                                    dm_items.push(
                                        div()
                                            .px(px(8.0))
                                            .py(px(12.0))
                                            .text_color(rgb(0x80848e))
                                            .text_size(px(12.0))
                                            .child("Nenhuma conversa direta aberta."),
                                    );
                                } else {
                                    for dm in &state.dm_channels {
                                        let dm_id = dm.id.clone();
                                        let dm_name = dm.name.clone();
                                        let is_selected = selected_channel_id.as_deref() == Some(&dm_id);
                                        let mention_count = dm.mention_count;
                                        let unread = dm.unread;
                                        let entity = app_entity.clone();
                                        let is_group = dm.is_group_dm || dm.recipient_names.len() > 1;

                                        // Resolver presença e status customizado do destinatário da DM
                                        let recipient_id = dm.recipient_ids.first();
                                        let recipient_friend = state.friends.iter().find(|f| {
                                            if let Some(rid) = recipient_id {
                                                f.id == *rid
                                            } else {
                                                f.username == dm_name || f.display_name == dm_name
                                            }
                                        });

                                        let presence_info = recipient_id
                                            .and_then(|rid| state.get_user_presence(rid))
                                            .or_else(|| {
                                                recipient_friend.and_then(|f| state.get_user_presence(&f.id))
                                            });

                                        let presence_status = presence_info
                                            .map(|p| p.status)
                                            .or_else(|| recipient_friend.map(|f| crate::domain::user::PresenceStatus::from(f.presence)))
                                            .unwrap_or(PresenceStatus::Offline);

                                        let custom_status = presence_info
                                            .and_then(|p| p.custom_status.clone())
                                            .or_else(|| {
                                                recipient_friend.and_then(|f| {
                                                    f.custom_status
                                                        .as_ref()
                                                        .map(|text| crate::domain::user::CustomStatus::from_text(text.clone()))
                                                })
                                            });

                                        let game_activity = presence_info
                                            .and_then(|p| p.activity.as_ref())
                                            .filter(|a| a.activity_type == ActivityType::Playing || (!a.name.is_empty() && a.name != "Custom Status"));

                                        let avatar_el = render_dm_avatar(
                                            is_group,
                                            &dm_name,
                                            &dm.recipient_names,
                                            dm.icon_url.as_deref(),
                                            recipient_friend.and_then(|f| f.avatar_url.as_deref()),
                                            presence_status,
                                        );

                                        let subtitle_el = if is_group {
                                            None
                                        } else {
                                            render_dm_subtitle(
                                                custom_status.as_ref(),
                                                game_activity,
                                                presence_status == PresenceStatus::Offline,
                                            )
                                        };

                                        let is_pinned = dm.is_pinned || dm.topic.as_deref().map_or(false, |t| t.contains("pin") || t.contains("fixad"));
                                        let text_col_el = render_dm_text_column(
                                            dm_name,
                                            is_selected,
                                            unread,
                                            is_pinned,
                                            subtitle_el,
                                        );

                                        let mut row = div()
                                            .h(px(42.0))
                                            .px(px(8.0))
                                            .rounded(px(4.0))
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .cursor_pointer()
                                            .bg(if is_selected {
                                                rgb(0x3f4248)
                                            } else {
                                                rgba(0x00000000)
                                            })
                                            .hover(|s| s.bg(rgb(0x35373c)))
                                            .on_mouse_down(MouseButton::Left, {
                                                let click_entity = entity.clone();
                                                let click_cid = dm_id.clone();
                                                move |_event, _window, cx| {
                                                    let cid = click_cid.clone();
                                                    click_entity.update(cx, |s, cx| {
                                                        s.select_channel(cid);
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(10.0))
                                                    .child(avatar_el)
                                                    .child(text_col_el),
                                            );

                                        if mention_count > 0 {
                                            row = row.child(
                                                div()
                                                    .min_w(px(16.0))
                                                    .h(px(16.0))
                                                    .px(px(4.0))
                                                    .rounded_full()
                                                    .bg(rgb(0xf23f43))
                                                    .text_color(rgb(0xffffff))
                                                    .text_size(px(10.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(mention_count.to_string()),
                                            );
                                        }

                                        row = row.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(4.0))
                                                .child({
                                                    let add_entity = entity.clone();
                                                    let add_cid = dm_id.clone();
                                                    div()
                                                        .size(px(20.0))
                                                        .rounded(px(4.0))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_color(rgb(0x80848e))
                                                        .hover(|s| s.text_color(rgb(0xffffff)).bg(rgb(0x35373c)))
                                                        .cursor_pointer()
                                                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                            add_entity.update(cx, |s, cx| {
                                                                s.open_modal(ActiveModal::AddFriendToGroup(add_cid.clone()));
                                                                cx.notify();
                                                            });
                                                        })
                                                        .child(icon_user(px(14.0), rgb(0x80848e)))
                                                })
                                                .child(
                                                    div()
                                                        .size(px(16.0))
                                                        .rounded_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_color(rgb(0x80848e))
                                                        .hover(|s| s.text_color(rgb(0xffffff)).bg(rgb(0x1e1f22)))
                                                        .child(icon_close(px(12.0), rgb(0x80848e))),
                                                ),
                                        );

                                        dm_items.push(row);
                                    }
                                }
                                dm_items
                            } else if let Some(guild) = selected_guild.as_ref() {
                                let mut category_elements = Vec::new();

                                for category in &guild.categories {
                                    let cat_id = category.id.clone();
                                    let cat_name = category.name.clone();
                                    let is_collapsed = state.is_category_collapsed(&cat_id);
                                    let entity = app_entity.clone();

                                    // Category Header
                                    category_elements.push(
                                        div()
                                            .pt(px(12.0))
                                            .pb(px(4.0))
                                            .px(px(4.0))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_event, _window, cx| {
                                                    let cid = cat_id.clone();
                                                    entity.update(cx, |state, cx| {
                                                        state.toggle_category(&cid);
                                                        cx.notify();
                                                    });
                                                },
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(10.0))
                                                            .text_color(rgb(0x80848e))
                                                            .child(if is_collapsed { "›" } else { "⌵" }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_color(rgb(0x80848e))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_size(px(11.0))
                                                            .child(cat_name),
                                                    ),
                                            ),
                                    );

                                    // Channels in category
                                    if !is_collapsed {
                                        for channel in &category.channels {
                                            let ch_id = channel.id.clone();
                                            let ch_name = channel.name.clone();
                                            let ch_kind = channel.kind;
                                            let is_selected = selected_channel_id.as_deref() == Some(&ch_id);
                                            let is_unread = channel.unread;
                                            let mention_count = channel.mention_count;
                                            let entity = app_entity.clone();

                                            category_elements.push(
                                                div()
                                                    .h(px(34.0))
                                                    .px(px(8.0))
                                                    .rounded(px(4.0))
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .justify_between()
                                                    .cursor_pointer()
                                                    .bg(if is_selected {
                                                        rgb(0x3f4248)
                                                    } else {
                                                        rgb(0x2b2d31)
                                                    })
                                                    .hover(|s| {
                                                        s.bg(rgb(0x35373c))
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_event, _window, cx| {
                                                            let cid = ch_id.clone();
                                                            entity.update(cx, |state, cx| {
                                                                state.select_channel(cid);
                                                                cx.notify();
                                                            });
                                                        },
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .child(
                                                                div()
                                                                    .text_size(px(14.0))
                                                                    .text_color(if is_selected {
                                                                        rgb(0xffffff)
                                                                    } else {
                                                                        rgb(0x80848e)
                                                                    })
                                                                    .child(ch_kind.symbol()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(14.0))
                                                                    .font_weight(if is_selected || is_unread {
                                                                        FontWeight::BOLD
                                                                    } else {
                                                                        FontWeight::NORMAL
                                                                    })
                                                                    .text_color(if is_selected {
                                                                        rgb(0xffffff)
                                                                    } else if is_unread {
                                                                        rgb(0xf2f3f5)
                                                                    } else {
                                                                        rgb(0x80848e)
                                                                    })
                                                                    .child(ch_name),
                                                            ),
                                                    )
                                                    .children(if mention_count > 0 {
                                                        Some(
                                                            div()
                                                                .min_w(px(16.0))
                                                                .h(px(16.0))
                                                                .px(px(4.0))
                                                                .rounded_full()
                                                                .bg(rgb(0xf23f43))
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .text_color(rgb(0xffffff))
                                                                .text_size(px(10.0))
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(mention_count.to_string()),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            );
                                        }
                                    }
                                }

                                category_elements
                            } else {
                                Vec::new()
                            }),
                    ),
            )
            // =========================================================================
            // BOTTOM SECTION: Voice Call Panel Bar + User Controls Panel
            // =========================================================================
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    // 1. Active Voice Bar (se conectada)
                    .children(if let Some(voice) = active_voice {
                        let entity = app_entity.clone();
                        Some(
                            div()
                                .h(px(52.0))
                                .px(px(12.0))
                                .bg(rgb(0x232428))
                                .border_b_1()
                                .border_color(rgb(0x1e1f22))
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            icon_wifi(px(16.0), rgb(0x23a55a)),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_color(rgb(0x23a55a))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_size(px(12.0))
                                                        .child("Voz Conectada"),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(rgb(0x80848e))
                                                        .text_size(px(11.0))
                                                        .child(format!("{} / {}ms", voice.channel_name, voice.ping_ms)),
                                                ),
                                        ),
                                )
                                // Disconnect button
                                .child(
                                    div()
                                        .w(px(28.0))
                                        .h(px(28.0))
                                        .rounded(px(4.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .text_color(rgb(0x80848e))
                                        .hover(|s| s.bg(rgb(0xf23f43)).text_color(rgb(0xffffff)))
                                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                            entity.update(cx, |state, cx| {
                                                state.disconnect_voice();
                                                cx.notify();
                                            });
                                        })
                                        .child(icon_phone_hangup(px(16.0), rgb(0x80848e))),
                                ),
                        )
                    } else {
                        None
                    })
                    // 2. User Profile Bottom Panel (52px)
                    .child({
                        let entity = app_entity.clone();
                        let is_muted = current_user.is_muted;
                        let is_deafened = current_user.is_deafened;

                        let user_tag = if let Some(ref d) = current_user.discriminator {
                            if d != "0" && d != "0000" {
                                format!("{}#{}", current_user.username, d)
                            } else {
                                format!("@{}", current_user.username)
                            }
                        } else {
                            format!("@{}", current_user.username)
                        };

                        div()
                            .h(px(52.0))
                            .px(px(8.0))
                            .bg(rgb(0x232428))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            // User Profile (Avatar + Name)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(8.0))
                                    .p(px(4.0))
                                    .rounded(px(4.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x35373c)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        {
                                            let entity = entity.clone();
                                            move |_event, _window, cx| {
                                                entity.update(cx, |state, cx| {
                                                    state.open_modal(ActiveModal::UserSettings);
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    )
                                    // Avatar with Status Dot
                                    .child({
                                        let avatar_inner = if let Some(ref url) = current_user.avatar_url {
                                            div()
                                                .size_full()
                                                .rounded_full()
                                                .overflow_hidden()
                                                .child(img(url.clone()).size_full().rounded_full())
                                        } else {
                                            div()
                                                .size_full()
                                                .rounded_full()
                                                .bg(rgb(0x5865f2))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(rgb(0xffffff))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(14.0))
                                                .child(current_user.avatar_text)
                                        };

                                        div()
                                            .relative()
                                            .size(px(32.0))
                                            .rounded_full()
                                            .flex_shrink_0()
                                            .child(avatar_inner)
                                            .child(
                                                div()
                                                    .absolute()
                                                    .bottom(px(-2.0))
                                                    .right(px(-2.0))
                                                    .size(px(10.0))
                                                    .rounded_full()
                                                    .bg(rgb(current_user.presence.color_rgb()))
                                                    .border_2()
                                                    .border_color(rgb(0x232428)),
                                            )
                                    })
                                    // Display name (bold) & username (@tag or #tag)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_color(rgb(0xf2f3f5))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(13.0))
                                                    .child(current_user.display_name),
                                            )
                                            .child(
                                                div()
                                                    .text_color(rgb(0x949ba4))
                                                    .text_size(px(11.0))
                                                    .child(user_tag),
                                            ),
                                    ),
                            )
                            // Action Buttons (Mute, Deafen, Settings)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(2.0))
                                    // Mute Button
                                    .child({
                                        let entity = entity.clone();
                                        div()
                                            .size(px(32.0))
                                            .rounded(px(4.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x35373c)))
                                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                entity.update(cx, |state, cx| {
                                                    state.toggle_mute();
                                                    cx.notify();
                                                });
                                            })
                                            .child(icon_mic(px(18.0), if is_muted { rgb(0xf23f43) } else { rgb(0xdbdee1) }))
                                    })
                                    // Deafen Button
                                    .child({
                                        let entity = entity.clone();
                                        div()
                                            .size(px(32.0))
                                            .rounded(px(4.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x35373c)))
                                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                entity.update(cx, |state, cx| {
                                                    state.toggle_deafen();
                                                    cx.notify();
                                                });
                                            })
                                            .child(icon_headphones(px(18.0), if is_deafened { rgb(0xf23f43) } else { rgb(0xdbdee1) }))
                                    })
                                    // Settings Button
                                    .child({
                                        let entity = entity.clone();
                                        div()
                                            .size(px(32.0))
                                            .rounded(px(4.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x35373c)))
                                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                entity.update(cx, |state, cx| {
                                                    state.open_modal(ActiveModal::UserSettings);
                                                    cx.notify();
                                                });
                                            })
                                            .child(icon_gear(px(18.0), rgb(0xdbdee1)))
                                    })
                            )
                    })
            )
    }
}

fn render_dm_avatar(
    is_group: bool,
    dm_name: &str,
    recipient_names: &[String],
    icon_url: Option<&str>,
    recipient_friend_avatar: Option<&str>,
    presence_status: PresenceStatus,
) -> AnyElement {
    if is_group {
        let names = if !recipient_names.is_empty() {
            recipient_names.to_vec()
        } else {
            dm_name.split(',').map(|s| s.trim().to_string()).collect()
        };
        let first_initial = names
            .first()
            .and_then(|n| n.chars().next())
            .unwrap_or('A')
            .to_uppercase()
            .to_string();
        let second_badge = if names.len() > 2 {
            format!("+{}", names.len() - 1)
        } else {
            names
                .get(1)
                .and_then(|n| n.chars().next())
                .unwrap_or('B')
                .to_uppercase()
                .to_string()
        };

        div()
            .size(px(32.0))
            .relative()
            .flex_shrink_0()
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size(px(20.0))
                    .rounded_full()
                    .bg(rgb(0x5865f2))
                    .border_2()
                    .border_color(rgb(0x2b2d31))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .child(first_initial),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(0.0))
                    .right(px(0.0))
                    .size(px(20.0))
                    .rounded_full()
                    .bg(rgb(0xeb459e))
                    .border_2()
                    .border_color(rgb(0x2b2d31))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .text_size(px(9.0))
                    .font_weight(FontWeight::BOLD)
                    .child(second_badge),
            )
            .into_any_element()
    } else {
        let avatar_src = icon_url.or(recipient_friend_avatar).map(|s| s.to_string());
        Avatar::new(dm_name.to_string())
            .size(AvatarSize::Px32)
            .opt_src(avatar_src)
            .status(presence_status)
            .cutout_color(rgb(0x2b2d31))
            .into_any_element()
    }
}

fn render_dm_subtitle(
    custom_status: Option<&crate::domain::user::CustomStatus>,
    game_activity: Option<&crate::domain::user::Activity>,
    is_offline: bool,
) -> Option<AnyElement> {
    if is_offline {
        return None;
    }

    if let Some(cs) = custom_status {
        let has_text = cs.text.as_ref().map_or(false, |t| !t.trim().is_empty());
        let has_emoji = cs.emoji.is_some();
        if has_text || has_emoji {
            let mut sub_div = div()
                .h(px(14.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .overflow_hidden();

            if let Some(ref emoji) = cs.emoji {
                if let Some(url) = emoji.image_url() {
                    sub_div = sub_div.child(
                        img(url)
                            .size(px(14.0))
                            .flex_shrink_0(),
                    );
                } else if !emoji.name.is_empty() {
                    sub_div = sub_div.child(
                        div()
                            .size(px(14.0))
                            .text_size(px(11.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .child(emoji.name.clone()),
                    );
                }
            }

            if let Some(ref text) = cs.text {
                if !text.trim().is_empty() {
                    sub_div = sub_div.child(
                        div()
                            .text_size(px(11.0))
                            .line_height(px(14.0))
                            .text_color(text_muted())
                            .truncate()
                            .child(text.clone()),
                    );
                }
            }

            return Some(sub_div.into_any_element());
        }
    }

    if let Some(act) = game_activity {
        let game_text = format!("Jogando {}", act.name);
        return Some(
            div()
                .h(px(14.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(px(14.0))
                        .text_color(text_muted())
                        .truncate()
                        .child(game_text),
                )
                .into_any_element(),
        );
    }

    None
}

fn render_dm_text_column(
    dm_name: String,
    is_selected: bool,
    unread: bool,
    is_pinned: bool,
    subtitle_el: Option<AnyElement>,
) -> AnyElement {
    let title_color = if is_selected {
        rgb(0xffffff)
    } else if unread {
        rgb(0xf2f3f5)
    } else {
        rgb(0x949ba4)
    };
    let font_weight = if is_selected || unread {
        FontWeight::SEMIBOLD
    } else {
        FontWeight::NORMAL
    };

    let mut title_row = div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_color(title_color)
                .font_weight(font_weight)
                .text_size(px(14.0))
                .child(dm_name),
        );

    if is_pinned {
        title_row = title_row.child(icon_pin(px(12.0), rgb(0x949ba4)));
    }

    let mut col = div()
        .flex()
        .flex_col()
        .justify_center()
        .overflow_hidden()
        .child(title_row);

    if let Some(sub) = subtitle_el {
        col = col.child(sub);
    }

    col.into_any_element()
}

