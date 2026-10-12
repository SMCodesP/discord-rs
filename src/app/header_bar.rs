use gpui::component::input::{InputEvent, InputState};
use gpui::component::popover::Popover;
use gpui::component::Selectable;
use gpui::prelude::*;
use gpui::*;

use crate::components::{Avatar, AvatarSize, IconButton, TextInput};
use crate::tokens::colors::{self, border_subtle};
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;
use super::state::{ActiveModal, AppState, ChannelItemState};

/// Chat and channel top header bar (49px high: 48px content + 1px subtle bottom border)
pub struct HeaderBar {
    state: Entity<AppState>,
    search_input_state: Option<Entity<InputState>>,
    search_input_sub: Option<Subscription>,
    search_popover_open: bool,
}

impl HeaderBar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self {
            state,
            search_input_state: None,
            search_input_sub: None,
            search_popover_open: false,
        }
    }

    fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(ref st) = self.search_input_state {
            return st.clone();
        }
        let initial_query = self.state.read(cx).search_query.clone();
        let st = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(initial_query)
        });
        let state_entity = self.state.clone();
        let st_clone = st.clone();
        let sub = cx.subscribe_in(&st, window, move |this, _input, event: &InputEvent, _window, cx| {
            match event {
                InputEvent::Focus => {
                    let text = st_clone.read(cx).value().to_string();
                    this.search_popover_open = text.is_empty();
                    cx.notify();
                }
                InputEvent::Blur => {
                    this.search_popover_open = false;
                    cx.notify();
                }
                InputEvent::Change => {
                    let text = st_clone.read(cx).value().to_string();
                    state_entity.update(cx, |s, cx| {
                        s.search_query = text.clone();
                        if text.is_empty() {
                            s.clear_search();
                        }
                        cx.notify();
                    });
                    this.search_popover_open = text.is_empty();
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    this.search_popover_open = false;
                    let text = st_clone.read(cx).value().to_string();
                    if text.trim().is_empty() {
                        state_entity.update(cx, |s, cx| {
                            s.clear_search();
                            s.close_search();
                            cx.notify();
                        });
                    } else {
                        let maybe_req = state_entity.update(cx, |s, cx| {
                            s.open_search();
                            s.set_search_offset(0);
                            let req = s.begin_search(&text);
                            cx.notify();
                            req
                        });
                        if let Some(req) = maybe_req {
                            let app_weak = state_entity.downgrade();
                            cx.spawn(|_header, cx: &mut AsyncApp| {
                                let cx = cx.clone();
                                async move {
                                    req.run_loop(app_weak, cx).await;
                                }
                            })
                            .detach();
                        }
                    }
                    cx.notify();
                }
            }
        });
        self.search_input_sub = Some(sub);
        self.search_input_state = Some(st.clone());
        st
    }
}

impl Render for HeaderBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search_input_st = self.ensure_search_input(window, cx);
        let current_query = self.state.read(cx).search_query.clone();
        if search_input_st.read(cx).value() != current_query.as_str() {
            let q: SharedString = current_query.into();
            search_input_st.update(cx, |s, cx| {
                s.set_value(q, window, cx);
            });
        }
        let is_search_popover_open = self.search_popover_open;
        let state = self.state.read(cx);
        let selected_channel_opt = if let Some((_, ch)) = state.get_selected_channel() {
            Some(ch.clone())
        } else if let Some(ref cid) = state.selected_channel_id {
            state.dm_channels.iter().find(|d| &d.id == cid).cloned()
        } else {
            None
        };
        let is_dm = state.selected_guild_id.is_none();
        let app_entity = self.state.clone();

        div()
            .h(px(49.0))
            .w_full()
            .bg(cx.discord().bg_chat)
            .pl(px(16.0))
            .pr(px(8.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(border_subtle())
            .flex_shrink_0()
            .child(render_channel_left_side(
                selected_channel_opt.as_ref(),
                is_dm,
                state,
                cx,
            ))
            .child(render_toolbar(
                app_entity,
                state,
                &search_input_st,
                is_search_popover_open,
                cx,
            ))
    }
}

fn render_channel_left_side(
    selected_channel: Option<&ChannelItemState>,
    is_dm: bool,
    state: &AppState,
    cx: &App,
) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .overflow_hidden();

    if let Some(channel) = selected_channel {
        if is_dm {
            row = row.child(render_dm_avatar(channel, state, cx));
            row = row.child(render_title(&channel.name, cx));
            if let Some(topic_el) = render_topic(channel.topic.as_deref(), cx) {
                row = row.child(topic_el);
            }
        } else {
            row = row.child(render_guild_channel_icon(channel, cx));
            row = row.child(render_title(&channel.name, cx));
            if let Some(topic_el) = render_topic(channel.topic.as_deref(), cx) {
                row = row.child(topic_el);
            }
        }
    } else if is_dm {
        row = row.children(render_friends_header_items(cx));
    } else {
        row = row.children(render_no_channel_header_items(cx));
    }

    row.into_any_element()
}

fn render_dm_avatar(channel: &ChannelItemState, state: &AppState, cx: &App) -> AnyElement {
    let recipient_id = channel.recipient_ids.first();
    let recipient_friend = state.friends.iter().find(|f| {
        if let Some(rid) = recipient_id {
            f.id == *rid
        } else {
            f.username == channel.name || f.display_name == channel.name
        }
    });

    let avatar_src = channel
        .icon_url
        .as_deref()
        .or_else(|| recipient_friend.and_then(|f| f.avatar_url.as_deref()));

    let presence_status = recipient_id
        .and_then(|rid| state.get_user_presence(rid))
        .or_else(|| recipient_friend.and_then(|f| state.get_user_presence(&f.id)))
        .map(|p| p.status)
        .or_else(|| recipient_friend.map(|f| crate::domain::user::PresenceStatus::from(f.presence)));

    Avatar::new(channel.name.clone())
        .size(AvatarSize::Px24)
        .opt_src(avatar_src.map(|s| s.to_string()))
        .opt_status(presence_status)
        .opt_decoration(channel.avatar_decoration_data.clone())
        .cutout_color(cx.discord().bg_chat)
        .into_any_element()
}

fn render_guild_channel_icon(channel: &ChannelItemState, cx: &App) -> AnyElement {
    render_icon(channel.kind.discord_icon(), px(20.0), cx.discord().text_secondary).into_any_element()
}

fn render_title(name: &str, cx: &App) -> AnyElement {
    div()
        .text_color(cx.discord().text_titles)
        .font_weight(FontWeight::BOLD)
        .text_size(px(16.0))
        .child(name.to_string())
        .into_any_element()
}

fn render_topic(topic: Option<&str>, cx: &App) -> Option<AnyElement> {
    let topic = topic?;
    let discord = cx.discord();
    Some(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .w(px(1.0))
                    .h(px(16.0))
                    .bg(discord.borders)
                    .mx(px(4.0)),
            )
            .child(
                div()
                    .text_color(discord.text_secondary)
                    .text_size(px(13.0))
                    .overflow_hidden()
                    .child(topic.to_string()),
            )
            .into_any_element(),
    )
}

fn render_friends_header_items(cx: &App) -> Vec<AnyElement> {
    let discord = cx.discord();
    vec![
        icon_friends(px(20.0), discord.text_secondary).into_any_element(),
        div()
            .text_color(discord.text_titles)
            .font_weight(FontWeight::BOLD)
            .text_size(px(16.0))
            .child("Amigos")
            .into_any_element(),
        div()
            .w(px(1.0))
            .h(px(16.0))
            .bg(discord.borders)
            .mx(px(4.0))
            .into_any_element(),
        div()
            .text_color(discord.text_secondary)
            .text_size(px(13.0))
            .child("Disponível")
            .into_any_element(),
    ]
}

fn render_no_channel_header_items(cx: &App) -> Vec<AnyElement> {
    let discord = cx.discord();
    vec![
        div()
            .text_color(discord.text_secondary)
            .text_size(px(16.0))
            .child("#")
            .into_any_element(),
        div()
            .text_color(discord.text_titles)
            .font_weight(FontWeight::BOLD)
            .text_size(px(16.0))
            .child("Selecione um canal")
            .into_any_element(),
    ]
}

fn render_toolbar(
    app_entity: Entity<AppState>,
    state: &AppState,
    search_input_st: &Entity<InputState>,
    is_search_popover_open: bool,
    cx: &App,
) -> AnyElement {
    let show_members = state.show_members;
    let mut toolbar = div()
        .flex()
        .flex_row()
        .items_center()
        .pl(px(8.0))
        .gap(px(8.0));

    // 0. Discord Connection Shortcut Button (only when disconnected)
    if !state.is_connected_to_gateway {
        toolbar = toolbar.child(render_login_button(app_entity.clone(), cx));
    }

    let is_dm_channel = state.selected_guild_id.is_none() && state.selected_channel_id.is_some();

    // Voice and Video call buttons (Only in DM / Group DM)
    if is_dm_channel {
        let channel_id = state.selected_channel_id.clone().unwrap_or_default();
        let channel_name = state
            .dm_channels
            .iter()
            .find(|d| d.id == channel_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "DM".into());
        let recipient_ids = state
            .dm_channels
            .iter()
            .find(|d| d.id == channel_id)
            .map(|d| d.recipient_ids.clone())
            .unwrap_or_default();

        let voice_entity = app_entity.clone();
        let cid_v = channel_id.clone();
        let cname_v = channel_name.clone();
        let recips_v = recipient_ids;
        toolbar = toolbar.child(
            IconButton::with_icon("header-btn-voice-call", |sz, col| {
                icon_phone(sz, col).into_any_element()
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(px(4.0))
            .tooltip("Iniciar chamada de voz")
            .on_click(move |_event, _window, cx| {
                // Proteção contra clique duplo: se já conectando ou conectado nesta chamada, ignora
                let is_already_connecting = {
                    let s = voice_entity.read(cx);
                    if let Some(ref call) = s.active_voice_call {
                        call.channel_id == cid_v
                            && !matches!(
                                call.connection_state,
                                crate::protocol::voice_session::VoiceSessionState::Disconnected { .. }
                            )
                    } else {
                        false
                    }
                };
                if is_already_connecting {
                    log::warn!("[HeaderBar] Chamada de voz já em andamento/conectando neste canal; ignorando duplo clique.");
                    return;
                }

                let cid = cid_v.clone();
                let cname = cname_v.clone();
                let recips = if recips_v.is_empty() {
                    None
                } else {
                    Some(recips_v.clone())
                };
                let entity = voice_entity.clone();

                // Ordem real (demonstrada em docs/ROADMAP_DISCORD_VOICE_AND_PARITY.md e doc Discord):
                // 1. Entra na chamada do canal de DM com guild_id: None explicitamente (Opcode 4).
                // 2. O client_manager / voice_session aguarda a confirmação de entrada (VoiceStateConfirmed)
                //    antes de disparar ring_call. Falhas no toque são tratadas como aviso não-fatal.
                entity.update(cx, |state, cx| {
                    state.connect_voice_dm_with_ring(&cid, &cname, recips);
                    cx.notify();
                });
            }),
        );

        // Botão de chamada de vídeo desabilitado: transmissão de vídeo ainda não suportada (Zero mocks)
        toolbar = toolbar.child(
            IconButton::with_icon("header-btn-video-call", |sz, col| {
                render_video_camera_icon(sz, col)
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(px(4.0))
            .disabled(true)
            .tooltip("Chamada de vídeo ainda não suportada nesta versão"),
        );
    } else {
        // 1. Notifications / Bell button (Guild text/voice channels)
        toolbar = toolbar.child(
            IconButton::with_icon("header-btn-notifications", |sz, col| {
                icon_bell(sz, col).into_any_element()
            })
            .size(px(28.0))
            .icon_size(px(16.0))
            .rounded(px(4.0))
            .tooltip("Notification Settings"),
        );
    }

    // 2. Pinned messages button
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-pinned", |sz, col| {
            icon_pin(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .tooltip("Pinned Messages"),
    );

    // 3. Member list toggle button
    let members_entity = app_entity.clone();
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-members", |sz, col| {
            icon_members(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .active(show_members)
        .tooltip(if show_members {
            "Hide Member List"
        } else {
            "Show Member List"
        })
        .on_click(move |_event, _window, cx| {
            members_entity.update(cx, |state, cx| {
                state.toggle_members();
                cx.notify();
            });
        }),
    );

    // 4. Search bar (Buscar)
    toolbar = toolbar.child(render_search_box(
        app_entity,
        search_input_st,
        is_search_popover_open,
        state.is_search_open,
        cx,
    ));

    // 5. Inbox button
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-inbox", |sz, col| {
            icon_inbox(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .tooltip("Inbox"),
    );

    // 6. Help button
    toolbar = toolbar.child(
        IconButton::with_icon("header-btn-help", |sz, col| {
            icon_info(sz, col).into_any_element()
        })
        .size(px(28.0))
        .icon_size(px(16.0))
        .rounded(px(4.0))
        .tooltip("Help"),
    );

    toolbar.into_any_element()
}

fn render_login_button(entity: Entity<AppState>, cx: &App) -> AnyElement {
    let discord = cx.discord();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .h(px(28.0))
        .rounded(px(4.0))
        .bg(discord.blurple)
        .hover(|s| s.bg(colors::blurple_hover()))
        .cursor_pointer()
        .text_color(discord.white)
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity.update(cx, |state, cx| {
                state.open_modal(ActiveModal::Login);
                cx.notify();
            });
        })
        .child(icon_key(px(14.0), discord.white))
        .child("Entrar no Discord")
        .into_any_element()
}

#[derive(IntoElement)]
struct HeaderSearchTrigger {
    input_elem: AnyElement,
    input_st: Entity<InputState>,
    app_entity: Entity<AppState>,
    is_open: bool,
}

impl Selectable for HeaderSearchTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.is_open = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.is_open
    }
}

impl RenderOnce for HeaderSearchTrigger {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let state_entity = self.app_entity;
        let input_st = self.input_st;
        div()
            .w(px(144.0))
            .on_key_down(move |ev: &KeyDownEvent, window, cx| {
                if ev.keystroke.key == "escape" {
                    input_st.update(cx, |s, cx| {
                        s.set_value("", window, cx);
                    });
                    state_entity.update(cx, |s, cx| {
                        s.clear_search();
                        s.close_search();
                        cx.notify();
                    });
                }
            })
            .child(self.input_elem)
    }
}

fn render_search_box(
    entity: Entity<AppState>,
    search_input_st: &Entity<InputState>,
    is_popover_open: bool,
    _is_search_open: bool,
    _cx: &App,
) -> AnyElement {
    let input_field = TextInput::new("header-search-input", search_input_st)
        .placeholder("Buscar")
        .show_paste(false)
        .show_clear(true);

    let trigger = HeaderSearchTrigger {
        input_elem: input_field.into_any_element(),
        input_st: search_input_st.clone(),
        app_entity: entity.clone(),
        is_open: is_popover_open,
    };

    let app_ent = entity.clone();
    Popover::new("header-search-popover")
        .trigger(trigger)
        .open(is_popover_open)
        .anchor(Anchor::TopLeft)
        .offset(px(4.0))
        .content(move |_state, _window, _cx| {
            crate::chat::search::render_search_popover_content(app_ent.clone())
        })
        .into_any_element()
}

fn render_video_camera_icon(size: Pixels, color: Rgba) -> AnyElement {
    div()
        .size(size)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(
                    div()
                        .w(px(10.0))
                        .h(px(8.0))
                        .rounded(px(1.5))
                        .bg(color),
                )
                .child(
                    div()
                        .w(px(3.0))
                        .h(px(6.0))
                        .rounded(px(1.0))
                        .bg(color),
                ),
        )
        .into_any_element()
}

