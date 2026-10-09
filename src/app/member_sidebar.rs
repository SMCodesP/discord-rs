use gpui::*;
use super::state::{AppState, PresenceStatus};

/// Server member list sidebar (240px wide) with role groupings
pub struct MemberSidebar {
    state: Entity<AppState>,
}

impl MemberSidebar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }
}

impl Render for MemberSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let members = state.members.clone();

        // Organize members into groups preserving order: Admin, Moderador, Bots, Membros, Offline
        let mut groups: Vec<(&'static str, Vec<super::state::MemberItemState>)> = vec![
            ("ADMINISTRADOR", Vec::new()),
            ("MODERADOR", Vec::new()),
            ("BOTS", Vec::new()),
            ("DISPONÍVEL", Vec::new()),
            ("OFFLINE", Vec::new()),
        ];

        for m in members {
            if m.presence == PresenceStatus::Offline {
                groups[4].1.push(m);
            } else if m.role_group == "ADMINISTRADOR" {
                groups[0].1.push(m);
            } else if m.role_group == "MODERADOR" {
                groups[1].1.push(m);
            } else if m.is_bot {
                groups[2].1.push(m);
            } else {
                groups[3].1.push(m);
            }
        }

        div()
            .w(px(240.0))
            .h_full()
            .min_h(px(0.0))
            .bg(rgb(0x2b2d31))
            .overflow_hidden()
            .flex()
            .flex_col()
            .p(px(8.0))
            .flex_shrink_0()
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .id("member-sidebar-scroll")
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(
                        groups
                            .into_iter()
                            .filter(|(_, list)| !list.is_empty())
                            .map(|(title, member_list)| {
                        let count = member_list.len();

                        div()
                            .flex()
                            .flex_col()
                            .child(
                                // Group Header: "ROLE — COUNT"
                                div()
                                    .pt(px(14.0))
                                    .pb(px(4.0))
                                    .px(px(8.0))
                                    .text_color(rgb(0x80848e))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .child(format!("{} — {}", title, count)),
                            )
                            .children(member_list.into_iter().map(|member| {
                                let status_color = member.presence.color_rgb();
                                let role_color = member.role_color;
                                let is_bot = member.is_bot;
                                let activity = member.activity.clone();

                                div()
                                    .h(px(44.0))
                                    .px(px(8.0))
                                    .rounded(px(4.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(10.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0x35373c)))
                                    // Avatar with Presence indicator
                                    .child(
                                        div()
                                            .relative()
                                            .w(px(32.0))
                                            .h(px(32.0))
                                            .rounded_full()
                                            .bg(rgb(0x5865f2))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(rgb(0xffffff))
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(14.0))
                                            .child(member.avatar_text)
                                            .child(
                                                div()
                                                    .absolute()
                                                    .bottom(px(-1.0))
                                                    .right(px(-1.0))
                                                    .w(px(10.0))
                                                    .h(px(10.0))
                                                    .rounded_full()
                                                    .bg(rgb(status_color))
                                                    .border_2()
                                                    .border_color(rgb(0x2b2d31)),
                                            ),
                                    )
                                    // Info column (Name + Bot Tag + Activity)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.0))
                                                    .child(
                                                        div()
                                                            .text_color(rgb(role_color))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_size(px(14.0))
                                                            .child(member.name),
                                                    )
                                                    .children(if is_bot {
                                                        Some(
                                                            div()
                                                                .px(px(4.0))
                                                                .py(px(1.0))
                                                                .rounded(px(3.0))
                                                                .bg(rgb(0x5865f2))
                                                                .text_color(rgb(0xffffff))
                                                                .text_size(px(9.0))
                                                                .font_weight(FontWeight::BOLD)
                                                                .child("BOT"),
                                                        )
                                                    } else {
                                                        None
                                                    }),
                                            )
                                            .children(activity.map(|act| {
                                                div()
                                                    .text_color(rgb(0x80848e))
                                                    .text_size(px(11.0))
                                                    .child(act)
                                            }))
                                    )
                            }))
                    })),
            )
    }
}
