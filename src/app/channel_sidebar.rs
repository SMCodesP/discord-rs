use gpui::*;
use crate::tokens::icons::*;
use super::state::{ActiveModal, AppState, FriendsTab};

/// Channel sidebar (240px wide) containing:
/// - Server header
/// - Collapsible categories & channel lists
/// - Active voice status bar (when connected)
/// - Bottom user profile & audio controls panel
pub struct ChannelSidebar {
    state: Entity<AppState>,
    context_menu_dm: Option<(String, Point<Pixels>)>,
    focus_handle: FocusHandle,
}

impl ChannelSidebar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        // Initialize pinned_dms_path in AppState for the live application if not yet set
        state.update(cx, |s, _cx| {
            if s.pinned_dms_path.is_none() {
                s.set_pinned_dms_path(Some(crate::protocol::pinned_dms::pinned_dms_cache_path()));
            }
        });

        Self {
            state,
            context_menu_dm: None,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for ChannelSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected_guild = state.get_selected_guild().cloned();
        let selected_channel_id = state.selected_channel_id.clone();
        let active_voice = state.active_voice_call.clone();
        let current_user = state.current_user.clone();
        let is_dm = state.selected_guild_id.is_none();
        let app_entity = self.state.clone();
        let self_entity = cx.entity().clone();
        let viewport = window.viewport_size();

        let context_menu_overlay = if let Some((ref target_dm_id, click_pos)) = self.context_menu_dm {
            let dm_id = target_dm_id.clone();
            let is_pinned = state.dm_channels.iter().find(|d| d.id == dm_id).map(|d| d.is_pinned).unwrap_or(false)
                || state.pinned_dms.contains(&dm_id);
            let app_entity_menu = self.state.clone();
            let self_close = cx.entity().clone();

            let pin_label = if is_pinned {
                "Desafixar conversa"
            } else {
                "Fixar conversa"
            };

            let self_close_left = self_close.clone();
            let self_close_right = self_close.clone();

            Some(
                deferred(
                    div()
                        .child(
                            // Full-window transparent backdrop to catch clicks outside and dismiss the context menu
                            anchored()
                                .position(point(px(0.0), px(0.0)))
                                .child(
                                    div()
                                        .w(viewport.width)
                                        .h(viewport.height)
                                        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                            self_close_left.update(cx, |this, cx| {
                                                this.context_menu_dm = None;
                                                cx.notify();
                                            });
                                        })
                                        .on_mouse_down(MouseButton::Right, move |_ev, _win, cx| {
                                            self_close_right.update(cx, |this, cx| {
                                                this.context_menu_dm = None;
                                                cx.notify();
                                            });
                                        })
                                )
                        )
                        .child(
                            // Context menu anchored at the exact click position, snapping dynamically to window bounds
                            anchored()
                                .position(click_pos)
                                .snap_to_window()
                                .child(
                                    div()
                                        .w(px(180.0))
                                        .bg(rgb(0x111214))
                                        .border_1()
                                        .border_color(rgb(0x232428))
                                        .rounded(px(6.0))
                                        .shadow_lg()
                                        .p(px(6.0))
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .on_mouse_down(MouseButton::Left, |_ev, _win, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_down(MouseButton::Right, |_ev, _win, cx| {
                                            cx.stop_propagation();
                                        })
                                        .child(
                                            div()
                                                .h(px(32.0))
                                                .px(px(8.0))
                                                .rounded(px(4.0))
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .cursor_pointer()
                                                .hover(|s| s.bg(rgb(0x4752c4)).text_color(rgb(0xffffff)))
                                                .text_color(rgb(0xdbdee1))
                                                .text_size(px(13.0))
                                                .on_mouse_down(MouseButton::Left, {
                                                    let app = app_entity_menu.clone();
                                                    let target_id = dm_id.clone();
                                                    let self_close = cx.entity().clone();
                                                    move |_ev, _win, cx| {
                                                        app.update(cx, |state, cx| {
                                                            state.toggle_pin_dm_channel(&target_id);
                                                            cx.notify();
                                                        });
                                                        self_close.update(cx, |this, cx| {
                                                            this.context_menu_dm = None;
                                                            cx.notify();
                                                        });
                                                    }
                                                })
                                                .child(pin_label)
                                                .child(icon_pin(px(14.0), rgb(0x949ba4)))
                                        )
                                )
                        )
                )
            )
        } else {
            None
        };

        div()
            .track_focus(&self.focus_handle)
            .key_context("channel_sidebar")
            .on_key_down({
                let self_esc = cx.entity().clone();
                move |ev: &KeyDownEvent, _window, cx| {
                    if ev.keystroke.key == "escape" {
                        self_esc.update(cx, |this, cx| {
                            if this.context_menu_dm.is_some() {
                                this.context_menu_dm = None;
                                cx.notify();
                            }
                        });
                    }
                }
            })
            .relative()
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

                                        let avatar_el = if let Some(ref icon_url) = dm.icon_url {
                                            div()
                                                .size(px(32.0))
                                                .rounded_full()
                                                .overflow_hidden()
                                                .flex_shrink_0()
                                                .child(img(icon_url.clone()).size_full().rounded_full())
                                        } else if is_group {
                                            let names = if !dm.recipient_names.is_empty() {
                                                dm.recipient_names.clone()
                                            } else {
                                                dm_name.split(',').map(|s| s.trim().to_string()).collect()
                                            };
                                            let first_initial = names.get(0).and_then(|n| n.chars().next()).unwrap_or('A').to_uppercase().to_string();
                                            let second_badge = if names.len() > 2 {
                                                format!("+{}", names.len() - 1)
                                            } else {
                                                names.get(1).and_then(|n| n.chars().next()).unwrap_or('B').to_uppercase().to_string()
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
                                                        .child(first_initial)
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
                                                        .child(second_badge)
                                                )
                                        } else {
                                            let avatar_char = dm_name.chars().next().unwrap_or('@').to_uppercase().to_string();
                                            div()
                                                .size(px(32.0))
                                                .rounded_full()
                                                .bg(rgb(0x5865f2))
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(rgb(0xffffff))
                                                .text_size(px(14.0))
                                                .font_weight(FontWeight::BOLD)
                                                .child(avatar_char)
                                        };

                                        dm_items.push(
                                            div()
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
                                                .on_mouse_down(MouseButton::Right, {
                                                    let self_ctx = self_entity.clone();
                                                    let menu_cid = dm_id.clone();
                                                    move |event, window, cx| {
                                                        let pos = event.position;
                                                        self_ctx.update(cx, |this, cx| {
                                                            this.context_menu_dm = Some((menu_cid.clone(), pos));
                                                            window.focus(&this.focus_handle);
                                                            cx.notify();
                                                        });
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(10.0))
                                                        .child({
                                                            let mut av = div().child(avatar_el).cursor_pointer();
                                                            if let Some(uid) = dm.recipient_ids.first().cloned() {
                                                                let av_entity = entity.clone();
                                                                av = av.on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                                                                    let uid = uid.clone();
                                                                    av_entity.update(cx, |s, cx| {
                                                                        s.open_user_profile(&uid);
                                                                        cx.notify();
                                                                    });
                                                                });
                                                            }
                                                            av
                                                        })
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .items_center()
                                                                .gap(px(6.0))
                                                                .child(
                                                                    div()
                                                                        .text_color(if is_selected {
                                                                            rgb(0xffffff)
                                                                        } else if unread {
                                                                            rgb(0xf2f3f5)
                                                                        } else {
                                                                            rgb(0x949ba4)
                                                                        })
                                                                        .font_weight(if is_selected || unread {
                                                                            FontWeight::SEMIBOLD
                                                                        } else {
                                                                            FontWeight::NORMAL
                                                                        })
                                                                        .text_size(px(14.0))
                                                                        .child(dm_name),
                                                                )
                                                                .children(if dm.is_pinned || dm.topic.as_deref().map_or(false, |t| t.contains("pin") || t.contains("fixad")) {
                                                                    Some(icon_pin(px(14.0), rgb(0x949ba4)))
                                                                } else {
                                                                    None
                                                                }),
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
                                                            .text_color(rgb(0xffffff))
                                                            .text_size(px(10.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(mention_count.to_string()),
                                                    )
                                                } else {
                                                    None
                                                })
                                                .child(
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
                                                        )
                                                ),
                                        );
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
            .children(context_menu_overlay)
    }
}
