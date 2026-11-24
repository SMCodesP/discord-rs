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
    Member {
        member: MemberItemState,
        avatar_url: Option<String>,
        subtitle: Option<String>,
    },
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
        let avatar_urls = cached_member_avatar_urls(state);
        let member_item = |member: MemberItemState| MemberSidebarItem::Member {
            avatar_url: avatar_urls.get(&member.id).cloned(),
            subtitle: member_subtitle(&member, state),
            member,
        };

        let has_server_groups = members.iter().any(|m| m.id.starts_with("group:"));

        if has_server_groups {
            let mut items = Vec::new();
            for m in members {
                if m.is_unloaded() {
                    continue;
                }
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
                    items.push(member_item(m));
                }
            }
            self.items = items;
            return;
        }

        let mut group_order: Vec<String> = Vec::new();
        let mut group_map: std::collections::HashMap<String, Vec<MemberItemState>> = std::collections::HashMap::new();
        let mut offline_list: Vec<MemberItemState> = Vec::new();

        for m in members {
            if m.is_unloaded() {
                continue;
            }
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
                        items.push(member_item(member.clone()));
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
                items.push(member_item(member));
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
                    MemberSidebarItem::Header { .. } => size(px(224.0), px(42.0)),
                    MemberSidebarItem::Member { subtitle, .. } => {
                        size(px(224.0), if subtitle.is_some() { px(44.0) } else { px(42.0) })
                    }
                })
                .collect(),
        );

        let entity = cx.entity().clone();
        let app_entity = self.state.clone();
        let items = self.items.clone();
        let active_gid = self.current_guild_id.clone();
        let active_cid = self.current_channel_id.clone();

        div()
            .w(members_width())
            .h_full()
            .min_h(px(0.0))
            .bg(bg_channels())
            .border_l_1()
            .border_color(border_subtle())
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
                    move |this, range, _window, cx| {
                        let app = app_entity.clone();

                        if let (Some(_gid), Some(_cid)) = (&active_gid, &active_cid) {
                            let ranges = crate::protocol::cache::lazy_guilds::calculate_ranges_for_viewport(
                                range.start,
                                range.end,
                            );
                            if this.last_requested_ranges.as_ref() != Some(&ranges) {
                                this.last_requested_ranges = Some(ranges.clone());
                                app.update(cx, |state, _cx| {
                                    state.update_member_viewport(ranges);
                                });
                            }
                        }

                        range
                            .filter_map(|ix| items.get(ix))
                            .map(|item| match item {
                                MemberSidebarItem::Header { title, count } => {
                                    render_group_header(title.clone(), *count)
                                }
                                MemberSidebarItem::Member {
                                    member,
                                    avatar_url,
                                    subtitle,
                                } => render_member_row(
                                    member,
                                    avatar_url.as_deref(),
                                    subtitle.as_deref(),
                                    &app,
                                ),
                            })
                            .collect::<Vec<_>>()
                    },
                )
            )
    }
}

/// Resolves avatar URLs only from real cached Discord payloads. The lazy member
/// list does not include avatar hashes, so no URL or fallback identity is
/// invented here.
fn cached_member_avatar_urls(state: &AppState) -> std::collections::HashMap<String, String> {
    let mut urls = std::collections::HashMap::new();

    if !state.current_user.id.is_empty() {
        if let Some(url) = state.current_user.avatar_url.clone() {
            urls.insert(state.current_user.id.clone(), url);
        }
    }
    for friend in &state.friends {
        if let Some(url) = friend.avatar_url.clone() {
            urls.entry(friend.id.clone()).or_insert(url);
        }
    }
    for message in state.messages_by_channel.values().flatten() {
        if let Some(url) = message.author.avatar_url() {
            urls.entry(message.author.id.clone()).or_insert(url);
        }
    }

    urls
}

/// Prefers the user's real custom status, then the activity already carried by
/// the member payload. Offline rows do not show stale presence text.
fn member_subtitle(member: &MemberItemState, state: &AppState) -> Option<String> {
    if member.presence == PresenceStatus::Offline {
        return None;
    }

    state
        .presences_by_user
        .get(&member.id)
        .and_then(|presence| presence.custom_status.as_ref())
        .and_then(|status| status.text.as_ref())
        .map(|text| text.trim())
        .filter(|text| !text.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            member
                .activity
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(ToString::to_string)
        })
        .or_else(|| {
            state
                .presences_by_user
                .get(&member.id)
                .and_then(|presence| presence.activity.as_ref())
                .map(|activity| activity.name.trim())
                .filter(|name| !name.is_empty())
                .map(ToString::to_string)
        })
}

fn group_header_text(title: &str, count: usize) -> String {
    format!("{} — {}", title.to_uppercase(), count)
}

fn render_group_header(title: SharedString, count: usize) -> AnyElement {
    div()
        .w_full()
        .min_w(px(0.0))
        .h(px(42.0))
        .pt(px(20.0))
        .pb(px(4.0))
        .px(px(16.0))
        .text_color(text_muted())
        .font_weight(FontWeight::MEDIUM)
        .text_size(crate::tokens::typography::font_size_sm())
        .line_height(crate::tokens::typography::line_height_sm())
        .child(group_header_text(&title, count))
        .into_any_element()
}

fn render_member_row(
    member: &MemberItemState,
    avatar_url: Option<&str>,
    subtitle: Option<&str>,
    app_entity: &Entity<AppState>,
) -> AnyElement {
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
    let menu_name = member.name.clone();

    let mut row = div()
        .id(ElementId::Name(format!("member-row-{}", member.id).into()))
        .w_full()
        .min_w(px(0.0))
        .overflow_hidden()
        .h(if subtitle.is_some() { px(44.0) } else { px(42.0) })
        .px(px(8.0))
        .py(px(1.0))
        .rounded(radius_md()) // 8px per member-sidebar.md
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
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
        .opt_src(avatar_url.map(str::to_string))
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
                .font_weight(FontWeight::MEDIUM)
                .text_size(crate::tokens::typography::font_size_sm())
                .line_height(px(20.0))
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(member.name.clone()),
        );

    if member.is_bot {
        name_line = name_line.child(render_bot_tag());
    }

    text_col = text_col.child(name_line);

    if let Some(subtitle) = subtitle {
        text_col = text_col.child(
            div()
                .text_color(text_muted())
                .text_size(px(11.0))
                .line_height(crate::tokens::typography::line_height_tight())
                .w_full()
                .min_w(px(0.0))
                .truncate()
                .child(subtitle.to_string()),
        );
    }

    row = row.child(text_col);

    // Context menu using gpui-kit ContextMenuExt & PopupMenu (per context-menu-user.md)
    let app1 = entity_menu.clone();
    let uid1 = menu_uid.clone();
    let app2 = entity_menu.clone();
    let uid2 = menu_uid.clone();
    let name2 = menu_name.clone();
    let app3 = entity_menu.clone();
    let uid3 = menu_uid.clone();

    row.context_menu(move |menu: PopupMenu, _window, _cx| {
        let a1 = app1.clone();
        let u1 = uid1.clone();
        let a2 = app2.clone();
        let u2 = uid2.clone();
        let n2 = name2.clone();
        let a3 = app3.clone();
        let u3 = uid3.clone();
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
                gpui::component::menu::PopupMenuItem::new("Mensagem").on_click(move |_ev, _win, cx| {
                    a2.update(cx, |s, cx| {
                        open_member_dm(s, &u2, &n2);
                        cx.notify();
                    });
                }),
            )
            .separator()
            .item(
                gpui::component::menu::PopupMenuItem::new("Mencionar").on_click(move |_ev, _win, cx| {
                    a3.update(cx, |s, cx| {
                        mention_member(s, &u3);
                        cx.notify();
                    });
                }),
            )
    })
    .into_any_element()
}

/// Opens (or reuses) the 1:1 DM with a guild member.
pub fn open_member_dm(state: &mut AppState, user_id: &str, display_name: &str) {
    if !user_id.is_empty() {
        state.open_dm_with_user(user_id, display_name);
    }
}

/// Inserts a real Discord mention token for the member into the composer draft.
pub fn mention_member(state: &mut AppState, user_id: &str) {
    if user_id.is_empty() {
        return;
    }
    let token = format!("<@{}>", user_id);
    if !state.active_draft.is_empty() && !state.active_draft.ends_with(' ') {
        state.active_draft.push(' ');
    }
    state.active_draft.push_str(&token);
}

fn render_bot_tag() -> AnyElement {
    div()
        .flex()
        .items_center()
        .px(px(4.0))
        .py(px(1.0))
        .h(px(16.0))
        .rounded_xs()
        .bg(blurple())
        .text_color(crate::tokens::colors::white())
        .text_size(crate::tokens::typography::font_size_xxs())
        .font_weight(FontWeight::BOLD)
        .flex_shrink_0()
        .child("BOT")
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::{Author, Message};
    use crate::domain::user::{CustomStatus, Presence};

    fn member(presence: PresenceStatus) -> MemberItemState {
        MemberItemState {
            id: "member-1".into(),
            name: "Member".into(),
            avatar_text: "M".into(),
            presence,
            role_group: "ONLINE".into(),
            role_color: 0,
            is_bot: false,
            activity: Some("Atividade real".into()),
            avatar_decoration_data: None,
        }
    }

    #[test]
    fn group_headers_are_uppercase_and_keep_server_count() {
        assert_eq!(group_header_text("staff", 3), "STAFF — 3");
        assert_eq!(group_header_text("offline", 0), "OFFLINE — 0");
    }

    #[test]
    fn subtitle_prefers_custom_status_and_hides_offline_stale_activity() {
        let mut state = AppState::default();
        let online = member(PresenceStatus::Online);
        state.presences_by_user.insert(
            online.id.clone(),
            Presence::new(&online.id, crate::domain::user::PresenceStatus::Online)
                .with_custom_status(CustomStatus::from_text("Status real")),
        );

        assert_eq!(member_subtitle(&online, &state).as_deref(), Some("Status real"));
        assert_eq!(
            member_subtitle(&member(PresenceStatus::Offline), &state),
            None
        );
    }

    #[test]
    fn avatar_urls_come_only_from_cached_real_payloads() {
        let mut state = AppState::default();
        let mut author = Author::new("member-1", "member");
        author.avatar = Some("a_real_hash".into());
        state.messages_by_channel.insert(
            "channel-1".into(),
            vec![Message::new("message-1", "channel-1", author, "conteúdo")],
        );

        assert_eq!(
            cached_member_avatar_urls(&state).get("member-1").map(String::as_str),
            Some("https://cdn.discordapp.com/avatars/member-1/a_real_hash.png")
        );
        assert!(!cached_member_avatar_urls(&state).contains_key("unknown-member"));
    }
}
