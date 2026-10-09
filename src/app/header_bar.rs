use gpui::*;
use crate::tokens::icons::*;
use super::state::AppState;

/// Chat and channel top header bar (48px high)
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
        let show_members = state.show_members;
        let is_dm = state.selected_guild_id.is_none();
        let app_entity = self.state.clone();

        div()
            .h(px(48.0))
            .w_full()
            .bg(rgb(0x313338))
            .px(px(16.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(0x1f2023))
            .flex_shrink_0()
            // =========================================================================
            // LEFT SIDE: Channel icon + Name + Divider + Topic
            // =========================================================================
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .overflow_hidden()
                    .children(if let Some(channel) = selected_channel_opt.as_ref() {
                        let mut left_items = vec![
                            // Icon
                            div()
                                .text_color(rgb(0x80848e))
                                .text_size(px(20.0))
                                .child(channel.kind.symbol()),
                            // Name
                            div()
                                .text_color(rgb(0xf2f3f5))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(16.0))
                                .child(channel.name.clone()),
                        ];

                        // Divider & Topic if available
                        if let Some(topic) = channel.topic.as_ref() {
                            left_items.push(
                                div()
                                    .w(px(1.0))
                                    .h(px(16.0))
                                    .bg(rgb(0x4e5058))
                                    .mx(px(4.0)),
                            );
                            left_items.push(
                                div()
                                    .text_color(rgb(0x80848e))
                                    .text_size(px(13.0))
                                    .overflow_hidden()
                                    .child(topic.clone()),
                            );
                        }

                        left_items
                    } else if is_dm {
                        vec![
                            icon_friends(px(20.0), rgb(0x80848e)),
                            div()
                                .text_color(rgb(0xf2f3f5))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(16.0))
                                .child("Amigos"),
                            div()
                                .w(px(1.0))
                                .h(px(16.0))
                                .bg(rgb(0x4e5058))
                                .mx(px(4.0)),
                            div()
                                .text_color(rgb(0x80848e))
                                .text_size(px(13.0))
                                .child("Disponível"),
                        ]
                    } else {
                        vec![
                            div()
                                .text_color(rgb(0x80848e))
                                .text_size(px(16.0))
                                .child("#"),
                            div()
                                .text_color(rgb(0xf2f3f5))
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(16.0))
                                .child("Selecione um canal"),
                        ]
                    }),
            )
            // =========================================================================
            // RIGHT SIDE: Actions (Notifications, Pins, Member Toggle, Search, Inbox)
            // =========================================================================
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    // 0. Discord Connection Shortcut Button / Status Badge
                    .child(if !state.is_connected_to_gateway {
                        let entity = app_entity.clone();
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .px(px(10.0))
                            .h(px(28.0))
                            .rounded(px(4.0))
                            .bg(rgb(0x5865f2))
                            .hover(|s| s.bg(rgb(0x4752c4)))
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                entity.update(cx, |state, cx| {
                                    state.open_modal(crate::app::state::ActiveModal::Login);
                                    cx.notify();
                                });
                            })
                            .child(icon_key(px(14.0), rgb(0xffffff)))
                            .child("Entrar no Discord")
                    } else {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(8.0))
                            .h(px(24.0))
                            .rounded(px(4.0))
                            .bg(rgba(0x23a55a22))
                            .border_1()
                            .border_color(rgb(0x23a55a))
                            .text_color(rgb(0x23a55a))
                            .text_size(px(11.0))
                            .font_weight(FontWeight::BOLD)
                            .child("● Gateway Conectado")
                    })
                    // 1. Notifications / Bell button
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(0x80848e))
                            .hover(|s| s.text_color(rgb(0xdbdee1)))
                            .child(icon_bell(px(16.0), rgb(0x80848e))),
                    )
                    // 2. Pinned messages button
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(0x80848e))
                            .hover(|s| s.text_color(rgb(0xdbdee1)))
                            .child(icon_pin(px(16.0), rgb(0x80848e))),
                    )
                    // 3. Member list toggle button
                    .child({
                        let entity = app_entity.clone();
                        let icon_col = if show_members {
                            rgb(0xffffff)
                        } else {
                            rgb(0x80848e)
                        };
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(if show_members {
                                rgb(0x35373c)
                            } else {
                                rgb(0x313338)
                            })
                            .text_color(icon_col)
                            .hover(|s| s.text_color(rgb(0xdbdee1)))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                entity.update(cx, |state, cx| {
                                    state.toggle_members();
                                    cx.notify();
                                });
                            })
                            .child(icon_members(px(16.0), icon_col))
                    })
                    // 4. Search bar (Buscar)
                    .child({
                        let entity = app_entity.clone();
                        let is_search_open = state.is_search_open;
                        let query = state.search_query.clone();
                        div()
                            .w(px(144.0))
                            .h(px(24.0))
                            .px(px(6.0))
                            .rounded(px(4.0))
                            .bg(if is_search_open { rgb(0x35373c) } else { rgb(0x1e1f22) })
                            .border_1()
                            .border_color(if is_search_open { rgb(0x5865f2) } else { rgb(0x1e1f22) })
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0x35373c)))
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                entity.update(cx, |state, cx| {
                                    state.toggle_search();
                                    cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .text_color(if !query.is_empty() { rgb(0xf2f3f5) } else { rgb(0x80848e) })
                                    .text_size(px(12.0))
                                    .child(if !query.is_empty() { query } else { "Buscar".to_string() }),
                            )
                            .child(icon_search(px(14.0), if is_search_open { rgb(0xffffff) } else { rgb(0x80848e) }))
                    })
                    // 5. Inbox button
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(0x80848e))
                            .hover(|s| s.text_color(rgb(0xdbdee1)))
                            .child(icon_inbox(px(16.0), rgb(0x80848e))),
                    )
                    // 6. Help button
                    .child(
                        div()
                            .w(px(28.0))
                            .h(px(28.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(rgb(0x80848e))
                            .hover(|s| s.text_color(rgb(0xdbdee1)))
                            .child(icon_info(px(16.0), rgb(0x80848e))),
                    ),
            )
    }
}
