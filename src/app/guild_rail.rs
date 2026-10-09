use gpui::*;
use super::state::{ActiveModal, AppState, GuildItemState};
use crate::tokens::icons::*;

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

        let render_guild_item = |guild: &GuildItemState| {
            let is_selected = selected_guild_id.as_deref() == Some(&guild.id);
            let guild_id = guild.id.clone();
            let acronym = guild.acronym.clone();
            let mention_count = guild.mention_count;
            let unread = guild.unread;
            let entity = app_entity.clone();

            let pill_height = if is_selected {
                px(40.0)
            } else if unread || mention_count > 0 {
                px(8.0)
            } else {
                px(0.0)
            };

            let icon_url = guild.icon_url.clone();
            let squircle_radius = if is_selected { px(16.0) } else { px(24.0) };

            div()
                .relative()
                .w(px(72.0))
                .h(px(48.0))
                .flex()
                .items_center()
                .justify_center()
                // Pill indicator
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .w(px(4.0))
                        .rounded_r(px(4.0))
                        .bg(rgb(0xffffff))
                        .h(pill_height),
                )
                // Guild Icon Container with Squircle & overflow_hidden
                .child(
                    div()
                        .relative()
                        .w(px(48.0))
                        .h(px(48.0))
                        .child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .overflow_hidden()
                                .rounded(squircle_radius)
                                .bg(if is_selected {
                                    rgb(0x5865f2)
                                } else {
                                    rgb(0x313338)
                                })
                                .text_color(if is_selected {
                                    rgb(0xffffff)
                                } else {
                                    rgb(0xdbdee1)
                                })
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(14.0))
                                .hover(|s| {
                                    s.rounded(px(16.0))
                                        .bg(rgb(0x5865f2))
                                        .text_color(rgb(0xffffff))
                                })
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    let gid = guild_id.clone();
                                    entity.update(cx, |state, cx| {
                                        state.select_guild(Some(gid));
                                        cx.notify();
                                    });
                                })
                                .children(if let Some(ref url) = icon_url {
                                    if !url.is_empty() {
                                        Some(
                                            img(url.clone())
                                                .size_full()
                                                .rounded(squircle_radius),
                                        )
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                })
                                .children(if icon_url.as_ref().map_or(true, |u| u.is_empty()) {
                                    Some(div().child(acronym))
                                } else {
                                    None
                                }),
                        )
                        // Mention badge
                        .children(if mention_count > 0 {
                            Some(
                                div()
                                    .absolute()
                                    .bottom(px(-2.0))
                                    .right(px(-2.0))
                                    .min_w(px(18.0))
                                    .h(px(18.0))
                                    .px(px(4.0))
                                    .rounded_full()
                                    .bg(rgb(0xf23f43))
                                    .border_2()
                                    .border_color(rgb(0x1e1f22))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(0xffffff))
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(mention_count.to_string()),
                            )
                        } else {
                            None
                        }),
                )
        };

        let guilds_elements: Vec<AnyElement> = if !guild_folders.is_empty() {
            let guilds_by_id: std::collections::HashMap<String, &GuildItemState> =
                guilds.iter().map(|g| (g.id.clone(), g)).collect();

            let mut elements = Vec::new();

            for folder in &guild_folders {
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
                    let folder_mention_count: u32 = child_guilds.iter().map(|g| g.mention_count).sum();
                    let folder_unread: bool = child_guilds.iter().any(|g| g.unread);
                    let folder_color_rgb = folder.color.unwrap_or(0x313338);

                    if is_expanded {
                        let fid = folder_id.clone();
                        let entity = app_entity.clone();
                        elements.push(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(8.0))
                                .w_full()
                                .py(px(4.0))
                                .rounded(px(24.0))
                                .bg(crate::tokens::colors::with_alpha(folder_color_rgb, 0.25))
                                // Header / collapse button
                                .child(
                                    div()
                                        .w(px(48.0))
                                        .h(px(32.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .rounded(px(16.0))
                                        .hover(|s| s.bg(rgb(0x35373c)))
                                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                            let fid = fid.clone();
                                            entity.update(cx, |state, cx| {
                                                state.toggle_folder(&fid);
                                                cx.notify();
                                            });
                                        })
                                        .child(icon_folder(px(20.0), rgb(0xdbdee1))),
                                )
                                // Child guild items
                                .children(child_guilds.into_iter().map(&render_guild_item))
                                .into_any_element(),
                        );
                    } else {
                        let fid = folder_id.clone();
                        let entity = app_entity.clone();
                        elements.push(
                            div()
                                .relative()
                                .w(px(72.0))
                                .h(px(48.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                // Pill indicator if child has unread or mention
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .w(px(4.0))
                                        .rounded_r(px(4.0))
                                        .bg(rgb(0xffffff))
                                        .h(if folder_mention_count > 0 || folder_unread {
                                            px(8.0)
                                        } else {
                                            px(0.0)
                                        }),
                                )
                                .child(
                                    div()
                                        .relative()
                                        .w(px(48.0))
                                        .h(px(48.0))
                                        .child(
                                            div()
                                                .size_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .rounded(px(24.0))
                                                .bg(rgb(folder_color_rgb))
                                                .text_color(rgb(0xffffff))
                                                .hover(|s| s.rounded(px(16.0)).bg(rgb(0x5865f2)))
                                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                                    let fid = fid.clone();
                                                    entity.update(cx, |state, cx| {
                                                        state.toggle_folder(&fid);
                                                        cx.notify();
                                                    });
                                                })
                                                .child(icon_folder(px(20.0), rgb(0xffffff))),
                                        )
                                        // Folder mention badge
                                        .children(if folder_mention_count > 0 {
                                            Some(
                                                div()
                                                    .absolute()
                                                    .bottom(px(-2.0))
                                                    .right(px(-2.0))
                                                    .min_w(px(18.0))
                                                    .h(px(18.0))
                                                    .px(px(4.0))
                                                    .rounded_full()
                                                    .bg(rgb(0xf23f43))
                                                    .border_2()
                                                    .border_color(rgb(0x1e1f22))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(rgb(0xffffff))
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(folder_mention_count.to_string()),
                                            )
                                        } else {
                                            None
                                        }),
                                )
                                .into_any_element(),
                        );
                    }
                } else if folder.guild_ids.len() == 1 {
                    if let Some(gid) = folder.guild_ids.first() {
                        if let Some(guild) = guilds_by_id.get(gid) {
                            elements.push(render_guild_item(guild).into_any_element());
                        }
                    }
                }
            }

            elements
        } else {
            guilds.iter().map(|g| render_guild_item(g).into_any_element()).collect()
        };

        div()
            .flex()
            .flex_col()
            .w(px(72.0))
            .h_full()
            .min_h(px(0.0))
            .bg(rgb(0x1e1f22))
            .items_center()
            .py(px(12.0))
            .gap(px(8.0))
            .flex_shrink_0()
            .overflow_hidden()
            // 1. Direct Messages / Home button
            .child({
                let is_selected = selected_guild_id.is_none();
                let entity = app_entity.clone();

                div()
                    .relative()
                    .w(px(72.0))
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    // Pill indicator on left edge
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .w(px(4.0))
                            .rounded_r(px(4.0))
                            .bg(rgb(0xffffff))
                            .h(if is_selected { px(40.0) } else { px(0.0) }),
                    )
                    // Icon squircle
                    .child(
                        div()
                            .w(px(48.0))
                            .h(px(48.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_size(px(20.0))
                            .bg(if is_selected {
                                rgb(0x5865f2)
                            } else {
                                rgb(0x313338)
                            })
                            .text_color(rgb(0xffffff))
                            .rounded(if is_selected { px(16.0) } else { px(24.0) })
                            .hover(|s| {
                                s.rounded(px(16.0))
                                    .bg(rgb(0x5865f2))
                                    .text_color(rgb(0xffffff))
                            })
                            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                entity.update(cx, |state, cx| {
                                    state.select_guild(None);
                                    cx.notify();
                                });
                            })
                            .child(icon_gamepad(px(20.0), if is_selected { rgb(0xffffff) } else { rgb(0xdbdee1) })),
                    )
            })
            // 2. Separator
            .child(
                div()
                    .w(px(32.0))
                    .h(px(2.0))
                    .my(px(2.0))
                    .rounded_full()
                    .bg(rgb(0x35363c)),
            )
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
            .child(
                div()
                    .w(px(32.0))
                    .h(px(2.0))
                    .my(px(2.0))
                    .rounded_full()
                    .bg(rgb(0x35363c)),
            )
            // 5. Add Server button (+)
            .child({
                let entity = app_entity.clone();
                div()
                    .w(px(48.0))
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(0x313338))
                    .text_color(rgb(0x23a55a))
                    .rounded(px(24.0))
                    .hover(|s| {
                        s.rounded(px(16.0))
                            .bg(rgb(0x23a55a))
                            .text_color(rgb(0xffffff))
                    })
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.open_modal(ActiveModal::AddServer);
                            cx.notify();
                        });
                    })
                    .child(icon_plus(px(20.0), rgb(0x23a55a)))
            })
            // 6. Explore Discoverable Servers button (Compass)
            .child({
                let entity = app_entity.clone();
                div()
                    .w(px(48.0))
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(rgb(0x313338))
                    .text_color(rgb(0x23a55a))
                    .rounded(px(24.0))
                    .hover(|s| {
                        s.rounded(px(16.0))
                            .bg(rgb(0x23a55a))
                            .text_color(rgb(0xffffff))
                    })
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.open_modal(ActiveModal::ExploreServers);
                            cx.notify();
                        });
                    })
                    .child(icon_compass(px(20.0), rgb(0x23a55a)))
            })
    }
}
