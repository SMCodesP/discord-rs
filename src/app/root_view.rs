use gpui::*;
use gpui::component::WindowExt;
use gpui::component::dialog::Dialog;
use gpui::component::input::{AnyInputState, InputEvent, InputState};
use super::channel_sidebar::ChannelSidebar;
use super::guild_rail::GuildRail;
use super::header_bar::HeaderBar;
use super::member_sidebar::MemberSidebar;
use super::state::{ActiveModal, AppState, ChannelKind};
use super::user_settings::UserSettingsModal;
use super::voice_call_panel::VoiceCallPanel;
use crate::chat::ChatView;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::banner::Banner;
use crate::components::button::{Button, ButtonSize};
use crate::components::text_input::TextInput;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// RootView that composes the native Discord App Shell in GPUI:
/// - If not connected to Gateway: Mandatory full-screen Discord login screen with auto-focus.
/// - When connected: Complete 4-column Discord App Shell:
///   - Guild Rail (72px)
///   - Channel Sidebar (240px)
///   - Main Center Content (Flex-1: Header Bar + Chat / Voice Call Stage)
///   - Member Sidebar (240px, toggled)
///   - Modal Overlays (User Settings, Add Server, Explore, Manage Connection)
pub struct RootView {
    pub state: Entity<AppState>,
    pub telemetry: crate::telemetry::TelemetryHandle,
    guild_rail: Entity<GuildRail>,
    channel_sidebar: Entity<ChannelSidebar>,
    header_bar: Entity<HeaderBar>,
    member_sidebar: Entity<MemberSidebar>,
    voice_panel: Entity<VoiceCallPanel>,
    user_settings: Entity<UserSettingsModal>,
    pub chat_view: Entity<ChatView>,
    token_login_state: Option<Entity<InputState>>,
    token_login_sub: Option<Subscription>,
    token_modal_state: Option<Entity<InputState>>,
    token_modal_sub: Option<Subscription>,
    quick_msg_state: Option<Entity<InputState>>,
    quick_msg_sub: Option<Subscription>,
    friends_search_state: Option<Entity<InputState>>,
    friends_search_sub: Option<Subscription>,
    friends_search_query: String,
    add_friend_state: Option<Entity<InputState>>,
    add_friend_sub: Option<Subscription>,
    active_modal_open: Option<ActiveModal>,
    profile_modal_open: bool,
}

impl RootView {
    pub fn new(telemetry: crate::telemetry::TelemetryHandle, cx: &mut Context<Self>) -> Self {
        let (client_manager, mut event_rx) = crate::protocol::ClientManager::new();
        let cm_auto_login = client_manager.clone();
        let tel_init = telemetry.clone();
        let state = cx.new(|_cx| {
            let mut s = AppState::new();
            s.telemetry = Some(tel_init);
            s.client_manager = Some(client_manager.clone());
            if let Some(ref token) = s.auth_token {
                let cm = cm_auto_login;
                let auth = if s.is_user_token {
                    crate::protocol::AuthToken::user(token.clone())
                } else {
                    crate::protocol::AuthToken::bot(token.clone())
                };
                tokio::spawn(async move {
                    if let Err(e) = cm.connect_with_token(auth).await {
                        log::error!("Falha no auto-login ao inicializar com token salvo: {:?}", e);
                    }
                });
            }
            s
        });

        let state_weak = state.downgrade();
        cx.spawn(|_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok(evt) = event_rx.recv().await {
                    let _ = cx.update(|cx| {
                        let _ = state_weak.update(cx, |state, cx| {
                            state.handle_transport_event(evt);
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();

        cx.set_global(crate::app::state::AppStateGlobal(state.clone()));

        let guild_rail = cx.new(|cx| GuildRail::new(state.clone(), cx));
        let channel_sidebar = cx.new(|cx| ChannelSidebar::new(state.clone(), cx));
        let header_bar = cx.new(|cx| HeaderBar::new(state.clone(), cx));
        let member_sidebar = cx.new(|cx| MemberSidebar::new(state.clone(), cx));
        let voice_panel = cx.new(|cx| VoiceCallPanel::new(state.clone(), cx));
        let user_settings = cx.new(|cx| UserSettingsModal::new(state.clone(), cx));
        let chat_view = cx.new(|cx| ChatView::new(state.clone(), cx));

        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        Self {
            state,
            telemetry,
            guild_rail,
            channel_sidebar,
            header_bar,
            member_sidebar,
            voice_panel,
            user_settings,
            chat_view,
            token_login_state: None,
            token_login_sub: None,
            token_modal_state: None,
            token_modal_sub: None,
            quick_msg_state: None,
            quick_msg_sub: None,
            friends_search_state: None,
            friends_search_sub: None,
            friends_search_query: String::new(),
            add_friend_state: None,
            add_friend_sub: None,
            active_modal_open: None,
            profile_modal_open: false,
        }
    }

    pub fn with_state(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let telemetry = state.read(cx).telemetry.clone().unwrap_or_else(|| crate::telemetry::global().clone());
        let guild_rail = cx.new(|cx| GuildRail::new(state.clone(), cx));
        let channel_sidebar = cx.new(|cx| ChannelSidebar::new(state.clone(), cx));
        let header_bar = cx.new(|cx| HeaderBar::new(state.clone(), cx));
        let member_sidebar = cx.new(|cx| MemberSidebar::new(state.clone(), cx));
        let voice_panel = cx.new(|cx| VoiceCallPanel::new(state.clone(), cx));
        let user_settings = cx.new(|cx| UserSettingsModal::new(state.clone(), cx));
        let chat_view = cx.new(|cx| ChatView::new(state.clone(), cx));

        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();

        cx.set_global(crate::app::state::AppStateGlobal(state.clone()));

        Self {
            state,
            telemetry,
            guild_rail,
            channel_sidebar,
            header_bar,
            member_sidebar,
            voice_panel,
            user_settings,
            chat_view,
            token_login_state: None,
            token_login_sub: None,
            token_modal_state: None,
            token_modal_sub: None,
            quick_msg_state: None,
            quick_msg_sub: None,
            friends_search_state: None,
            friends_search_sub: None,
            friends_search_query: String::new(),
            add_friend_state: None,
            add_friend_sub: None,
            active_modal_open: None,
            profile_modal_open: false,
        }
    }
}

/// Creates a token field kit state once, with a `Change` subscription that
/// mirrors the kit text back into `AppState::token_input`.
fn ensure_token_input(
    slot: &mut Option<Entity<InputState>>,
    sub: &mut Option<Subscription>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    if slot.is_some() {
        return;
    }
    let initial = app_state.read(cx).token_input.clone();
    let st = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(initial)
            .masked(true)
    });
    let app = app_state.clone();
    let sub_new = cx.subscribe_in(&st, window, move |_view, st, event: &InputEvent, _window, cx| {
        if matches!(event, InputEvent::Change) {
            let v = st.read(cx).value().to_string();
            app.update(cx, |s, cx| {
                s.token_input = v;
                cx.notify();
            });
        }
    });
    *slot = Some(st);
    *sub = Some(sub_new);
}

/// Caller-to-kit sync for token fields (placeholder + value).
/// Kit-to-caller flows through the subscription; `set_value` emits no event.
fn sync_token_input(
    st: &Entity<InputState>,
    app_state: &Entity<AppState>,
    placeholder: SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    if st.read(cx).presentation().placeholder() != &placeholder {
        let ph = placeholder.clone();
        st.update(cx, |s, cx| s.set_placeholder(ph, window, cx));
    }
    let want: SharedString = app_state.read(cx).token_input.clone().into();
    if st.read(cx).value() != want {
        st.update(cx, |s, cx| s.set_value(want.clone(), window, cx));
    }
}

/// Creates the popout quick-message kit state once, with `Change` (mirror to
/// `popout.quick_message`) and `PressEnter` (send) subscriptions.
fn ensure_quick_input(
    slot: &mut Option<Entity<InputState>>,
    sub: &mut Option<Subscription>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    if slot.is_some() {
        return;
    }
    let st = cx.new(|cx| InputState::new(window, cx));
    let app = app_state.clone();
    let sub_new = cx.subscribe_in(
        &st,
        window,
        move |_view, st, event: &InputEvent, _window, cx| match event {
            InputEvent::Change => {
                let v = st.read(cx).value().to_string();
                app.update(cx, |s, cx| {
                    if let Some(ref mut p) = s.profile_popout {
                        p.quick_message = v;
                    }
                    cx.notify();
                });
            }
            InputEvent::PressEnter { .. } => {
                app.update(cx, |s, cx| {
                    s.send_profile_popout_quick_message();
                    cx.notify();
                });
            }
            _ => {}
        },
    );
    *slot = Some(st);
    *sub = Some(sub_new);
}

/// Caller-to-kit sync for the quick message (value + placeholder).
fn sync_quick_input(
    st: &Entity<InputState>,
    want_value: String,
    want_placeholder: String,
    window: &mut Window,
    cx: &mut App,
) {
    let want_ph: SharedString = want_placeholder.into();
    if st.read(cx).presentation().placeholder() != &want_ph {
        let ph = want_ph.clone();
        st.update(cx, |s, cx| s.set_placeholder(ph, window, cx));
    }
    let want: SharedString = want_value.into();
    if st.read(cx).value() != want {
        st.update(cx, |s, cx| s.set_value(want.clone(), window, cx));
    }
}

/// Creates the friends search kit state once, with a `Change` subscription that
/// updates `RootView::friends_search_query` and requests a re-render.
fn ensure_friends_search_input(
    slot: &mut Option<Entity<InputState>>,
    sub: &mut Option<Subscription>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    if slot.is_some() {
        return;
    }
    let st = cx.new(|cx| InputState::new(window, cx));
    let sub_new = cx.subscribe_in(&st, window, move |view, st, event: &InputEvent, _window, cx| {
        if matches!(event, InputEvent::Change) {
            view.friends_search_query = st.read(cx).value().to_string();
            cx.notify();
        }
    });
    *slot = Some(st);
    *sub = Some(sub_new);
}

/// Creates the add friend kit state once, with `Change` (mirroring to `AppState::add_friend_input`)
/// and `PressEnter` (calling `AppState::send_friend_request`) subscriptions.
fn ensure_add_friend_input(
    slot: &mut Option<Entity<InputState>>,
    sub: &mut Option<Subscription>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    if slot.is_some() {
        return;
    }
    let initial = app_state.read(cx).add_friend_input.clone();
    let st = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(initial)
    });
    let app = app_state.clone();
    let sub_new = cx.subscribe_in(&st, window, move |_view, st, event: &InputEvent, _window, cx| {
        match event {
            InputEvent::Change => {
                let v = st.read(cx).value().to_string();
                app.update(cx, |s, cx| {
                    s.add_friend_input = v;
                    cx.notify();
                });
            }
            InputEvent::PressEnter { .. } => {
                app.update(cx, |s, cx| {
                    let target = s.add_friend_input.clone();
                    if !s.is_sending_friend_request && !target.trim().is_empty() {
                        s.send_friend_request(&target);
                        cx.notify();
                    }
                });
            }
            _ => {}
        }
    });
    *slot = Some(st);
    *sub = Some(sub_new);
}

/// Caller-to-kit sync for the add friend input field.
fn sync_add_friend_input(
    st: &Entity<InputState>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) {
    let want: SharedString = app_state.read(cx).add_friend_input.clone().into();
    if st.read(cx).value() != want {
        st.update(cx, |s, cx| s.set_value(want.clone(), window, cx));
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Kit input states live on this view (never in AppState). Ensure +
        // caller-to-kit sync before borrowing AppState for the tree.
        let app = self.state.clone();
        ensure_token_input(
            &mut self.token_login_state,
            &mut self.token_login_sub,
            &app,
            window,
            cx,
        );
        ensure_token_input(
            &mut self.token_modal_state,
            &mut self.token_modal_sub,
            &app,
            window,
            cx,
        );
        ensure_friends_search_input(
            &mut self.friends_search_state,
            &mut self.friends_search_sub,
            window,
            cx,
        );
        ensure_add_friend_input(
            &mut self.add_friend_state,
            &mut self.add_friend_sub,
            &app,
            window,
            cx,
        );
        if let Some(ref add_st) = self.add_friend_state {
            sync_add_friend_input(add_st, &app, window, cx);
        }
        let is_user_account = self.state.read(cx).is_user_token;
        let login_placeholder: SharedString = if is_user_account {
            "Cole seu token de usuário aqui...".into()
        } else {
            "Cole o token do Bot (ex: MTI0...)".into()
        };
        if let Some(st) = self.token_login_state.clone() {
            sync_token_input(&st, &app, login_placeholder.clone(), window, cx);
        }
        if let Some(st) = self.token_modal_state.clone() {
            sync_token_input(&st, &app, login_placeholder.clone(), window, cx);
        }

        // Quick-message state: sync while a popout is open, drop on close so
        // no text leaks between users.
        let quick_sync: Option<(String, String)> = self
            .state
            .read(cx)
            .profile_popout
            .as_ref()
            .map(|p| {
                let uname = p
                    .data
                    .as_ref()
                    .map(|d| d.user.username.clone())
                    .unwrap_or_else(|| p.user_id.clone());
                (
                    p.quick_message.clone(),
                    format!("Conversar com @{}...", uname),
                )
            });
        if let Some((ref want_val, ref want_ph)) = quick_sync {
            ensure_quick_input(
                &mut self.quick_msg_state,
                &mut self.quick_msg_sub,
                &app,
                window,
                cx,
            );
            if let Some(st) = self.quick_msg_state.clone() {
                sync_quick_input(&st, want_val.clone(), want_ph.clone(), window, cx);
            }
        } else {
            self.quick_msg_state = None;
            self.quick_msg_sub = None;
        }

        let (is_connected_to_gateway, active_modal, is_prof_modal) = {
            let s = self.state.read(cx);
            (
                s.is_connected_to_gateway,
                s.active_modal.clone(),
                s.is_profile_modal_open(),
            )
        };

        // =========================================================================
        // MANDATORY INITIAL LOGIN SCREEN:
        // If not connected to the live Discord Gateway, show the full-screen login.
        // No chat background or sidebar is leaked.
        // =========================================================================
        if !is_connected_to_gateway {
            // Autofocus the kit token field itself (its own focus handle).
            if let Some(st) = self.token_login_state.clone() {
                let fh = AnyInputState::from(st).focus_handle(cx);
                if !fh.is_focused(window) {
                    window.focus(&fh, cx);
                }
            }
            let state_snapshot = self.state.read(cx).clone();
            return render_login_screen(
                self.state.clone(),
                &state_snapshot,
                self.token_login_state.as_ref().expect("login token state"),
            )
            .into_any_element();
        }

        // =========================================================================
        // AUTHENTICATED APP SHELL:
        // Transition seamlessly to the full native Discord layout once connected.
        // =========================================================================

        // Synchronize ActiveModal state with WindowState dialog layer
        let current_modal = active_modal.clone();
        if self.active_modal_open != current_modal {
            if self.active_modal_open.is_some() {
                window.close_all_dialogs(cx);
            }
            self.active_modal_open = current_modal.clone();
            if let Some(ref modal) = current_modal {
                let entity = self.state.clone();
                match modal {
                    ActiveModal::UserSettings => {}
                    ActiveModal::AddServer => {
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_add_server_dialog(dialog, entity.clone(), vh, cx)
                        });
                    }
                    ActiveModal::ExploreServers => {
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_explore_dialog(dialog, entity.clone(), vh, cx)
                        });
                    }
                    ActiveModal::Login | ActiveModal::LoginDiscord => {
                        let token_state = self.token_modal_state.as_ref().expect("modal token state").clone();
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_login_dialog(dialog, entity.clone(), &token_state, vh, cx)
                        });
                    }
                    ActiveModal::AddFriendToGroup(channel_id) => {
                        let cid = channel_id.clone();
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_add_friend_to_group_dialog(dialog, entity.clone(), cid.clone(), vh, cx)
                        });
                    }
                    ActiveModal::CreateGroupDm => {
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_create_group_dm_dialog(dialog, entity.clone(), vh, cx)
                        });
                    }
                }
            }
        } else if self.active_modal_open.is_some()
            && self.active_modal_open != Some(ActiveModal::UserSettings)
            && !window.has_active_dialog(cx)
        {
            self.active_modal_open = None;
            let entity = self.state.clone();
            entity.update(cx, |s, cx| {
                if s.active_modal.is_some() && s.active_modal != Some(ActiveModal::UserSettings) {
                    s.close_modal();
                    cx.notify();
                }
            });
        }

        // Synchronize UserProfileModal with WindowState dialog layer
        if self.profile_modal_open != is_prof_modal {
            if self.profile_modal_open {
                // Fechamos apenas o diálogo do topo (o modal de perfil), em vez de
                // `window.close_all_dialogs(cx)`, para preservar diálogos de fundo
                // se múltiplos estiverem empilhados no WindowState dialog layer.
                window.close_dialog(cx);
            }
            self.profile_modal_open = is_prof_modal;
            if is_prof_modal {
                let entity = self.state.clone();
                window.open_dialog(cx, move |dialog, window, cx| {
                    let vh = window.viewport_size().height;
                    crate::app::user_profile_popout::build_user_profile_modal_dialog(dialog, entity.clone(), vh, cx)
                });
            }
        } else if self.profile_modal_open && !window.has_active_dialog(cx) {
            self.profile_modal_open = false;
            let entity = self.state.clone();
            entity.update(cx, |s, cx| {
                if s.is_profile_modal_open() {
                    s.close_user_profile_modal();
                    cx.notify();
                }
            });
        }

        let state = self.state.read(cx);
        let show_members = state.show_members && state.selected_guild_id.is_some();
        let selected_channel = state.get_selected_channel().map(|(_, ch)| ch.clone());
        let in_voice_channel = selected_channel
            .as_ref()
            .map(|ch| ch.kind == ChannelKind::Voice)
            .unwrap_or(false);
        let app_entity = self.state.clone();

        div()
            .relative()
            .size_full()
            .bg(bg_chat())
            .text_color(text_primary())
            .overflow_hidden()
            .flex()
            .flex_row()
            // =========================================================================
            // COLUMN 1: Server Rail (72px)
            // =========================================================================
            .child(self.guild_rail.clone())
            // =========================================================================
            // COLUMN 2: Channel Sidebar (240px)
            // =========================================================================
            .child(self.channel_sidebar.clone())
            // =========================================================================
            // COLUMN 3: Main Chat / Voice Call Stage Area (Flex-1)
            // =========================================================================
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.0))
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .bg(bg_chat())
                    .overflow_hidden()
                    // Top Header Bar (48px) - shown for chat / voice channels; Friends Hub renders its own unified header bar
                    .children((!(state.selected_guild_id.is_none() && state.selected_channel_id.is_none())).then(|| self.header_bar.clone()))
                    // Center Content (Voice Panel if in voice channel, Friends View if in Home with no channel selected, else Chat area)
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(if in_voice_channel {
                                self.voice_panel.clone().into_any_element()
                            } else if state.selected_guild_id.is_none() && state.selected_channel_id.is_none() {
                                let search_state = self
                                    .friends_search_state
                                    .as_ref()
                                    .expect("friends_search_state must be initialized");
                                let add_friend_state = self
                                    .add_friend_state
                                    .as_ref()
                                    .expect("add_friend_state must be initialized");
                                render_friends_view(
                                    app_entity.clone(),
                                    state,
                                    search_state,
                                    &self.friends_search_query,
                                    add_friend_state,
                                )
                                .into_any_element()
                            } else {
                                self.chat_view.clone().into_any_element()
                            }),
                    ),
            )
            // =========================================================================
            // COLUMN 4: Member Sidebar (240px, toggled)
            // =========================================================================
            .children(if show_members {
                Some(self.member_sidebar.clone())
            } else {
                None
            })
            // =========================================================================
            // MODAL OVERLAY LAYER (User Settings is full-screen, others use Dialog)
            // =========================================================================
            .children(if let Some(ActiveModal::UserSettings) = active_modal {
                Some(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(self.user_settings.clone()),
                )
            } else {
                None
            })
            // =========================================================================
            // USER PROFILE POPOUT OVERLAY (Fase 4)
            // =========================================================================
            .children(if let Some(ref popout) = state.profile_popout {
                let entity = app_entity.clone();
                let popout_state = popout.clone();
                let catalog = state.profile_effects_catalog.clone();
                let presence = state.presences_by_user.get(&popout.user_id).cloned();
                Some(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(crate::app::user_profile_popout::render_user_profile_popout(
                            entity,
                            &popout_state,
                            state.profile_anchor,
                            &catalog,
                            presence.as_ref(),
                            self.quick_msg_state.as_ref().expect("quick message state"),
                        )),
                )
            } else {
                None
            })
            .into_any_element()
    }
}

/// Renders the mandatory full-screen initial login view when not connected to Discord Gateway.
/// Focuses the token input automatically, provides smooth connecting state, and removes demo mode.
fn render_login_screen(
    entity: Entity<AppState>,
    state: &AppState,
    token_state: &Entity<InputState>,
) -> impl IntoElement {
    let entity_connect = entity.clone();
    let entity_bot_toggle = entity.clone();
    let entity_user_toggle = entity.clone();

    let is_user = state.is_user_token;
    let is_connecting = state.is_connecting;

    // Status banner
    let banner = if let Some(ref err) = state.auth_error {
        Banner::danger("login-err", err.clone())
            .title("Erro na Autenticação")
    } else if is_connecting {
        Banner::info(
            "login-connecting",
            "Conectando ao Gateway WebSocket v10 e sincronizando dados da conta...",
        )
        .title("Autenticando")
    } else {
        Banner::info(
            "login-info",
            "Insira seu Bot Token do Discord Developer Portal ou Token de Usuário para acessar o Discord RS.",
        )
        .title("Autenticação Necessária")
    };

    let text_input = TextInput::new("discord-token-input", token_state)
        .label("TOKEN DE AUTENTICAÇÃO DO DISCORD")
        .placeholder(if is_user {
            "Cole seu token de usuário aqui..."
        } else {
            "Cole o token do Bot (ex: MTI0...)"
        })
        .secret(true)
        .full_width(true)
        .disabled(is_connecting);

    div()
        .size_full()
        .bg(rgb(HEX_BG_SERVER))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .p(space_6())
        .child(
            div()
                .w(px(520.0))
                .p(px(32.0))
                .rounded(radius_md())
                .bg(bg_channels())
                .border_1()
                .border_color(borders())
                .flex()
                .flex_col()
                .gap(space_4())
                // Header with Discord icon
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .text_center()
                        .gap(space_2())
                        .child(
                            div()
                                .size(px(52.0))
                                .rounded(radius_md())
                                .bg(blurple())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_gamepad(px(28.0), crate::tokens::colors::white())),
                        )
                        .child(
                            div()
                                .text_color(text_titles())
                                .font_weight(FontWeight::BOLD)
                                .text_size(font_size_2xl())
                                .child("Boas-vindas ao Discord RS"),
                        )
                        .child(
                            div()
                                .text_color(text_muted())
                                .text_size(font_size_sm())
                                .child("Cliente nativo desktop construído em Rust + GPUI"),
                        ),
                )
                // Status Banner
                .child(banner)
                // Mode Toggle: Bot vs User
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(space_2())
                        .child(
                            div()
                                .text_color(text_muted())
                                .text_size(font_size_xs())
                                .font_weight(FontWeight::BOLD)
                                .child("TIPO DE CONTA"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(space_3())
                                .w_full()
                                // Bot Token button
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(40.0))
                                        .rounded(radius_sm())
                                        .border_1()
                                        .border_color(if !is_user { blurple() } else { borders() })
                                        .bg(if !is_user { with_alpha(HEX_BLURPLE, 0.15) } else { bg_chat() })
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(space_2())
                                        .hover(|s| s.opacity(0.85))
                                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                            entity_bot_toggle.update(cx, |s, cx| {
                                                s.is_user_token = false;
                                                cx.notify();
                                            });
                                        })
                                        .child(icon_bot(px(18.0), if !is_user { text_titles() } else { text_muted() }))
                                        .child(
                                            div()
                                                .text_color(if !is_user { text_titles() } else { text_muted() })
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(font_size_sm())
                                                .child("Bot Token"),
                                        ),
                                )
                                // User Token button
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(40.0))
                                        .rounded(radius_sm())
                                        .border_1()
                                        .border_color(if is_user { blurple() } else { borders() })
                                        .bg(if is_user { with_alpha(HEX_BLURPLE, 0.15) } else { bg_chat() })
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(space_2())
                                        .hover(|s| s.opacity(0.85))
                                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                            entity_user_toggle.update(cx, |s, cx| {
                                                s.is_user_token = true;
                                                cx.notify();
                                            });
                                        })
                                        .child(icon_user(px(18.0), if is_user { text_titles() } else { text_muted() }))
                                        .child(
                                            div()
                                                .text_color(if is_user { text_titles() } else { text_muted() })
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_size(font_size_sm())
                                                .child("Conta de Usuário"),
                                        ),
                                ),
                        ),
                )
                // Token TextInput
                .child(text_input)
                // Action Buttons
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(space_2())
                        .pt(space_2())
                        // Connect button with loading feedback
                        .child(if is_connecting {
                            div()
                                .w_full()
                                .h(px(44.0))
                                .rounded(radius_sm())
                                .bg(blurple_hover())
                                .opacity(0.85)
                                .cursor_not_allowed()
                                .text_color(crate::tokens::colors::white())
                                .font_weight(FontWeight::BOLD)
                                .text_size(font_size_md())
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(space_2())
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.0))
                                        .child(div().size(px(6.0)).rounded_full().bg(crate::tokens::colors::white()))
                                        .child(div().size(px(6.0)).rounded_full().bg(crate::tokens::colors::white()).opacity(0.7))
                                        .child(div().size(px(6.0)).rounded_full().bg(crate::tokens::colors::white()).opacity(0.4)),
                                )
                                .child("Conectando ao Discord...")
                        } else {
                            div()
                                .w_full()
                                .h(px(44.0))
                                .rounded(radius_sm())
                                .bg(blurple())
                                .text_color(crate::tokens::colors::white())
                                .font_weight(FontWeight::BOLD)
                                .text_size(font_size_md())
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(blurple_hover()))
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    entity_connect.update(cx, |s, cx| {
                                        let token = s.token_input.clone();
                                        let is_u = s.is_user_token;
                                        s.authenticate(token, is_u);
                                        cx.notify();
                                    });
                                })
                                .child("Conectar ao Discord")
                        })
                        // Security notice footer
                        .child(
                            div()
                                .pt(space_2())
                                .w_full()
                                .max_w(px(456.0))
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_center()
                                .gap(px(6.0))
                                .text_color(text_muted())
                                .text_size(px(12.0))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .child(icon_lock(px(14.0), text_muted())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .text_center()
                                        .child("Armazenamento seguro de token com permissões restritas (0600) em ~/.config/discord-rs/auth.json"),
                                ),
                        ),
                ),
        )
}

/// Builds the Dialog for "Conectar ao Discord" (Bot / User Token modal inside App Shell)
fn build_login_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    token_state: &Entity<InputState>,
    vh: Pixels,
    cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let state = entity.read(cx);
    let is_user = state.is_user_token;
    let is_connecting = state.is_connecting;

    let header = div()
        .flex()
        .flex_col()
        .gap(space_1())
        .items_center()
        .text_center()
        .child(
            div()
                .text_color(text_titles())
                .font_weight(FontWeight::BOLD)
                .text_size(font_size_2xl())
                .child("Gerenciar Conexão do Discord"),
        )
        .child(
            div()
                .text_color(text_muted())
                .text_size(font_size_sm())
                .child("Acesse servidores, canais e mensagens em tempo real via Gateway v10"),
        );

    let banner = build_login_banner(state);
    let toggle = build_login_mode_toggle(entity.clone(), is_user);
    let text_input = build_login_token_input(token_state, is_user, is_connecting);
    let actions = build_login_actions(entity.clone(), is_connecting);

    let margin_top = (vh - px(411.0)).max(px(20.0)) / 2.0;

    dialog
        .w(px(520.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .child(header)
                .child(banner)
                .child(toggle)
                .child(text_input)
                .child(actions),
        )
}

fn build_login_banner(state: &AppState) -> AnyElement {
    if let Some(ref err) = state.auth_error {
        Banner::danger("login-err", err.clone())
            .title("Erro na Autenticação")
            .into_any_element()
    } else if state.is_connected_to_gateway {
        Banner::success(
            "login-success",
            format!("Conectado como @{}", state.current_user.username),
        )
        .title("Conectado ao Discord")
        .into_any_element()
    } else if state.is_connecting {
        Banner::info("login-connecting", "Conectando ao Gateway WebSocket v10 e REST API...")
            .title("Autenticando")
            .into_any_element()
    } else {
        Banner::info(
            "login-info",
            "Insira um Bot Token do Discord Developer Portal ou Token de Usuário para sincronizar mensagens em tempo real.",
        )
        .title("Gerenciar Conexão do Discord")
        .into_any_element()
    }
}

fn build_login_mode_toggle(entity: Entity<AppState>, is_user: bool) -> AnyElement {
    let entity_bot_toggle = entity.clone();
    let entity_user_toggle = entity;

    div()
        .flex()
        .flex_col()
        .gap(space_2())
        .child(
            div()
                .text_color(text_muted())
                .text_size(font_size_xs())
                .font_weight(FontWeight::BOLD)
                .child("TIPO DE CONTA"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(space_3())
                .w_full()
                .child(
                    div()
                        .flex_1()
                        .h(px(40.0))
                        .rounded(radius_sm())
                        .border_1()
                        .border_color(if !is_user { blurple() } else { borders() })
                        .bg(if !is_user { with_alpha(HEX_BLURPLE, 0.15) } else { bg_chat() })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(space_2())
                        .hover(|s| s.opacity(0.85))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            entity_bot_toggle.update(cx, |s, cx| {
                                s.is_user_token = false;
                                cx.notify();
                            });
                        })
                        .child(icon_bot(px(18.0), if !is_user { text_titles() } else { text_muted() }))
                        .child(
                            div()
                                .text_color(if !is_user { text_titles() } else { text_muted() })
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(font_size_sm())
                                .child("Bot Token"),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(40.0))
                        .rounded(radius_sm())
                        .border_1()
                        .border_color(if is_user { blurple() } else { borders() })
                        .bg(if is_user { with_alpha(HEX_BLURPLE, 0.15) } else { bg_chat() })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(space_2())
                        .hover(|s| s.opacity(0.85))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            entity_user_toggle.update(cx, |s, cx| {
                                s.is_user_token = true;
                                cx.notify();
                            });
                        })
                        .child(icon_user(px(18.0), if is_user { text_titles() } else { text_muted() }))
                        .child(
                            div()
                                .text_color(if is_user { text_titles() } else { text_muted() })
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(font_size_sm())
                                .child("Conta de Usuário"),
                        ),
                ),
        )
        .into_any_element()
}

fn build_login_token_input(
    token_state: &Entity<InputState>,
    is_user: bool,
    is_connecting: bool,
) -> AnyElement {
    TextInput::new("discord-token-modal-input", token_state)
        .label("TOKEN DE AUTENTICAÇÃO DO DISCORD")
        .placeholder(if is_user {
            "Cole seu token de usuário aqui..."
        } else {
            "Cole o token do Bot (ex: MTI0...)"
        })
        .secret(true)
        .full_width(true)
        .disabled(is_connecting)
        .into_any_element()
}

fn build_login_actions(entity: Entity<AppState>, is_connecting: bool) -> AnyElement {
    let entity_connect = entity.clone();
    let entity_close = entity;

    div()
        .flex()
        .flex_col()
        .gap(space_2())
        .pt(space_2())
        .child(
            Button::blurple(
                "login-connect-action-btn",
                if is_connecting {
                    "Conectando ao Discord..."
                } else {
                    "Conectar ao Discord"
                },
            )
            .size(ButtonSize::Lg)
            .full_width()
            .disabled(is_connecting)
            .on_click(move |_event, _window, cx| {
                entity_connect.update(cx, |s, cx| {
                    let token = s.token_input.clone();
                    let is_u = s.is_user_token;
                    s.authenticate(token, is_u);
                    cx.notify();
                });
            }),
        )
        .child(
            Button::ghost("login-back-action-btn", "Voltar")
                .size(ButtonSize::Md)
                .full_width()
                .on_click(move |_event, window, cx| {
                    window.close_dialog(cx);
                    entity_close.update(cx, |s, cx| {
                        s.close_modal();
                        cx.notify();
                    });
                }),
        )
        .into_any_element()
}

/// Builds the Dialog for "Criar seu servidor"
fn build_add_server_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    vh: Pixels,
    _cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let entity_back = entity;

    let margin_top = (vh - px(196.0)).max(px(20.0)) / 2.0;

    dialog
        .w(px(440.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .items_center()
                .text_center()
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(22.0))
                        .child("Criar seu servidor"),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(13.0))
                        .child("Seu servidor é onde você e seus amigos se reúnem."),
                )
                .child(
                    Button::blurple("create-server-action", "Criar o Meu Próprio")
                        .size(ButtonSize::Lg)
                        .full_width(),
                )
                .child(
                    Button::gray("create-server-back", "Voltar")
                        .size(ButtonSize::Md)
                        .full_width()
                        .on_click(move |_event, window, cx| {
                            window.close_dialog(cx);
                            entity_back.update(cx, |s, cx| {
                                s.close_modal();
                                cx.notify();
                            });
                        }),
                ),
        )
}

/// Builds the Dialog for "Descubra Comunidades"
fn build_explore_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    vh: Pixels,
    _cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let entity_btn = entity;

    let margin_top = (vh - px(144.0)).max(px(20.0)) / 2.0;

    dialog
        .w(px(480.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .items_center()
                .text_center()
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(22.0))
                        .child("Descubra Comunidades"),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(13.0))
                        .child("Encontre comunidades incríveis de Rust, GPUI, Gaming e Música."),
                )
                .child(
                    Button::blurple("explore-servers-close", "Fechar")
                        .size(ButtonSize::Md)
                        .full_width()
                        .on_click(move |_event, window, cx| {
                            window.close_dialog(cx);
                            entity_btn.update(cx, |s, cx| {
                                s.close_modal();
                                cx.notify();
                            });
                        }),
                ),
        )
}

/// Builds the Dialog to add friends to an existing DM channel / group DM
fn build_add_friend_to_group_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    channel_id: String,
    vh: Pixels,
    cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let state = entity.read(cx);
    let current_dm = state.dm_channels.iter().find(|c| c.id == channel_id);
    let current_recipients = current_dm.map(|c| c.recipient_ids.clone()).unwrap_or_default();

    let mut friend_rows = Vec::new();
    for friend in &state.friends {
        let is_already_member = current_recipients.contains(&friend.id);
        friend_rows.push(render_add_friend_row(
            entity.clone(),
            channel_id.clone(),
            friend.id.clone(),
            friend.username.clone(),
            friend.display_name.clone(),
            is_already_member,
        ));
    }

    let friend_count = state.friends.len();
    let list_h = if friend_count == 0 {
        50.0
    } else {
        (friend_count as f32 * 46.0).min(260.0)
    };
    let approx_h = 98.0 + list_h;
    let margin_top = (vh - px(approx_h)).max(px(20.0)) / 2.0;

    if friend_rows.is_empty() {
        friend_rows.push(
            div()
                .p(space_4())
                .text_center()
                .text_color(legacy_949ba4())
                .text_size(px(13.0))
                .child("Nenhum amigo disponível na sua lista para adicionar.")
                .into_any_element(),
        );
    }

    let header = div()
        .flex()
        .flex_col()
        .gap(space_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(space_2())
                .child(icon_friends(px(20.0), blurple()))
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child("Adicionar amigos à conversa"),
                ),
        )
        .child(
            div()
                .text_color(legacy_949ba4())
                .text_size(px(13.0))
                .child("Selecione amigos para adicionar a este grupo de conversa direta."),
        );

    dialog
        .w(px(460.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .child(header)
                .child(
                    div()
                        .id("add-friend-scroll")
                        .max_h(px(260.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .children(friend_rows),
                ),
        )
}

fn render_add_friend_row(
    entity: Entity<AppState>,
    channel_id: String,
    fid: String,
    fname: String,
    dname: String,
    is_already_member: bool,
) -> AnyElement {
    let entity_add = entity;
    let cid = channel_id;
    let friend_id = fid.clone();
    let friend_name = fname.clone();

    div()
        .h(px(44.0))
        .px(space_2())
        .rounded(radius_sm())
        .bg(bg_channels())
        .hover(|s| s.bg(channel_hover()))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(Avatar::new(dname.clone()).size(AvatarSize::Px32))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_color(text_titles())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child(dname),
                        )
                        .child(
                            div()
                                .text_color(legacy_949ba4())
                                .text_size(px(11.0))
                                .child(format!("@{}", fname)),
                        ),
                ),
        )
        .child(if is_already_member {
            div()
                .px(px(10.0))
                .py(px(4.0))
                .rounded(radius_sm())
                .bg(channel_active())
                .text_color(legacy_949ba4())
                .text_size(px(12.0))
                .child("Já adicionado")
                .into_any_element()
        } else {
            Button::blurple(format!("add-friend-btn-{}", fid), "Adicionar")
                .size(ButtonSize::Sm)
                .on_click(move |_ev, window, cx| {
                    entity_add.update(cx, |s, cx| {
                        s.add_recipient_to_group(&cid, &friend_id, &friend_name);
                        s.close_modal();
                        cx.notify();
                    });
                    window.close_dialog(cx);
                })
                .into_any_element()
        })
        .into_any_element()
}

/// Builds the Dialog to create a brand new Group DM
fn build_create_group_dm_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    vh: Pixels,
    cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let state = entity.read(cx);

    let mut friend_rows = Vec::new();
    for friend in &state.friends {
        friend_rows.push(render_create_group_dm_row(
            entity.clone(),
            friend.id.clone(),
            friend.username.clone(),
            friend.display_name.clone(),
        ));
    }

    let friend_count = state.friends.len();
    let list_h = if friend_count == 0 {
        50.0
    } else {
        (friend_count as f32 * 46.0).min(260.0)
    };
    let approx_h = 98.0 + list_h;
    let margin_top = (vh - px(approx_h)).max(px(20.0)) / 2.0;

    if friend_rows.is_empty() {
        friend_rows.push(
            div()
                .p(space_4())
                .text_center()
                .text_color(legacy_949ba4())
                .text_size(px(13.0))
                .child("Nenhum amigo disponível na sua lista para criar grupo.")
                .into_any_element(),
        );
    }

    let header = div()
        .flex()
        .flex_col()
        .gap(space_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(space_2())
                .child(icon_friends(px(20.0), blurple()))
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child("Criar DM em Grupo"),
                ),
        )
        .child(
            div()
                .text_color(legacy_949ba4())
                .text_size(px(13.0))
                .child("Selecione um amigo para iniciar um novo grupo de conversa."),
        );

    dialog
        .w(px(460.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .child(header)
                .child(
                    div()
                        .id("create-group-scroll")
                        .max_h(px(260.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .children(friend_rows),
                ),
        )
}

fn render_create_group_dm_row(
    entity: Entity<AppState>,
    fid: String,
    fname: String,
    dname: String,
) -> AnyElement {
    let entity_create = entity;
    let friend_id = fid.clone();
    let friend_name = fname.clone();

    div()
        .h(px(44.0))
        .px(space_2())
        .rounded(radius_sm())
        .bg(bg_channels())
        .hover(|s| s.bg(channel_hover()))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(Avatar::new(dname.clone()).size(AvatarSize::Px32))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_color(text_titles())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child(dname),
                        )
                        .child(
                            div()
                                .text_color(legacy_949ba4())
                                .text_size(px(11.0))
                                .child(format!("@{}", fname)),
                        ),
                ),
        )
        .child(
            Button::blurple(format!("create-dm-btn-{}", fid), "Criar Grupo")
                .size(ButtonSize::Sm)
                .on_click(move |_ev, window, cx| {
                    entity_create.update(cx, |s, cx| {
                        s.create_group_dm(vec![friend_id.clone()], vec![friend_name.clone()], None);
                        s.close_modal();
                        cx.notify();
                    });
                    window.close_dialog(cx);
                }),
        )
        .into_any_element()
}

/// Renders the Friends list view ("Amigos") when on Home without an active DM channel.
/// Delegates to `src/app/friends_hub.rs` which implements full visual parity (Tabs, Active Now 360px, Message Requests).
fn render_friends_view(
    entity: Entity<AppState>,
    state: &AppState,
    search_state: &Entity<InputState>,
    search_query: &str,
    add_friend_state: &Entity<InputState>,
) -> Div {
    super::friends_hub::render_friends_hub(
        entity,
        state,
        search_state,
        search_query,
        add_friend_state,
    )
}


