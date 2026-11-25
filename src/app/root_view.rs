use gpui::*;
use gpui::component::WindowExt;
use gpui::component::dialog::Dialog;
use gpui::component::input::{AnyInputState, InputEvent, InputState};
use super::channel_sidebar::ChannelSidebar;
use super::guild_rail::GuildRail;
use super::header_bar::HeaderBar;
use super::member_sidebar::MemberSidebar;
use super::state::{ActiveModal, AppState, ChannelKind, FriendItemState};
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
    friends_list_state: Option<ListState>,
    guild_nick_state: Option<Entity<InputState>>,
    guild_nick_sub: Option<Subscription>,
    quick_switcher_state: Option<Entity<InputState>>,
    quick_switcher_sub: Option<Subscription>,
    active_modal_open: Option<ActiveModal>,
    profile_modal_open: bool,
    spotify_timer: Option<gpui::Task<()>>,
    login_screen: Option<Entity<crate::app::login_screen::LoginScreen>>,
    focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(telemetry: crate::telemetry::TelemetryHandle, cx: &mut Context<Self>) -> Self {
        let (client_manager, _event_rx) = crate::protocol::ClientManager::new();
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
        let mut event_rx = client_manager.subscribe();
        cx.spawn(|_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                use tokio::sync::broadcast::error::RecvError;

                loop {
                    let deadline = {
                        let mut next = None;
                        let _ = cx.update(|cx| {
                            let _ = state_weak.update(cx, |state, _cx| {
                                next = state.next_transport_batch_deadline();
                            });
                        });
                        next
                    };

                    let sleep = async {
                        if let Some(deadline) = deadline {
                            tokio::time::sleep_until(tokio::time::Instant::from_std(deadline))
                                .await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    };

                    tokio::select! {
                        recv = event_rx.recv() => {
                            match recv {
                                Ok(evt) => {
                                    let _ = cx.update(|cx| {
                                        let _ = state_weak.update(cx, |state, cx| {
                                            let now = std::time::Instant::now();
                                            let mut ready = Vec::new();
                                            if let Some(immediate) = state.push_transport_event(evt, now) {
                                                ready.push(immediate);
                                            }
                                            ready.extend(state.flush_due_transport_events(now));
                                            if !ready.is_empty() {
                                                state.apply_transport_events(ready, cx);
                                            }
                                        });
                                    });
                                }
                                Err(RecvError::Lagged(_)) => continue,
                                Err(RecvError::Closed) => break,
                            }
                        }
                        _ = sleep => {
                            let _ = cx.update(|cx| {
                                let _ = state_weak.update(cx, |state, cx| {
                                    let due = state.flush_due_transport_events(std::time::Instant::now());
                                    if !due.is_empty() {
                                        state.apply_transport_events(due, cx);
                                    }
                                });
                            });
                        }
                    }
                }
            }
        })
        .detach();

        Self::spawn_voice_subscriptions(state.clone(), client_manager.clone(), cx);

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
            friends_list_state: None,
            add_friend_state: None,
            add_friend_sub: None,
            guild_nick_state: None,
            guild_nick_sub: None,
            quick_switcher_state: None,
            quick_switcher_sub: None,
            active_modal_open: None,
            profile_modal_open: false,
            spotify_timer: None,
            login_screen: None,
            focus_handle: cx.focus_handle(),
        }
    }

    fn spawn_voice_subscriptions(
        state: Entity<AppState>,
        client_manager: crate::protocol::ClientManager,
        cx: &mut Context<Self>,
    ) {
        let state_weak = state.downgrade();
        let cm_voice = client_manager.clone();
        let mut voice_rx = client_manager.subscribe_voice_state();
        cx.spawn(|_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok((reported_state, reported_ping)) = voice_rx.recv().await {
                    let maybe_session = cm_voice.active_voice_session().await;
                    let _ = cx.update(|cx| {
                        let _ = state_weak.update(cx, |state, cx| {
                            if let Some(session) = maybe_session {
                                // Lê a sessão ativa, não um evento atrasado de uma
                                // sessão anterior, para evitar estado cruzado.
                                state.set_voice_session(session.clone());
                                state.update_voice_session_state(
                                    session.state(),
                                    session.ping_ms(),
                                );
                            } else {
                                state.update_voice_session_state(reported_state, reported_ping);
                            }
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();

        let state_weak_events = state.downgrade();
        let mut voice_evt_rx = client_manager.subscribe_voice_events();
        cx.spawn(|_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok(event) = voice_evt_rx.recv().await {
                    let speaking = match &event {
                        crate::protocol::voice_session::VoiceSessionEvent::Speaking {
                            user_id: Some(user_id),
                            is_speaking,
                            ..
                        } => Some((user_id.to_string(), *is_speaking)),
                        _ => None,
                    };
                    let _ = cx.update(|cx| {
                        let _ = state_weak_events.update(cx, |state, cx| {
                            if let Some((user_id, is_speaking)) = speaking {
                                state.set_participant_speaking(&user_id, is_speaking);
                            }
                            // Também atualiza avisos não fatais da sessão.
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();
    }

    pub fn with_state(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let telemetry = state.read(cx).telemetry.clone().unwrap_or_else(|| crate::telemetry::global().clone());
        let voice_client_manager = state.read(cx).client_manager.clone();
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

        if let Some(client_manager) = voice_client_manager {
            Self::spawn_voice_subscriptions(state.clone(), client_manager, cx);
        }

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
            friends_list_state: None,
            add_friend_state: None,
            add_friend_sub: None,
            guild_nick_state: None,
            guild_nick_sub: None,
            quick_switcher_state: None,
            quick_switcher_sub: None,
            active_modal_open: None,
            profile_modal_open: false,
            spotify_timer: None,
            login_screen: None,
            focus_handle: cx.focus_handle(),
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
                if let Some(req) = app.update(cx, |s, cx| {
                    let r = s.begin_profile_popout_quick_message();
                    cx.notify();
                    r
                }) {
                    let app_weak = app.downgrade();
                    cx.spawn(|_view, cx: &mut AsyncApp| {
                        let cx = cx.clone();
                        async move {
                            let res = req.execute().await;
                            let _ = cx.update(|cx| {
                                let _ = app_weak.update(cx, |s, cx| {
                                    s.finish_profile_popout_quick_message(&req, res);
                                    cx.notify();
                                });
                            });
                        }
                    })
                    .detach();
                }
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
    let st = cx.new(|cx| InputState::new(window, cx).default_value(initial));
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

fn ensure_guild_nick_input(
    slot: &mut Option<Entity<InputState>>,
    sub: &mut Option<Subscription>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<RootView>,
) {
    if slot.is_some() {
        return;
    }
    let initial = app_state.read(cx).guild_nickname_draft.clone();
    let st = cx.new(|cx| InputState::new(window, cx).default_value(initial));
    let app = app_state.clone();
    let sub_new = cx.subscribe_in(&st, window, move |_view, st, event: &InputEvent, _window, cx| {
        if matches!(event, InputEvent::Change) {
            let v = st.read(cx).value().to_string();
            app.update(cx, |s, cx| {
                s.guild_nickname_draft = v;
                cx.notify();
            });
        }
    });
    *slot = Some(st);
    *sub = Some(sub_new);
}

fn sync_guild_nick_input(
    st: &Entity<InputState>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) {
    let want: SharedString = app_state.read(cx).guild_nickname_draft.clone().into();
    if st.read(cx).value() != want {
        st.update(cx, |s, cx| s.set_value(want.clone(), window, cx));
    }
}

fn ensure_quick_switcher_input(
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
    let sub_new = cx.subscribe_in(&st, window, move |_view, st, event: &InputEvent, _window, cx| {
        match event {
            InputEvent::Change => {
                let v = st.read(cx).value().to_string();
                app.update(cx, |s, cx| {
                    s.set_quick_switcher_query(v);
                    cx.notify();
                });
            }
            InputEvent::PressEnter { .. } => {
                app.update(cx, |s, cx| {
                    s.confirm_quick_switcher();
                    cx.notify();
                });
            }
            _ => {}
        }
    });
    *slot = Some(st);
    *sub = Some(sub_new);
}

fn sync_quick_switcher_input(
    st: &Entity<InputState>,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) {
    let want: SharedString = app_state.read(cx).quick_switcher_query().to_string().into();
    let ph: SharedString = "Where would you like to go?".into();
    if st.read(cx).presentation().placeholder() != &ph {
        st.update(cx, |s, cx| s.set_placeholder(ph.clone(), window, cx));
    }
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
        if self.friends_list_state.is_none() {
            self.friends_list_state = Some(ListState::new(0, ListAlignment::Top, px(120.0)));
        }
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

        let is_guild_nick_modal = matches!(
            self.state.read(cx).active_modal,
            Some(ActiveModal::EditGuildNickname { .. })
        );
        if is_guild_nick_modal {
            ensure_guild_nick_input(&mut self.guild_nick_state, &mut self.guild_nick_sub, &app, window, cx);
            if let Some(st) = self.guild_nick_state.clone() {
                sync_guild_nick_input(&st, &app, window, cx);
            }
        } else {
            self.guild_nick_state = None;
            self.guild_nick_sub = None;
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

        let quick_switcher_open = self.state.read(cx).quick_switcher.open;
        if quick_switcher_open {
            ensure_quick_switcher_input(
                &mut self.quick_switcher_state,
                &mut self.quick_switcher_sub,
                &app,
                window,
                cx,
            );
            if let Some(st) = self.quick_switcher_state.clone() {
                sync_quick_switcher_input(&st, &app, window, cx);
                window.focus(&st.read(cx).focus_handle(cx), cx);
            }
        } else {
            self.quick_switcher_state = None;
            self.quick_switcher_sub = None;
        }

        let (is_connected_to_gateway, active_modal, is_prof_modal, quick_switcher_open_state) = {
            let s = self.state.read(cx);
            (
                s.is_connected_to_gateway,
                s.active_modal.clone(),
                s.is_profile_modal_open(),
                s.quick_switcher.open,
            )
        };

        // =========================================================================
        // MANDATORY INITIAL LOGIN SCREEN:
        // If not connected to the live Discord Gateway, show the full-screen login.
        // No chat background or sidebar is leaked.
        // =========================================================================
        if !is_connected_to_gateway {
            let screen = self.login_screen.get_or_insert_with(|| {
                let app_state = self.state.clone();
                cx.new(|cx| crate::app::login_screen::LoginScreen::new(app_state, window, cx))
            });
            return screen.clone().into_any_element();
        }
        // Release QR WebSocket session once authenticated
        self.login_screen = None;

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
                    ActiveModal::EditGuildNickname { guild_id } => {
                        let gid = guild_id.clone();
                        let nick_state = self.guild_nick_state.as_ref().expect("guild nick state").clone();
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_edit_guild_nickname_dialog(dialog, entity.clone(), gid.clone(), &nick_state, vh, cx)
                        });
                    }
                    ActiveModal::LeaveGuild { guild_id } => {
                        let gid = guild_id.clone();
                        window.open_dialog(cx, move |dialog, window, cx| {
                            let vh = window.viewport_size().height;
                            build_leave_guild_dialog(dialog, entity.clone(), gid.clone(), vh, cx)
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
            .map(|ch| ch.kind.is_voice_like())
            .unwrap_or(false);
        let show_voice_stage = in_voice_channel
            || state
                .active_voice_call
                .as_ref()
                .map(|call| state.selected_channel_id.as_deref() == Some(&call.channel_id))
                .unwrap_or(false);
        let app_entity = self.state.clone();
        let app_entity_keys = app_entity.clone();

        div()
            .relative()
            .size_full()
            .bg(bg_chat())
            .text_color(text_primary())
            .overflow_hidden()
            .flex()
            .flex_row()
            .track_focus(&self.focus_handle)
            .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                let key = ev.keystroke.key.to_lowercase();
                let is_cmdk = (ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform) && key == "k";
                if is_cmdk {
                    app_entity_keys.update(cx, |s, cx| {
                        s.toggle_quick_switcher();
                        cx.notify();
                    });
                    return;
                }

                let quick_open = app_entity_keys.read(cx).quick_switcher.open;
                if quick_open {
                    match key.as_str() {
                        "escape" => {
                            app_entity_keys.update(cx, |s, cx| {
                                s.close_quick_switcher();
                                cx.notify();
                            });
                        }
                        "down" | "arrowdown" | "tab" if !ev.keystroke.modifiers.shift => {
                            app_entity_keys.update(cx, |s, cx| {
                                s.quick_switcher_move_selection(1);
                                cx.notify();
                            });
                        }
                        "up" | "arrowup" | "tab" if ev.keystroke.modifiers.shift => {
                            app_entity_keys.update(cx, |s, cx| {
                                s.quick_switcher_move_selection(-1);
                                cx.notify();
                            });
                        }
                        "enter" => {
                            app_entity_keys.update(cx, |s, cx| {
                                s.confirm_quick_switcher();
                                cx.notify();
                            });
                        }
                        _ => {}
                    }
                }
            })
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
                    .children(render_reconnect_banner(state))
                    // Center Content (Voice Panel if in voice channel or active call, Friends View if in Home with no channel selected, else Chat area)
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(if show_voice_stage {
                                self.spotify_timer = None;
                                self.voice_panel.clone().into_any_element()
                            } else if state.selected_guild_id.is_none() && state.selected_channel_id.is_none() {
                                super::friends_hub::sync_spotify_progress_timer(
                                    &mut self.spotify_timer,
                                    state,
                                    &app_entity,
                                    cx,
                                );
                                let search_state = self
                                    .friends_search_state
                                    .as_ref()
                                    .expect("friends_search_state must be initialized");
                                let add_friend_state = self
                                    .add_friend_state
                                    .as_ref()
                                    .expect("add_friend_state must be initialized");
                                let friends_list_state = self
                                    .friends_list_state
                                    .as_ref()
                                    .expect("friends_list_state must be initialized");
                                render_friends_view(
                                    app_entity.clone(),
                                    state,
                                    search_state,
                                    &self.friends_search_query,
                                    add_friend_state,
                                    friends_list_state,
                                    window.viewport_size().width.as_f32(),
                                )
                                .into_any_element()
                            } else {
                                self.spotify_timer = None;
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
            .children(if quick_switcher_open_state {
                self.quick_switcher_state.as_ref().map(|input| {
                    render_quick_switcher_overlay(app_entity.clone(), self.state.read(cx), input)
                })
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

/// Row data shared by the "add to group" and "create group" friend pickers.
/// Carries the avatar fields straight from the live friend relationship so both
/// dialogs render the same `Avatar` (URL, status and decoration).
#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupDmFriendRow {
    id: String,
    username: String,
    display_name: String,
    avatar_url: Option<String>,
    avatar_decoration_data: Option<crate::domain::user::AvatarDecorationData>,
    presence: crate::app::state::PresenceStatus,
    show_status: bool,
}

impl GroupDmFriendRow {
    fn from_friend(friend: &FriendItemState) -> Self {
        Self {
            id: friend.id.clone(),
            username: friend.username.clone(),
            display_name: friend.display_name.clone(),
            avatar_url: friend.avatar_url.clone(),
            avatar_decoration_data: friend.avatar_decoration_data.clone(),
            presence: friend.presence,
            show_status: friend.relationship_type == 1,
        }
    }
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
            friend,
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
    friend: &FriendItemState,
    is_already_member: bool,
) -> AnyElement {
    let row = GroupDmFriendRow::from_friend(friend);
    let entity_add = entity;
    let cid = channel_id;
    let fid = row.id.clone();
    let friend_id = fid.clone();
    let friend_name = row.username.clone();

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
                .child(
                    Avatar::new(row.display_name.clone())
                        .opt_src(row.avatar_url.clone())
                        .size(AvatarSize::Px32)
                        .opt_status(row.show_status.then(|| row.presence.into()))
                        .opt_decoration(row.avatar_decoration_data.clone())
                        .bg_color(blurple())
                        .cutout_color(bg_channels()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_color(text_titles())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child(row.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_color(legacy_949ba4())
                                .text_size(px(11.0))
                                .child(format!("@{}", row.username)),
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
            friend,
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
    friend: &FriendItemState,
) -> AnyElement {
    let row = GroupDmFriendRow::from_friend(friend);
    let entity_create = entity;
    let fid = row.id.clone();
    let friend_id = fid.clone();
    let friend_name = row.username.clone();

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
                .child(
                    Avatar::new(row.display_name.clone())
                        .opt_src(row.avatar_url.clone())
                        .size(AvatarSize::Px32)
                        .opt_status(row.show_status.then(|| row.presence.into()))
                        .opt_decoration(row.avatar_decoration_data.clone())
                        .bg_color(blurple())
                        .cutout_color(bg_channels()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_color(text_titles())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child(row.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_color(legacy_949ba4())
                                .text_size(px(11.0))
                                .child(format!("@{}", row.username)),
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

fn build_edit_guild_nickname_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    guild_id: String,
    nick_state: &Entity<InputState>,
    vh: Pixels,
    _cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let entity_save = entity.clone();
    let entity_cancel = entity.clone();
    let gid = guild_id.clone();
    let state = entity.read(_cx);
    let guild_name = state
        .guilds
        .iter()
        .find(|g| g.id == guild_id)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "Servidor".to_string());
    let pending = state.pending_guild_action.as_deref() == Some("nickname");
    let margin_top = (vh - px(220.0)).max(px(20.0)) / 2.0;

    dialog
        .w(px(440.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.pending_guild_action = None;
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child("Alterar apelido"),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(13.0))
                        .child(format!("Defina seu apelido em {}", guild_name)),
                )
                .child(
                    TextInput::new("guild-nickname-input", nick_state)
                        .placeholder("Apelido (vazio remove)")
                        .show_clear(true),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(space_2())
                        .child(
                            Button::gray("guild-nick-cancel", "Cancelar")
                                .size(ButtonSize::Md)
                                .on_click(move |_ev, window, cx| {
                                    window.close_dialog(cx);
                                    entity_cancel.update(cx, |s, cx| {
                                        s.pending_guild_action = None;
                                        s.close_modal();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child({
                            let mut btn = Button::blurple("guild-nick-save", if pending { "Salvando..." } else { "Salvar" })
                                .size(ButtonSize::Md);
                            if !pending {
                                btn = btn.on_click(move |_ev, _window, cx| {
                                    entity_save.update(cx, |s, cx| {
                                        let nick = s.guild_nickname_draft.clone();
                                        s.submit_guild_nickname(gid.clone(), nick);
                                        cx.notify();
                                    });
                                });
                            }
                            btn
                        }),
                ),
        )
}

fn build_leave_guild_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    guild_id: String,
    vh: Pixels,
    _cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let entity_leave = entity.clone();
    let entity_cancel = entity.clone();
    let gid = guild_id.clone();
    let state = entity.read(_cx);
    let guild_name = state
        .guilds
        .iter()
        .find(|g| g.id == guild_id)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "Servidor".to_string());
    let pending = state.pending_guild_action.as_deref() == Some("leave");
    let margin_top = (vh - px(180.0)).max(px(20.0)) / 2.0;

    dialog
        .w(px(440.0))
        .margin_top(margin_top)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.pending_guild_action = None;
                s.close_modal();
                cx.notify();
            });
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(space_4())
                .child(
                    div()
                        .text_color(text_titles())
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child(format!("Sair de '{}'", guild_name)),
                )
                .child(
                    div()
                        .text_color(text_muted())
                        .text_size(px(13.0))
                        .child(format!(
                            "Tem certeza que deseja sair de {}? Você não poderá voltar a menos que seja convidado novamente.",
                            guild_name
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(space_2())
                        .child(
                            Button::gray("guild-leave-cancel", "Cancelar")
                                .size(ButtonSize::Md)
                                .on_click(move |_ev, window, cx| {
                                    window.close_dialog(cx);
                                    entity_cancel.update(cx, |s, cx| {
                                        s.pending_guild_action = None;
                                        s.close_modal();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child({
                            let mut btn = Button::danger("guild-leave-confirm", if pending { "Saindo..." } else { "Sair do servidor" })
                                .size(ButtonSize::Md);
                            if !pending {
                                btn = btn.on_click(move |_ev, _window, cx| {
                                    entity_leave.update(cx, |s, cx| {
                                        s.confirm_leave_guild(gid.clone());
                                        cx.notify();
                                    });
                                });
                            }
                            btn
                        }),
                ),
        )
}

/// Renders the Friends list view ("Amigos") when on Home without an active DM channel.
/// Delegates to `src/app/friends_hub.rs` which implements full visual parity (Tabs, Active Now 360px, Message Requests).
fn render_friends_view(
    entity: Entity<AppState>,
    state: &AppState,
    search_state: &Entity<InputState>,
    search_query: &str,
    add_friend_state: &Entity<InputState>,
    list_state: &ListState,
    viewport_width: f32,
) -> Div {
    super::friends_hub::render_friends_hub_for_width(
        entity,
        state,
        search_state,
        search_query,
        add_friend_state,
        list_state,
        viewport_width,
    )
}

fn render_reconnect_banner(state: &AppState) -> Option<AnyElement> {
    if state.is_gateway_reconnecting {
        let attempt = state.reconnect_attempt.unwrap_or(1);
        let detail = state
            .reconnect_delay_ms
            .map(|ms| format!("Reconectando... tentativa {attempt} em {ms}ms"))
            .unwrap_or_else(|| format!("Reconectando... tentativa {attempt}"));
        return Some(
            div()
                .id("gateway-reconnect-banner")
                .w_full()
                .h(px(36.0))
                .min_h(px(36.0))
                .px(px(28.0))
                .pl(px(32.0))
                .flex()
                .items_center()
                .bg(status_ausente())
                .text_color(crate::tokens::colors::white())
                .text_size(px(14.0))
                .font_weight(FontWeight(500.0))
                .child(detail)
                .into_any_element(),
        );
    }

    let reason = state.reconnect_reason.as_deref().or(state.auth_error.as_deref());
    if let Some(reason) = reason {
        let msg = if reason.to_lowercase().contains("autent") {
            "Conexão perdida. Faça login novamente.".to_string()
        } else {
            format!("Sem conexão com o Gateway: {reason}")
        };
        return Some(
            div()
                .id("gateway-offline-banner")
                .w_full()
                .h(px(36.0))
                .min_h(px(36.0))
                .px(px(28.0))
                .pl(px(32.0))
                .flex()
                .items_center()
                .bg(status_dnd())
                .text_color(crate::tokens::colors::white())
                .text_size(px(14.0))
                .font_weight(FontWeight(500.0))
                .child(msg)
                .into_any_element(),
        );
    }

    None
}



#[cfg(test)]
mod group_dm_avatar_tests {
    use super::*;
    use crate::app::state::{FriendItemState, PresenceStatus as AppPresenceStatus};
    use crate::domain::user::AvatarDecorationData;

    fn friend(relationship_type: u32) -> FriendItemState {
        FriendItemState {
            id: "synth-group-user-1".to_string(),
            username: "synth_group_user".to_string(),
            display_name: "Synth Group User".to_string(),
            avatar_url: Some("https://cdn.discordapp.com/avatars/synth-group-user-1/synth_hash.png".to_string()),
            avatar_text: "S".to_string(),
            presence: AppPresenceStatus::Idle,
            custom_status: Some("synthetic status".to_string()),
            relationship_type,
            avatar_decoration_data: Some(AvatarDecorationData::with_sku(
                "a_synth_group_deco",
                "synth-sku-group",
            )),
        }
    }

    #[::core::prelude::v1::test]
    fn group_dm_picker_rows_propagate_avatar_url_status_and_decoration() {
        let friend = friend(1);
        let row = GroupDmFriendRow::from_friend(&friend);

        assert_eq!(row.id, friend.id);
        assert_eq!(row.username, friend.username);
        assert_eq!(row.display_name, friend.display_name);
        assert_eq!(row.avatar_url, friend.avatar_url);
        assert_eq!(
            row.avatar_decoration_data.as_ref().map(|d| d.asset.as_str()),
            Some("a_synth_group_deco")
        );
        assert_eq!(row.presence, AppPresenceStatus::Idle);
        assert!(row.show_status, "accepted friends show presence");
    }

    #[::core::prelude::v1::test]
    fn group_dm_picker_hides_status_for_non_accepted_relationships() {
        let row = GroupDmFriendRow::from_friend(&friend(3));
        assert!(!row.show_status, "pending relationships must not show presence");
        assert!(row.avatar_decoration_data.is_some());
    }
}

fn render_quick_switcher_overlay(
    entity: Entity<AppState>,
    state: &AppState,
    input_state: &Entity<InputState>,
) -> impl IntoElement {
    let results = state.quick_switcher.results.clone();
    let selected = state.quick_switcher.selected_index;
    let query_empty = state.quick_switcher.query.trim().is_empty();

    let mut list = div().flex().flex_col().gap(px(2.0)).w_full();
    if results.is_empty() {
        list = list.child(
            div()
                .px(px(12.0))
                .py(px(10.0))
                .text_size(px(14.0))
                .text_color(text_secondary())
                .child(if query_empty { "No recent channels" } else { "No results found" }),
        );
    } else {
        for (idx, item) in results.iter().enumerate().take(20) {
            let is_selected = idx == selected;
            let ent = entity.clone();
            let row_idx = idx;
            let bg = if is_selected { rgba(0x5865F233) } else { rgba(0x00000000) };
            let hover_bg = rgba(0x5865F21A);
            let label = item.label.clone();
            let detail = item.detail.clone().unwrap_or_default();
            let icon = match item.kind {
                crate::app::state::QuickSwitcherKind::Dm => icon_user(px(16.0), text_secondary()),
                crate::app::state::QuickSwitcherKind::Channel => icon_hash(px(16.0), text_secondary()),
                crate::app::state::QuickSwitcherKind::Guild => icon_compass(px(16.0), text_secondary()),
            };
            list = list.child(
                div()
                    .id(("quick-switcher-row", idx))
                    .w_full()
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(4.0))
                    .bg(bg)
                    .hover(move |s| s.bg(hover_bg))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                        ent.update(cx, |s, cx| {
                            s.quick_switcher_select_index(row_idx);
                            s.confirm_quick_switcher();
                            cx.notify();
                        });
                    })
                    .child(icon)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(text_primary())
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(text_secondary())
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(detail),
                            ),
                    ),
            );
        }
    }

    let entity_backdrop = entity.clone();
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_start()
        .justify_center()
        .pt(px(80.0))
        .bg(rgba(0x00000066))
        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
            entity_backdrop.update(cx, |s, cx| {
                s.close_quick_switcher();
                cx.notify();
            });
        })
        .child(
            div()
                .w(px(570.0))
                .max_w(px(570.0))
                .rounded(px(8.0))
                .bg(bg_floating())
                .border_1()
                .border_color(border_subtle())
                .p(px(20.0))
                .pt(px(12.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .on_mouse_down(MouseButton::Left, |_ev, _window, _cx| {
                    // prevent backdrop close when clicking inside
                })
                .child(
                    div()
                        .w_full()
                        .child(TextInput::new("quick-switcher-input", input_state))
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text_secondary())
                        .child("Search for servers, channels, or DMs"),
                )
                .child(list),
        )
}
