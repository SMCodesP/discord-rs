use gpui::component::tooltip::Tooltip;
use gpui::prelude::*;
use gpui::*;

use super::state::{ActiveModal, AppState, GuildItemState};
use crate::components::guild_item::{GuildFolderChild, GuildFolderItem, GuildItem};
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::spacing::*;

/// Server navigation sidebar (72px wide)
pub struct GuildRail {
    state: Entity<AppState>,
}

impl GuildRail {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }
}

impl Render for GuildRail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected_guild_id = state.selected_guild_id.clone();
        let guilds = state.guilds.clone();
        let guild_folders = state.guild_folders.clone();
        let expanded_folders = state.expanded_folders.clone();
        let app_entity = self.state.clone();

        let guilds_elements = render_guilds_list(
            &guilds,
            &guild_folders,
            &expanded_folders,
            selected_guild_id.as_deref(),
            &app_entity,
        );

        div()
            .flex()
            .flex_col()
            .w(rail_width())
            .h_full()
            .min_h(px(0.0))
            .bg(bg_server())
            .items_center()
            .py(px(12.0))
            .gap(px(8.0))
            .flex_shrink_0()
            .overflow_hidden()
            // 1. Direct Messages / Home button
            .child(render_home_button(selected_guild_id.is_none(), &app_entity))
            // 2. Separator
            .child(render_separator())
            // 3. Guilds list (scrollable)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .id("guild-rail-scroll")
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .items_center()
                    .children(guilds_elements),
            )
            // 4. Separator
            .child(render_separator())
            // 5. Add Server button (+)
            .child(render_add_server_button(&app_entity))
            // 6. Explore Discoverable Servers button (Compass)
            .child(render_explore_button(&app_entity))
    }
}

fn render_home_button(is_selected: bool, app_entity: &Entity<AppState>) -> AnyElement {
    let entity = app_entity.clone();
    GuildItem::new("guild-item-home", "Mensagens diretas")
        .is_home(true)
        .selected(is_selected)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.select_guild(None);
                cx.notify();
            });
        })
        .into_any_element()
}

fn render_separator() -> AnyElement {
    div()
        .w(px(32.0))
        .h(px(2.0))
        .my(px(2.0))
        .rounded(radius_full())
        .bg(borders())
        .into_any_element()
}

fn render_add_server_button(app_entity: &Entity<AppState>) -> AnyElement {
    let entity = app_entity.clone();
    div()
        .id("guild-rail-add-server")
        .w(px(40.0))
        .h(px(40.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(bg_chat())
        .text_color(status_online())
        .rounded(radius_server_hover())
        .hover(|s| {
            s.rounded(radius_server_hover())
                .bg(status_online())
                .text_color(crate::tokens::colors::white())
        })
        .tooltip(|window, cx| Tooltip::new("Adicionar um servidor").build(window, cx))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.open_modal(ActiveModal::AddServer);
                cx.notify();
            });
        })
        .child(icon_plus(px(20.0), status_online()))
        .into_any_element()
}

fn render_explore_button(app_entity: &Entity<AppState>) -> AnyElement {
    let entity = app_entity.clone();
    div()
        .id("guild-rail-explore")
        .w(px(40.0))
        .h(px(40.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(bg_chat())
        .text_color(status_online())
        .rounded(radius_server_hover())
        .hover(|s| {
            s.rounded(radius_server_hover())
                .bg(status_online())
                .text_color(crate::tokens::colors::white())
        })
        .tooltip(|window, cx| Tooltip::new("Descobrir servidores públicos").build(window, cx))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.open_modal(ActiveModal::ExploreServers);
                cx.notify();
            });
        })
        .child(icon_compass(px(20.0), status_online()))
        .into_any_element()
}

fn render_guilds_list(
    guilds: &[GuildItemState],
    guild_folders: &[crate::protocol::GuildFolder],
    expanded_folders: &std::collections::HashSet<String>,
    selected_guild_id: Option<&str>,
    app_entity: &Entity<AppState>,
) -> Vec<AnyElement> {
    if !guild_folders.is_empty() {
        let guilds_by_id: std::collections::HashMap<String, &GuildItemState> =
            guilds.iter().map(|g| (g.id.clone(), g)).collect();

        let mut elements = Vec::new();

        for folder in guild_folders {
            if folder.guild_ids.len() > 1 {
                let folder_id = folder
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("folder-{}", folder.guild_ids.join("-")));
                let is_expanded = expanded_folders.contains(&folder_id);
                let child_guilds: Vec<&GuildItemState> = folder
                    .guild_ids
                    .iter()
                    .filter_map(|gid| guilds_by_id.get(gid).copied())
                    .collect();

                let mut folder_item = GuildFolderItem::new(
                    folder_id.clone(),
                    format!("Pasta ({})", child_guilds.len()),
                )
                .expanded(is_expanded);

                if let Some(c) = folder.color {
                    folder_item = folder_item.color(rgb(c));
                }

                for child in &child_guilds {
                    let mut cg = GuildFolderChild::new(child.id.clone(), child.name.clone())
                        .selected(selected_guild_id == Some(&child.id))
                        .unread(child.unread)
                        .mention_count(child.mention_count);
                    if let Some(ref url) = child.icon_url {
                        cg = cg.icon_url(url.clone());
                    }
                    folder_item = folder_item.child_guild(cg);
                }

                let fid = folder_id.clone();
                let entity = app_entity.clone();
                folder_item = folder_item.on_toggle(move |_ev, _win, cx| {
                    let fid = fid.clone();
                    entity.update(cx, |state, cx| {
                        state.toggle_folder(&fid);
                        cx.notify();
                    });
                });

                let child_gids: Vec<String> = child_guilds.iter().map(|g| g.id.clone()).collect();
                let entity2 = app_entity.clone();
                folder_item = folder_item.on_child_click(move |idx, _ev, _win, cx| {
                    if let Some(gid) = child_gids.get(idx).cloned() {
                        entity2.update(cx, |state, cx| {
                            state.select_guild(Some(gid));
                            cx.notify();
                        });
                    }
                });

                elements.push(folder_item.into_any_element());
            } else if folder.guild_ids.len() == 1 {
                if let Some(gid) = folder.guild_ids.first() {
                    if let Some(guild) = guilds_by_id.get(gid) {
                        elements.push(render_single_guild_item(guild, selected_guild_id, app_entity));
                    }
                }
            }
        }

        elements
    } else {
        guilds
            .iter()
            .map(|g| render_single_guild_item(g, selected_guild_id, app_entity))
            .collect()
    }
}

fn render_single_guild_item(
    guild: &GuildItemState,
    selected_guild_id: Option<&str>,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let is_selected = selected_guild_id == Some(&guild.id);
    let gid = guild.id.clone();
    let entity = app_entity.clone();

    let mut item = GuildItem::new(guild.id.clone(), guild.name.clone())
        .selected(is_selected)
        .unread(guild.unread)
        .mention_count(guild.mention_count);

    if let Some(ref url) = guild.icon_url {
        item = item.icon_url(url.clone());
    }

    item = item.on_click(move |_event, _window, cx| {
        let gid = gid.clone();
        entity.update(cx, |state, cx| {
            state.select_guild(Some(gid));
            cx.notify();
        });
    });

    item.into_any_element()
}
