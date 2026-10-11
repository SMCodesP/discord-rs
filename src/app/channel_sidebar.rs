use gpui::component::menu::{ContextMenuExt, PopupMenuItem, PopupMenu};
use gpui::prelude::*;
use gpui::*;

use super::state::{ActiveModal, ActiveVoiceCall, AppState, ChannelItemState, ChannelKind, CurrentUserProfile, FriendsTab};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::badge::MentionBadge;
use crate::components::button::IconButton;
use crate::domain::user::{ActivityType, PresenceStatus};
use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;

#[inline]
fn icon_speaker_off(size: Pixels, color: impl Into<Hsla>) -> Div {
    icon_headphones_off(size, color)
}

/// Channel sidebar containing:
/// - Server header / DM header
/// - Collapsible categories & channel lists / DM list
/// - Active voice status bar (when connected)
/// - Bottom user profile & audio controls panel (359x58px, 32x32 buttons)
pub struct ChannelSidebar {
    state: Entity<AppState>,
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
            focus_handle: cx.focus_handle(),
        }
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

        let header_el = if let Some(ref guild) = selected_guild {
            render_server_header(&guild.name)
        } else {
            render_dm_search_header()
        };

        let scroll_content: Vec<AnyElement> = if is_dm {
            render_dm_section(state, selected_channel_id.as_deref(), &app_entity)
        } else if let Some(ref guild) = selected_guild {
            render_guild_channels_section(state, guild, selected_channel_id.as_deref(), &app_entity, &current_user)
        } else {
            Vec::new()
        };

        div()
            .track_focus(&self.focus_handle)
            .key_context("channel_sidebar")
            .relative()
            .flex()
            .flex_col()
            .w(px(240.0))
            .h_full()
            .min_h(px(0.0))
            .bg(bg_channels())
            .justify_between()
            .flex_shrink_0()
            // =========================================================================
            // TOP SECTION: Header + Scrollable Channels / DMs
            // =========================================================================
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(header_el)
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .min_h(px(0.0))
                            .id("channels-scroll-area")
                            .overflow_y_scroll()
                            .p(px(8.0))
                            .gap(px(2.0))
                            .children(scroll_content),
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
                    .children(active_voice.map(|v| render_voice_bar(&v, &app_entity)))
                    .child(render_user_panel(&current_user, &app_entity)),
            )
    }
}

// ============================================================================
// Header elements
// ============================================================================

fn render_server_header(guild_name: &str) -> AnyElement {
    div()
        .h(px(49.0)) // 303x49 per channel-sidebar.md
        .px(px(16.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(border_subtle())
        .cursor_pointer()
        .hover(|s| s.bg(channel_hover()))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .overflow_hidden()
                .child(
                    div()
                        .text_color(text_header())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(15.0))
                        .child(guild_name.to_string()),
                )
                .child(
                    div().child(icon_check(px(14.0), blurple())),
                ),
        )
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size(px(16.0))
                .text_color(text_normal()),
        )
        .into_any_element()
}

fn render_dm_search_header() -> AnyElement {
    div()
        .h(px(48.0))
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .border_b_1()
        .border_color(border_subtle())
        .child(
            div()
                .w_full()
                .h(px(28.0))
                .px(px(8.0))
                .rounded(radius_sm())
                .bg(bg_server())
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(12.0))
                        .child("Encontre ou comece uma conversa"),
                ),
        )
        .into_any_element()
}

// ============================================================================
// Direct Messages Section
// ============================================================================

fn render_dm_section(
    state: &AppState,
    selected_channel_id: Option<&str>,
    app_entity: &Entity<AppState>,
) -> Vec<AnyElement> {
    let is_amigos_active = selected_channel_id.is_none() && state.friends_tab != FriendsTab::MessageRequests;
    let is_msg_requests_active = selected_channel_id.is_none() && state.friends_tab == FriendsTab::MessageRequests;
    let pending_req_count = state.message_requests.iter().filter(|r| !r.is_spam).count();

    let mut items = vec![
        render_dm_nav_button(
            "Amigos",
            icon_friends(px(20.0), if is_amigos_active { white() } else { text_muted() }),
            is_amigos_active,
            None,
            {
                let entity = app_entity.clone();
                move |_ev, _win, cx| {
                    entity.update(cx, |s, cx| {
                        s.selected_channel_id = None;
                        s.friends_tab = FriendsTab::Online;
                        cx.notify();
                    });
                }
            },
        ),
        render_dm_nav_button(
            "Solicitações de mensagens",
            icon_inbox(px(20.0), if is_msg_requests_active { white() } else { text_muted() }),
            is_msg_requests_active,
            if pending_req_count > 0 { Some(pending_req_count) } else { None },
            {
                let entity = app_entity.clone();
                move |_ev, _win, cx| {
                    entity.update(cx, |s, cx| {
                        s.selected_channel_id = None;
                        s.friends_tab = FriendsTab::MessageRequests;
                        cx.notify();
                    });
                }
            },
        ),
        render_dm_nav_button(
            "Nitro",
            icon_sparkles(px(20.0), text_muted()),
            false,
            None,
            |_ev, _win, _cx| {},
        ),
        render_dm_nav_button(
            "Loja",
            icon_gift(px(20.0), text_muted()),
            false,
            None,
            |_ev, _win, _cx| {},
        ),
        render_dm_section_header(app_entity),
    ];

    if state.dm_channels.is_empty() {
        items.push(
            div()
                .px(px(8.0))
                .py(px(12.0))
                .text_color(text_muted())
                .text_size(px(12.0))
                .child("Nenhuma conversa direta aberta.")
                .into_any_element(),
        );
    } else {
        for dm in &state.dm_channels {
            items.push(render_dm_item(dm, state, selected_channel_id, app_entity));
        }
    }

    items
}

fn render_dm_nav_button(
    label: &'static str,
    icon: Div,
    is_active: bool,
    badge_count: Option<usize>,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut row = div()
        .h(px(36.0))
        .px(px(12.0))
        .rounded(radius_sm())
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer();

    if is_active {
        row = row.bg(channel_active()).text_color(white());
    } else {
        row = row
            .hover(|s| s.bg(channel_hover()).text_color(text_normal()))
            .text_color(text_muted());
    }

    row = row
        .font_weight(FontWeight::SEMIBOLD)
        .on_mouse_down(MouseButton::Left, move |ev, win, cx| on_click(ev, win, cx))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(icon)
                .child(
                    div()
                        .truncate()
                        .child(label),
                ),
        );

    if let Some(count) = badge_count {
        row = row.child(MentionBadge::new(count as u32));
    }

    row.into_any_element()
}

fn render_dm_section_header(app_entity: &Entity<AppState>) -> AnyElement {
    let entity = app_entity.clone();
    div()
        .pt(px(16.0))
        .pb(px(4.0))
        .px(px(8.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_color(text_muted())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .child("MENSAGENS DIRETAS"),
        )
        .child(
            IconButton::with_icon("dm-create-group-btn", |sz, col| {
                icon_plus(sz, col).into_any_element()
            })
            .size(px(18.0))
            .icon_size(px(12.0))
            .rounded(px(3.0))
            .tooltip("Criar DM")
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |s, cx| {
                    s.open_modal(ActiveModal::CreateGroupDm);
                    cx.notify();
                });
            }),
        )
        .into_any_element()
}

fn render_dm_item(
    dm: &ChannelItemState,
    state: &AppState,
    selected_channel_id: Option<&str>,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let dm_id = dm.id.clone();
    let dm_name = dm.name.clone();
    let is_selected = selected_channel_id == Some(&dm_id);
    let mention_count = dm.mention_count;
    let unread = dm.unread;
    let entity = app_entity.clone();
    let is_group = dm.is_group_dm || dm.recipient_names.len() > 1;

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
        .or_else(|| recipient_friend.and_then(|f| state.get_user_presence(&f.id)));

    let presence_status = presence_info
        .map(|p| p.status)
        .or_else(|| recipient_friend.map(|f| crate::domain::user::PresenceStatus::from(f.presence)))
        .unwrap_or(PresenceStatus::Offline);

    let custom_status = if let Some(p) = presence_info {
        p.custom_status.clone()
    } else {
        recipient_friend.and_then(|f| {
            f.custom_status
                .as_ref()
                .map(|text| crate::domain::user::CustomStatus::from_text(text.clone()))
        })
    };

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
        dm.avatar_decoration_data.clone(),
    );

    let subtitle_el = if is_group {
        None
    } else {
        render_dm_subtitle(custom_status.as_ref(), game_activity, presence_status == PresenceStatus::Offline)
    };

    let is_pinned = dm.is_pinned || dm.topic.as_deref().map_or(false, |t| t.contains("pin") || t.contains("fixad"));
    let text_col_el = render_dm_text_column(dm_name, is_selected, unread, is_pinned, subtitle_el);

    let click_entity = entity.clone();
    let click_cid = dm_id.clone();

    // DM row geometry: 258x42, padding-left 8px, gap 8px, radius 8px per dm-list.md
    let mut row = div()
        .id(ElementId::Name(format!("dm-item-{}", dm_id).into()))
        .h(px(42.0))
        .px(px(8.0))
        .rounded(radius_md()) // 8px radius per dm-list.md
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer();

    if is_selected {
        row = row.bg(channel_active());
    } else {
        row = row.hover(|s| s.bg(channel_hover()));
    }

    row = row
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let cid = click_cid.clone();
            click_entity.update(cx, |s, cx| {
                s.select_channel(cid);
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0)) // 8px gap per dm-list.md
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child({
                    let mut av = div().child(avatar_el).cursor_pointer();
                    if let Some(uid) = dm.recipient_ids.first().cloned() {
                        let av_entity = entity.clone();
                        av = av.on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                            let uid = uid.clone();
                            let pos = ev.position;
                            av_entity.update(cx, |s, cx| {
                                s.open_user_profile(&uid, Some(pos));
                                cx.notify();
                            });
                        });
                    }
                    av
                })
                .child(text_col_el),
        );

    // Mention badge
    if mention_count > 0 {
        row = row.child(MentionBadge::new(mention_count));
    }

    // Action icons
    let add_entity = entity.clone();
    let add_cid = dm_id.clone();

    row = row.child(
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(
                IconButton::with_icon(format!("dm-add-friend-{}", dm_id), |sz, col| {
                    icon_user(sz, col).into_any_element()
                })
                .size(px(20.0))
                .icon_size(px(14.0))
                .rounded(radius_sm())
                .tooltip("Adicionar amigos à DM")
                .on_click(move |_event, _window, cx| {
                    let cid = add_cid.clone();
                    add_entity.update(cx, |s, cx| {
                        s.open_modal(ActiveModal::AddFriendToGroup(cid));
                        cx.notify();
                    });
                }),
            )
            .child(
                IconButton::with_icon(format!("dm-close-{}", dm_id), |sz, col| {
                    icon_close(sz, col).into_any_element()
                })
                .size(px(16.0))
                .icon_size(px(12.0))
                .rounded(radius_full())
                .tooltip("Fechar DM"),
            ),
    );

    // Context menu via gpui-kit ContextMenuExt & PopupMenu
    let pin_entity = entity.clone();
    let pin_cid = dm_id.clone();
    let pin_label = if is_pinned { "Desafixar conversa" } else { "Fixar conversa" };

    row.context_menu(move |menu: PopupMenu, _window, _cx| {
        let p_app = pin_entity.clone();
        let p_id = pin_cid.clone();
        menu.item(
            PopupMenuItem::new(pin_label).on_click(move |_ev, _win, cx| {
                p_app.update(cx, |state, cx| {
                    state.toggle_pin_dm_channel(&p_id);
                    cx.notify();
                });
            }),
        )
    })
    .into_any_element()
}

// ============================================================================
// Guild Channels Section
// ============================================================================

fn render_guild_channels_section(
    state: &AppState,
    guild: &super::state::GuildItemState,
    selected_channel_id: Option<&str>,
    app_entity: &Entity<AppState>,
    current_user: &CurrentUserProfile,
) -> Vec<AnyElement> {
    let mut category_elements = Vec::new();

    for category in &guild.categories {
        let cat_id = category.id.clone();
        let cat_name = category.name.clone();
        let is_collapsed = state.is_category_collapsed(&cat_id);

        category_elements.push(render_category_header(&cat_id, &cat_name, is_collapsed, app_entity));

        if !is_collapsed {
            for channel in &category.channels {
                category_elements.push(render_channel_entry(channel, state, selected_channel_id, app_entity, current_user));
            }
        }
    }

    category_elements
}

fn render_category_header(
    cat_id: &str,
    cat_name: &str,
    is_collapsed: bool,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let entity = app_entity.clone();
    let cid = cat_id.to_string();

    let toggle_entity = app_entity.clone();
    let toggle_cid = cid.clone();

    let chevron_btn = IconButton::with_icon(format!("cat-chevron-{}", cat_id), move |sz, col| {
        let path = if is_collapsed {
            "icons/chevron-right.svg"
        } else {
            "icons/chevron-down.svg"
        };
        svg().path(path).size(sz).text_color(col).into_any_element()
    })
    .size(px(16.0))
    .icon_size(px(10.0))
    .rounded(radius_xs())
    .on_click(move |_event, _window, cx| {
        let cid = toggle_cid.clone();
        toggle_entity.update(cx, |state, cx| {
            state.toggle_category(&cid);
            cx.notify();
        });
    });

    div()
        .pt(px(12.0))
        .pb(px(4.0))
        .px(px(4.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(chevron_btn)
                .child(
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            let cid = cid.clone();
                            entity.update(cx, |state, cx| {
                                state.toggle_category(&cid);
                                cx.notify();
                            });
                        })
                        .text_color(text_muted())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child(cat_name.to_string()),
                ),
        )
        .into_any_element()
}

fn render_channel_entry(
    channel: &ChannelItemState,
    state: &AppState,
    selected_channel_id: Option<&str>,
    app_entity: &Entity<AppState>,
    current_user: &CurrentUserProfile,
) -> AnyElement {
    let ch_id = channel.id.clone();
    let ch_name = channel.name.clone();
    let ch_kind = channel.kind;
    let is_selected = selected_channel_id == Some(&ch_id);
    let is_unread = channel.unread;
    let mention_count = channel.mention_count;
    let entity = app_entity.clone();

    let is_voice = ch_kind == ChannelKind::Voice;
    let voice_states = if is_voice {
        state.voice_states_for_channel(&ch_id)
    } else {
        &[]
    };

    let capacity_badge = if is_voice {
        let count = voice_states.len();
        let limit = channel.user_limit.unwrap_or(0);
        if limit > 0 {
            let is_full = count >= limit as usize;
            Some(
                div()
                    .h(px(18.0))
                    .px(px(6.0))
                    .rounded(px(9.0))
                    .bg(if is_full { status_dnd() } else { bg_user_bar() })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .font_weight(if is_full { FontWeight::BOLD } else { FontWeight::MEDIUM })
                    .text_color(if is_full { white() } else { interactive_normal() })
                    .child(format!("{}/{}", count, limit)),
            )
        } else if count > 0 {
            Some(
                div()
                    .h(px(18.0))
                    .px(px(6.0))
                    .rounded(px(9.0))
                    .bg(bg_user_bar())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(interactive_normal())
                    .child(count.to_string()),
            )
        } else {
            None
        }
    } else {
        None
    };

    let click_cid = ch_id.clone();
    let click_entity = entity.clone();

    // Channel line geometry: 286x32 r8 per channel-sidebar.md
    let mut row = div()
        .id(ElementId::Name(format!("channel-row-{}", ch_id).into()))
        .h(px(32.0)) // 32px per channel-sidebar.md
        .px(px(8.0))
        .py(px(4.0))
        .rounded(radius_md()) // 8px per channel-sidebar.md
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .cursor_pointer();

    if is_selected {
        row = row.bg(channel_active());
    } else {
        row = row.hover(|s| s.bg(channel_hover()));
    }

    let channel_icon_color = if is_selected {
        white()
    } else if is_unread {
        text_header()
    } else {
        channels_default()
    };

    row = row
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let cid = click_cid.clone();
            click_entity.update(cx, |state, cx| {
                state.select_channel(cid);
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(render_icon(ch_kind.discord_icon(), px(18.0), channel_icon_color))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(if is_selected || is_unread { FontWeight::BOLD } else { FontWeight::NORMAL })
                        .text_color(if is_selected { white() } else if is_unread { text_header() } else { text_muted() })
                        .truncate()
                        .child(ch_name),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(capacity_badge)
                .children(if mention_count > 0 {
                    Some(MentionBadge::new(mention_count).into_any_element())
                } else {
                    None
                }),
        );

    // Context menu via gpui-kit ContextMenuExt
    let mut channel_container = div()
        .flex()
        .flex_col()
        .child(
            row.context_menu(|menu: PopupMenu, _window, _cx| {
                menu
                    .item(PopupMenuItem::new("Marcar como lido"))
                    .item(PopupMenuItem::new("Silenciar canal"))
                    .separator()
                    .item(PopupMenuItem::new("Copiar link do canal"))
            })
        );

    // Voice participants under channel
    if is_voice && !voice_states.is_empty() {
        for vs in voice_states {
            channel_container = channel_container.child(render_voice_user_row(vs, state, current_user));
        }
    }

    channel_container.into_any_element()
}

fn render_voice_user_row(
    vs: &crate::domain::VoiceState,
    state: &AppState,
    current_user: &CurrentUserProfile,
) -> AnyElement {
    let vs_uid = vs.user_id.clone();
    let display_name = if let Some(ref name) = vs.user_name {
        name.clone()
    } else if vs_uid == current_user.id {
        current_user.display_name.clone()
    } else if let Some(m) = state.members.iter().find(|m| m.id == vs_uid) {
        m.name.clone()
    } else if let Some(f) = state.friends.iter().find(|f| f.id == vs_uid) {
        f.display_name.clone()
    } else {
        String::new()
    };

    let avatar_url = vs.avatar_url.clone().or_else(|| {
        if vs_uid == current_user.id {
            current_user.avatar_url.clone()
        } else {
            state.friends.iter().find(|f| f.id == vs_uid).and_then(|f| f.avatar_url.clone())
        }
    });

    let is_speaking = vs.is_speaking;
    let is_muted = vs.is_muted();
    let is_deafened = vs.is_deafened();
    let is_streaming = vs.self_stream;

    // Mini-avatar 24x24 using crate::components::Avatar (zero rounded_full)
    let avatar_el = Avatar::new(display_name.clone())
        .size(AvatarSize::Px24)
        .opt_src(avatar_url)
        .cutout_color(bg_channels())
        .opt_decoration(vs.avatar_decoration_data.clone());

    div()
        .h(px(28.0))
        .pl(px(32.0))
        .pr(px(8.0))
        .rounded(radius_sm())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .hover(|s| s.bg(channel_hover()))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .overflow_hidden()
                .child(
                    div()
                        .size(px(24.0))
                        .rounded(radius_full())
                        .border(if is_speaking { px(2.0) } else { px(0.0) })
                        .border_color(status_online())
                        .child(avatar_el),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(interactive_normal())
                        .overflow_hidden()
                        .child(display_name),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(if is_streaming {
                    Some(
                        div()
                            .px(px(4.0))
                            .py(px(1.0))
                            .rounded_xs()
                            .bg(status_dnd())
                            .text_color(white())
                            .text_size(px(9.0))
                            .font_weight(FontWeight::BOLD)
                            .child("AO VIVO"),
                    )
                } else {
                    None
                })
                .children(if is_muted {
                    Some(icon_mic_off(px(14.0), status_dnd()))
                } else {
                    None
                })
                .children(if is_deafened {
                    Some(icon_speaker_off(px(14.0), status_dnd()))
                } else {
                    None
                }),
        )
        .into_any_element()
}

// ============================================================================
// Voice bar & User panel
// ============================================================================

fn render_voice_bar(voice: &ActiveVoiceCall, app_entity: &Entity<AppState>) -> AnyElement {
    let entity = app_entity.clone();
    div()
        .h(px(52.0))
        .px(px(12.0))
        .bg(bg_user_bar())
        .border_b_1()
        .border_color(bg_server())
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
                .child(icon_wifi(px(16.0), status_online()))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_color(status_online())
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(12.0))
                                .child("Voz Conectada"),
                        )
                        .child(
                            div()
                                .text_color(text_muted())
                                .text_size(px(11.0))
                                .child(format!("{} / {}ms", voice.channel_name, voice.ping_ms)),
                        ),
                ),
        )
        .child(
            IconButton::with_icon("voice-disconnect-btn", |sz, col| {
                icon_phone_hangup(sz, col).into_any_element()
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(radius_sm())
            .danger(true)
            .tooltip("Desconectar")
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.disconnect_voice();
                    cx.notify();
                });
            }),
        )
        .into_any_element()
}

fn render_user_panel(current_user: &CurrentUserProfile, app_entity: &Entity<AppState>) -> AnyElement {
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

    // User Panel (359x58px r8 per user-panel.md)
    div()
        .h(px(58.0)) // 58px per user-panel.md
        .px(px(8.0))
        .bg(bg_user_bar())
        .rounded(radius_md()) // 8px per user-panel.md
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
                .rounded(radius_sm())
                .cursor_pointer()
                .hover(|s| s.bg(channel_hover()))
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    move |_event, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.open_modal(ActiveModal::UserSettings);
                            cx.notify();
                        });
                    }
                })
                .child(
                    Avatar::new(current_user.display_name.clone())
                        .size(AvatarSize::Px32)
                        .opt_src(current_user.avatar_url.clone())
                        .status(current_user.presence.into())
                        .cutout_color(bg_user_bar())
                        .opt_decoration(current_user.avatar_decoration_data.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_color(text_header())
                                .font_weight(FontWeight::BOLD)
                                .text_size(px(13.0))
                                .child(current_user.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_color(interactive_normal())
                                .text_size(px(11.0))
                                .child(user_tag),
                        ),
                ),
        )
        // 32x32 Action Buttons (per user-panel.md)
        .child(render_user_audio_buttons(is_muted, is_deafened, app_entity))
        .into_any_element()
}

fn render_user_audio_buttons(
    is_muted: bool,
    is_deafened: bool,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let entity = app_entity.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        // Mute Button (32x32)
        .child({
            let entity = entity.clone();
            let mute_tooltip = if is_muted { "Ativar microfone" } else { "Desativar microfone" };
            IconButton::with_icon("user-panel-mute-btn", move |sz, _col| {
                if is_muted {
                    icon_mic_off(sz, status_dnd()).into_any_element()
                } else {
                    icon_mic(sz, text_normal()).into_any_element()
                }
            })
            .size(px(32.0))
            .icon_size(px(18.0))
            .rounded(radius_md())
            .danger(is_muted)
            .tooltip(mute_tooltip)
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.toggle_mute();
                    cx.notify();
                });
            })
        })
        // Deafen Button (32x32)
        .child({
            let entity = entity.clone();
            let deafen_tooltip = if is_deafened { "Desativar áudio" } else { "Ensordecer" };
            IconButton::with_icon("user-panel-deafen-btn", move |sz, _col| {
                if is_deafened {
                    icon_headphones_off(sz, status_dnd()).into_any_element()
                } else {
                    icon_headphones(sz, text_normal()).into_any_element()
                }
            })
            .size(px(32.0))
            .icon_size(px(18.0))
            .rounded(radius_md())
            .danger(is_deafened)
            .tooltip(deafen_tooltip)
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.toggle_deafen();
                    cx.notify();
                });
            })
        })
        // Settings Button (32x32)
        .child({
            let entity = entity.clone();
            IconButton::with_icon("user-panel-settings-btn", |sz, _col| {
                icon_gear(sz, text_normal()).into_any_element()
            })
            .size(px(32.0))
            .icon_size(px(18.0))
            .rounded(radius_md())
            .tooltip("Configurações de Usuário")
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| {
                    state.open_modal(ActiveModal::UserSettings);
                    cx.notify();
                });
            })
        })
        .into_any_element()
}

// ============================================================================
// DM Subtitle & Avatar helpers
// ============================================================================

fn render_dm_avatar(
    is_group: bool,
    dm_name: &str,
    recipient_names: &[String],
    icon_url: Option<&str>,
    recipient_friend_avatar: Option<&str>,
    presence_status: PresenceStatus,
    avatar_decoration: Option<crate::domain::user::AvatarDecorationData>,
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
            .size(px(40.0)) // 40x40 per dm-list.md
            .relative()
            .flex_shrink_0()
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .size(px(24.0))
                    .rounded(radius_full())
                    .bg(blurple())
                    .border_2()
                    .border_color(bg_channels())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(white())
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .child(first_initial),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(0.0))
                    .right(px(0.0))
                    .size(px(24.0))
                    .rounded(radius_full())
                    .bg(badge_brand_bubble())
                    .border_2()
                    .border_color(bg_channels())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(white())
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .child(second_badge),
            )
            .into_any_element()
    } else {
        let avatar_src = icon_url.or(recipient_friend_avatar).map(|s| s.to_string());
        Avatar::new(dm_name.to_string())
            .size(AvatarSize::Px32) // 32px photo inside 42px row
            .opt_src(avatar_src)
            .status(presence_status.into())
            .cutout_color(bg_channels())
            .opt_decoration(avatar_decoration)
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
        white()
    } else if unread {
        text_header()
    } else {
        interactive_normal()
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
        title_row = title_row.child(icon_pin(px(12.0), text_muted()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_dm_text_column_creation() {
        let el = render_dm_text_column("Alice".to_string(), false, false, true, None);
        // Ensure it builds without panic
        let _ = el;
    }

    #[::core::prelude::v1::test]
    fn test_server_header_creation() {
        let el = render_server_header("Rust Server");
        let _ = el;
    }
}
