use gpui::component::input::{InputEvent, InputState};
use gpui::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui::component::popover::Popover;
use gpui::component::tooltip::Tooltip;
use gpui::component::Selectable;
use gpui::prelude::*;
use gpui::*;

use super::state::{
    ActiveModal, ActiveVoiceCall, AppState, CategoryItemState, ChannelItemState, ChannelKind,
    CurrentUserProfile, FriendsTab, SettingsTab, VoiceQuality,
};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::badge::MentionBadge;
use crate::components::button::{Button, ButtonSize, IconButton};
use crate::components::status_picker::{
    CustomStatusDraft, CustomStatusExpiration, UserStatusSelection,
};
use crate::components::text_input::TextInput;
use crate::domain::user::{ActivityType, PresenceStatus};
use crate::i18n::t;
use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

#[inline]
fn icon_speaker_off(size: Pixels, color: impl Into<Hsla>) -> Div {
    icon_headphones_off(size, color)
}

// ============================================================================
// Measured Discord Desktop channel-sidebar metrics
// ============================================================================
//
// Source: `/tmp/opencode/discord-shell-measurements.md` as recorded in
// `docs/PARITY_MASTER_TASKS.md` (T-072):
// * rail 72px + channel sidebar 272px = 344px total; the 4px resize handle
//   lives inside the sidebar's right gutter.
// * channel link 255x32, radius 8px, padding 4px 8px, icon 20px,
//   label 16px/500/24px.
// * normal label #8989aa, selected label #e1e1fe, selected background #131316.
// * user panel 328x58 at x=8 with an 8px bottom margin; user block 153x46,
//   controls 146x32 with 8px gap and 32x32/r8 buttons.

/// Narrowest channel sidebar the resize interaction allows.
pub const RAW_CHANNEL_SIDEBAR_MIN_WIDTH: f32 = 220.0;

/// Widest channel sidebar the resize interaction allows.
pub const RAW_CHANNEL_SIDEBAR_MAX_WIDTH: f32 = 420.0;

/// Width of the draggable divider inside the sidebar's right edge.
pub const RAW_CHANNEL_SIDEBAR_HANDLE_WIDTH: f32 = 4.0;

/// Measured Discord Desktop channel link height (32px).
pub const RAW_SIDEBAR_CHANNEL_LINK_HEIGHT: f32 = 32.0;

/// Measured Discord Desktop channel link radius (8px).
pub const RAW_SIDEBAR_CHANNEL_LINK_RADIUS: f32 = 8.0;

/// Measured user block inside the floating panel (153x46px).
pub const RAW_USER_PANEL_PROFILE_WIDTH: f32 = 153.0;
pub const RAW_USER_PANEL_PROFILE_HEIGHT: f32 = 46.0;

/// Measured audio-control cluster inside the floating panel (146x32px, gap 8px).
pub const RAW_USER_PANEL_CONTROLS_WIDTH: f32 = 146.0;
pub const RAW_USER_PANEL_CONTROLS_HEIGHT: f32 = 32.0;
pub const RAW_USER_PANEL_CONTROLS_GAP: f32 = 8.0;

/// Measured normal channel label colour (#8989aa).
pub const HEX_SIDEBAR_CHANNEL_TEXT: u32 = 0x8989aa;
/// Measured selected channel label colour (#e1e1fe).
pub const HEX_SIDEBAR_CHANNEL_SELECTED_TEXT: u32 = 0xe1e1fe;
/// Measured selected channel background (#131316).
pub const HEX_SIDEBAR_CHANNEL_SELECTED_BG: u32 = 0x131316;

/// Normal channel label colour from the official measurement.
#[inline]
pub fn sidebar_channel_text() -> Rgba {
    rgb(HEX_SIDEBAR_CHANNEL_TEXT)
}

/// Selected channel label colour from the official measurement.
#[inline]
pub fn sidebar_channel_selected_text() -> Rgba {
    rgb(HEX_SIDEBAR_CHANNEL_SELECTED_TEXT)
}

/// Selected channel background from the official measurement.
#[inline]
pub fn sidebar_channel_selected_bg() -> Rgba {
    rgb(HEX_SIDEBAR_CHANNEL_SELECTED_BG)
}

/// Clamps a requested sidebar width to the range the resize interaction supports.
#[inline]
pub fn clamp_channel_sidebar_width(width: f32) -> f32 {
    width.clamp(
        RAW_CHANNEL_SIDEBAR_MIN_WIDTH,
        RAW_CHANNEL_SIDEBAR_MAX_WIDTH,
    )
}

/// Total first-column width (rail + channel sidebar) at the default state.
#[inline]
pub fn default_channel_sidebar_shell_width() -> f32 {
    RAW_RAIL_WIDTH + RAW_CHANNEL_SIDEBAR_WIDTH
}

/// Channel link width for a given sidebar width: sidebar minus the 8px left and
/// 9px right gutters measured on Discord Desktop (255px at the 272px default).
#[inline]
pub fn channel_link_width(sidebar_width: f32) -> f32 {
    (clamp_channel_sidebar_width(sidebar_width) - RAW_SPACE_4 - 9.0).max(0.0)
}

/// Floating user-panel width tracks the sidebar so the measured 8px inset on
/// both sides is preserved while resizing (328px at the 272px default).
#[inline]
pub fn user_panel_width_for_sidebar(sidebar_width: f32) -> f32 {
    (clamp_channel_sidebar_width(sidebar_width) + RAW_RAIL_WIDTH
        - 2.0 * RAW_USER_PANEL_MARGIN)
        .max(0.0)
}

/// Path of the local preference that persists the channel sidebar width.
pub fn channel_sidebar_width_cache_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|base| base.join("discord-rs").join("channel_sidebar_width"))
}

/// Loads a persisted sidebar width, clamped to the supported range.
pub fn load_channel_sidebar_width_from_path(
    path: &std::path::Path,
) -> Option<f32> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value = raw.trim().parse::<f32>().ok()?;
    value.is_finite().then(|| clamp_channel_sidebar_width(value))
}

/// Persists the sidebar width, writing through a temporary sibling file so a
/// crash cannot leave a truncated preference behind.
pub fn save_channel_sidebar_width_to_path(path: &std::path::Path, width: f32) -> std::io::Result<()> {
    let width = clamp_channel_sidebar_width(width);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, format!("{}\n", width))?;
    std::fs::rename(tmp, path)
}

/// Capacity badge text for a voice channel; mirrors what the channel row shows.
pub fn voice_capacity_badge_text(count: usize, limit: Option<u32>) -> Option<String> {
    match limit {
        Some(limit) if limit > 0 => Some(format!("{}/{}", count, limit)),
        _ if count > 0 => Some(count.to_string()),
        _ => None,
    }
}

/// Whether a voice channel is at or above its configured user limit.
#[inline]
pub fn voice_capacity_is_full(count: usize, limit: Option<u32>) -> bool {
    matches!(limit, Some(limit) if limit > 0 && count >= limit as usize)
}

/// Payload used by the sidebar resize drag.
#[derive(Clone, Copy)]
struct SidebarResizeDrag;

/// Invisible drag proxy; the drag API requires a renderable value.
struct SidebarResizeGhost;

impl Render for SidebarResizeGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size(px(0.0))
    }
}

/// Channel sidebar containing:
/// - Server header / DM header
/// - Collapsible categories & channel lists / DM list
/// - Active voice status bar (when connected)
/// - Floating bottom user profile & audio controls panel (328x58px, 32x32 buttons)
pub struct ChannelSidebar {
    state: Entity<AppState>,
    focus_handle: FocusHandle,
    dm_search_state: Option<Entity<InputState>>,
    dm_search_sub: Option<Subscription>,
    dm_search_query: String,
    width: f32,
    width_cache_path: Option<std::path::PathBuf>,
    status_text_state: Option<Entity<InputState>>,
    status_emoji_state: Option<Entity<InputState>>,
    status_text_sub: Option<Subscription>,
    status_emoji_sub: Option<Subscription>,
    status_picker_was_open: bool,
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

        let width_cache_path = channel_sidebar_width_cache_path();
        let width = width_cache_path
            .as_deref()
            .and_then(load_channel_sidebar_width_from_path)
            .unwrap_or(RAW_CHANNEL_SIDEBAR_WIDTH);

        Self {
            state,
            focus_handle: cx.focus_handle(),
            dm_search_state: None,
            dm_search_sub: None,
            dm_search_query: String::new(),
            width,
            width_cache_path,
            status_text_state: None,
            status_emoji_state: None,
            status_text_sub: None,
            status_emoji_sub: None,
            status_picker_was_open: false,
        }
    }

    /// Current sidebar width, including the 4px resize gutter.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Applies a resized width and persists it when it changes by at least a
    /// whole pixel. Called from the resize drag listener.
    pub fn set_width(&mut self, width: f32) {
        let clamped = clamp_channel_sidebar_width(width).round();
        if (self.width - clamped).abs() < 0.5 {
            return;
        }
        self.width = clamped;
        if let Some(path) = self.width_cache_path.as_deref() {
            if let Err(error) = save_channel_sidebar_width_to_path(path, clamped) {
                log::warn!("Falha ao salvar largura da sidebar: {error}");
            }
        }
    }

    fn ensure_dm_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dm_search_state.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&st, window, |this, st, event: &InputEvent, _window, cx| {
            if matches!(event, InputEvent::Change) {
                this.dm_search_query = st.read(cx).value().to_string();
                cx.notify();
            }
        });
        self.dm_search_state = Some(st);
        self.dm_search_sub = Some(sub);
    }

    fn ensure_status_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.status_text_state.is_some() && self.status_emoji_state.is_some() {
            return;
        }

        let text_st = cx.new(|cx| InputState::new(window, cx));
        let app_text = self.state.clone();
        let text_sub = cx.subscribe_in(&text_st, window, move |_this, input, event: &InputEvent, _window, cx| {
            match event {
                InputEvent::Change => {
                    let value = input.read(cx).value().to_string();
                    app_text.update(cx, |state, cx| {
                        state.set_status_picker_text(value);
                        cx.notify();
                    });
                }
                InputEvent::PressEnter { .. } => {
                    app_text.update(cx, |state, cx| {
                        state.save_status_picker();
                        cx.notify();
                    });
                }
                _ => {}
            }
        });

        let emoji_st = cx.new(|cx| InputState::new(window, cx));
        let app_emoji = self.state.clone();
        let emoji_sub = cx.subscribe_in(&emoji_st, window, move |_this, input, event: &InputEvent, _window, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().trim().to_string();
                let emoji = if value.is_empty() {
                    None
                } else {
                    Some(crate::domain::user::CustomStatusEmoji::unicode(value))
                };
                app_emoji.update(cx, |state, cx| {
                    state.set_status_picker_emoji(emoji);
                    cx.notify();
                });
            }
        });

        self.status_text_state = Some(text_st);
        self.status_emoji_state = Some(emoji_st);
        self.status_text_sub = Some(text_sub);
        self.status_emoji_sub = Some(emoji_sub);
    }
}

impl Render for ChannelSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_dm_search_input(window, cx);
        self.ensure_status_inputs(window, cx);
        let dm_search_state = self
            .dm_search_state
            .as_ref()
            .expect("dm_search_state must be initialized")
            .clone();
        let status_text_state = self
            .status_text_state
            .as_ref()
            .expect("status_text_state must be initialized")
            .clone();
        let status_emoji_state = self
            .status_emoji_state
            .as_ref()
            .expect("status_emoji_state must be initialized")
            .clone();
        let dm_query = self.dm_search_query.clone();

        let (status_picker_open, picker_text, picker_emoji) = {
            let state = self.state.read(cx);
            (
                state.status_picker_open,
                state
                    .status_picker_custom_status
                    .text
                    .clone()
                    .unwrap_or_default(),
                state
                    .status_picker_custom_status
                    .emoji
                    .as_ref()
                    .map(|emoji| emoji.name.clone())
                    .unwrap_or_default(),
            )
        };

        // Seed the inputs only when the popover opens. While it stays open the
        // InputState is the source of truth and must not be reset on every frame.
        if status_picker_open && !self.status_picker_was_open {
            status_text_state.update(cx, |input, cx| {
                input.set_value(picker_text, window, cx);
            });
            status_emoji_state.update(cx, |input, cx| {
                input.set_value(picker_emoji, window, cx);
            });
        }
        self.status_picker_was_open = status_picker_open;

        let state = self.state.read(cx);
        let selected_guild = state.get_selected_guild().cloned();
        let selected_channel_id = state.selected_channel_id.clone();
        let active_voice = state.active_voice_call.clone();
        let current_user = state.current_user.clone();
        let is_dm = state.selected_guild_id.is_none();
        let app_entity = self.state.clone();
        let sidebar_entity = cx.entity().clone();
        let sidebar_width = self.width;
        let panel_width = user_panel_width_for_sidebar(sidebar_width);

        let header_el = if let Some(ref guild) = selected_guild {
            render_server_header(guild, &app_entity)
        } else {
            render_dm_search_header(&dm_search_state)
        };

        let scroll_content: Vec<AnyElement> = if is_dm {
            render_dm_section(state, selected_channel_id.as_deref(), &dm_query, &app_entity)
        } else if let Some(ref guild) = selected_guild {
            render_guild_channels_section(
                state,
                guild,
                selected_channel_id.as_deref(),
                &app_entity,
                &current_user,
            )
        } else {
            Vec::new()
        };

        div()
            .track_focus(&self.focus_handle)
            .key_context("channel_sidebar")
            .relative()
            .flex()
            .flex_col()
            .w(px(sidebar_width))
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
                            .pt(space_4())
                            .pb(space_4())
                            .pl(space_4())
                            .pr(px(9.0)) // 272 - 8 - 9 = measured 255px row
                            .gap(px(2.0))
                            .children(scroll_content),
                    ),
            )
            // =========================================================================
            // BOTTOM SECTION: Voice Call Panel Bar + User Controls Panel
            // =========================================================================
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .children(active_voice.as_ref().map(|v| {
                        render_voice_bar(
                            v,
                            &app_entity,
                            selected_channel_id.as_deref() != Some(v.channel_id.as_str()),
                        )
                    }))
                    // Discord measures 50px of channel-navigation clearance;
                    // the 58px panel floats over the final 16px of that area.
                    .child(div().h(channel_navigation_reserve()))
                    .child(render_user_panel(&current_user, &app_entity))
                    .children(state.status_picker_open.then(|| {
                        render_status_picker(
                            state,
                            &status_text_state,
                            &status_emoji_state,
                            &app_entity,
                        )
                    })),
            )
            .child(render_resize_handle(sidebar_width, sidebar_entity))
    }
}

fn render_resize_handle(_sidebar_width: f32, sidebar_entity: Entity<ChannelSidebar>) -> AnyElement {
    let drag_entity = sidebar_entity.clone();
    div()
        .id("channel-sidebar-resize-handle")
        .absolute()
        .top_0()
        .right_0()
        .h_full()
        .w(px(RAW_CHANNEL_SIDEBAR_HANDLE_WIDTH))
        .cursor(CursorStyle::ResizeLeftRight)
        .hover(|style| style.bg(channel_hover()))
        .on_drag(SidebarResizeDrag, move |_drag, _offset, _window, cx| {
            cx.new(|_| SidebarResizeGhost)
        })
        .on_drag_move::<SidebarResizeDrag>(move |event, _window, cx| {
            let requested = event.event.position.x.as_f32() - RAW_RAIL_WIDTH;
            drag_entity.update(cx, |this, cx| {
                this.set_width(requested);
                cx.notify();
            });
        })
        .into_any_element()
}

// ============================================================================
// Header elements
// ============================================================================

fn render_server_header(guild: &super::state::GuildItemState, app_entity: &Entity<AppState>) -> AnyElement {
    let typography = typo_server_header();
    let guild_id = guild.id.clone();
    let nick_entity = app_entity.clone();
    let nick_gid = guild_id.clone();
    let leave_entity = app_entity.clone();
    let leave_gid = guild_id.clone();

    div()
        .h(header_height())
        .px(space_4())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .rounded_tl(radius_lg())
        .border_b_1()
        .border_color(border_subtle())
        .cursor_pointer()
        .hover(|style| style.bg(channel_hover()))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_color(text_header())
                .font_weight(typography.weight)
                .text_size(typography.size)
                .line_height(typography.line_height)
                .truncate()
                .child(guild.name.clone()),
        )
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size(px(16.0))
                .text_color(text_normal()),
        )
        .context_menu(move |menu: PopupMenu, _window, _cx| {
            let nick_entity = nick_entity.clone();
            let nick_gid = nick_gid.clone();
            let leave_entity = leave_entity.clone();
            let leave_gid = leave_gid.clone();
            menu.item(
                PopupMenuItem::new("Alterar apelido").on_click(move |_ev, _win, cx| {
                    nick_entity.update(cx, |s, cx| {
                        s.open_edit_guild_nickname(nick_gid.clone());
                        cx.notify();
                    });
                }),
            )
            .separator()
            .item(
                PopupMenuItem::new("Sair do servidor").on_click(move |_ev, _win, cx| {
                    leave_entity.update(cx, |s, cx| {
                        s.open_modal(ActiveModal::LeaveGuild {
                            guild_id: leave_gid.clone(),
                        });
                        cx.notify();
                    });
                }),
            )
        })
        .into_any_element()
}

fn render_dm_search_header(search_state: &Entity<InputState>) -> AnyElement {
    div()
        .h(header_height())
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .rounded_tl(radius_lg())
        .border_b_1()
        .border_color(border_subtle())
        .child(
            div()
                .w_full()
                .child(
                    TextInput::new("dm-search-input", search_state)
                        .placeholder("Encontre ou comece uma conversa")
                        .show_paste(false)
                        .show_clear(true),
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
    search_query: &str,
    app_entity: &Entity<AppState>,
) -> Vec<AnyElement> {
    let is_amigos_active =
        selected_channel_id.is_none() && state.friends_tab != FriendsTab::MessageRequests;
    let is_msg_requests_active =
        selected_channel_id.is_none() && state.friends_tab == FriendsTab::MessageRequests;
    let pending_req_count = state.message_requests.iter().filter(|r| !r.is_spam).count();

    let mut items = vec![
        render_dm_nav_button(
            "Amigos",
            icon_friends(
                px(20.0),
                if is_amigos_active {
                    white()
                } else {
                    text_muted()
                },
            ),
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
            icon_inbox(
                px(20.0),
                if is_msg_requests_active {
                    white()
                } else {
                    text_muted()
                },
            ),
            is_msg_requests_active,
            if pending_req_count > 0 {
                Some(pending_req_count)
            } else {
                None
            },
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
            {
                let entity = app_entity.clone();
                move |_ev, _win, cx| {
                    entity.update(cx, |s, cx| {
                        open_nitro_settings(s);
                        cx.notify();
                    });
                }
            },
        ),
        render_dm_nav_button(
            "Loja",
            icon_gift(px(20.0), text_muted()),
            false,
            None,
            move |_ev, _win, cx| {
                cx.open_url("https://discord.com/shop");
            },
        ),
        render_dm_section_header(app_entity),
    ];

    let query = search_query.trim().to_lowercase();
    let visible_dms: Vec<&ChannelItemState> = state
        .dm_channels
        .iter()
        .filter(|dm| {
            if query.is_empty() {
                return true;
            }
            dm.name.to_lowercase().contains(&query)
                || dm
                    .recipient_names
                    .iter()
                    .any(|name| name.to_lowercase().contains(&query))
        })
        .collect();

    if visible_dms.is_empty() {
        let empty_text = if query.is_empty() {
            "Nenhuma conversa direta aberta."
        } else {
            "Nenhuma conversa encontrada."
        };
        items.push(
            div()
                .px(px(8.0))
                .py(px(12.0))
                .text_color(text_muted())
                .text_size(px(12.0))
                .child(empty_text)
                .into_any_element(),
        );
    } else {
        for dm in visible_dms {
            items.push(render_dm_item(dm, state, selected_channel_id, app_entity));
        }
    }

    items
}

/// Opens the official Nitro settings tab through the existing User Settings modal.
pub fn open_nitro_settings(state: &mut AppState) {
    state.set_settings_tab(SettingsTab::Nitro);
    state.open_modal(ActiveModal::UserSettings);
}

/// Closes a DM locally: removes it from the sidebar and pinned list, and leaves
/// the Friends Hub selected when the closed DM was open.
pub fn close_dm_channel(state: &mut AppState, channel_id: &str) -> bool {
    let before = state.dm_channels.len();
    state.dm_channels.retain(|dm| dm.id != channel_id);
    state.pinned_dms.retain(|id| id != channel_id);
    if state.selected_channel_id.as_deref() == Some(channel_id) {
        state.selected_channel_id = None;
        state.friends_tab = FriendsTab::Online;
    }
    state.dm_channels.len() != before
}

/// Marks a guild channel or DM as read, clearing unread and mention badges and ACKing.
pub fn mark_channel_read(state: &mut AppState, channel_id: &str) -> bool {
    state.mark_channel_read(channel_id)
}

/// Toggles local mute for a channel and returns the new muted state.
pub fn toggle_channel_mute(state: &mut AppState, channel_id: &str) -> bool {
    if state.muted_channels.remove(channel_id) {
        false
    } else {
        state.muted_channels.insert(channel_id.to_string());
        true
    }
}

/// Whether the channel is locally muted.
pub fn is_channel_muted(state: &AppState, channel_id: &str) -> bool {
    state.muted_channels.contains(channel_id)
}

pub fn focus_active_voice_stage(state: &mut AppState) -> bool {
    let Some(channel_id) = state
        .active_voice_call
        .as_ref()
        .map(|call| call.channel_id.clone())
    else {
        return false;
    };
    if state.selected_channel_id.as_deref() == Some(channel_id.as_str()) {
        return false;
    }

    let guild_id = state
        .guilds
        .iter()
        .find(|guild| {
            guild
                .categories
                .iter()
                .any(|category| category.channels.iter().any(|channel| channel.id == channel_id))
        })
        .map(|guild| guild.id.clone());

    if let Some(guild_id) = guild_id {
        state.selected_guild_id = Some(guild_id);
    } else if state.dm_channels.iter().any(|channel| channel.id == channel_id) {
        state.selected_guild_id = None;
    }

    state.selected_channel_id = Some(channel_id);
    true
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
                .child(div().truncate().child(label)),
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
                .font_weight(WEIGHT_BOLD)
                .text_size(font_size_xs())
                .line_height(line_height_xs())
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

    let game_activity = presence_info.and_then(|p| p.activity.as_ref()).filter(|a| {
        a.activity_type == ActivityType::Playing
            || (!a.name.is_empty() && a.name != "Custom Status")
    });

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
        render_dm_subtitle(
            custom_status.as_ref(),
            game_activity,
            presence_status == PresenceStatus::Offline,
        )
    };

    let is_pinned = dm.is_pinned
        || dm
            .topic
            .as_deref()
            .map_or(false, |t| t.contains("pin") || t.contains("fixad"));
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
    let close_entity = entity.clone();
    let close_cid = dm_id.clone();

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
                .tooltip("Fechar DM")
                .on_click(move |_event, _window, cx| {
                    let cid = close_cid.clone();
                    close_entity.update(cx, |s, cx| {
                        close_dm_channel(s, &cid);
                        cx.notify();
                    });
                }),
            ),
    );

    // Context menu via gpui-kit ContextMenuExt & PopupMenu
    let pin_entity = entity.clone();
    let pin_cid = dm_id.clone();
    let pin_label = if is_pinned {
        "Desafixar conversa"
    } else {
        "Fixar conversa"
    };

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

/// Official Discord Desktop section divider metrics.
///
/// Source: inspected Discord Desktop DOM classes `.sectionDivider_u88IQl`
/// (1px high, 12px vertical / 8px horizontal margins) wrapping the full-width
/// `.divider_DERYpk` line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SectionDividerMetrics {
    pub height: f32,
    pub margin_y: f32,
    pub margin_x: f32,
    pub full_width: bool,
}

impl Default for SectionDividerMetrics {
    fn default() -> Self {
        Self {
            height: 1.0,
            margin_y: 12.0,
            margin_x: 8.0,
            full_width: true,
        }
    }
}

/// One element in the guild channel section, used by both rendering and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GuildChannelSectionItem<'a> {
    Divider,
    Header {
        id: &'a str,
        name: &'a str,
        collapsed: bool,
    },
    Channel(&'a ChannelItemState),
}

/// Whether a category state is the real unlabeled group created for channels
/// without a parent. That group never receives a synthetic header.
pub(crate) fn is_unlabeled_category(category: &CategoryItemState) -> bool {
    category.name.trim().is_empty()
}

/// Plans the official order of a guild channel section.
///
/// * A divider separates every pair of adjacent groups.
/// * The unlabeled group has no header and is never collapsible.
/// * Labeled categories keep their real header even when they have no channels.
/// * A collapsed category hides its channels but keeps its header.
pub(crate) fn plan_guild_channel_section<'a>(
    categories: &'a [CategoryItemState],
    is_collapsed: impl Fn(&str) -> bool,
) -> Vec<GuildChannelSectionItem<'a>> {
    let mut plan = Vec::new();

    for (index, category) in categories.iter().enumerate() {
        if index > 0 {
            plan.push(GuildChannelSectionItem::Divider);
        }

        let unlabeled = is_unlabeled_category(category);
        let collapsed = !unlabeled && is_collapsed(&category.id);
        if !unlabeled {
            plan.push(GuildChannelSectionItem::Header {
                id: &category.id,
                name: &category.name,
                collapsed,
            });
        }

        if !collapsed {
            plan.extend(
                category
                    .channels
                    .iter()
                    .map(GuildChannelSectionItem::Channel),
            );
        }
    }

    plan
}

fn render_section_divider() -> AnyElement {
    let metrics = SectionDividerMetrics::default();
    div()
        .h(px(metrics.height))
        .my(px(metrics.margin_y))
        .mx(px(metrics.margin_x))
        .w_full()
        .child(div().w_full().h_full().bg(border_subtle()))
        .into_any_element()
}

fn render_guild_channels_section(
    state: &AppState,
    guild: &super::state::GuildItemState,
    selected_channel_id: Option<&str>,
    app_entity: &Entity<AppState>,
    current_user: &CurrentUserProfile,
) -> Vec<AnyElement> {
    let mut category_elements = Vec::new();

    for item in plan_guild_channel_section(&guild.categories, |category_id| {
        state.is_category_collapsed(category_id)
    }) {
        match item {
            GuildChannelSectionItem::Divider => {
                category_elements.push(render_section_divider());
            }
            GuildChannelSectionItem::Header {
                id,
                name,
                collapsed,
            } => category_elements.push(render_category_header(
                id,
                name,
                collapsed,
                app_entity,
            )),
            GuildChannelSectionItem::Channel(channel) => {
                category_elements.push(render_channel_entry(
                    &guild.id,
                    channel,
                    state,
                    selected_channel_id,
                    app_entity,
                    current_user,
                ));
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
    let typography = typo_category_header();

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
                        .font_weight(typography.weight)
                        .text_size(typography.size)
                        .line_height(typography.line_height)
                        .child(cat_name.to_uppercase()),
                ),
        )
        .into_any_element()
}

fn render_channel_icon(
    kind: ChannelKind,
    color: Rgba,
    group_name: SharedString,
    hover_color: Option<Rgba>,
) -> AnyElement {
    let mut glyph = svg()
        .path(kind.discord_icon().asset_path())
        .size(px(20.0))
        .text_color(color);
    if let Some(hover_color) = hover_color {
        glyph = glyph.group_hover(group_name, move |style| style.text_color(hover_color));
    }

    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.0))
        .flex_shrink_0()
        .child(glyph)
        .into_any_element()
}

fn render_channel_entry(
    guild_id: &str,
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
    let is_muted = is_channel_muted(state, &ch_id);
    let entity = app_entity.clone();
    let typography = typo_channel_item();

    let is_voice = ch_kind.is_voice_like();
    let is_forum = ch_kind == ChannelKind::Forum;
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
                    .font_weight(if is_full {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(if is_full {
                        white()
                    } else {
                        interactive_normal()
                    })
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
    } else if is_forum {
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
                .child("Forum"),
        )
    } else {
        None
    };

    let click_cid = ch_id.clone();
    let click_entity = entity.clone();
    let group_name: SharedString = format!("channel-row-{ch_id}").into();

    // Measured Discord Desktop channel row: 32px high, 8px radius, 20px icon
    // and 16px/500/24px channel name.
    let mut row = div()
        .id(ElementId::Name(format!("channel-row-{}", ch_id).into()))
        .group(group_name.clone())
        .relative()
        .h(channel_item_height())
        .px(space_4())
        .py(space_2())
        .rounded(radius_md())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .cursor_pointer();

    if is_selected {
        row = row.bg(sidebar_channel_selected_bg());
    } else {
        row = row.hover(|style| style.bg(channel_hover()));
    }

    if is_unread {
        row = row.child(
            div()
                .absolute()
                .left_0()
                .w(channel_unread_width())
                .h(guild_pill_unread_height())
                .rounded_r(radius_sm())
                .bg(sidebar_channel_selected_text()),
        );
    }

    let channel_icon_color = if is_selected || is_unread {
        sidebar_channel_selected_text()
    } else {
        sidebar_channel_text()
    };

    let mut channel_name = div()
        .text_size(typography.size)
        .font_weight(typography.weight)
        .line_height(typography.line_height)
        .text_color(channel_icon_color)
        .truncate()
        .child(ch_name);
    if !is_selected && !is_unread {
        channel_name = channel_name
            .group_hover(group_name.clone(), |style| style.text_color(sidebar_channel_selected_text()));
    }

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
                .gap(space_4())
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(render_channel_icon(
                    ch_kind,
                    channel_icon_color,
                    group_name.clone(),
                    (!is_selected).then_some(sidebar_channel_selected_text()),
                ))
                .child(channel_name),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(space_2())
                .flex_shrink_0()
                .children(capacity_badge)
                .children(if mention_count > 0 {
                    Some(MentionBadge::new(mention_count).into_any_element())
                } else {
                    None
                }),
        );

    // Context menu via gpui-kit ContextMenuExt
    let read_entity = entity.clone();
    let read_id = ch_id.clone();
    let mute_entity = entity.clone();
    let mute_id = ch_id.clone();
    let mute_label = if is_muted {
        "Reativar canal"
    } else {
        "Silenciar canal"
    };
    let channel_link = format!("https://discord.com/channels/{}/{}", guild_id, ch_id);
    let mut channel_container =
        div()
            .flex()
            .flex_col()
            .child(row.context_menu(move |menu: PopupMenu, _window, _cx| {
                let link = channel_link.clone();
                let cid_read = read_id.clone();
                let ent_read = read_entity.clone();
                let cid_mute = mute_id.clone();
                let ent_mute = mute_entity.clone();
                menu.item(
                    PopupMenuItem::new("Marcar como lido").on_click(move |_ev, _win, cx| {
                        let cid = cid_read.clone();
                        ent_read.update(cx, |state, cx| {
                            mark_channel_read(state, &cid);
                            cx.notify();
                        });
                    }),
                )
                .item(
                    PopupMenuItem::new(mute_label).on_click(move |_ev, _win, cx| {
                        let cid = cid_mute.clone();
                        ent_mute.update(cx, |state, cx| {
                            toggle_channel_mute(state, &cid);
                            cx.notify();
                        });
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::new("Copiar link do canal").on_click(move |_ev, _win, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(link.clone()));
                    }),
                )
            }));

    // Voice participants under channel
    if is_voice && !voice_states.is_empty() {
        for vs in voice_states {
            channel_container =
                channel_container.child(render_voice_user_row(vs, state, current_user));
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
            state
                .friends
                .iter()
                .find(|f| f.id == vs_uid)
                .and_then(|f| f.avatar_url.clone())
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
        .h(voice_user_height())
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
                .child({
                    let mut avatar_wrapper = div()
                        .relative()
                        .size(px(24.0))
                        .rounded(radius_full())
                        .child(avatar_el);
                    if is_speaking {
                        avatar_wrapper = avatar_wrapper.child(
                            div()
                                .absolute()
                                .top(px(-2.0))
                                .left(px(-2.0))
                                .size(px(28.0))
                                .rounded(radius_full())
                                .border_2()
                                .border_color(status_online()),
                        );
                    }
                    avatar_wrapper
                })
                .child(
                    div()
                        .text_size(font_size_sm())
                        .font_weight(WEIGHT_MEDIUM)
                        .line_height(line_height_sm())
                        .text_color(interactive_normal())
                        .overflow_hidden()
                        .truncate()
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

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceQualitySummary {
    pub quality: VoiceQuality,
    pub ping_ms: Option<u32>,
    pub packet_loss_pct: Option<f32>,
    pub codec: Option<String>,
    pub sent_packets: u64,
    pub recv_packets: u64,
    pub lost_packets: u64,
}

impl VoiceQualitySummary {
    pub fn from_call(call: &ActiveVoiceCall) -> Self {
        Self {
            quality: call.quality(),
            ping_ms: call.ping_ms,
            packet_loss_pct: call.packet_loss_pct,
            codec: call.codec.clone(),
            sent_packets: call.sent_packets,
            recv_packets: call.recv_packets,
            lost_packets: call.lost_packets,
        }
    }

    pub fn rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![("Qualidade", self.quality.label().to_string())];
        if let Some(ms) = self.ping_ms {
            rows.push(("RTT", format!("{ms} ms")));
        }
        if let Some(loss) = self.packet_loss_pct {
            rows.push(("Perda de pacotes", format!("{loss:.1}%")));
        }
        if let Some(codec) = self.codec.as_deref() {
            rows.push(("Codec", codec.to_string()));
        }
        rows
    }
}

fn voice_quality_color(quality: VoiceQuality) -> Rgba {
    match quality {
        VoiceQuality::Unknown => status_online(),
        VoiceQuality::Fine => status_positive(),
        VoiceQuality::Average => status_warning(),
        VoiceQuality::Bad => status_dnd(),
    }
}

fn voice_connection_color(voice: &ActiveVoiceCall) -> Rgba {
    match voice.connection_state {
        crate::protocol::voice_session::VoiceSessionState::Connected => status_online(),
        crate::protocol::voice_session::VoiceSessionState::Connecting
        | crate::protocol::voice_session::VoiceSessionState::Reconnecting => status_idle(),
        crate::protocol::voice_session::VoiceSessionState::Disconnected { .. } => status_dnd(),
    }
}

fn voice_bar_title(voice: &ActiveVoiceCall) -> String {
    match voice.connection_state {
        crate::protocol::voice_session::VoiceSessionState::Connected => "Voz Conectada".to_string(),
        crate::protocol::voice_session::VoiceSessionState::Connecting => {
            "Conectando ao RTC...".to_string()
        }
        crate::protocol::voice_session::VoiceSessionState::Reconnecting => {
            "Reconectando...".to_string()
        }
        crate::protocol::voice_session::VoiceSessionState::Disconnected { ref reason } => {
            if let Some(code) = crate::protocol::voice_gateway::extract_close_code(reason) {
                format!("Desconectado (código {})", code)
            } else if reason.contains("4006") {
                "Desconectado (código 4006)".to_string()
            } else if reason.contains("Opcode 2")
                || reason.contains("Timed out")
                || reason.contains("Tempo limite")
            {
                "Desconectado (tempo limite)".to_string()
            } else if !reason.is_empty() && reason != "Sessão inicializada" {
                format!("Desconectado ({})", reason)
            } else {
                "Desconectado".to_string()
            }
        }
    }
}

#[derive(IntoElement)]
struct VoiceBarTrigger {
    is_open: bool,
    child: AnyElement,
}

impl Selectable for VoiceBarTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.is_open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_open
    }
}

impl RenderOnce for VoiceBarTrigger {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("voice-bar-quality-trigger")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .min_w(px(0.0))
            .flex_1()
            .rounded(radius_sm())
            .cursor_pointer()
            .hover(|style| style.bg(channel_hover()))
            .tooltip(|window, cx| Tooltip::new("Detalhes da conexão de voz").build(window, cx))
            .child(self.child)
    }
}

fn render_voice_quality_popout(app_entity: &Entity<AppState>, cx: &App) -> AnyElement {
    let state = app_entity.read(cx);
    let Some(call) = state.active_voice_call.as_ref() else {
        return div().into_any_element();
    };
    let summary = VoiceQualitySummary::from_call(call);
    let quality_color = voice_quality_color(summary.quality);

    div()
        .w(px(214.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_color(text_header())
                .font_weight(FontWeight::BOLD)
                .text_size(px(13.0))
                .child("Conexão de voz"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon_wifi(px(14.0), quality_color))
                .child(
                    div()
                        .text_color(quality_color)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child(summary.quality.label()),
                ),
        )
        .child(
            div()
                .text_color(text_muted())
                .text_size(px(11.0))
                .truncate()
                .child(format!("{} • {}", call.guild_name, call.channel_name)),
        )
        .children(summary.rows().into_iter().map(|(label, value)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(11.0))
                        .child(label),
                )
                .child(
                    div()
                        .text_color(interactive_normal())
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(11.0))
                        .child(value),
                )
        }))
        .into_any_element()
}

fn render_voice_bar(
    voice: &ActiveVoiceCall,
    app_entity: &Entity<AppState>,
    show_return_to_stage: bool,
) -> AnyElement {
    let entity = app_entity.clone();
    let quality = voice.quality();
    let quality_color = if matches!(
        voice.connection_state,
        crate::protocol::voice_session::VoiceSessionState::Connected
    ) {
        voice_quality_color(quality)
    } else {
        voice_connection_color(voice)
    };
    let return_entity = app_entity.clone();

    let identity = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .min_w(px(0.0))
        .flex_1()
        .overflow_hidden()
        .child(icon_wifi(px(16.0), quality_color).flex_shrink_0())
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .flex_1()
                .overflow_hidden()
                .child(
                    div()
                        .text_color(voice_connection_color(voice))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .truncate()
                        .child(voice_bar_title(voice)),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(11.0))
                        .truncate()
                        .child(format!("{} • {}", voice.guild_name, voice.channel_name)),
                ),
        )
        .into_any_element();

    let trigger = VoiceBarTrigger {
        is_open: false,
        child: identity,
    };

    let quality_popover = Popover::new("voice-bar-quality-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger_style(StyleRefinement::default().w_full())
        .trigger(trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| render_voice_quality_popout(&entity, cx)
        });

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
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .min_w(px(0.0))
                .flex_1()
                .overflow_hidden()
                .child(quality_popover),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .flex_shrink_0()
                .gap(px(4.0))
                .children(show_return_to_stage.then(|| {
                    IconButton::with_icon("voice-return-stage-btn", |sz, col| {
                        icon_speaker(sz, col).into_any_element()
                    })
                    .size(px(28.0))
                    .icon_size(px(16.0))
                    .rounded(radius_sm())
                    .tooltip("Voltar ao palco de voz")
                    .on_click(move |_event, _window, cx| {
                        return_entity.update(cx, |state, cx| {
                            focus_active_voice_stage(state);
                            cx.notify();
                        });
                    })
                }))
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
                ),
        )
        .into_any_element()
}

fn status_selection_color(status: UserStatusSelection) -> Rgba {
    match status {
        UserStatusSelection::Online => status_online(),
        UserStatusSelection::Idle => status_ausente(),
        UserStatusSelection::Dnd => status_dnd(),
        UserStatusSelection::Invisible => status_offline(),
    }
}

fn status_selection_label(status: UserStatusSelection) -> &'static str {
    match status {
        UserStatusSelection::Online => t("status.online"),
        UserStatusSelection::Idle => t("status.idle"),
        UserStatusSelection::Dnd => t("status.dnd"),
        UserStatusSelection::Invisible => t("status.invisible"),
    }
}

fn expiration_label(expiration: CustomStatusExpiration) -> &'static str {
    match expiration {
        CustomStatusExpiration::Today => t("status.expire_today"),
        CustomStatusExpiration::FourHours => "4h",
        CustomStatusExpiration::OneHour => "1h",
        CustomStatusExpiration::ThirtyMinutes => "30m",
        CustomStatusExpiration::Never => t("status.expire_never"),
    }
}

fn render_status_picker_row(
    status: UserStatusSelection,
    selected: bool,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let entity = app_entity.clone();
    let background = if selected {
        channel_hover()
    } else {
        transparent()
    };
    let mut row = div()
        .id(SharedString::from(format!("status-picker-option-{}", status.as_gateway_str())))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .h(px(38.0))
        .px(px(8.0))
        .rounded(radius_sm())
        .cursor_pointer()
        .bg(background)
        .hover(|style| style.bg(channel_hover()))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.apply_status_selection(status);
                cx.notify();
            });
        })
        .child(
            div()
                .w(px(10.0))
                .h(px(10.0))
                .rounded_full()
                .bg(status_selection_color(status))
                .flex_shrink_0(),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_size(px(14.0))
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_header())
                .child(status_selection_label(status)),
        );

    row = row.child(if selected {
        icon_check(px(16.0), interactive_normal()).into_any_element()
    } else {
        div().w(px(16.0)).into_any_element()
    });
    row.into_any_element()
}

fn render_status_expiration_choice(
    expiration: CustomStatusExpiration,
    selected: bool,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let entity = app_entity.clone();
    div()
        .id(SharedString::from(format!(
            "status-picker-expiration-{:?}",
            expiration
        )))
        .px(px(7.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .rounded(radius_sm())
        .cursor_pointer()
        .bg(if selected { channel_hover() } else { transparent() })
        .hover(|style| style.bg(channel_hover()))
        .text_size(px(12.0))
        .font_weight(WEIGHT_MEDIUM)
        .text_color(if selected {
            text_header()
        } else {
            interactive_normal()
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_status_picker_expiration(expiration);
                cx.notify();
            });
        })
        .child(expiration_label(expiration))
        .into_any_element()
}

fn render_status_picker_custom_editor(
    draft: &CustomStatusDraft,
    text_state: &Entity<InputState>,
    emoji_state: &Entity<InputState>,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let entity = app_entity.clone();
    let has_custom = !draft.is_empty();
    let mut clear_action: Option<AnyElement> = None;
    if has_custom {
        clear_action = Some(
            div()
                .id("status-picker-clear-custom")
                .mt(px(2.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .text_size(px(12.0))
                .text_color(status_danger())
                .hover(|style| style.text_color(text_header()))
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.clear_status_picker_custom_status();
                        cx.notify();
                    });
                })
                .child(t("status.custom_clear"))
                .into_any_element(),
        );
    }

    let mut expiration_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .flex_wrap()
        .child(
            div()
                .mr(px(2.0))
                .text_size(px(12.0))
                .text_color(text_muted())
                .child(t("status.expiration")),
        );
    for expiration in CustomStatusExpiration::ALL {
        expiration_row = expiration_row.child(render_status_expiration_choice(
            expiration,
            draft.expiration == expiration,
            app_entity,
        ));
    }

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_muted())
                .child(t("status.custom")),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(6.0))
                .child(
                    div().w(px(58.0)).child(
                        TextInput::new("status-picker-emoji-input", emoji_state)
                            .placeholder("Emoji")
                            .show_paste(false)
                            .show_clear(false)
                            .full_width(true),
                    ),
                )
                .child(
                    div().flex_1().min_w(px(0.0)).child(
                        TextInput::new("status-picker-text-input", text_state)
                            .placeholder(t("status.custom_placeholder"))
                            .show_paste(false)
                            .show_clear(false)
                            .full_width(true),
                    ),
                ),
        )
        .children(clear_action)
        .child(expiration_row)
        .into_any_element()
}

fn render_status_picker(
    state: &AppState,
    text_state: &Entity<InputState>,
    emoji_state: &Entity<InputState>,
    app_entity: &Entity<AppState>,
) -> AnyElement {
    let current_user = &state.current_user;
    let draft = state.status_picker_custom_status.clone();
    let username = current_user.username.clone();
    let display_name = current_user.display_name.clone();
    let avatar_url = current_user.avatar_url.clone();
    let presence: PresenceStatus = current_user.presence.into();
    let selected = state.status_picker_status;
    let error = state.status_update_error.clone();

    let mut rows = div().flex().flex_col().gap(px(1.0));
    for status in UserStatusSelection::ALL {
        rows = rows.child(render_status_picker_row(
            status,
            status == selected,
            app_entity,
        ));
    }

    let save_entity = app_entity.clone();
    let copy_entity = app_entity.clone();
    let username_for_copy = username.clone();

    div()
        .absolute()
        .left(px(RAW_USER_PANEL_MARGIN))
        .bottom(px(RAW_USER_PANEL_MARGIN + RAW_USER_BAR_HEIGHT + 8.0))
        .w(px(300.0))
        .max_h(px(520.0))
        .id("status-picker-popover")
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(8.0))
        .bg(bg_user_bar())
        .border_1()
        .border_color(border_subtle())
        .rounded(radius_md())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px(px(4.0))
                .child(
                    Avatar::new(display_name)
                        .size(AvatarSize::Px32)
                        .opt_src(avatar_url)
                        .status(presence)
                        .cutout_color(bg_user_bar()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(WEIGHT_MEDIUM)
                                .text_color(text_header())
                                .truncate()
                                .child(current_user.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(interactive_normal())
                                .truncate()
                                .child(format!("@{}", username)),
                        ),
                ),
        )
        .child(rows)
        .child(div().h(px(1.0)).bg(border_subtle()).my(px(2.0)))
        .child(render_status_picker_custom_editor(
            &draft, text_state, emoji_state, app_entity,
        ))
        .children(error.map(|error| {
            div()
                .text_size(px(12.0))
                .text_color(status_danger())
                .child(error)
                .into_any_element()
        }))
        .child(
            Button::blurple("status-picker-save", t("status.save"))
                .size(ButtonSize::Sm)
                .full_width()
                .on_click(move |_event, _window, cx| {
                    save_entity.update(cx, |state, cx| {
                        state.save_status_picker();
                        cx.notify();
                    });
                }),
        )
        .child(
            Button::gray("status-picker-copy-username", "Copiar username")
                .size(ButtonSize::Sm)
                .full_width()
                .on_click(move |_event, _window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(username_for_copy.clone()));
                    copy_entity.update(cx, |state, cx| {
                        state.close_status_picker();
                        cx.notify();
                    });
                }),
        )
        .into_any_element()
}

fn render_user_panel(
    current_user: &CurrentUserProfile,
    app_entity: &Entity<AppState>,
) -> AnyElement {
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

    let name_typography = typo_user_bar_name();
    let subtext_typography = typo_user_bar_subtext();

    // Measured Discord Desktop: 328x58 floating panel, 8px from the combined
    // rail/sidebar edges. The sidebar root begins at x=72, so -64 places the
    // panel's left edge at the measured app x=8.
    div()
        .absolute()
        .left(px(RAW_SPACE_4 - RAW_RAIL_WIDTH))
        .bottom(user_panel_margin())
        .w(user_panel_width())
        .h(user_bar_height())
        .px(user_panel_margin())
        .bg(bg_user_bar())
        .rounded(radius_md())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(space_4())
                .py(px(5.0))
                .px(space_2())
                .rounded(radius_sm())
                .cursor_pointer()
                .hover(|style| style.bg(channel_hover()))
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    move |_event, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.toggle_status_picker();
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
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .text_color(text_header())
                                .font_weight(name_typography.weight)
                                .text_size(name_typography.size)
                                .line_height(name_typography.line_height)
                                .truncate()
                                .child(current_user.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_color(interactive_normal())
                                .font_weight(subtext_typography.weight)
                                .text_size(subtext_typography.size)
                                .line_height(subtext_typography.line_height)
                                .truncate()
                                .child(user_tag),
                        ),
                ),
        )
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
        .gap(space_4())
        // Mute Button (32x32)
        .child({
            let entity = entity.clone();
            let mute_tooltip = if is_muted {
                "Ativar microfone"
            } else {
                "Desativar microfone"
            };
            IconButton::with_icon("user-panel-mute-btn", move |sz, _col| {
                if is_muted {
                    icon_mic_off(sz, status_dnd()).into_any_element()
                } else {
                    icon_mic(sz, interactive_normal()).into_any_element()
                }
            })
            .size(user_control_size())
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
            let deafen_tooltip = if is_deafened {
                "Desativar áudio"
            } else {
                "Ensordecer"
            };
            IconButton::with_icon("user-panel-deafen-btn", move |sz, _col| {
                if is_deafened {
                    icon_headphones_off(sz, status_dnd()).into_any_element()
                } else {
                    icon_headphones(sz, interactive_normal()).into_any_element()
                }
            })
            .size(user_control_size())
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
                icon_gear(sz, interactive_normal()).into_any_element()
            })
            .size(user_control_size())
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
                    sub_div = sub_div.child(img(url).size(px(14.0)).flex_shrink_0());
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

    let mut title_row = div().flex().items_center().gap(px(6.0)).child(
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
    fn test_server_header_guild_state() {
        let guild = super::super::state::GuildItemState {
            id: "1".into(),
            name: "Rust Server".into(),
            acronym: "RS".into(),
            icon_url: None,
            icon_hash: None,
            unread: false,
            mention_count: 0,
            categories: vec![],
            emojis: vec![],
            stickers: vec![],
        };
        assert_eq!(guild.name, "Rust Server");
        assert_eq!(guild.id, "1");
    }

    #[::core::prelude::v1::test]
    fn test_channel_icons_build_with_hover_color() {
        let text = render_channel_icon(
            ChannelKind::Text,
            channels_default(),
            "test-text-channel".into(),
            Some(text_header()),
        );
        let voice = render_channel_icon(
            ChannelKind::Voice,
            channels_default(),
            "test-voice-channel".into(),
            Some(text_header()),
        );
        let _ = (text, voice);
    }

    fn channel(id: &str, name: &str) -> ChannelItemState {
        ChannelItemState {
            id: id.into(),
            name: name.into(),
            kind: ChannelKind::Text,
            topic: None,
            unread: false,
            mention_count: 0,
            icon_url: None,
            is_pinned: false,
            last_message_id: None,
            last_read_message_id: None,
            is_group_dm: false,
            recipient_ids: Vec::new(),
            recipient_names: Vec::new(),
            user_limit: None,
            avatar_decoration_data: None,
            thread_metadata: None,
            default_tag_ids: Vec::new(),
            available_tags: Vec::new(),
            flags: 0,
        }
    }

    fn category(id: &str, name: &str, channels: Vec<ChannelItemState>) -> CategoryItemState {
        CategoryItemState {
            id: id.into(),
            name: name.into(),
            channels,
        }
    }

    #[::core::prelude::v1::test]
    fn section_divider_matches_official_metrics() {
        let metrics = SectionDividerMetrics::default();
        assert_eq!(metrics.height, 1.0);
        assert_eq!(metrics.margin_y, 12.0);
        assert_eq!(metrics.margin_x, 8.0);
        assert!(metrics.full_width);
    }

    #[::core::prelude::v1::test]
    fn section_plan_snapshot_covers_divider_and_collapse() {
        let categories = vec![
            category(
                "cat-uncategorized",
                "",
                vec![channel("c-top", "top"), channel("c-orphan", "orphan")],
            ),
            category("cat-empty", "EMPTY", Vec::new()),
            category("cat-text", "TEXT", vec![channel("c-general", "general")]),
        ];

        let collapsed = |id: &str| id == "cat-empty";
        let plan = plan_guild_channel_section(&categories, collapsed);

        assert_eq!(
            plan,
            vec![
                GuildChannelSectionItem::Channel(&categories[0].channels[0]),
                GuildChannelSectionItem::Channel(&categories[0].channels[1]),
                GuildChannelSectionItem::Divider,
                GuildChannelSectionItem::Header {
                    id: "cat-empty",
                    name: "EMPTY",
                    collapsed: true,
                },
                GuildChannelSectionItem::Divider,
                GuildChannelSectionItem::Header {
                    id: "cat-text",
                    name: "TEXT",
                    collapsed: false,
                },
                GuildChannelSectionItem::Channel(&categories[2].channels[0]),
            ]
        );

        assert!(!plan.iter().any(|item| matches!(
            item,
            GuildChannelSectionItem::Header { name: "", .. }
        )));
        assert!(plan.iter().any(|item| matches!(
            item,
            GuildChannelSectionItem::Channel(channel) if channel.id == "c-general"
        )));
    }

    #[::core::prelude::v1::test]
    fn collapsed_category_hides_only_its_own_channels() {
        let categories = vec![
            category("cat-text", "TEXT", vec![channel("c-general", "general")]),
            category("cat-voice", "VOICE", vec![channel("c-lobby", "lobby")]),
        ];

        let plan = plan_guild_channel_section(&categories, |id| id == "cat-text");
        let channel_ids: Vec<&str> = plan
            .iter()
            .filter_map(|item| match item {
                GuildChannelSectionItem::Channel(channel) => Some(channel.id.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(channel_ids, vec!["c-lobby"]);
        assert!(plan.iter().any(|item| matches!(
            item,
            GuildChannelSectionItem::Header { id: "cat-text", collapsed: true, .. }
        )));
    }
}
