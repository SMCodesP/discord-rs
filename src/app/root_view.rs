use gpui::*;
use super::channel_sidebar::ChannelSidebar;
use super::guild_rail::GuildRail;
use super::header_bar::HeaderBar;
use super::member_sidebar::MemberSidebar;
use super::state::{ActiveModal, AppState, ChannelKind};
use super::user_settings::UserSettingsModal;
use super::voice_call_panel::VoiceCallPanel;
use crate::chat::ChatView;
use crate::components::banner::Banner;
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
    login_focus_handle: FocusHandle,
}

impl RootView {
    pub fn new(telemetry: crate::telemetry::TelemetryHandle, cx: &mut Context<Self>) -> Self {
        let (client_manager, mut event_rx) = crate::protocol::ClientManager::new_mock();
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
        let login_focus_handle = cx.focus_handle();

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
            login_focus_handle,
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
        let login_focus_handle = cx.focus_handle();

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
            login_focus_handle,
        }
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        // =========================================================================
        // MANDATORY INITIAL LOGIN SCREEN:
        // If not connected to the live Discord Gateway, show the full-screen login.
        // No chat background or sidebar is leaked.
        // =========================================================================
        if !state.is_connected_to_gateway {
            if !self.login_focus_handle.is_focused(window) {
                window.focus(&self.login_focus_handle);
            }
            return render_login_screen(self.state.clone(), state, &self.login_focus_handle).into_any_element();
        }

        // =========================================================================
        // AUTHENTICATED APP SHELL:
        // Transition seamlessly to the full native Discord layout once connected.
        // =========================================================================
        let show_members = state.show_members && state.selected_guild_id.is_some();
        let active_modal = state.active_modal.clone();
        let selected_channel = state.get_selected_channel().map(|(_, ch)| ch.clone());
        let in_voice_channel = selected_channel
            .as_ref()
            .map(|ch| ch.kind == ChannelKind::Voice)
            .unwrap_or(false);
        let app_entity = self.state.clone();
        let login_focus = self.login_focus_handle.clone();

        div()
            .relative()
            .size_full()
            .bg(rgb(0x313338))
            .text_color(rgb(0xdbdee1))
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
                    .bg(rgb(0x313338))
                    .overflow_hidden()
                    // Top Header Bar (48px)
                    .child(self.header_bar.clone())
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
                                render_friends_view(app_entity.clone(), state).into_any_element()
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
            // MODAL OVERLAY LAYER (Settings, Add Server, Explore, Manage Connection)
            // =========================================================================
            .children(if let Some(modal) = active_modal {
                let entity = app_entity.clone();
                let lf = login_focus.clone();
                Some(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .child(match modal {
                            ActiveModal::UserSettings => self.user_settings.clone().into_any_element(),
                            ActiveModal::AddServer => render_add_server_modal(entity).into_any_element(),
                            ActiveModal::ExploreServers => render_explore_modal(entity).into_any_element(),
                            ActiveModal::Login | ActiveModal::LoginDiscord => {
                                render_login_modal(entity, &state, &lf).into_any_element()
                            }
                            ActiveModal::AddFriendToGroup(channel_id) => {
                                render_add_friend_to_group_modal(entity, &state, channel_id).into_any_element()
                            }
                            ActiveModal::CreateGroupDm => {
                                render_create_group_dm_modal(entity, &state).into_any_element()
                            }
                        }),
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
                            &catalog,
                            presence.as_ref(),
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
    focus_handle: &FocusHandle,
) -> impl IntoElement {
    let entity_connect = entity.clone();
    let entity_bot_toggle = entity.clone();
    let entity_user_toggle = entity.clone();
    let entity_input = entity.clone();

    let is_user = state.is_user_token;
    let token_val = state.token_input.clone();
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

    let text_input = TextInput::new("discord-token-input")
        .label("TOKEN DE AUTENTICAÇÃO DO DISCORD")
        .placeholder(if is_user {
            "Cole seu token de usuário aqui..."
        } else {
            "Cole o token do Bot (ex: MTI0...)"
        })
        .value(token_val)
        .secret(true)
        .full_width(true)
        .track_focus(focus_handle)
        .disabled(is_connecting)
        .on_change(move |new_val, _window, cx| {
            entity_input.update(cx, |s, cx| {
                s.token_input = new_val;
                cx.notify();
            });
        });

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
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_center()
                                .gap(px(6.0))
                                .text_color(text_muted())
                                .text_size(px(12.0))
                                .child(icon_lock(px(14.0), text_muted()))
                                .child("Armazenamento seguro de token com permissões restritas (0600) em ~/.config/discord-rs/auth.json"),
                        ),
                ),
        )
}

/// Renders the "Conectar ao Discord" modal dialog (Bot / User Token) when opened inside the App Shell
fn render_login_modal(
    entity: Entity<AppState>,
    state: &AppState,
    focus_handle: &FocusHandle,
) -> impl IntoElement {
    let entity_close = entity.clone();
    let entity_connect = entity.clone();
    let entity_bot_toggle = entity.clone();
    let entity_user_toggle = entity.clone();
    let entity_input = entity.clone();

    let is_user = state.is_user_token;
    let token_val = state.token_input.clone();
    let is_connecting = state.is_connecting;

    // Top status banner
    let banner = if let Some(ref err) = state.auth_error {
        Banner::danger("login-err", err.clone())
            .title("Erro na Autenticação")
    } else if state.is_connected_to_gateway {
        Banner::success(
            "login-success",
            format!("Conectado como @{}", state.current_user.username),
        )
        .title("Conectado ao Discord")
    } else if is_connecting {
        Banner::info("login-connecting", "Conectando ao Gateway WebSocket v10 e REST API...")
            .title("Autenticando")
    } else {
        Banner::info(
            "login-info",
            "Insira um Bot Token do Discord Developer Portal ou Token de Usuário para sincronizar mensagens em tempo real.",
        )
        .title("Gerenciar Conexão do Discord")
    };

    let text_input = TextInput::new("discord-token-modal-input")
        .label("TOKEN DE AUTENTICAÇÃO DO DISCORD")
        .placeholder(if is_user {
            "Cole seu token de usuário aqui..."
        } else {
            "Cole o token do Bot (ex: MTI0...)"
        })
        .value(token_val)
        .secret(true)
        .full_width(true)
        .track_focus(focus_handle)
        .disabled(is_connecting)
        .on_change(move |new_val, _window, cx| {
            entity_input.update(cx, |s, cx| {
                s.token_input = new_val;
                cx.notify();
            });
        });

    div()
        .size_full()
        .bg(rgba(0x000000bb))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(520.0))
                .p(px(28.0))
                .rounded(radius_md())
                .bg(bg_channels())
                .border_1()
                .border_color(borders())
                .flex()
                .flex_col()
                .gap(space_4())
                // Header
                .child(
                    div()
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
                        // Voltar button
                        .child(
                            div()
                                .w_full()
                                .h(px(36.0))
                                .rounded(radius_sm())
                                .text_color(text_muted())
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(font_size_sm())
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.text_color(text_titles()))
                                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                    entity_close.update(cx, |s, cx| {
                                        s.close_modal();
                                        cx.notify();
                                    });
                                })
                                .child("Voltar"),
                        ),
                ),
        )
}

/// Renders the "Criar um Servidor" modal overlay
fn render_add_server_modal(entity: Entity<AppState>) -> impl IntoElement {
    div()
        .size_full()
        .bg(rgba(0x000000bb))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(440.0))
                .p(px(24.0))
                .rounded(px(8.0))
                .bg(rgb(0x313338))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .items_center()
                .child(
                    div()
                        .text_color(rgb(0xf2f3f5))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(22.0))
                        .child("Criar seu servidor"),
                )
                .child(
                    div()
                        .text_color(rgb(0x80848e))
                        .text_size(px(13.0))
                        .child("Seu servidor é onde você e seus amigos se reúnem."),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(44.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x5865f2))
                        .text_color(rgb(0xffffff))
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x4752c4)))
                        .child("Criar o Meu Próprio"),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(40.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x3f4248))
                        .text_color(rgb(0xffffff))
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x4e5058)))
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            entity.update(cx, |state, cx| {
                                state.close_modal();
                                cx.notify();
                            });
                        })
                        .child("Voltar"),
                ),
        )
}

/// Renders the "Explorar Servidores Públicos" modal overlay
fn render_explore_modal(entity: Entity<AppState>) -> impl IntoElement {
    div()
        .size_full()
        .bg(rgba(0x000000bb))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(480.0))
                .p(px(24.0))
                .rounded(px(8.0))
                .bg(rgb(0x313338))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .items_center()
                .child(
                    div()
                        .text_color(rgb(0xf2f3f5))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(22.0))
                        .child("Descubra Comunidades"),
                )
                .child(
                    div()
                        .text_color(rgb(0x80848e))
                        .text_size(px(13.0))
                        .child("Encontre comunidades incríveis de Rust, GPUI, Gaming e Música."),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(40.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x5865f2))
                        .text_color(rgb(0xffffff))
                        .font_weight(FontWeight::SEMIBOLD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x4752c4)))
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            entity.update(cx, |state, cx| {
                                state.close_modal();
                                cx.notify();
                            });
                        })
                        .child("Fechar"),
                ),
        )
}

/// Renders modal to add friends to an existing DM channel / group DM
fn render_add_friend_to_group_modal(
    entity: Entity<AppState>,
    state: &AppState,
    channel_id: String,
) -> impl IntoElement {
    let entity_close = entity.clone();
    let current_dm = state.dm_channels.iter().find(|c| c.id == channel_id);
    let current_recipients = current_dm.map(|c| c.recipient_ids.clone()).unwrap_or_default();

    let mut friend_rows = Vec::new();
    for friend in &state.friends {
        let is_already_member = current_recipients.contains(&friend.id);
        let fid = friend.id.clone();
        let fname = friend.username.clone();
        let dname = friend.display_name.clone();
        let entity_add = entity.clone();
        let cid = channel_id.clone();

        friend_rows.push(
            div()
                .h(px(44.0))
                .px(px(8.0))
                .rounded(px(4.0))
                .bg(rgb(0x2b2d31))
                .hover(|s| s.bg(rgb(0x35373c)))
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
                            div()
                                .size(px(32.0))
                                .rounded_full()
                                .bg(rgb(0x5865f2))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_user(px(18.0), rgb(0xffffff))),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_color(rgb(0xf2f3f5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_size(px(14.0))
                                        .child(dname),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x949ba4))
                                        .text_size(px(11.0))
                                        .child(format!("@{}", fname)),
                                 ),
                        ),
                )
                .child(if is_already_member {
                    div()
                        .px(px(10.0))
                        .py(px(4.0))
                        .rounded(px(3.0))
                        .bg(rgb(0x3f4248))
                        .text_color(rgb(0x949ba4))
                        .text_size(px(12.0))
                        .child("Já adicionado")
                } else {
                    div()
                        .px(px(12.0))
                        .py(px(5.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x5865f2))
                        .hover(|s| s.bg(rgb(0x4752c4)))
                        .cursor_pointer()
                        .text_color(rgb(0xffffff))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(12.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            entity_add.update(cx, |s, cx| {
                                s.add_recipient_to_group(&cid, &fid, &fname);
                                cx.notify();
                            });
                        })
                        .child(icon_plus(px(12.0), rgb(0xffffff)))
                        .child("Adicionar")
                }),
        );
    }

    if friend_rows.is_empty() {
        friend_rows.push(
            div()
                .p(px(16.0))
                .text_center()
                .text_color(rgb(0x949ba4))
                .text_size(px(13.0))
                .child("Nenhum amigo disponível na sua lista para adicionar."),
        );
    }

    div()
        .size_full()
        .bg(rgba(0x000000bb))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(460.0))
                .p(px(24.0))
                .rounded(px(8.0))
                .bg(rgb(0x313338))
                .border_1()
                .border_color(rgb(0x1e1f22))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon_friends(px(20.0), rgb(0x5865f2)))
                                .child(
                                    div()
                                        .text_color(rgb(0xf2f3f5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(20.0))
                                        .child("Adicionar amigos à conversa"),
                                ),
                        )
                        .child(
                            div()
                                .size(px(24.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(rgb(0x949ba4))
                                .hover(|s| s.text_color(rgb(0xffffff)).bg(rgb(0x35373c)))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity_close.update(cx, |s, cx| {
                                        s.close_modal();
                                        cx.notify();
                                    });
                                })
                                .child(icon_close(px(16.0), rgb(0x949ba4))),
                        ),
                )
                .child(
                    div()
                        .text_color(rgb(0x949ba4))
                        .text_size(px(13.0))
                        .child("Selecione amigos para adicionar a este grupo de conversa direta."),
                )
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

/// Renders modal to create a brand new Group DM
fn render_create_group_dm_modal(
    entity: Entity<AppState>,
    state: &AppState,
) -> impl IntoElement {
    let entity_close = entity.clone();

    let mut friend_rows = Vec::new();
    for friend in &state.friends {
        let fid = friend.id.clone();
        let fname = friend.username.clone();
        let dname = friend.display_name.clone();
        let entity_create = entity.clone();

        friend_rows.push(
            div()
                .h(px(44.0))
                .px(px(8.0))
                .rounded(px(4.0))
                .bg(rgb(0x2b2d31))
                .hover(|s| s.bg(rgb(0x35373c)))
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
                            div()
                                .size(px(32.0))
                                .rounded_full()
                                .bg(rgb(0x5865f2))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_user(px(18.0), rgb(0xffffff))),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_color(rgb(0xf2f3f5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_size(px(14.0))
                                        .child(dname),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x949ba4))
                                        .text_size(px(11.0))
                                        .child(format!("@{}", fname)),
                                ),
                        ),
                )
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(5.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x5865f2))
                        .hover(|s| s.bg(rgb(0x4752c4)))
                        .cursor_pointer()
                        .text_color(rgb(0xffffff))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(12.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            entity_create.update(cx, |s, cx| {
                                s.create_group_dm(vec![fid.clone()], vec![fname.clone()], None);
                                cx.notify();
                            });
                        })
                        .child(icon_plus(px(12.0), rgb(0xffffff)))
                        .child("Criar Grupo")
                ),
        );
    }

    if friend_rows.is_empty() {
        friend_rows.push(
            div()
                .p(px(16.0))
                .text_center()
                .text_color(rgb(0x949ba4))
                .text_size(px(13.0))
                .child("Nenhum amigo disponível na sua lista para criar grupo."),
        );
    }

    div()
        .size_full()
        .bg(rgba(0x000000bb))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(460.0))
                .p(px(24.0))
                .rounded(px(8.0))
                .bg(rgb(0x313338))
                .border_1()
                .border_color(rgb(0x1e1f22))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(icon_friends(px(20.0), rgb(0x5865f2)))
                                .child(
                                    div()
                                        .text_color(rgb(0xf2f3f5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(20.0))
                                        .child("Criar DM em Grupo"),
                                ),
                        )
                        .child(
                            div()
                                .size(px(24.0))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(rgb(0x949ba4))
                                .hover(|s| s.text_color(rgb(0xffffff)).bg(rgb(0x35373c)))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity_close.update(cx, |s, cx| {
                                        s.close_modal();
                                        cx.notify();
                                    });
                                })
                                .child(icon_close(px(16.0), rgb(0x949ba4))),
                        ),
                )
                .child(
                    div()
                        .text_color(rgb(0x949ba4))
                        .text_size(px(13.0))
                        .child("Selecione um amigo para iniciar um novo grupo de conversa."),
                )
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

/// Renders the Friends list view ("Amigos") when on Home without an active DM channel.
/// Delegates to `src/app/friends_hub.rs` which implements full visual parity (Tabs, Active Now 360px, Message Requests).
fn render_friends_view(
    entity: Entity<AppState>,
    state: &AppState,
) -> Div {
    super::friends_hub::render_friends_hub(entity, state)
}


