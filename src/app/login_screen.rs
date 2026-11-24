//! Official Discord-parity Login Screen:
//! - Left column: Email/Phone + Password (`POST /api/v9/auth/login`), 2FA TOTP
//!   (`POST /api/v9/auth/mfa/totp`), and collapsible Token / Bot fallback.
//! - Right column: Live Discord Remote Auth v2 QR Code
//!   (`wss://remote-auth-gateway.discord.gg/?v=2` with RSA-2048 OAEP-SHA256)
//!   rendered natively in GPUI.

use std::time::Duration;

use gpui::component::input::{InputEvent, InputState};
use gpui::prelude::*;
use gpui::*;

use crate::app::state::AppState;
use crate::components::{Banner, TextInput};
use crate::protocol::auth_login::{LoginOutcome, login_with_password, verify_mfa_totp};
use crate::protocol::remote_auth::{QrMatrix, RemoteAuthEvent, start_remote_auth_session};
use crate::tokens::icons::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoginFormMode {
    Credentials,
    Mfa { ticket: String },
    Token,
}

#[derive(Debug, Clone)]
enum QrUiState {
    Connecting,
    Ready {
        matrix: QrMatrix,
    },
    Scanned {
        username: String,
        avatar_url: Option<String>,
    },
    Expired,
    Cancelled,
    Error(String),
}

pub struct LoginScreen {
    app_state: Entity<AppState>,
    mode: LoginFormMode,
    email_input: Entity<InputState>,
    password_input: Entity<InputState>,
    mfa_input: Entity<InputState>,
    token_input: Entity<InputState>,
    qr_state: QrUiState,
    qr_handle: Option<tokio::task::JoinHandle<()>>,
    submitting: bool,
    local_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl Drop for LoginScreen {
    fn drop(&mut self) {
        if let Some(handle) = self.qr_handle.take() {
            handle.abort();
        }
    }
}

impl LoginScreen {
    pub fn new(
        app_state: Entity<AppState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let email_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("nome@exemplo.com ou telefone")
        });
        let password_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Sua senha do Discord")
                .masked(true)
        });
        let mfa_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("000000")
        });
        let token_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Cole seu token de usuário ou Bot aqui...")
                .masked(true)
        });

        let sub_email = cx.subscribe(&email_input, |this, _input, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit_credentials(cx);
            }
        });
        let sub_pass = cx.subscribe(&password_input, |this, _input, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit_credentials(cx);
            }
        });
        let sub_mfa = cx.subscribe(&mfa_input, |this, _input, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.submit_mfa(cx);
            }
        });
        let sub_token = cx.subscribe(&token_input, |this, input, ev: &InputEvent, cx| {
            match ev {
                InputEvent::Change => {
                    let val = input.read(cx).value().to_string();
                    this.app_state.update(cx, |s, _| s.token_input = val);
                }
                InputEvent::PressEnter { .. } => {
                    this.submit_token(cx);
                }
                _ => {}
            }
        });

        let mut screen = Self {
            app_state,
            mode: LoginFormMode::Credentials,
            email_input,
            password_input,
            mfa_input,
            token_input,
            qr_state: QrUiState::Connecting,
            qr_handle: None,
            submitting: false,
            local_error: None,
            _subscriptions: vec![sub_email, sub_pass, sub_mfa, sub_token],
        };

        screen.start_qr_session(cx);
        screen
    }

    pub fn start_qr_session(&mut self, cx: &mut Context<Self>) {
        if let Some(old) = self.qr_handle.take() {
            old.abort();
        }
        self.qr_state = QrUiState::Connecting;
        let (mut rx, handle) = start_remote_auth_session();
        self.qr_handle = Some(handle);
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;

                let mut drained = Vec::new();
                while let Ok(ev) = rx.try_recv() {
                    drained.push(ev);
                }

                let mut should_stop = false;
                if !drained.is_empty() {
                    let update_res = this.update(cx, |screen, cx| {
                        for ev in drained {
                            match ev {
                                RemoteAuthEvent::Connecting => {
                                    screen.qr_state = QrUiState::Connecting;
                                }
                                RemoteAuthEvent::QrReady { matrix, .. } => {
                                    screen.qr_state = QrUiState::Ready { matrix };
                                }
                                RemoteAuthEvent::Scanned {
                                    username,
                                    avatar_url,
                                    ..
                                } => {
                                    screen.qr_state = QrUiState::Scanned {
                                        username,
                                        avatar_url,
                                    };
                                }
                                RemoteAuthEvent::Authenticated { token } => {
                                    screen.local_error = None;
                                    screen.submitting = true;
                                    screen.app_state.update(cx, |state, cx| {
                                        state.is_user_token = true;
                                        state.token_input = token.clone();
                                        state.authenticate(token, true);
                                        cx.notify();
                                    });
                                    should_stop = true;
                                }
                                RemoteAuthEvent::Cancelled => {
                                    screen.qr_state = QrUiState::Cancelled;
                                    should_stop = true;
                                }
                                RemoteAuthEvent::Expired => {
                                    screen.qr_state = QrUiState::Expired;
                                    should_stop = true;
                                }
                                RemoteAuthEvent::Error(msg) => {
                                    screen.qr_state = QrUiState::Error(msg);
                                    should_stop = true;
                                }
                            }
                        }
                        cx.notify();
                    });
                    if update_res.is_err() || should_stop {
                        break;
                    }
                } else if this.upgrade().is_none() {
                    break;
                }
            }
        })
        .detach();
    }

    fn submit_credentials(&mut self, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        let login = self.email_input.read(cx).value().to_string();
        let password = self.password_input.read(cx).value().to_string();
        if login.trim().is_empty() || password.is_empty() {
            self.local_error = Some("Preencha seu e-mail/telefone e senha.".to_string());
            cx.notify();
            return;
        }

        self.submitting = true;
        self.local_error = None;
        cx.notify();

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let outcome = login_with_password(&login, &password).await;
            let _ = tx.send(outcome);
        });

        cx.spawn(async move |this, cx| {
            let outcome = loop {
                match rx.try_recv() {
                    Ok(v) => break v,
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                    Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                        break LoginOutcome::Error("Requisição de login interrompida.".to_string());
                    }
                }
            };

            let _ = this.update(cx, |screen, cx| {
                screen.handle_login_outcome(outcome, cx);
            });
        })
        .detach();
    }

    fn submit_mfa(&mut self, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        let ticket = match &self.mode {
            LoginFormMode::Mfa { ticket } => ticket.clone(),
            _ => return,
        };
        let code = self.mfa_input.read(cx).value().to_string();
        if code.trim().is_empty() {
            self.local_error = Some("Digite o código 2FA de 6 dígitos.".to_string());
            cx.notify();
            return;
        }

        self.submitting = true;
        self.local_error = None;
        cx.notify();

        let (tx, mut rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let outcome = verify_mfa_totp(&ticket, &code).await;
            let _ = tx.send(outcome);
        });

        cx.spawn(async move |this, cx| {
            let outcome = loop {
                match rx.try_recv() {
                    Ok(v) => break v,
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                    Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                        break LoginOutcome::Error("Verificação 2FA interrompida.".to_string());
                    }
                }
            };

            let _ = this.update(cx, |screen, cx| {
                screen.handle_login_outcome(outcome, cx);
            });
        })
        .detach();
    }

    fn submit_token(&mut self, cx: &mut Context<Self>) {
        let val = self.token_input.read(cx).value().to_string();
        self.local_error = None;
        self.app_state.update(cx, |state, cx| {
            let is_user = state.is_user_token;
            state.token_input = val.clone();
            state.authenticate(val, is_user);
            cx.notify();
        });
    }

    fn handle_login_outcome(&mut self, outcome: LoginOutcome, cx: &mut Context<Self>) {
        self.submitting = false;
        match outcome {
            LoginOutcome::Authenticated { token } => {
                self.local_error = None;
                self.app_state.update(cx, |state, cx| {
                    state.is_user_token = true;
                    state.token_input = token.clone();
                    state.authenticate(token, true);
                    cx.notify();
                });
            }
            LoginOutcome::MfaRequired { ticket, .. } => {
                self.local_error = None;
                self.mode = LoginFormMode::Mfa { ticket };
            }
            LoginOutcome::CaptchaRequired => {
                self.local_error = Some(
                    "O Discord solicitou verificação CAPTCHA para login por senha neste dispositivo. Escaneie o Código QR ao lado com o app do Discord no celular para entrar na hora sem CAPTCHA."
                        .to_string(),
                );
            }
            LoginOutcome::Error(msg) => {
                self.local_error = Some(msg);
            }
        }
        cx.notify();
    }
}

impl Render for LoginScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let is_connecting = self.submitting || state.is_connecting;
        let active_error = self
            .local_error
            .clone()
            .or_else(|| state.auth_error.clone());
        let is_user_token = state.is_user_token;

        let left_col = match &self.mode {
            LoginFormMode::Credentials => {
                render_credentials_column(cx, &self.email_input, &self.password_input, is_connecting, active_error.as_deref())
            }
            LoginFormMode::Mfa { .. } => {
                render_mfa_column(cx, &self.mfa_input, is_connecting, active_error.as_deref())
            }
            LoginFormMode::Token => {
                render_token_column(
                    cx,
                    self.app_state.clone(),
                    &self.token_input,
                    is_user_token,
                    is_connecting,
                    active_error.as_deref(),
                )
            }
        };

        let right_col = render_qr_column(cx, &self.qr_state);

        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(rgb(0x16194f))
            // Cosmic Discord-style layered glow background
            .child(render_cosmic_backdrop())
            // Top-left official Discord wordmark
            .child(render_top_left_brand())
            // Centered 2-column official Discord login card
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p(px(24.0))
                    .child(
                        div()
                            .w(px(784.0))
                            .max_w_full()
                            .p(px(32.0))
                            .rounded(px(8.0))
                            .bg(rgb(0x2b2d31))
                            .border_1()
                            .border_color(rgba(0xffffff12))
                            .shadow_lg()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(32.0))
                            .child(left_col)
                            .child(
                                div()
                                    .w(px(1.0))
                                    .h(px(300.0))
                                    .bg(rgba(0xffffff10)),
                            )
                            .child(right_col),
                    ),
            )
    }
}

fn render_cosmic_backdrop() -> AnyElement {
    div()
        .absolute()
        .inset_0()
        // Upper-right bright blurple nebula glow
        .child(
            div()
                .absolute()
                .top(px(-180.0))
                .right(px(-140.0))
                .size(px(680.0))
                .rounded_full()
                .bg(rgba(0x404eed55)),
        )
        // Lower-left deep indigo glow
        .child(
            div()
                .absolute()
                .bottom(px(-220.0))
                .left(px(-160.0))
                .size(px(700.0))
                .rounded_full()
                .bg(rgba(0x2d248a66)),
        )
        // Center-bottom vibrant blue accent
        .child(
            div()
                .absolute()
                .bottom(px(-140.0))
                .right(px(80.0))
                .size(px(520.0))
                .rounded_full()
                .bg(rgba(0x3b48f544)),
        )
        // Subtle decorative stars & pluses matching the Discord cosmic background
        .child(div().absolute().top(px(84.0)).left(px(320.0)).size(px(4.0)).rounded_full().bg(rgba(0xffffff44)))
        .child(div().absolute().top(px(140.0)).right(px(260.0)).size(px(5.0)).rounded_full().bg(rgba(0xffffff55)))
        .child(div().absolute().bottom(px(120.0)).left(px(210.0)).size(px(4.0)).rounded_full().bg(rgba(0xffffff44)))
        .child(div().absolute().bottom(px(190.0)).right(px(190.0)).size(px(5.0)).rounded_full().bg(rgba(0xffffff44)))
        .child(
            div()
                .absolute()
                .bottom(px(110.0))
                .left(px(380.0))
                .text_size(px(22.0))
                .text_color(rgba(0x8ea1ff44))
                .child("+"),
        )
        .child(
            div()
                .absolute()
                .top(px(96.0))
                .right(px(380.0))
                .text_size(px(18.0))
                .text_color(rgba(0x8ea1ff38))
                .child("+"),
        )
        .into_any_element()
}

fn render_top_left_brand() -> AnyElement {
    div()
        .absolute()
        .top(px(28.0))
        .left(px(36.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(30.0))
                .rounded(px(8.0))
                .bg(rgb(0x5865f2))
                .flex()
                .items_center()
                .justify_center()
                .child(icon_gamepad(px(18.0), rgb(0xffffff))),
        )
        .child(
            div()
                .text_color(rgb(0xffffff))
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_size(px(20.0))
                .child("Discord"),
        )
        .into_any_element()
}

fn render_status_banner(active_error: Option<&str>, is_connecting: bool) -> Option<AnyElement> {
    if let Some(err) = active_error {
        Some(
            Banner::danger("login-err", err.to_string())
                .title("Erro na Autenticação")
                .into_any_element(),
        )
    } else if is_connecting {
        Some(
            Banner::info("login-connecting", "Autenticando e conectando ao Discord...")
                .title("Conectando")
                .into_any_element(),
        )
    } else {
        None
    }
}

fn render_field_label(label: &str) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xb5bac1))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xf23f42))
                .child("*"),
        )
        .into_any_element()
}

fn render_credentials_column(
    cx: &mut Context<LoginScreen>,
    email_input: &Entity<InputState>,
    password_input: &Entity<InputState>,
    is_connecting: bool,
    active_error: Option<&str>,
) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(16.0))
        // Title & Subtitle
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .gap(px(6.0))
                .mb(px(4.0))
                .child(
                    div()
                        .text_size(px(24.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf2f3f5))
                        .child("Boas-vindas de volta!"),
                )
                .child(
                    div()
                        .text_size(px(15.0))
                        .text_color(rgb(0xb5bac1))
                        .child("Estamos muito animados em te ver novamente!"),
                ),
        )
        .children(render_status_banner(active_error, is_connecting))
        // Email field
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_field_label("E-MAIL OU NÚMERO DE TELEFONE"))
                .child(
                    TextInput::new("discord-login-email", email_input)
                        .placeholder("nome@exemplo.com")
                        .full_width(true)
                        .disabled(is_connecting),
                ),
        )
        // Password field
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_field_label("SENHA"))
                .child(
                    TextInput::new("discord-login-password", password_input)
                        .placeholder("••••••••")
                        .secret(true)
                        .full_width(true)
                        .disabled(is_connecting),
                ),
        )
        // Primary Blurple Submit Button
        .child(
            div()
                .id("login-submit-credentials-btn")
                .w_full()
                .h(px(44.0))
                .mt(px(4.0))
                .rounded(px(4.0))
                .bg(if is_connecting {
                    rgb(0x4752c4)
                } else {
                    rgb(0x5865f2)
                })
                .hover(|s| s.bg(rgb(0x4752c4)))
                .cursor(if is_connecting {
                    CursorStyle::OperationNotAllowed
                } else {
                    CursorStyle::PointingHand
                })
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xffffff))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(15.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev, _window, cx| {
                        this.submit_credentials(cx);
                    }),
                )
                .child(if is_connecting {
                    "Entrando..."
                } else {
                    "Entrar"
                }),
        )
        // Footer toggle to Token / Bot mode
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .text_size(px(13.0))
                .child(
                    div()
                        .text_color(rgb(0x949ba4))
                        .child("Prefere usar Token ou Bot?"),
                )
                .child(
                    div()
                        .id("switch-to-token-login")
                        .text_color(rgb(0x00a8fc))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _ev, _window, cx| {
                                this.mode = LoginFormMode::Token;
                                this.local_error = None;
                                cx.notify();
                            }),
                        )
                        .child("Entrar com Token"),
                ),
        )
        .into_any_element()
}

fn render_mfa_column(
    cx: &mut Context<LoginScreen>,
    mfa_input: &Entity<InputState>,
    is_connecting: bool,
    active_error: Option<&str>,
) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf2f3f5))
                        .child("Autenticação em dois fatores"),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(0xb5bac1))
                        .child("Digite o código de 6 dígitos do seu app autenticador."),
                ),
        )
        .children(render_status_banner(active_error, is_connecting))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_field_label("CÓDIGO DE AUTENTICAÇÃO (2FA)"))
                .child(
                    TextInput::new("discord-login-mfa", mfa_input)
                        .placeholder("000000")
                        .full_width(true)
                        .disabled(is_connecting),
                ),
        )
        .child(
            div()
                .id("login-submit-mfa-btn")
                .w_full()
                .h(px(44.0))
                .rounded(px(4.0))
                .bg(rgb(0x5865f2))
                .hover(|s| s.bg(rgb(0x4752c4)))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xffffff))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(15.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev, _window, cx| {
                        this.submit_mfa(cx);
                    }),
                )
                .child(if is_connecting {
                    "Verificando..."
                } else {
                    "Verificar e Entrar"
                }),
        )
        .child(
            div()
                .id("mfa-back-btn")
                .text_size(px(13.0))
                .text_color(rgb(0x00a8fc))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev, _window, cx| {
                        this.mode = LoginFormMode::Credentials;
                        this.local_error = None;
                        cx.notify();
                    }),
                )
                .child("← Voltar para E-mail e Senha"),
        )
        .into_any_element()
}

fn render_token_column(
    cx: &mut Context<LoginScreen>,
    app_state: Entity<AppState>,
    token_input: &Entity<InputState>,
    is_user: bool,
    is_connecting: bool,
    active_error: Option<&str>,
) -> AnyElement {
    let app_bot = app_state.clone();
    let app_user = app_state;

    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf2f3f5))
                        .child("Entrar com Token"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0xb5bac1))
                        .child("Para Bot Token ou acesso avançado. Para contas pessoais, prefira o Código QR."),
                ),
        )
        .children(render_status_banner(active_error, is_connecting))
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .child(
                    div()
                        .id("token-mode-bot")
                        .flex_1()
                        .h(px(36.0))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(if !is_user {
                            rgb(0x5865f2)
                        } else {
                            rgba(0xffffff18)
                        })
                        .bg(if !is_user {
                            rgba(0x5865f226)
                        } else {
                            rgb(0x1e1f22)
                        })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.0))
                        .text_size(px(13.0))
                        .text_color(if !is_user {
                            rgb(0xffffff)
                        } else {
                            rgb(0xb5bac1)
                        })
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            app_bot.update(cx, |s, cx| {
                                s.is_user_token = false;
                                cx.notify();
                            });
                        })
                        .child(icon_bot(px(15.0), rgb(0xb5bac1)))
                        .child("Bot Token"),
                )
                .child(
                    div()
                        .id("token-mode-user")
                        .flex_1()
                        .h(px(36.0))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(if is_user {
                            rgb(0x5865f2)
                        } else {
                            rgba(0xffffff18)
                        })
                        .bg(if is_user {
                            rgba(0x5865f226)
                        } else {
                            rgb(0x1e1f22)
                        })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.0))
                        .text_size(px(13.0))
                        .text_color(if is_user {
                            rgb(0xffffff)
                        } else {
                            rgb(0xb5bac1)
                        })
                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                            app_user.update(cx, |s, cx| {
                                s.is_user_token = true;
                                cx.notify();
                            });
                        })
                        .child(icon_user(px(15.0), rgb(0xb5bac1)))
                        .child("Conta de Usuário"),
                ),
        )
        .child(
            TextInput::new("discord-token-input", token_input)
                .label("TOKEN DE AUTENTICAÇÃO")
                .placeholder(if is_user {
                    "Cole seu token de usuário aqui..."
                } else {
                    "Cole o token do Bot (ex: MTI0...)"
                })
                .secret(true)
                .full_width(true)
                .disabled(is_connecting),
        )
        .child(
            div()
                .id("login-submit-token-btn")
                .w_full()
                .h(px(44.0))
                .rounded(px(4.0))
                .bg(rgb(0x5865f2))
                .hover(|s| s.bg(rgb(0x4752c4)))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0xffffff))
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(15.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev, _window, cx| {
                        this.submit_token(cx);
                    }),
                )
                .child(if is_connecting {
                    "Conectando..."
                } else {
                    "Conectar ao Discord"
                }),
        )
        .child(
            div()
                .id("token-back-btn")
                .text_size(px(13.0))
                .text_color(rgb(0x00a8fc))
                .cursor_pointer()
                .hover(|s| s.underline())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _ev, _window, cx| {
                        this.mode = LoginFormMode::Credentials;
                        this.local_error = None;
                        cx.notify();
                    }),
                )
                .child("← Voltar para E-mail e Senha"),
        )
        .into_any_element()
}

fn render_qr_column(cx: &mut Context<LoginScreen>, qr_state: &QrUiState) -> AnyElement {
    let qr_visual: AnyElement = match qr_state {
        QrUiState::Connecting => div()
            .size(px(176.0))
            .rounded(px(8.0))
            .bg(rgb(0x1e1f22))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(icon_gamepad(px(32.0), rgb(0x5865f2)))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0xb5bac1))
                    .child("Gerando QR Code..."),
            )
            .into_any_element(),
        QrUiState::Ready { matrix } => render_qr_matrix(matrix),
        QrUiState::Scanned {
            username,
            avatar_url,
        } => {
            let avatar_el: AnyElement = if let Some(url) = avatar_url {
                img(url.clone())
                    .size(px(80.0))
                    .rounded_full()
                    .into_any_element()
            } else {
                div()
                    .size(px(80.0))
                    .rounded_full()
                    .bg(rgb(0x5865f2))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(28.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(
                        username
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string(),
                    )
                    .into_any_element()
            };
            div()
                .size(px(176.0))
                .rounded(px(8.0))
                .bg(rgb(0x1e1f22))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .child(avatar_el)
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0x23a55a))
                        .child(format!("@{}", username)),
                )
                .into_any_element()
        }
        QrUiState::Expired | QrUiState::Cancelled | QrUiState::Error(_) => {
            let msg = match qr_state {
                QrUiState::Expired => "Código QR expirado",
                QrUiState::Cancelled => "Login cancelado no celular",
                QrUiState::Error(_) => "Erro ao gerar QR",
                _ => "",
            };
            div()
                .size(px(176.0))
                .rounded(px(8.0))
                .bg(rgb(0x1e1f22))
                .p(px(12.0))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0xb5bac1))
                        .child(msg),
                )
                .child(
                    div()
                        .id("qr-retry-btn")
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded(px(4.0))
                        .bg(rgb(0x5865f2))
                        .hover(|s| s.bg(rgb(0x4752c4)))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _ev, _window, cx| {
                                this.start_qr_session(cx);
                            }),
                        )
                        .child("Gerar novo QR"),
                )
                .into_any_element()
        }
    };

    let (title, subtitle) = match qr_state {
        QrUiState::Scanned { .. } => (
            "Verifique seu celular!",
            "Confirme o login no app móvel do Discord para entrar imediatamente.",
        ),
        QrUiState::Error(err) => ("Entrar com Código QR", err.as_str()),
        _ => (
            "Entrar com Código QR",
            "Escaneie isto com o app móvel do Discord para fazer login imediatamente.",
        ),
    };

    div()
        .w(px(240.0))
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .gap(px(16.0))
        .child(qr_visual)
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf2f3f5))
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0xb5bac1))
                        .child(subtitle.to_string()),
                ),
        )
        .into_any_element()
}

fn render_qr_matrix(matrix: &QrMatrix) -> AnyElement {
    let n = matrix.width.max(1);
    let inner_px = 160.0_f32;
    let cell = (inner_px / n as f32).floor().max(2.0);
    let grid_px = cell * n as f32;

    let mut rows = Vec::with_capacity(n);
    for y in 0..n {
        let mut row = div().flex().flex_row().h(px(cell));
        for x in 0..n {
            let dark = matrix.is_dark(x, y);
            row = row.child(
                div()
                    .w(px(cell))
                    .h(px(cell))
                    .bg(if dark { rgb(0x000000) } else { rgb(0xffffff) }),
            );
        }
        rows.push(row);
    }

    div()
        .relative()
        .size(px(176.0))
        .rounded(px(8.0))
        .bg(rgb(0xffffff))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(grid_px))
                .h(px(grid_px))
                .flex()
                .flex_col()
                .children(rows),
        )
        // Centered Discord badge overlay (QR Error Correction handles center occlusion cleanly)
        .child(
            div()
                .absolute()
                .size(px(36.0))
                .rounded_full()
                .bg(rgb(0xffffff))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(28.0))
                        .rounded_full()
                        .bg(rgb(0x111214))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_gamepad(px(16.0), rgb(0xffffff))),
                ),
        )
        .into_any_element()
}
