//! Discord RS - Friends Hub & Message Requests View
//!
//! Home "Amigos" view on kit components + theme tokens:
//! - Tab bar: styled tab items matching Discord parity (32px h, 8px radius,
//!   `control_secondary_bg_active` selected bg, `text_primary` unselected fg)
//! - Text buttons: `crate::components::Button` (kit wrapper); circular icon
//!   buttons: kit `Button` (ghost) with kit tooltip
//! - Avatars: `crate::components::Avatar` (status overlay + decoration)
//! - Colors: `crate::tokens` accessors only; literals kept are inventoried as
//!   TOKEN FALTANDO in the task report (untouchable `src/tokens`).
//!
//! INPUT PENDENTE: the search box and the add-friend field need the kit
//! `Input`, which requires an `Entity<InputState>` created via `cx` plus
//! storage in `AppState` — both owned by other fronts (state/components).
//! They stay static token-styled boxes with identical text/behavior.
//! DECORAÇÃO: `FriendItemState.avatar_decoration_data` é repassado ao `Avatar`
//! (quando presente no modelo). `MessageRequestItem` e `ActiveNowItem` não têm
//! decoração na API atual.

use gpui::component::Disableable as _;
use gpui::component::button::{Button as KitButton, ButtonVariants as _};
use gpui::component::input::InputState;
use gpui::*;
use crate::app::state::*;
use crate::components::{Avatar, AvatarSize, Button, ButtonSize, TextInput};
use crate::domain::user::{AvatarDecorationData, PresenceStatus as DomainPresence};
use crate::tokens::colors::white;
use crate::tokens::*;

/// Maps state presence to the domain presence the `Avatar` component takes.
fn domain_presence(p: PresenceStatus) -> DomainPresence {
    match p {
        PresenceStatus::Online => DomainPresence::Online,
        PresenceStatus::Idle => DomainPresence::Idle,
        PresenceStatus::DoNotDisturb => DomainPresence::DoNotDisturb,
        PresenceStatus::Offline => DomainPresence::Offline,
    }
}

/// Main entry point for the Friends Hub ("Amigos") view when on Home.
pub fn render_friends_hub(
    entity: Entity<AppState>,
    state: &AppState,
    search_state: &Entity<InputState>,
    search_query: &str,
    add_friend_state: &Entity<InputState>,
) -> Div {
    let mut container = div()
        .size_full()
        .bg(bg_chat())
        .text_color(text_primary())
        .flex()
        .flex_col()
        .overflow_hidden();

    container = container.child(render_header(entity.clone(), state));

    let left_column = if state.friends_tab == FriendsTab::MessageRequests {
        render_message_requests_view(entity.clone(), state)
    } else if state.friends_tab == FriendsTab::AddFriend {
        render_add_friend_view(entity.clone(), state, add_friend_state)
    } else {
        render_friends_list_view(entity.clone(), state, search_state, search_query)
    };

    container.child(
        div()
            .flex_1()
            .w_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            .child(left_column)
            .child(render_active_now_panel(entity.clone(), state)),
    )
}

/// Top navigation header bar (48px): section label + kit tab bar + inbox/help.
fn render_header(entity: Entity<AppState>, state: &AppState) -> Div {
    div()
        .w_full()
        .h(px(49.0))
        .border_b_1()
        .border_color(border_subtle())
        .pl(px(16.0))
        .pr(px(16.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(render_header_tabs(entity.clone(), state))
        .child(render_header_actions())
}

/// Left side of the header: section icon + title + separator + tab bar.
fn render_header_tabs(entity: Entity<AppState>, state: &AppState) -> AnyElement {
    let entity_home = entity.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity_home.update(cx, |s, cx| {
                        s.friends_tab = FriendsTab::Online;
                        cx.notify();
                    });
                })
                .child(if state.friends_tab == FriendsTab::MessageRequests {
                    icon_inbox(px(20.0), text_muted())
                } else {
                    icon_friends(px(20.0), text_muted())
                })
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_size(font_size_md())
                        .text_color(white())
                        .child(if state.friends_tab == FriendsTab::MessageRequests {
                            "Solicitações de mensagens"
                        } else {
                            "Amigos"
                        }),
                ),
        )
        .child(div().w(px(1.0)).h(px(16.0)).bg(channel_active()))
        .child(render_tab_bar(entity.clone(), state))
        .into_any_element()
}

/// Kit tab bar for the friends tabs (or the message-requests subtabs).
fn render_tab_bar(entity: Entity<AppState>, state: &AppState) -> AnyElement {
    if state.friends_tab == FriendsTab::MessageRequests {
        render_msgreq_tab_bar(entity, state)
    } else {
        render_friends_tab_bar(entity, state)
    }
}

/// Count pill shown inside tabs (Pendente, Solicitações, subtabs).
fn tab_count_badge(count: usize, bg: Rgba) -> AnyElement {
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded_full()
        .bg(bg)
        .text_size(px(11.0))
        .text_color(white())
        .child(count.to_string())
        .into_any_element()
}

/// Renders an individual tab item matching measured Discord parity (DOM/CSS):
/// - 32px height, padding 4px 12px, radius 8px, 16px/20px w400.
/// - Selected: bg rgba(151,151,159,0.2) (`control_secondary_bg_active()`), text white.
/// - Unselected: bg transparent, text `text_primary()` (oklab 0.7419... / #dbdee1),
///   hover bg `control_secondary_bg_default()` (0x97979f1f) and text white.
///
/// NOTE on gpui-kit `TabBar`/`Tab`:
/// `TabBar::pill` hardcodes `cx.theme().radius_full()` (tab_bar.rs:266, tab.rs:348)
/// and indicator background `cx.theme().tokens.primary` (tab_bar.rs:265),
/// rendering an unstyleable, fully-rounded black pill instead of Discord's 8px-rounded
/// `rgba(151,151,159,0.2)` surface. Hence individual styled tab items are used.
fn render_tab_item(
    id: impl Into<ElementId>,
    label: &'static str,
    icon: Option<AnyElement>,
    badge: Option<AnyElement>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut tab = div()
        .id(id)
        .h(px(32.0))
        .px(px(12.0))
        .py(px(4.0))
        .rounded(px(8.0))
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .text_size(px(16.0))
        .line_height(px(20.0))
        .font_weight(FontWeight::NORMAL);

    if selected {
        tab = tab
            .bg(control_secondary_bg_active())
            .text_color(white());
    } else {
        tab = tab
            .bg(transparent())
            .text_color(text_primary())
            .hover(|s| s.bg(control_secondary_bg_default()).text_color(white()));
    }

    if let Some(ic) = icon {
        tab = tab.child(ic);
    }
    tab = tab.child(label);
    if let Some(b) = badge {
        tab = tab.child(b);
    }
    tab.on_click(move |ev, window, cx| on_click(ev, window, cx))
        .into_any_element()
}

/// Main tabs: Disponível, Todos, Pendente, (Bloqueado se houver) + Adicionar amigo.
fn render_friends_tab_bar(entity: Entity<AppState>, state: &AppState) -> AnyElement {
    let pending_friends = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 3)
        .count();
    let blocked_count = state
        .friends
        .iter()
        .filter(|f| f.relationship_type == 2)
        .count();

    let e_online = entity.clone();
    let e_all = entity.clone();
    let e_pending = entity.clone();
    let e_blocked = entity.clone();

    let show_blocked = blocked_count > 0 || state.friends_tab == FriendsTab::Blocked;

    let mut tab_bar = div()
        .id("friends-tabbar")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(render_tab_item(
            "tab-online",
            "Disponível",
            None,
            None,
            state.friends_tab == FriendsTab::Online,
            move |_ev, _window, cx| {
                e_online.update(cx, |s, cx| {
                    s.select_friends_tab(FriendsTab::Online);
                    cx.notify();
                });
            },
        ))
        .child(render_tab_item(
            "tab-all",
            "Todos",
            None,
            None,
            state.friends_tab == FriendsTab::All,
            move |_ev, _window, cx| {
                e_all.update(cx, |s, cx| {
                    s.select_friends_tab(FriendsTab::All);
                    cx.notify();
                });
            },
        ))
        .child(render_tab_item(
            "tab-pending",
            "Pendente",
            None,
            (pending_friends > 0).then(|| tab_count_badge(pending_friends, legacy_da373c())),
            state.friends_tab == FriendsTab::Pending,
            move |_ev, _window, cx| {
                e_pending.update(cx, |s, cx| {
                    s.select_friends_tab(FriendsTab::Pending);
                    cx.notify();
                });
            },
        ));

    if show_blocked {
        tab_bar = tab_bar.child(render_tab_item(
            "tab-blocked",
            "Bloqueado",
            None,
            None,
            state.friends_tab == FriendsTab::Blocked,
            move |_ev, _window, cx| {
                e_blocked.update(cx, |s, cx| {
                    s.select_friends_tab(FriendsTab::Blocked);
                    cx.notify();
                });
            },
        ));
    }

    tab_bar = tab_bar.child(render_add_friend_tab_button(
        entity.clone(),
        state.friends_tab == FriendsTab::AddFriend,
    ));

    tab_bar.into_any_element()
}

/// "Adicionar amigo" brand button (TabBar suffix of the main tab bar).
/// Uses `crate::components::Button` with Blurple styling matching modern Discord parity.
fn render_add_friend_tab_button(entity: Entity<AppState>, _selected: bool) -> AnyElement {
    let mut btn = Button::blurple("btn-add-friend", "Adicionar amigo")
        .size(ButtonSize::Sm);
    btn = btn.on_click(move |_ev, _window, cx| {
        entity.update(cx, |s, cx| {
            s.select_friends_tab(FriendsTab::AddFriend);
            cx.notify();
        });
    });
    btn.into_any_element()
}

/// Subtabs of the message-requests view: Você pode conhecer, Spam, Voltar.
fn render_msgreq_tab_bar(entity: Entity<AppState>, state: &AppState) -> AnyElement {
    let pending_req = state
        .message_requests
        .iter()
        .filter(|r| !r.is_spam)
        .count();
    let spam_count = state
        .message_requests
        .iter()
        .filter(|r| r.is_spam)
        .count();

    let e_you_may_know = entity.clone();
    let e_spam = entity.clone();
    let e_back = entity.clone();

    div()
        .id("msgreq-tabbar")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(render_tab_item(
            "subtab-you-may-know",
            "Você pode conhecer",
            None,
            (pending_req > 0).then(|| tab_count_badge(pending_req, blurple())),
            state.message_requests_subtab == MessageRequestsSubTab::YouMayKnow,
            move |_ev, _window, cx| {
                e_you_may_know.update(cx, |s, cx| {
                    s.select_message_requests_subtab(MessageRequestsSubTab::YouMayKnow);
                    cx.notify();
                });
            },
        ))
        .child(render_tab_item(
            "subtab-spam",
            "Spam",
            None,
            (spam_count > 0).then(|| tab_count_badge(spam_count, legacy_da373c())),
            state.message_requests_subtab == MessageRequestsSubTab::Spam,
            move |_ev, _window, cx| {
                e_spam.update(cx, |s, cx| {
                    s.select_message_requests_subtab(MessageRequestsSubTab::Spam);
                    cx.notify();
                });
            },
        ))
        .child(render_tab_item(
            "subtab-back",
            "Voltar para Amigos",
            None,
            None,
            false,
            move |_ev, _window, cx| {
                e_back.update(cx, |s, cx| {
                    s.friends_tab = FriendsTab::Online;
                    cx.notify();
                });
            },
        ))
        .into_any_element()
}

/// Right side of the header: new group dm + inbox + help icons per friends-online.png.
fn render_header_actions() -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .child(icon_chat(px(20.0), text_muted()))
        .child(
            div()
                .w(px(1.0))
                .h(px(16.0))
                .bg(border_subtle())
                .mx(px(2.0)),
        )
        .child(icon_inbox(px(20.0), text_muted()))
        .child(icon_help(px(20.0), text_muted()))
        .into_any_element()
}

/// Message Requests view: header w/ clear-spam action, empty state, cards.
fn render_message_requests_view(entity: Entity<AppState>, state: &AppState) -> Div {
    let mut container = div()
        .flex_1()
        .flex()
        .flex_col()
        .overflow_hidden()
        .p(px(24.0))
        .gap(px(16.0));

    container = container.child(render_msgreq_top_bar(entity.clone(), state));

    let is_spam_tab = state.message_requests_subtab == MessageRequestsSubTab::Spam;
    let reqs: Vec<_> = state
        .message_requests
        .iter()
        .filter(|r| r.is_spam == is_spam_tab)
        .collect();

    if reqs.is_empty() {
        return container.child(render_msgreq_empty(is_spam_tab));
    }

    let mut list = div()
        .id("message-requests-list")
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .overflow_y_scroll();
    for req in reqs {
        list = list.child(render_msgreq_card(entity.clone(), req));
    }
    container.child(list)
}

/// "SOLICITAÇÕES ... — N" row with the "Limpar tudo" danger button on spam.
fn render_msgreq_top_bar(entity: Entity<AppState>, state: &AppState) -> AnyElement {
    let is_spam_tab = state.message_requests_subtab == MessageRequestsSubTab::Spam;
    let count = state
        .message_requests
        .iter()
        .filter(|r| r.is_spam == is_spam_tab)
        .count();

    let mut bar = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .pb(px(8.0))
        .border_b_1()
        .border_color(channel_hover())
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(text_muted())
                .child(if is_spam_tab {
                    format!("SOLICITAÇÕES DE SPAM — {}", count)
                } else {
                    format!("SOLICITAÇÕES DE MENSAGENS PENDENTES — {}", count)
                }),
        );

    if is_spam_tab && count > 0 {
        let mut clear =
            Button::danger("msgreq-clear-spam", "Limpar tudo").size(ButtonSize::Sm);
        clear = clear.on_click(move |_ev, _window, cx| {
            entity.update(cx, |s, cx| {
                s.clear_spam_requests();
                cx.notify();
            });
        });
        bar = bar.child(clear);
    }
    bar.into_any_element()
}

/// Empty state of the message-requests view (same texts as today).
fn render_msgreq_empty(is_spam_tab: bool) -> AnyElement {
    div()
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
                .bg(bg_channels())
                .flex()
                .items_center()
                .justify_center()
                .child(if is_spam_tab {
                    icon_shield(px(40.0), text_muted())
                } else {
                    icon_inbox(px(40.0), text_muted())
                }),
        )
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(16.0))
                .text_color(white())
                .child(if is_spam_tab {
                    "Nenhuma solicitação de spam"
                } else {
                    "Você não tem nenhuma solicitação de mensagem"
                }),
        )
        .child(
            div()
                .text_size(px(14.0))
                .text_color(text_muted())
                .child(if is_spam_tab {
                    "Sua caixa de entrada está limpa de spam. Mensagens suspeitas de não-amigos serão filtradas aqui."
                } else {
                    "Quando pessoas que não estão na sua lista de amigos lhe enviarem uma mensagem direta, ela aparecerá aqui para você aceitar ou ignorar."
                }),
        )
        .into_any_element()
}

/// One message-request card: Avatar + details + Aceitar/Ignorar buttons.
fn render_msgreq_card(entity: Entity<AppState>, req: &MessageRequestItem) -> AnyElement {
    div()
        .w_full()
        .min_h(px(68.0))
        .p(px(14.0))
        .rounded(px(8.0))
        .bg(bg_channels())
        .border_1()
        .border_color(channel_hover())
        .hover(|s| s.bg(channel_hover()).border_color(borders()))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(render_msgreq_card_left(req))
        .child(render_msgreq_card_actions(entity, req))
        .into_any_element()
}

/// Left side of a request card: Avatar with status + name/preview/timestamp.
fn render_msgreq_card_left(req: &MessageRequestItem) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .flex_1()
        .child(
            Avatar::new(req.avatar_text.clone())
                .opt_src(req.avatar_url.clone())
                .size(AvatarSize::Px40)
                .opt_status(Some(domain_presence(req.presence)))
                .bg_color(blurple())
                .cutout_color(bg_channels()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .child(render_msgreq_card_title(req))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(text_primary())
                        .child(format!("\"{}\"", req.preview_message)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(text_muted())
                        .child(req.timestamp.clone()),
                ),
        )
        .into_any_element()
}

/// Display name + @username + mutual-guild chip of a request card.
fn render_msgreq_card_title(req: &MessageRequestItem) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(8.0))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(15.0))
                .text_color(white())
                .child(req.display_name.clone()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(text_muted())
                .child(format!("@{}", req.username)),
        )
        .children(req.mutual_guild_name.as_ref().map(|guild_name| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .px(px(6.0))
                .py(px(1.0))
                .rounded(px(4.0))
                .bg(bg_server())
                .text_size(px(11.0))
                .text_color(legacy_949ba4())
                .child(icon_shield(px(12.0), legacy_949ba4()))
                .child(guild_name.clone())
        }))
        .into_any_element()
}

/// Aceitar (blurple) + Ignorar (gray) buttons of a request card.
fn render_msgreq_card_actions(entity: Entity<AppState>, req: &MessageRequestItem) -> AnyElement {
    let accept_id = req.id.clone();
    let ignore_id = req.id.clone();
    let entity_accept = entity.clone();
    let entity_ignore = entity.clone();

    let mut accept = Button::new("msgreq-accept", "Aceitar").size(ButtonSize::Sm);
    accept = accept.on_click(move |_ev, _window, cx| {
        let rid = accept_id.clone();
        entity_accept.update(cx, |s, cx| {
            s.accept_message_request(&rid);
            cx.notify();
        });
    });
    let mut ignore = Button::gray("msgreq-ignore", "Ignorar").size(ButtonSize::Sm);
    ignore = ignore.on_click(move |_ev, _window, cx| {
        let rid = ignore_id.clone();
        entity_ignore.update(cx, |s, cx| {
            s.ignore_message_request(&rid);
            cx.notify();
        });
    });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(accept)
        .child(ignore)
        .into_any_element()
}

/// Row data for the friends list (derived from state, no mocks).
#[derive(Debug, Clone)]
pub struct FriendRowData {
    pub user_id: String,
    pub display_name: String,
    pub username: String,
    pub avatar_text: String,
    pub avatar_url: Option<String>,
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    pub status: String,
    pub channel_id: String,
    pub presence: PresenceStatus,
    pub relationship_type: u32,
}

/// Friends list view (Online, All, Pending, Blocked): search + rows/empty.
fn render_friends_list_view(
    entity: Entity<AppState>,
    state: &AppState,
    search_state: &Entity<InputState>,
    search_query: &str,
) -> Div {
    let rows = collect_friend_rows(state, search_query);
    let is_empty = rows.is_empty();

    div()
        .flex_1()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(render_friends_search(search_state))
        .child(if is_empty {
            render_friends_empty(state.friends_tab)
        } else {
            render_friends_rows(entity, state, rows)
        })
}

/// Builds the visible friend rows: real relationships first, then offline
/// demo fallbacks (members, DMs) exactly as today — no invented data.
pub fn collect_friend_rows(state: &AppState, search_query: &str) -> Vec<FriendRowData> {
    let mut friends: Vec<FriendRowData> = Vec::new();
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for f in &state.friends {
        let lower = f.username.to_lowercase();
        if !seen_names.insert(lower) {
            continue;
        }
        let status = if let Some(ref s) = f.custom_status {
            s.clone()
        } else {
            match f.presence {
                PresenceStatus::Online => "Disponível".into(),
                PresenceStatus::Idle => "Ausente".into(),
                PresenceStatus::DoNotDisturb => "Não Perturbar".into(),
                PresenceStatus::Offline => "Offline".into(),
            }
        };
        friends.push(FriendRowData {
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
            avatar_decoration_data: f.avatar_decoration_data.clone(),
            status,
            channel_id: find_dm_for(state, &f.id, &f.username, &f.display_name),
            presence: f.presence,
            relationship_type: f.relationship_type,
        });
    }

    if !state.is_connected_to_gateway && state.auth_token.is_none() && state.friends.is_empty() {
        for m in &state.members {
            if m.is_bot || m.id == state.current_user.id {
                continue;
            }
            let lower = m.name.to_lowercase();
            if !seen_names.insert(lower.clone()) {
                continue;
            }
            let status = if let Some(ref act) = m.activity {
                act.clone()
            } else {
                match m.presence {
                    PresenceStatus::Online => "Disponível".into(),
                    PresenceStatus::Idle => "Ausente".into(),
                    PresenceStatus::DoNotDisturb => "Não Perturbar".into(),
                    PresenceStatus::Offline => "Offline".into(),
                }
            };
            friends.push(FriendRowData {
                user_id: m.id.clone(),
                display_name: m.name.clone(),
                username: format!("@{}", lower.replace(' ', "_")),
                avatar_text: if m.avatar_text.is_empty() {
                    m.name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .to_string()
                } else {
                    m.avatar_text.clone()
                },
                avatar_url: None,
                avatar_decoration_data: None,
                status,
                channel_id: find_dm_for(state, &m.id, &m.name, &m.name),
                presence: m.presence,
                relationship_type: 1,
            });
        }

        for dm in &state.dm_channels {
            if dm.id == "dm-group-gpui" {
                continue;
            }
            let lower = dm.name.to_lowercase();
            if !seen_names.insert(lower.clone()) {
                continue;
            }
            friends.push(FriendRowData {
                user_id: dm.recipient_ids.first().cloned().unwrap_or_default(),
                display_name: dm.name.clone(),
                username: format!("@{}", lower.replace(' ', "_")),
                avatar_text: dm
                    .name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string(),
                avatar_url: dm.icon_url.clone(),
                avatar_decoration_data: None,
                status: dm.topic.clone().unwrap_or_else(|| "Disponível".into()),
                channel_id: dm.id.clone(),
                presence: PresenceStatus::Online,
                relationship_type: 1,
            });
        }
    }

    let mut filtered: Vec<FriendRowData> = match state.friends_tab {
        FriendsTab::Online => friends
            .into_iter()
            .filter(|f| f.relationship_type == 1 && f.presence != PresenceStatus::Offline)
            .collect(),
        FriendsTab::All => friends
            .into_iter()
            .filter(|f| f.relationship_type == 1)
            .collect(),
        FriendsTab::Pending => friends
            .into_iter()
            .filter(|f| f.relationship_type == 3 || f.relationship_type == 4)
            .collect(),
        FriendsTab::Blocked => friends
            .into_iter()
            .filter(|f| f.relationship_type == 2)
            .collect(),
        _ => Vec::new(),
    };

    let q = search_query.trim().to_lowercase();
    if !q.is_empty() {
        filtered.retain(|r| {
            r.display_name.to_lowercase().contains(&q)
                || r.username.to_lowercase().contains(&q)
        });
    }

    filtered
}

/// Finds the 1:1 DM channel for a user, falling back to `dm-<id>` as today.
fn find_dm_for(state: &AppState, user_id: &str, username: &str, display_name: &str) -> String {
    state
        .dm_channels
        .iter()
        .find(|d| {
            !d.is_group_dm
                && (d.recipient_ids.contains(&user_id.to_string())
                    || d.name.eq_ignore_ascii_case(username)
                    || d.name.eq_ignore_ascii_case(display_name))
        })
        .map(|d| d.id.clone())
        .unwrap_or_else(|| format!("dm-{}", user_id))
}

/// Interactive search box bound to TextInput + kit InputState.
fn render_friends_search(search_state: &Entity<InputState>) -> AnyElement {
    div()
        .px(px(24.0))
        .pt(px(16.0))
        .pb(px(8.0))
        .child(
            TextInput::new("friends-search-input", search_state)
                .placeholder("Buscar")
                .show_paste(false)
                .show_clear(true),
        )
        .into_any_element()
}

/// Scrollable rows with the section header ("DISPONÍVEL — N", ...).
fn render_friends_rows(
    entity: Entity<AppState>,
    state: &AppState,
    rows: Vec<FriendRowData>,
) -> AnyElement {
    let count_text = match state.friends_tab {
        FriendsTab::Online => format!("DISPONÍVEL — {}", rows.len()),
        FriendsTab::All => format!("TODOS OS AMIGOS — {}", rows.len()),
        FriendsTab::Pending => format!("PENDENTE — {}", rows.len()),
        FriendsTab::Blocked => format!("BLOQUEADOS — {}", rows.len()),
        _ => format!("AMIGOS — {}", rows.len()),
    };

    let mut list = div()
        .id("friends-scroll-list")
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .px(px(24.0))
        .py(px(16.0))
        .gap(px(4.0))
        .overflow_y_scroll()
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(text_muted())
                .pb(px(8.0))
                .child(count_text),
        );
    for (idx, row) in rows.into_iter().enumerate() {
        list = list.child(render_friend_row(entity.clone(), state, row, idx));
    }
    list.into_any_element()
}

/// Empty state per tab (same icons/texts as today).
fn render_friends_empty(tab: FriendsTab) -> AnyElement {
    let (empty_icon, empty_title, empty_desc) = match tab {
        FriendsTab::Online => (
            icon_friends(px(48.0), text_muted()),
            "Ninguém está disponível para jogar com você.",
            "Quando seus amigos ficarem online, eles aparecerão aqui.",
        ),
        FriendsTab::All => (
            icon_friends(px(48.0), text_muted()),
            "O Wumpus está esperando por amigos.",
            "Você não tem nenhum amigo adicionado ainda. Use o botão Adicionar amigo para começar!",
        ),
        FriendsTab::Pending => (
            icon_inbox(px(48.0), text_muted()),
            "Não há solicitações de amizade pendentes.",
            "Aqui você verá as solicitações de amizade recebidas e enviadas.",
        ),
        FriendsTab::Blocked => (
            icon_close(px(48.0), text_muted()),
            "Você não bloqueou ninguém.",
            "Pessoas que você bloquear aparecerão aqui para fácil gerenciamento.",
        ),
        _ => (
            icon_friends(px(48.0), text_muted()),
            "Nenhum amigo encontrado ou lista vazia",
            "Quando você adicionar amigos ou receber mensagens diretas, eles aparecerão aqui.",
        ),
    };

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
                .bg(bg_channels())
                .flex()
                .items_center()
                .justify_center()
                .child(empty_icon),
        )
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(font_size_lg())
                .text_color(white())
                .child(empty_title),
        )
        .child(
            div()
                .text_size(font_size_sm())
                .text_color(text_muted())
                .child(empty_desc),
        )
        .into_any_element()
}

/// Circular kit icon button with tooltip (friend-row actions).
/// Hover fill stays theme-driven (ghost): the old green/red fills have no
/// kit equivalent without `cx` for a Custom variant.
fn circle_action_button(
    id: SharedString,
    icon: AnyElement,
    tooltip: SharedString,
    pending: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut btn = KitButton::new(id)
        .ghost()
        .compact()
        .tooltip(tooltip)
        .w(px(36.0))
        .h(px(36.0))
        .rounded(px(18.0))
        .bg(bg_channels())
        .child(icon);
    if pending {
        btn = btn.disabled(true).opacity(0.4);
    } else {
        btn = btn
            .cursor_pointer()
            .on_click(move |_ev, window, cx| action(window, cx));
    }
    btn.into_any_element()
}

/// One friend row: Avatar + names/status + actions by relationship type.
fn render_friend_row(
    entity: Entity<AppState>,
    state: &AppState,
    row: FriendRowData,
    idx: usize,
) -> AnyElement {
    let rel_type = row.relationship_type;
    let friend_uid = row.user_id.clone();

    let mut friend_row = div()
        .id(SharedString::from(format!("friend-row-{}", idx)))
        .w_full()
        .h(px(61.0))
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .border_t_1()
        .border_color(channel_hover())
        .rounded(radius_md());

    if rel_type == 1 {
        let entity_open = entity.clone();
        let uid = row.user_id.clone();
        let uname = row.display_name.clone();
        let cid = row.channel_id.clone();
        friend_row = friend_row
            .cursor_pointer()
            .hover(|s| s.bg(channel_hover()))
            .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                let uid = uid.clone();
                let uname = uname.clone();
                let cid = cid.clone();
                entity_open.update(cx, |state, cx| {
                    if !uid.is_empty() {
                        state.open_dm_with_user(&uid, &uname);
                    } else {
                        state.select_channel(cid);
                    }
                    cx.notify();
                });
            });
    }

    let status_text = match rel_type {
        3 => "Solicitação de amizade recebida".to_string(),
        4 => "Solicitação de amizade enviada".to_string(),
        2 => "Bloqueado".to_string(),
        _ => row.status.clone(),
    };
    let pending = state.pending_relationship_actions.contains(&friend_uid);

    friend_row
        .child(render_friend_row_left(entity.clone(), &row, status_text))
        .child(render_friend_row_actions(entity, &row, idx, pending))
        .into_any_element()
}

/// Left side of a friend row: clickable Avatar + name/handle/status.
fn render_friend_row_left(
    entity: Entity<AppState>,
    row: &FriendRowData,
    status_text: String,
) -> AnyElement {
    let entity_avatar = entity.clone();
    let avatar_uid = row.user_id.clone();
    let entity_name = entity.clone();
    let name_uid = row.user_id.clone();
    let show_status = row.relationship_type == 1;

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                    if !avatar_uid.is_empty() {
                        let pos = ev.position;
                        entity_avatar.update(cx, |s, cx| {
                            s.open_user_profile(&avatar_uid, Some(pos));
                            cx.notify();
                        });
                    }
                })
                .child(
                    Avatar::new(row.avatar_text.clone())
                        .opt_src(row.avatar_url.clone())
                        .size(AvatarSize::Px40)
                        .opt_status(show_status.then(|| domain_presence(row.presence)))
                        .opt_decoration(row.avatar_decoration_data.clone())
                        .bg_color(blurple())
                        .cutout_color(bg_chat()),
                ),
        )
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
                                .text_color(white())
                                .cursor_pointer()
                                .hover(|s| s.underline())
                                .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                                    if !name_uid.is_empty() {
                                        let pos = ev.position;
                                        entity_name.update(cx, |s, cx| {
                                            s.open_user_profile(&name_uid, Some(pos));
                                            cx.notify();
                                        });
                                    }
                                })
                                .child(row.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_muted())
                                .child(row.username.clone()),
                        ),
                )
                .child(
                    div()
                        .text_size(font_size_xs())
                        .text_color(if row.relationship_type == 2 {
                            status_danger()
                        } else {
                            text_muted()
                        })
                        .child(status_text),
                ),
        )
        .into_any_element()
}

/// Right side of a friend row, by relationship type (same actions as today).
fn render_friend_row_actions(
    entity: Entity<AppState>,
    row: &FriendRowData,
    idx: usize,
    pending: bool,
) -> AnyElement {
    match row.relationship_type {
        3 => render_incoming_actions(entity, &row.user_id, idx, pending),
        4 => render_outgoing_actions(entity, &row.user_id, idx, pending),
        2 => render_blocked_actions(entity, &row.user_id, idx, pending),
        _ => render_mutual_actions(entity, row, idx),
    }
}

/// Incoming request: accept (green check) + decline (red close).
fn render_incoming_actions(
    entity: Entity<AppState>,
    user_id: &str,
    idx: usize,
    pending: bool,
) -> AnyElement {
    let accept_uid = user_id.to_string();
    let decline_uid = user_id.to_string();
    let entity_accept = entity.clone();
    let entity_decline = entity.clone();

    div()
        .flex()
        .flex_row()
        .gap(px(8.0))
        .child(circle_action_button(
            SharedString::from(format!("btn-accept-{}", idx)),
            icon_check(px(18.0), status_online()).into_any_element(),
            "Aceitar".into(),
            pending,
            move |_window, cx| {
                let uid = accept_uid.clone();
                entity_accept.update(cx, |s, cx| {
                    s.accept_friend_request(&uid);
                    cx.notify();
                });
            },
        ))
        .child(circle_action_button(
            SharedString::from(format!("btn-decline-{}", idx)),
            icon_close(px(18.0), status_danger()).into_any_element(),
            "Ignorar".into(),
            pending,
            move |_window, cx| {
                let uid = decline_uid.clone();
                entity_decline.update(cx, |s, cx| {
                    s.remove_relationship(&uid);
                    cx.notify();
                });
            },
        ))
        .into_any_element()
}

/// Outgoing request: cancel (X) button.
fn render_outgoing_actions(
    entity: Entity<AppState>,
    user_id: &str,
    idx: usize,
    pending: bool,
) -> AnyElement {
    let cancel_uid = user_id.to_string();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(circle_action_button(
            SharedString::from(format!("btn-cancel-{}", idx)),
            icon_close(px(18.0), text_muted()).into_any_element(),
            "Cancelar pedido".into(),
            pending,
            move |_window, cx| {
                let uid = cancel_uid.clone();
                entity.update(cx, |s, cx| {
                    s.remove_relationship(&uid);
                    cx.notify();
                });
            },
        ))
        .into_any_element()
}

/// Blocked user: "Desbloquear" gray button.
fn render_blocked_actions(
    entity: Entity<AppState>,
    user_id: &str,
    idx: usize,
    pending: bool,
) -> AnyElement {
    let unblock_uid = user_id.to_string();
    let mut unblock = Button::gray(
        SharedString::from(format!("btn-unblock-{}", idx)),
        "Desbloquear",
    )
    .size(ButtonSize::Sm)
    .disabled(pending);
    if !pending {
        unblock = unblock.on_click(move |_ev, _window, cx| {
            let uid = unblock_uid.clone();
            entity.update(cx, |s, cx| {
                s.remove_relationship(&uid);
                cx.notify();
            });
        });
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .child(unblock)
        .into_any_element()
}

/// Mutual friend: DM button + "more" button (both kit, with tooltips).
fn render_mutual_actions(entity: Entity<AppState>, row: &FriendRowData, idx: usize) -> AnyElement {
    let dm_uid = row.user_id.clone();
    let dm_uname = row.display_name.clone();
    let dm_cid = row.channel_id.clone();
    let entity_dm = entity.clone();

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(circle_action_button(
            SharedString::from(format!("friend-dm-btn-{}", idx)),
            icon_chat(px(18.0), text_muted()).into_any_element(),
            "Mensagem".into(),
            false,
            move |_window, cx| {
                let uid = dm_uid.clone();
                let uname = dm_uname.clone();
                let cid = dm_cid.clone();
                entity_dm.update(cx, |state, cx| {
                    if !uid.is_empty() {
                        state.open_dm_with_user(&uid, &uname);
                    } else {
                        state.select_channel(cid);
                    }
                    cx.notify();
                });
            },
        ))
        .child(
            KitButton::new(SharedString::from(format!("friend-more-btn-{}", idx)))
                .ghost()
                .compact()
                .tooltip("Mais")
                .w(px(36.0))
                .h(px(36.0))
                .rounded(px(18.0))
                .bg(bg_channels())
                .text_color(text_muted())
                .child(div().text_size(px(16.0)).child("⋮")),
        )
        .into_any_element()
}

/// Add Friend view: header + static input card w/ send Button + discover row.
pub fn render_add_friend_view(
    entity: Entity<AppState>,
    state: &AppState,
    add_friend_state: &Entity<InputState>,
) -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .px(px(32.0))
        .py(px(24.0))
        .gap(px(16.0))
        .overflow_hidden()
        .child(render_add_friend_header())
        .child(render_add_friend_card(entity.clone(), state, add_friend_state))
        .children(state.add_friend_feedback.clone().map(|(ok, msg)| {
            div()
                .text_size(px(13.0))
                .text_color(if ok {
                    status_online()
                } else {
                    status_danger()
                })
                .child(msg)
        }))
        .child(
            div()
                .w_full()
                .h(px(1.0))
                .bg(bg_chat_hover())
                .my(px(16.0)),
        )
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(14.0))
                .text_color(text_muted())
                .child("OUTRAS FORMAS DE ADICIONAR AMIGOS"),
        )
        .child(render_discover_row())
}

/// "ADICIONAR AMIGO" title + description (same texts as today).
fn render_add_friend_header() -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(16.0))
                .text_color(white())
                .child("ADICIONAR AMIGO"),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(text_muted())
                .child("Você pode adicionar amigos usando o nome de usuário do Discord."),
        )
        .into_any_element()
}

/// Interactive username box bound to TextInput + kit InputState + "Enviar pedido de amizade" Button.
/// Sends only via `send_friend_request`; feedback comes from the API result.
fn render_add_friend_card(
    entity: Entity<AppState>,
    state: &AppState,
    add_friend_state: &Entity<InputState>,
) -> AnyElement {
    let current_input = state.add_friend_input.clone();
    let can_send = !state.is_sending_friend_request && !current_input.trim().is_empty();

    div()
        .w_full()
        .min_h(px(52.0))
        .bg(bg_server())
        .border_1()
        .border_color(bg_chat_hover())
        .rounded(px(8.0))
        .px(px(12.0))
        .py(px(6.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .child(
                    TextInput::new("add-friend-input", add_friend_state)
                        .placeholder("Insira um nome de usuário")
                        .show_paste(false)
                        .show_clear(false),
                ),
        )
        .child(render_send_request_button(entity, current_input, can_send))
        .into_any_element()
}

/// Send button: disabled (opacity .6 per Button) until text exists.
fn render_send_request_button(
    entity: Entity<AppState>,
    target: String,
    can_send: bool,
) -> AnyElement {
    let label = if target.trim().is_empty() || can_send {
        "Enviar pedido de amizade"
    } else {
        "Enviando..."
    };
    let mut btn =
        Button::new("btn-send-friend-request", label).size(ButtonSize::Sm);
    // TOKEN FALTANDO (desabilitado): fundo 0x3c4270 + texto 0x949ba4 do
    // estado sem texto; o Button desabilitado usa opacidade .5 sobre o tema.
    if !can_send && label == "Enviar pedido de amizade" {
        btn = btn.disabled(true);
    } else if can_send {
        btn = btn.on_click(move |_ev, _window, cx| {
            let t = target.clone();
            entity.update(cx, |s, cx| {
                s.send_friend_request(&t);
                cx.notify();
            });
        });
    } else {
        btn = btn.disabled(true);
    }
    btn.into_any_element()
}

/// "Descobrir servidores públicos" row (display-only, as today: no handler).
fn render_discover_row() -> AnyElement {
    div()
        .w_full()
        .h(px(60.0))
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(bg_channels())
        .border_1()
        .border_color(bg_chat_hover())
        .hover(|s| s.bg(channel_hover()).cursor_pointer())
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .size(px(36.0))
                        .rounded_full()
                        .bg(status_online())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_compass(px(20.0), white())),
                )
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(14.0))
                        .text_color(white())
                        .child("Descobrir servidores públicos"),
                ),
        )
        .child(
            div()
                .text_size(px(16.0))
                .text_color(text_muted())
                .child("›"),
        )
        .into_any_element()
}

/// Active Now sidebar (360px): title + empty card or rich-presence cards.
pub fn render_active_now_panel(
    _entity: Entity<AppState>,
    state: &AppState,
) -> Stateful<Div> {
    let mut panel = div()
        .id("active-now-scroll")
        .w(px(360.0))
        .flex_shrink_0()
        .bg(bg_channels())
        .border_l_1()
        .border_color(channel_hover())
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(16.0))
        .overflow_y_scroll()
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(20.0))
                .text_color(text_titles())
                .child("Ativo agora"),
        );

    if state.active_now_items.is_empty() {
        return panel.child(render_active_now_empty());
    }
    for item in &state.active_now_items {
        panel = panel.child(render_active_now_card(item));
    }
    panel
}

/// "Por enquanto, está quieto..." empty card (same texts as today).
fn render_active_now_empty() -> AnyElement {
    div()
        .w_full()
        .p(px(16.0))
        .rounded(px(16.0))
        .bg(bg_channels())
        .border_1()
        .border_color(bg_chat_hover())
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(16.0))
                .text_color(white())
                .child("Por enquanto, está quieto..."),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(text_muted())
                .child("Quando seus amigos estiverem ativos em jogos ou ouvindo músicas, você verá as atividades deles aqui!"),
        )
        .into_any_element()
}

/// One rich-presence card: header (Avatar+names+app icon), details box,
/// optional display-only action label (no state action exists for it).
fn render_active_now_card(item: &ActiveNowItem) -> AnyElement {
    div()
        .w_full()
        .p(px(16.0))
        .rounded(px(16.0))
        .bg(bg_server())
        .border_1()
        .border_color(bg_chat_hover())
        .hover(|s| s.border_color(borders()))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(render_active_now_card_header(item))
        .child(render_active_now_details(item))
        .children(item.button_text.as_ref().map(|btn_label| {
            div()
                .w_full()
                .h(px(32.0))
                .rounded(px(4.0))
                .bg(blurple())
                .hover(|s| s.bg(blurple_hover()))
                .flex()
                .items_center()
                .justify_center()
                .text_color(white())
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .child(btn_label.clone())
        }))
        .into_any_element()
}

/// Card header: Avatar with status + name/subtitle + top-right app icon.
fn render_active_now_card_header(item: &ActiveNowItem) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    Avatar::new(item.avatar_text.clone())
                        .opt_src(item.avatar_url.clone())
                        .size(AvatarSize::Px32)
                        .opt_status(Some(domain_presence(item.presence)))
                        .bg_color(blurple())
                        .cutout_color(bg_server()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .text_color(white())
                                .child(item.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(legacy_949ba4())
                                .child(item.header_subtitle.clone()),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .child(match item.kind {
                    ActiveNowKind::Quest | ActiveNowKind::Game => {
                        icon_gamepad(px(16.0), legacy_949ba4())
                    }
                    ActiveNowKind::Spotify => icon_music(px(16.0), status_online()),
                    ActiveNowKind::RichPresence => {
                        icon_screen_share(px(16.0), legacy_949ba4())
                    }
                }),
        )
        .into_any_element()
}

/// Inner details box: big app icon + title/state/elapsed + Spotify progress.
fn render_active_now_details(item: &ActiveNowItem) -> AnyElement {
    div()
        .w_full()
        .p(px(12.0))
        .rounded(px(8.0))
        .bg(bg_input())
        .border_1()
        .border_color(bg_user_bar())
        .flex()
        .flex_row()
        .gap(px(12.0))
        .child(render_active_now_app_icon(item))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .overflow_hidden()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(14.0))
                        .text_color(text_titles())
                        .child(item.details_title.clone()),
                )
                .children(item.details_state.as_ref().map(|state_text| {
                    div()
                        .text_size(px(12.0))
                        .text_color(legacy_949ba4())
                        .child(state_text.clone())
                }))
                .children(item.elapsed_time_text.as_ref().map(|elapsed| {
                    div()
                        .text_size(px(12.0))
                        .text_color(text_muted())
                        .child(elapsed.clone())
                }))
                .children(render_spotify_progress(item)),
        )
        .into_any_element()
}

/// Big 48px app icon or album cover, rendered from real assets, with fallback to kind vector icon.
fn render_active_now_app_icon(item: &ActiveNowItem) -> AnyElement {
    if let Some(ref url) = item.large_image_url {
        let mut base = div()
            .relative()
            .size(px(48.0))
            .rounded(px(8.0))
            .overflow_hidden()
            .flex_shrink_0()
            .child(
                img(url.clone())
                    .size_full()
                    .rounded(px(8.0)),
            );

        if let Some(ref small_url) = item.small_image_url {
            base = base.child(
                div()
                    .absolute()
                    .bottom(px(-2.0))
                    .right(px(-2.0))
                    .size(px(20.0))
                    .rounded_full()
                    .border_2()
                    .border_color(bg_input())
                    .overflow_hidden()
                    .child(
                        img(small_url.clone())
                            .size_full()
                            .rounded_full(),
                    ),
            );
        }

        base.into_any_element()
    } else {
        // Sem imagem: exibe o ícone oficial da atividade sobre fundo neutro do Discord.
        div()
            .size(px(48.0))
            .rounded(px(8.0))
            .bg(bg_channels())
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .child(match item.kind {
                ActiveNowKind::Quest | ActiveNowKind::Game => {
                    icon_gamepad(px(24.0), text_muted()).into_any_element()
                }
                ActiveNowKind::Spotify => {
                    icon_music(px(24.0), status_online()).into_any_element()
                }
                ActiveNowKind::RichPresence => {
                    icon_screen_share(px(24.0), text_muted()).into_any_element()
                }
            })
            .into_any_element()
    }
}

/// Helper to compute Spotify progress text and ratio from timestamps.
fn compute_spotify_progress(
    start_ms: i64,
    end_ms: i64,
    now_ms: i64,
) -> Option<((String, String), f32)> {
    if end_ms <= start_ms {
        return None;
    }
    let total_ms = end_ms - start_ms;
    let elapsed_ms = (now_ms - start_ms).clamp(0, total_ms);
    let ratio = (elapsed_ms as f32 / total_ms as f32).clamp(0.0, 1.0);

    let cur_secs = elapsed_ms / 1000;
    let total_secs = total_ms / 1000;

    let cur_text = format!("{}:{:02}", cur_secs / 60, cur_secs % 60);
    let total_text = format!("{}:{:02}", total_secs / 60, total_secs % 60);

    Some(((cur_text, total_text), ratio))
}

/// Spotify progress row dynamically computed from timestamps (start/end).
/// Without real timestamps or progress data, no progress bar is shown (zero mocks).
fn render_spotify_progress(item: &ActiveNowItem) -> Option<AnyElement> {
    if item.kind != ActiveNowKind::Spotify {
        return None;
    }

    let (cur, total, progress_ratio) = if let (Some(start), Some(end)) = (item.timestamps_start, item.timestamps_end) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let start_ms = if start < 100_000_000_000 { start * 1000 } else { start };
        let end_ms = if end < 100_000_000_000 { end * 1000 } else { end };
        if let Some(((cur, total), ratio)) = compute_spotify_progress(start_ms, end_ms, now_ms) {
            (cur, total, ratio)
        } else {
            return None;
        }
    } else if let (Some(progress_text), Some(ratio)) = (&item.spotify_progress_text, item.spotify_progress_pct) {
        (progress_text.0.clone(), progress_text.1.clone(), ratio)
    } else {
        return None;
    };

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
                    .text_color(text_muted())
                    .child(cur),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(4.0))
                    .bg(channel_active())
                    .rounded_full()
                    .relative()
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress_ratio))
                            .bg(status_online())
                            .rounded_full(),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(text_muted())
                    .child(total),
            )
            .into_any_element(),
    )
}

/// Helper to check if any active Spotify item with timestamps is present in state.
pub fn has_spotify_with_timestamps(state: &AppState) -> bool {
    state.active_now_items.iter().any(|item| {
        item.kind == ActiveNowKind::Spotify
            && item.timestamps_start.is_some()
            && item.timestamps_end.is_some()
    })
}

/// Synchronizes a 1-second GPUI timer task for dynamic Spotify progress updates.
///
/// When `state.has_visible_spotify_progress()` is true:
/// If `timer_slot` is empty, spawns an asynchronous periodic task via GPUI's executor
/// that sleeps for 1 second (`cx.background_executor().timer(1s).await`), calls `tick_spotify_progress()`,
/// and emits `cx.notify()`.
///
/// When `state.has_visible_spotify_progress()` is false (either no Spotify items with timestamps
/// exist or the Friends Hub is not visible):
/// Sets `*timer_slot = None`, which drops the `Task<()>`, cleanly cancelling the GPUI task
/// without running an external thread.
pub fn sync_spotify_progress_timer<V: 'static>(
    timer_slot: &mut Option<Task<()>>,
    state: &AppState,
    app_entity: &Entity<AppState>,
    cx: &mut Context<V>,
) {
    if state.has_visible_spotify_progress() {
        if timer_slot.is_none() {
            let app_weak = app_entity.downgrade();
            let task = cx.spawn(|_view, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_secs(1))
                            .await;
                        let keep_going = cx.update(|cx| {
                            app_weak
                                .update(cx, |state, cx| {
                                    if state.has_visible_spotify_progress() {
                                        state.tick_spotify_progress();
                                        cx.notify();
                                        true
                                    } else {
                                        false
                                    }
                                })
                                .unwrap_or(false)
                        });

                        if !keep_going {
                            break;
                        }
                    }
                }
            });
            *timer_slot = Some(task);
        }
    } else {
        *timer_slot = None;
    }
}
