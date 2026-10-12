use gpui::component::menu::{ContextMenuExt, PopupMenu};
use gpui::component::v_virtual_list;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

use super::state::{AppState, MemberItemState, PresenceStatus};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::tokens::colors::*;
use crate::tokens::spacing::*;

/// An item row in the member sidebar virtual list
#[derive(Clone)]
enum MemberSidebarItem {
    Header { title: SharedString, count: usize },
    Member(MemberItemState),
}

/// Server member list sidebar (240px wide) with role groupings and virtual list
pub struct MemberSidebar {
    state: Entity<AppState>,
    items: Vec<MemberSidebarItem>,
    last_requested_ranges: Option<Vec<[usize; 2]>>,
    current_guild_id: Option<String>,
    current_channel_id: Option<String>,
}

impl MemberSidebar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self {
            state,
            items: Vec::new(),
            last_requested_ranges: None,
            current_guild_id: None,
            current_channel_id: None,
        }
    }

    fn rebuild_items(&mut self, cx: &App) {
        let state = self.state.read(cx);
        if self.current_guild_id != state.selected_guild_id
            || self.current_channel_id != state.selected_channel_id
        {
            self.current_guild_id = state.selected_guild_id.clone();
            self.current_channel_id = state.selected_channel_id.clone();
            self.last_requested_ranges = None;
        }
        let members = state.members.clone();

        let has_server_groups = members.iter().any(|m| m.id.starts_with("group:"));

        if has_server_groups {
            let mut items = Vec::new();
            for m in members {
                if m.id.starts_with("group:") {
                    let count = m
                        .activity
                        .as_deref()
                        .and_then(|c| c.parse::<usize>().ok())
                        .unwrap_or(0);
                    items.push(MemberSidebarItem::Header {
                        title: m.name.into(),
                        count,
                    });
                } else {
                    items.push(MemberSidebarItem::Member(m));
                }
            }
            self.items = items;
            return;
        }

        let mut group_order: Vec<String> = Vec::new();
        let mut group_map: std::collections::HashMap<String, Vec<MemberItemState>> = std::collections::HashMap::new();
        let mut offline_list: Vec<MemberItemState> = Vec::new();

        for m in members {
            if m.presence == PresenceStatus::Offline {
                offline_list.push(m);
            } else {
                let group_name = if !m.role_group.trim().is_empty() {
                    m.role_group.clone()
                } else if m.is_bot {
                    "BOTS".to_string()
                } else {
                    "DISPONÍVEL".to_string()
                };

                if !group_map.contains_key(&group_name) {
                    group_order.push(group_name.clone());
                }
                group_map.entry(group_name).or_default().push(m);
            }
        }

        let mut items = Vec::new();
        for title in group_order {
            if let Some(list) = group_map.get(&title) {
                if !list.is_empty() {
                    items.push(MemberSidebarItem::Header {
                        title: title.into(),
                        count: list.len(),
                    });
                    for member in list {
                        items.push(MemberSidebarItem::Member(member.clone()));
                    }
                }
            }
        }

        if !offline_list.is_empty() {
            items.push(MemberSidebarItem::Header {
                title: "OFFLINE".into(),
                count: offline_list.len(),
            });
            for member in offline_list {
                items.push(MemberSidebarItem::Member(member));
            }
        }

        self.items = items;
    }
}

impl Render for MemberSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rebuild_items(cx);

        let item_sizes: Rc<Vec<Size<Pixels>>> = Rc::new(
            self.items
                .iter()
                .map(|item| match item {
                    MemberSidebarItem::Header { .. } => size(px(224.0), px(40.0)),
                    MemberSidebarItem::Member(_) => size(px(224.0), px(44.0)),
                })
                .collect(),
        );

        let entity = cx.entity().clone();
        let app_entity = self.state.clone();
        let items = self.items.clone();
        let active_gid = self.current_guild_id.clone();
        let active_cid = self.current_channel_id.clone();

        div()
            .w(px(240.0))
            .h_full()
            .min_h(px(0.0))
            .bg(bg_channels())
            .overflow_hidden()
            .flex()
            .flex_col()
            .p(px(8.0))
            .flex_shrink_0()
            .child(
                v_virtual_list(
                    entity,
                    "member-sidebar-virtual-list",
                    item_sizes,
                    move |this, range, _window, _cx| {
                        let app = app_entity.clone();

                        if let (Some(gid), Some(cid)) = (&active_gid, &active_cid) {
                            let ranges = crate::protocol::cache::lazy_guilds::calculate_ranges_for_viewport(
                                range.start,
                                range.end,
                            );
                            if this.last_requested_ranges.as_ref() != Some(&ranges) {
                                this.last_requested_ranges = Some(ranges.clone());
                                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                                    let coordinator = crate::protocol::cache::lazy_guilds::LazyGuildCoordinator::shared();
                                    let gid_c = gid.clone();
                                    let cid_c = cid.clone();
                                    handle.spawn(async move {
                                        coordinator.update_viewport_ranges(&gid_c, &cid_c, ranges).await;
                                    });
                                }
                            }
                        }

                        range
                            .filter_map(|ix| items.get(ix))
                            .map(|item| match item {
                                MemberSidebarItem::Header { title, count } => {
                                    render_group_header(title.clone(), *count)
                                }
                                MemberSidebarItem::Member(member) => {
                                    render_member_row(member, &app)
                                }
                            })
                            .collect::<Vec<_>>()
                    },
                )
            )
    }
}

fn render_group_header(title: SharedString, count: usize) -> AnyElement {
    div()
        .w_full()
        .min_w(px(0.0))
        .h(px(40.0))
        .pt(px(20.0))
        .pb(px(4.0))
        .px(px(16.0))
        .text_color(text_muted())
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(12.0))
        .child(format!("{} — {}", title, count))
        .into_any_element()
}

fn render_member_row(member: &MemberItemState, app_entity: &Entity<AppState>) -> AnyElement {
    let role_color = if member.role_color != 0 {
        rgb(member.role_color)
    } else if member.presence == PresenceStatus::Offline {
        text_muted()
    } else {
        text_normal()
    };

    let member_uid = member.id.clone();
    let entity_profile = app_entity.clone();
    let entity_menu = app_entity.clone();
    let menu_uid = member.id.clone();

    let mut row = div()
        .id(ElementId::Name(format!("member-row-{}", member.id).into()))
        .w_full()
        .min_w(px(0.0))
        .overflow_hidden()
        .h(px(44.0))
        .px(px(8.0))
        .py(px(1.0))
        .rounded(radius_md()) // 8px per member-sidebar.md
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .cursor_pointer()
        .hover(|s| s.bg(channel_hover()))
        .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
            let uid = member_uid.clone();
            let pos = ev.position;
            entity_profile.update(cx, |s, cx| {
                let current_guild = s.selected_guild_id.clone();
                s.open_user_profile_in_guild(&uid, current_guild, Some(pos));
                cx.notify();
            });
        });

    // Avatar using crate::components::Avatar with status cutout & decoration (32px photo inside 40px bounding box)
    let avatar = Avatar::new(member.name.clone())
        .size(AvatarSize::Px32)
        .status(member.presence.into())
        .cutout_color(bg_channels())
        .opt_decoration(member.avatar_decoration_data.clone());

    row = row.child(avatar);

    // Text column (Name + Bot Tag + Activity)
    let mut text_col = div()
        .flex()
        .flex_col()
        .justify_center()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden();

    let mut name_line = div()
        .flex()
        .flex_row()
        .items_center()
        .min_w(px(0.0))
        .gap(px(4.0))
        .child(
            div()
                .text_color(role_color)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0))
                .truncate()
                .child(member.name.clone()),
        );

    if member.is_bot {
        name_line = name_line.child(render_bot_tag());
    }

    text_col = text_col.child(name_line);

    if let Some(ref act) = member.activity {
        text_col = text_col.child(
            div()
                .text_color(text_muted())
                .text_size(px(11.0))
                .w_full()
                .min_w(px(0.0))
                .truncate()
                .child(act.clone()),
        );
    }

    row = row.child(text_col);

    // Context menu using gpui-kit ContextMenuExt & PopupMenu (per context-menu-user.md)
    let app1 = entity_menu.clone();
    let uid1 = menu_uid.clone();
    let app2 = entity_menu.clone();
    let uid2 = menu_uid.clone();

    row.context_menu(move |menu: PopupMenu, _window, _cx| {
        let a1 = app1.clone();
        let u1 = uid1.clone();
        let a2 = app2.clone();
        let u2 = uid2.clone();
        menu
            .item(
                gpui::component::menu::PopupMenuItem::new("Perfil").on_click(move |ev, _win, cx| {
                    let pos = ev.position();
                    a1.update(cx, |s, cx| {
                        let current_guild = s.selected_guild_id.clone();
                        s.open_user_profile_in_guild(&u1, current_guild, Some(pos));
                        cx.notify();
                    });
                }),
            )
            .item(
                gpui::component::menu::PopupMenuItem::new("Mensagem").on_click(move |ev, _win, cx| {
                    let pos = ev.position();
                    a2.update(cx, |s, cx| {
                        s.open_user_profile(&u2, Some(pos));
                        cx.notify();
                    });
                }),
            )
            .separator()
            .item(gpui::component::menu::PopupMenuItem::new("Mencionar"))
    })
    .into_any_element()
}

fn render_bot_tag() -> AnyElement {
    div()
        .flex()
        .items_center()
        .px(px(4.0))
        .py(px(1.0))
        .rounded_xs()
        .bg(blurple())
        .text_color(crate::tokens::colors::white())
        .text_size(px(9.0))
        .font_weight(FontWeight::BOLD)
        .child("BOT")
        .into_any_element()
}
