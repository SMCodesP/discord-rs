//! Discord RS - Friends Hub & Message Requests View
//!
//! Provides the primary Home view with 1:1 visual parity to official Discord Desktop:
//! - Navigation header tabs: Disponível, Todos, Pendente, Bloqueado, Solicitações, Adicionar amigo
//! - Message Requests view with "Você pode conhecer" and "Spam" sub-tabs
//! - Active Now (Ativo Agora) sidebar with exact 360px width (#2b2d31) and Rich Presence cards

use gpui::prelude::FluentBuilder;
use gpui::*;
use crate::tokens::*;
use crate::app::state::*;

/// Main entry point for the Friends Hub ("Amigos") view when on Home.
pub fn render_friends_hub(
    entity: Entity<AppState>,
    state: &AppState,
) -> Div {
    let mut container = div()
        .size_full()
        .bg(rgb(0x313338))
        .text_color(rgb(0xdbdee1))
        .flex()
        .flex_col()
        .overflow_hidden();

    // 1. Top Subheader / Tab Bar (48px fixed height)
    let header = render_header(entity.clone(), state);
    container = container.child(header);

    // 2. Main Body: Split between Left (Friends List or Message Requests) and Right (Active Now 360px)
    let left_column = if state.friends_tab == FriendsTab::MessageRequests {
        render_message_requests_view(entity.clone(), state)
    } else {
        render_friends_list_view(entity.clone(), state)
    };

    let active_now_panel = render_active_now_panel(entity.clone(), state);

    let main_body = div()
        .flex_1()
        .w_full()
        .flex()
        .flex_row()
        .overflow_hidden()
        .child(left_column)
        .child(active_now_panel);

    container.child(main_body)
}

/// Renders the top navigation header bar (48px)
fn render_header(
    entity: Entity<AppState>,
    state: &AppState,
) -> Div {
    let pending_req_count = state.message_requests.iter().filter(|r| !r.is_spam).count();

    div()
        .w_full()
        .h(px(48.0))
        .border_b_1()
        .border_color(rgb(0x1f2023))
        .px(px(24.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.0))
                // Left Icon + Section Label
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, {
                            let entity = entity.clone();
                            move |_ev, _window, cx| {
                                entity.update(cx, |s, cx| {
                                    s.friends_tab = FriendsTab::Online;
                                    cx.notify();
                                });
                            }
                        })
                        .child(
                            if state.friends_tab == FriendsTab::MessageRequests {
                                icon_inbox(px(20.0), rgb(0x80848e))
                            } else {
                                icon_friends(px(20.0), rgb(0x80848e))
                            },
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_size(font_size_md())
                                .text_color(rgb(0xffffff))
                                .child(if state.friends_tab == FriendsTab::MessageRequests { "Solicitações de mensagens" } else { "Amigos" }),
                        ),
                )
                // Separator
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(16.0))
                        .bg(rgb(0x3f4248)),
                )
                // Tabs
                .children(if state.friends_tab == FriendsTab::MessageRequests {
                    vec![
                        // Subtab: Você pode conhecer
                        {
                            let is_selected = state.message_requests_subtab == MessageRequestsSubTab::YouMayKnow;
                            let entity = entity.clone();
                            div()
                                .id("subtab-you-may-know")
                                .px(px(10.0))
                                .py(px(3.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .when(is_selected, |d| d.bg(rgb(0x3f4248)).text_color(rgb(0xffffff)))
                                .when(!is_selected, |d| d.text_color(rgb(0xdbdee1)).hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff))))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_message_requests_subtab(MessageRequestsSubTab::YouMayKnow);
                                        cx.notify();
                                    });
                                })
                                .child("Você pode conhecer")
                                .children(if pending_req_count > 0 {
                                    Some(
                                        div()
                                            .px(px(6.0))
                                            .py(px(1.0))
                                            .rounded_full()
                                            .bg(rgb(0x5865f2))
                                            .text_size(px(11.0))
                                            .text_color(rgb(0xffffff))
                                            .child(pending_req_count.to_string())
                                    )
                                } else {
                                    None
                                })
                        },
                        // Subtab: Spam
                        {
                            let is_selected = state.message_requests_subtab == MessageRequestsSubTab::Spam;
                            let spam_count = state.message_requests.iter().filter(|r| r.is_spam).count();
                            let entity = entity.clone();
                            div()
                                .id("subtab-spam")
                                .px(px(10.0))
                                .py(px(3.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .when(is_selected, |d| d.bg(rgb(0x3f4248)).text_color(rgb(0xffffff)))
                                .when(!is_selected, |d| d.text_color(rgb(0xdbdee1)).hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff))))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_message_requests_subtab(MessageRequestsSubTab::Spam);
                                        cx.notify();
                                    });
                                })
                                .child("Spam")
                                .children(if spam_count > 0 {
                                    Some(
                                        div()
                                            .px(px(6.0))
                                            .py(px(1.0))
                                            .rounded_full()
                                            .bg(rgb(0xda373c))
                                            .text_size(px(11.0))
                                            .text_color(rgb(0xffffff))
                                            .child(spam_count.to_string())
                                    )
                                } else {
                                    None
                                })
                        },
                        // Voltar para Amigos
                        {
                            let entity = entity.clone();
                            div()
                                .id("btn-back-to-friends")
                                .px(px(10.0))
                                .py(px(3.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .text_color(rgb(0x80848e))
                                .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff)))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.friends_tab = FriendsTab::Online;
                                        cx.notify();
                                    });
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.0))
                                        .child(icon_friends(px(16.0), rgb(0xdbdee1)))
                                        .child("Voltar para Amigos"),
                                )
                        },
                    ]
                } else {
                    vec![
                        // Disponível
                        {
                            let is_selected = state.friends_tab == FriendsTab::Online;
                            let entity = entity.clone();
                            div()
                                .id("tab-online")
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .when(is_selected, |d| d.bg(rgb(0x3f4248)).text_color(rgb(0xffffff)))
                                .when(!is_selected, |d| d.text_color(rgb(0xdbdee1)).hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff))))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_friends_tab(FriendsTab::Online);
                                        cx.notify();
                                    });
                                })
                                .child("Disponível")
                        },
                        // Todos
                        {
                            let is_selected = state.friends_tab == FriendsTab::All;
                            let entity = entity.clone();
                            div()
                                .id("tab-all")
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .when(is_selected, |d| d.bg(rgb(0x3f4248)).text_color(rgb(0xffffff)))
                                .when(!is_selected, |d| d.text_color(rgb(0xdbdee1)).hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff))))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_friends_tab(FriendsTab::All);
                                        cx.notify();
                                    });
                                })
                                .child("Todos")
                        },
                        // Pendente
                        {
                            let is_selected = state.friends_tab == FriendsTab::Pending;
                            let entity = entity.clone();
                            div()
                                .id("tab-pending")
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .when(is_selected, |d| d.bg(rgb(0x3f4248)).text_color(rgb(0xffffff)))
                                .when(!is_selected, |d| d.text_color(rgb(0xdbdee1)).hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff))))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_friends_tab(FriendsTab::Pending);
                                        cx.notify();
                                    });
                                })
                                .child("Pendente")
                        },
                        // Bloqueado
                        {
                            let is_selected = state.friends_tab == FriendsTab::Blocked;
                            let entity = entity.clone();
                            div()
                                .id("tab-blocked")
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .when(is_selected, |d| d.bg(rgb(0x3f4248)).text_color(rgb(0xffffff)))
                                .when(!is_selected, |d| d.text_color(rgb(0xdbdee1)).hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff))))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_friends_tab(FriendsTab::Blocked);
                                        cx.notify();
                                    });
                                })
                                .child("Bloqueado")
                        },
                        // Solicitações de mensagens (com badge)
                        {
                            let is_selected = state.friends_tab == FriendsTab::MessageRequests;
                            let entity = entity.clone();
                            div()
                                .id("tab-msg-requests")
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(radius_xs())
                                .cursor_pointer()
                                .text_color(rgb(0xdbdee1))
                                .hover(|s| s.bg(rgb(0x35373c)).text_color(rgb(0xffffff)))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_friends_tab(FriendsTab::MessageRequests);
                                        cx.notify();
                                    });
                                })
                                .child(icon_inbox(px(16.0), if is_selected { rgb(0xffffff) } else { rgb(0xb5bac1) }))
                                .child("Solicitações")
                                .children(if pending_req_count > 0 {
                                    Some(
                                        div()
                                            .px(px(6.0))
                                            .py(px(1.0))
                                            .rounded_full()
                                            .bg(rgb(0x5865f2))
                                            .text_size(px(11.0))
                                            .text_color(rgb(0xffffff))
                                            .child(pending_req_count.to_string())
                                    )
                                } else {
                                    None
                                })
                        },
                        // Adicionar amigo (Botão Verde Destacado #248046)
                        {
                            let entity = entity.clone();
                            div()
                                .id("btn-add-friend")
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(radius_xs())
                                .bg(rgb(0x248046))
                                .text_color(rgb(0xffffff))
                                .text_size(font_size_sm())
                                .font_weight(FontWeight::MEDIUM)
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x1a6334)))
                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                    entity.update(cx, |s, cx| {
                                        s.select_friends_tab(FriendsTab::AddFriend);
                                        cx.notify();
                                    });
                                })
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(icon_plus(px(14.0), rgb(0xffffff)))
                                .child("Adicionar amigo")
                        },
                    ]
                }),
        )
        // Right Action Icons (Inbox, Help)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.0))
                .child(
                    div()
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(0xffffff)))
                        .child(icon_inbox(px(20.0), rgb(0xb5bac1))),
                )
                .child(
                    div()
                        .cursor_pointer()
                        .hover(|s| s.text_color(rgb(0xffffff)))
                        .child(icon_help(px(20.0), rgb(0xb5bac1))),
                ),
        )
}

/// Renders the Message Requests view matching docs/discord_official_message_requests.png
fn render_message_requests_view(
    entity: Entity<AppState>,
    state: &AppState,
) -> Div {
    let mut container = div()
        .flex_1()
        .flex()
        .flex_col()
        .overflow_hidden()
        .p(px(24.0))
        .gap(px(16.0));

    let is_spam_tab = state.message_requests_subtab == MessageRequestsSubTab::Spam;
    let filtered_requests: Vec<_> = state
        .message_requests
        .iter()
        .filter(|r| r.is_spam == is_spam_tab)
        .collect();

    // Top Header with Info and Clear Action
    let top_bar = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .pb(px(8.0))
        .border_b_1()
        .border_color(rgb(0x35373c))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x80848e))
                .child(if is_spam_tab {
                    format!("SOLICITAÇÕES DE SPAM — {}", filtered_requests.len())
                } else {
                    format!("SOLICITAÇÕES DE MENSAGENS PENDENTES — {}", filtered_requests.len())
                }),
        )
        .children(if is_spam_tab && !filtered_requests.is_empty() {
            let entity_clear = entity.clone();
            Some(
                div()
                    .px(px(10.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .bg(rgb(0x35373c))
                    .hover(|s| s.bg(rgb(0xda373c)).text_color(rgb(0xffffff)))
                    .text_color(rgb(0xda373c))
                    .text_size(px(12.0))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                        entity_clear.update(cx, |s, cx| {
                            s.clear_spam_requests();
                            cx.notify();
                        });
                    })
                    .child("Limpar tudo"),
            )
        } else {
            None
        });

    container = container.child(top_bar);

    if filtered_requests.is_empty() {
        let empty_view = div()
            .flex_1()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .child(
                div()
                    .size(px(80.0))
                    .rounded_full()
                    .bg(rgb(0x2b2d31))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if is_spam_tab { icon_shield(px(40.0), rgb(0x80848e)) } else { icon_inbox(px(40.0), rgb(0x80848e)) }),
            )
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(16.0))
                    .text_color(rgb(0xffffff))
                    .child(if is_spam_tab {
                        "Nenhuma solicitação de spam"
                    } else {
                        "Você não tem nenhuma solicitação de mensagem"
                    }),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(0x80848e))
                    .child(if is_spam_tab {
                        "Sua caixa de entrada está limpa de spam. Mensagens suspeitas de não-amigos serão filtradas aqui."
                    } else {
                        "Quando pessoas que não estão na sua lista de amigos lhe enviarem uma mensagem direta, ela aparecerá aqui para você aceitar ou ignorar."
                    }),
            );
        return container.child(empty_view);
    }

    // Scrollable list of message requests
    let mut list = div()
        .id("message-requests-list")
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .overflow_y_scroll();

    for req in filtered_requests {
        let req_id_accept = req.id.clone();
        let req_id_ignore = req.id.clone();
        let entity_accept = entity.clone();
        let entity_ignore = entity.clone();

        let presence_color = match req.presence {
            PresenceStatus::Online => 0x23a55a,
            PresenceStatus::Idle => 0xf0b232,
            PresenceStatus::DoNotDisturb => 0xf23f43,
            PresenceStatus::Offline => 0x80848e,
        };

        let card = div()
            .w_full()
            .min_h(px(68.0))
            .p(px(14.0))
            .rounded(px(8.0))
            .bg(rgb(0x2b2d31))
            .border_1()
            .border_color(rgb(0x35373c))
            .hover(|s| s.bg(rgb(0x35373c)).border_color(rgb(0x4e5058)))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.0))
            // Left details
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .flex_1()
                    // Avatar with presence
                    .child(
                        div()
                            .relative()
                            .size(px(40.0))
                            .rounded_full()
                            .bg(rgb(0x5865f2))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(HEX_WHITE))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.0))
                            .child(req.avatar_text.clone())
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(0.0))
                                    .right(px(0.0))
                                    .size(px(10.0))
                                    .rounded_full()
                                    .bg(rgb(presence_color))
                                    .border_2()
                                    .border_color(rgb(0x2b2d31)),
                            ),
                    )
                    // Text Details
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .flex_1()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_baseline()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(px(15.0))
                                            .text_color(rgb(0xffffff))
                                            .child(req.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(0x80848e))
                                            .child(format!("@{}", req.username)),
                                    )
                                    .children(if let Some(ref guild_name) = req.mutual_guild_name {
                                        Some(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(4.0))
                                                .px(px(6.0))
                                                .py(px(1.0))
                                                .rounded(px(4.0))
                                                .bg(rgb(0x1e1f22))
                                                .text_size(px(11.0))
                                                .text_color(rgb(0x949ba4))
                                                .child(icon_shield(px(12.0), rgb(0x949ba4)))
                                                .child(guild_name.clone()),
                                        )
                                    } else {
                                        None
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(0xdbdee1))
                                    .child(format!("\"{}\"", req.preview_message)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0x80848e))
                                    .child(req.timestamp.clone()),
                            ),
                    ),
            )
            // Right Actions
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    // Aceitar button
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(rgb(0x5865f2))
                            .hover(|s| s.bg(rgb(0x4752c4)))
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                let rid = req_id_accept.clone();
                                entity_accept.update(cx, |s, cx| {
                                    s.accept_message_request(&rid);
                                    cx.notify();
                                });
                            })
                            .child("Aceitar"),
                    )
                    // Ignorar button
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(6.0))
                            .rounded(px(4.0))
                            .bg(rgb(0x35373c))
                            .hover(|s| s.bg(rgb(0xda373c)).text_color(rgb(0xffffff)))
                            .cursor_pointer()
                            .text_color(rgb(0xdbdee1))
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                let rid = req_id_ignore.clone();
                                entity_ignore.update(cx, |s, cx| {
                                    s.ignore_message_request(&rid);
                                    cx.notify();
                                });
                            })
                            .child("Ignorar"),
                    ),
            );

        list = list.child(card);
    }

    container.child(list)
}

/// Renders the Friends List view (Online, All, Pending, Blocked)
fn render_friends_list_view(
    entity: Entity<AppState>,
    state: &AppState,
) -> Div {
    struct FriendItem {
        user_id: String,
        display_name: String,
        username: String,
        avatar_text: String,
        avatar_url: Option<String>,
        status: String,
        channel_id: String,
        presence_color: u32,
        presence_status: PresenceStatus,
    }

    let mut friends: Vec<FriendItem> = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Add from state.friends
    for f in &state.friends {
        let lower = f.username.to_lowercase();
        if seen_names.insert(lower.clone()) {
            let (status_desc, color) = if let Some(ref s) = f.custom_status {
                (s.clone(), match f.presence {
                    PresenceStatus::Online => 0x23a55a,
                    PresenceStatus::Idle => 0xf0b232,
                    PresenceStatus::DoNotDisturb => 0xf23f43,
                    PresenceStatus::Offline => 0x80848e,
                })
            } else {
                match f.presence {
                    PresenceStatus::Online => ("Disponível".into(), 0x23a55a),
                    PresenceStatus::Idle => ("Ausente".into(), 0xf0b232),
                    PresenceStatus::DoNotDisturb => ("Não Perturbar".into(), 0xf23f43),
                    PresenceStatus::Offline => ("Offline".into(), 0x80848e),
                }
            };
            let dm_id = state
                .dm_channels
                .iter()
                .find(|d| {
                    !d.is_group_dm
                        && (d.recipient_ids.contains(&f.id)
                            || d.name.eq_ignore_ascii_case(&f.username)
                            || d.name.eq_ignore_ascii_case(&f.display_name))
                })
                .map(|d| d.id.clone())
                .unwrap_or_else(|| format!("dm-{}", f.id));

            friends.push(FriendItem {
                user_id: f.id.clone(),
                display_name: if f.display_name.is_empty() {
                    f.username.clone()
                } else {
                    f.display_name.clone()
                },
                username: format!("@{}", f.username),
                avatar_text: if f.avatar_text.is_empty() {
                    f.display_name
                        .chars()
                        .next()
                        .or_else(|| f.username.chars().next())
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string()
                } else {
                    f.avatar_text.clone()
                },
                avatar_url: f.avatar_url.clone(),
                status: status_desc,
                channel_id: dm_id,
                presence_color: color,
                presence_status: f.presence,
            });
        }
    }

    // 2. Add from members
    for m in &state.members {
        if m.is_bot || m.id == state.current_user.id {
            continue;
        }
        let lower = m.name.to_lowercase();
        if seen_names.insert(lower.clone()) {
            let (status_desc, color) = if let Some(ref act) = m.activity {
                (act.clone(), match m.presence {
                    PresenceStatus::Online => 0x23a55a,
                    PresenceStatus::Idle => 0xf0b232,
                    PresenceStatus::DoNotDisturb => 0xf23f43,
                    PresenceStatus::Offline => 0x80848e,
                })
            } else {
                match m.presence {
                    PresenceStatus::Online => ("Disponível".into(), 0x23a55a),
                    PresenceStatus::Idle => ("Ausente".into(), 0xf0b232),
                    PresenceStatus::DoNotDisturb => ("Não Perturbar".into(), 0xf23f43),
                    PresenceStatus::Offline => ("Offline".into(), 0x80848e),
                }
            };
            let dm_id = state
                .dm_channels
                .iter()
                .find(|d| {
                    !d.is_group_dm
                        && (d.recipient_ids.contains(&m.id) || d.name.eq_ignore_ascii_case(&m.name))
                })
                .map(|d| d.id.clone())
                .unwrap_or_else(|| format!("dm-{}", m.id));

            friends.push(FriendItem {
                user_id: m.id.clone(),
                display_name: m.name.clone(),
                username: format!("@{}", lower.replace(' ', "_")),
                avatar_text: if m.avatar_text.is_empty() {
                    m.name.chars().next().unwrap_or('?').to_uppercase().to_string()
                } else {
                    m.avatar_text.clone()
                },
                avatar_url: None,
                status: status_desc,
                channel_id: dm_id,
                presence_color: color,
                presence_status: m.presence,
            });
        }
    }

    // 3. Add from dm_channels
    for dm in &state.dm_channels {
        let lower = dm.name.to_lowercase();
        if seen_names.insert(lower.clone()) {
            let dm_uid = dm.recipient_ids.first().cloned().unwrap_or_default();
            friends.push(FriendItem {
                user_id: dm_uid,
                display_name: dm.name.clone(),
                username: format!("@{}", lower.replace(' ', "_")),
                avatar_text: dm.name.chars().next().unwrap_or('?').to_uppercase().to_string(),
                avatar_url: dm.icon_url.clone(),
                status: dm.topic.clone().unwrap_or_else(|| "Disponível".into()),
                channel_id: dm.id.clone(),
                presence_color: 0x23a55a,
                presence_status: PresenceStatus::Online,
            });
        }
    }

    // Filter based on active friends_tab
    let filtered: Vec<FriendItem> = match state.friends_tab {
        FriendsTab::Online => friends
            .into_iter()
            .filter(|f| f.presence_status != PresenceStatus::Offline)
            .collect(),
        FriendsTab::All => friends,
        FriendsTab::Pending => Vec::new(),
        FriendsTab::Blocked => Vec::new(),
        FriendsTab::AddFriend => Vec::new(),
        FriendsTab::MessageRequests => Vec::new(),
    };

    let is_empty = filtered.is_empty();
    let count_text = match state.friends_tab {
        FriendsTab::Online => format!("DISPONÍVEL — {}", filtered.len()),
        FriendsTab::All => format!("TODOS OS AMIGOS — {}", filtered.len()),
        FriendsTab::Pending => "PENDENTES — 0".to_string(),
        FriendsTab::Blocked => "BLOQUEADOS — 0".to_string(),
        _ => format!("AMIGOS — {}", filtered.len()),
    };

    let mut list_container = div()
        .id("friends-scroll-list")
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .px(px(24.0))
        .py(px(16.0))
        .gap(px(4.0))
        .overflow_y_scroll();

    list_container = list_container.child(
        div()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0x80848e))
            .pb(px(8.0))
            .child(count_text),
    );

    for (idx, friend) in filtered.into_iter().enumerate() {
        let entity_click = entity.clone();
        let entity_btn = entity.clone();
        let target_cid = friend.channel_id.clone();
        let btn_cid = friend.channel_id.clone();
        let click_uid = friend.user_id.clone();
        let click_uname = friend.display_name.clone();
        let btn_uid = friend.user_id.clone();
        let btn_uname = friend.display_name.clone();

        let friend_row = div()
            .id(SharedString::from(format!("friend-row-{}", idx)))
            .w_full()
            .h(px(61.0))
            .px(px(12.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(rgb(0x35373c))
            .rounded(radius_md())
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x35373c)))
            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                let uid = click_uid.clone();
                let uname = click_uname.clone();
                let cid = target_cid.clone();
                entity_click.update(cx, |state, cx| {
                    if !uid.is_empty() {
                        state.open_dm_with_user(&uid, &uname);
                    } else {
                        state.select_channel(cid);
                    }
                    cx.notify();
                });
            })
            // Left: Avatar + Details
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    // Avatar Container with Status Dot
                    .child(
                        div()
                            .relative()
                            .child(if let Some(ref url) = friend.avatar_url {
                                if url.starts_with("http://") || url.starts_with("https://") {
                                    div()
                                        .size(px(36.0))
                                        .rounded_full()
                                        .overflow_hidden()
                                        .flex_shrink_0()
                                        .child(img(url.clone()).size_full())
                                } else {
                                    div()
                                        .size(avatar_md())
                                        .rounded_full()
                                        .bg(blurple())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(rgb(HEX_WHITE))
                                        .font_weight(WEIGHT_BOLD)
                                        .text_size(font_size_md())
                                        .child(friend.avatar_text.clone())
                                }
                            } else {
                                div()
                                    .size(avatar_md())
                                    .rounded_full()
                                    .bg(blurple())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(HEX_WHITE))
                                    .font_weight(WEIGHT_BOLD)
                                    .text_size(font_size_md())
                                    .child(friend.avatar_text)
                            })
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(0.0))
                                    .right(px(0.0))
                                    .size(px(10.0))
                                    .rounded_full()
                                    .bg(rgb(friend.presence_color))
                                    .border_2()
                                    .border_color(rgb(0x313338)),
                            ),
                    )
                    // Names & Status
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_baseline()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .font_weight(WEIGHT_SEMIBOLD)
                                            .text_size(font_size_base())
                                            .text_color(rgb(HEX_WHITE))
                                            .child(friend.display_name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(0x80848e))
                                            .child(friend.username),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(font_size_xs())
                                    .text_color(rgb(0x80848e))
                                    .child(friend.status),
                            ),
                    ),
            )
            // Right: DM Button & Options
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id(SharedString::from(format!("friend-dm-btn-{}", idx)))
                            .size(px(36.0))
                            .rounded_full()
                            .bg(rgb(0x2b2d31))
                            .hover(|s| s.bg(rgb(0x111214)).text_color(rgb(HEX_WHITE)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0x80848e))
                            .text_size(px(16.0))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                let uid = btn_uid.clone();
                                let uname = btn_uname.clone();
                                let cid = btn_cid.clone();
                                entity_btn.update(cx, |state, cx| {
                                    if !uid.is_empty() {
                                        state.open_dm_with_user(&uid, &uname);
                                    } else {
                                        state.select_channel(cid);
                                    }
                                    cx.notify();
                                });
                            })
                            .child(icon_chat(px(18.0), rgb(0x80848e))),
                    )
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded_full()
                            .bg(rgb(0x2b2d31))
                            .hover(|s| s.bg(rgb(0x111214)).text_color(rgb(HEX_WHITE)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(0x80848e))
                            .text_size(px(16.0))
                            .cursor_pointer()
                            .child("⋮"),
                    ),
            );

        list_container = list_container.child(friend_row);
    }

    div()
        .flex_1()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            // Search Bar (38px height, #1e1f22 bg, 4px radius)
            div()
                .px(px(24.0))
                .pt(px(16.0))
                .pb(px(8.0))
                .child(
                    div()
                        .h(px(38.0))
                        .bg(rgb(0x1e1f22))
                        .rounded(px(4.0))
                        .px(px(12.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(rgb(0x80848e))
                                .child("Buscar"),
                        )
                        .child(icon_search(px(16.0), rgb(0x80848e))),
                ),
        )
        .child(if is_empty {
            div()
                .flex_1()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.0))
                .p(px(24.0))
                .child(
                    div()
                        .size(px(80.0))
                        .rounded_full()
                        .bg(rgb(0x2b2d31))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_friends(px(40.0), rgb(0x80848e))),
                )
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_size(font_size_lg())
                        .text_color(rgb(0xffffff))
                        .child("Nenhum amigo encontrado ou lista vazia"),
                )
                .child(
                    div()
                        .text_size(font_size_sm())
                        .text_color(rgb(0x80848e))
                        .child("Quando você adicionar amigos ou receber mensagens diretas, eles aparecerão aqui."),
                )
                .into_any_element()
        } else {
            list_container.into_any_element()
        })
}

/// Renders the Active Now ("Ativo agora") right sidebar
/// Specifications:
/// - Exact fixed width: 360px
/// - Background: #2b2d31
/// - Left border: 1px solid #35373c
/// - Cards: Rich Presence matching docs/discord_official_active_now.png
pub fn render_active_now_panel(
    _entity: Entity<AppState>,
    state: &AppState,
) -> Stateful<Div> {
    let mut panel = div()
        .id("active-now-scroll")
        .w(px(360.0))
        .flex_shrink_0()
        .bg(rgb(0x2b2d31))
        .border_l_1()
        .border_color(rgb(0x35373c))
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(16.0))
        .overflow_y_scroll()
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(20.0))
                .text_color(rgb(0xf2f3f5))
                .child("Ativo agora"),
        );

    if state.active_now_items.is_empty() {
        // Empty state card
        let empty_card = div()
            .w_full()
            .p(px(16.0))
            .rounded(px(16.0))
            .bg(rgb(0x2b2d31))
            .border_1()
            .border_color(rgb(0x383a40))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(16.0))
                    .text_color(rgb(0xffffff))
                    .child("Por enquanto, está quieto..."),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(0x80848e))
                    .child("Quando seus amigos estiverem ativos em jogos ou ouvindo músicas, você verá as atividades deles aqui!"),
            );
        return panel.child(empty_card);
    }

    // Render Rich Presence activity cards
    for item in &state.active_now_items {
        let presence_color = match item.presence {
            PresenceStatus::Online => 0x23a55a,
            PresenceStatus::Idle => 0xf0b232,
            PresenceStatus::DoNotDisturb => 0xf23f43,
            PresenceStatus::Offline => 0x80848e,
        };

        let card = div()
            .w_full()
            .p(px(16.0))
            .rounded(px(16.0))
            .bg(rgb(0x1e1f22))
            .border_1()
            .border_color(rgb(0x383a40))
            .hover(|s| s.border_color(rgb(0x4e5058)))
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Top: Friend Avatar + Display Name + Header Subtitle + Top-Right App Icon
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            // Avatar with status
                            .child(
                                div()
                                    .relative()
                                    .size(px(32.0))
                                    .rounded_full()
                                    .bg(rgb(0x5865f2))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(14.0))
                                    .child(item.avatar_text.clone())
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(0.0))
                                            .right(px(0.0))
                                            .size(px(10.0))
                                            .rounded_full()
                                            .bg(rgb(presence_color))
                                            .border_2()
                                            .border_color(rgb(0x1e1f22)),
                                    ),
                            )
                            // Friend Name & Status
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(px(14.0))
                                            .text_color(rgb(0xffffff))
                                            .child(item.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(0x949ba4))
                                            .child(item.header_subtitle.clone()),
                                    ),
                            ),
                    )
                    // Top-right App symbol/icon
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(match item.kind {
                                ActiveNowKind::Quest | ActiveNowKind::Game => icon_gamepad(px(16.0), rgb(0x949ba4)),
                                ActiveNowKind::Spotify => icon_music(px(16.0), rgb(0x23a55a)),
                                ActiveNowKind::RichPresence => icon_screen_share(px(16.0), rgb(0x949ba4)),
                            }),
                    ),
            )
            // Inner Details Box (Rich Presence / Game / Spotify)
            .child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgb(0x111214))
                    .border_1()
                    .border_color(rgb(0x232428))
                    .flex()
                    .flex_row()
                    .gap(px(12.0))
                    // Big App Icon (48x48px)
                    .child(
                        div()
                            .size(px(48.0))
                            .rounded(px(8.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(item.kind == ActiveNowKind::RichPresence, |d| {
                                d.bg(rgb(0x007acc)).text_color(rgb(0xffffff)).text_size(px(20.0))
                            })
                            .when(item.kind == ActiveNowKind::Quest, |d| {
                                d.bg(rgb(0x2b2d31)).text_color(rgb(0xffffff)).text_size(px(22.0))
                            })
                            .when(item.kind == ActiveNowKind::Spotify, |d| {
                                d.bg(rgb(0x121212)).text_color(rgb(0x23a55a)).text_size(px(24.0))
                            })
                            .when(item.kind == ActiveNowKind::Game, |d| {
                                d.bg(rgb(0x248046)).text_color(rgb(0xffffff)).text_size(px(20.0))
                            })
                            .child(match item.kind {
                                ActiveNowKind::RichPresence => div().child("VS"),
                                ActiveNowKind::Quest => icon_gamepad(px(24.0), rgb(0xffffff)),
                                ActiveNowKind::Spotify => icon_music(px(24.0), rgb(0x23a55a)),
                                ActiveNowKind::Game => icon_gamepad(px(24.0), rgb(0xffffff)),
                            }),
                    )
                    // Details Content
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .overflow_hidden()
                            // Activity Details Title
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(14.0))
                                    .text_color(rgb(0xf2f3f5))
                                    .child(item.details_title.clone()),
                            )
                            // State / Subtitle
                            .children(item.details_state.as_ref().map(|state_text| {
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0x949ba4))
                                    .child(state_text.clone())
                            }))
                            // Elapsed Time
                            .children(item.elapsed_time_text.as_ref().map(|elapsed| {
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0x80848e))
                                    .child(elapsed.clone())
                            }))
                            // Spotify Progress Bar (#23a55a)
                            .children(if item.kind == ActiveNowKind::Spotify {
                                let (cur, total) = item
                                    .spotify_progress_text
                                    .clone()
                                    .unwrap_or_else(|| ("1:24".into(), "3:50".into()));
                                let progress_ratio = item.spotify_progress_pct.unwrap_or(0.36);

                                Some(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(8.0))
                                        .mt(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgb(0x80848e))
                                                .child(cur),
                                        )
                                        // Track + Green Progress Bar
                                        .child(
                                            div()
                                                .flex_1()
                                                .h(px(4.0))
                                                .bg(rgb(0x3f4248))
                                                .rounded_full()
                                                .relative()
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .w(px(progress_ratio * 120.0))
                                                        .bg(rgb(0x23a55a))
                                                        .rounded_full(),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(rgb(0x80848e))
                                                .child(total),
                                        ),
                                )
                            } else {
                                None
                            }),
                    ),
            )
            // Optional Action Button (e.g. "Aceitar missão", "Ouvir no Spotify")
            .children(item.button_text.as_ref().map(|btn_label| {
                div()
                    .w_full()
                    .h(px(32.0))
                    .rounded(px(4.0))
                    .bg(rgb(0x5865f2))
                    .hover(|s| s.bg(rgb(0x4752c4)))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xffffff))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(btn_label.clone())
            }));

        panel = panel.child(card);
    }

    panel
}
