//! Discord RS - Cozy Message List History
//! Implements canonical Discord Cozy style message history:
//! - 5-minute author grouping window
//! - User avatars with crate::components::Avatar (support for decorations & CDN URLs)
//! - Role-colored usernames and BOT badges
//! - Timestamps and date separators ("Hoje", "Ontem", "22 de Setembro de 2026")
//! - Floating hover action bar with kit IconButton and tooltips
//! - Kit PopupMenu message context menu
//! - Attachments & rich embed rendering
//! - Embedded reactions bar

use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, Utc};
use gpui::component::menu::{PopupMenu, PopupMenuItem};
use gpui::component::tooltip::Tooltip;
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::chat::markdown::render_markdown;
use crate::chat::polls::render_poll_card;
use crate::chat::reactions::{render_reaction_bar, AddReactionCallback, ReactionActionCallback};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::IconButton;
use crate::domain::message::{Attachment, Embed, Message};
use crate::tokens::colors::white;
use crate::tokens::colors::*;
use crate::tokens::icons::icon_smile;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

// ============================================================================
// Callback Type Aliases
// ============================================================================

pub type MessageActionCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;
pub type MessageActionPosCallback = Arc<dyn Fn(&str, Point<Pixels>, &mut Window, &mut App) + Send + Sync + 'static>;

// ============================================================================
// Date & Time Formatting Helpers (pt-BR Friendly)
// ============================================================================

/// Returns month name in Portuguese.
fn pt_month_name(month: u32) -> &'static str {
    match month {
        1 => "Janeiro",
        2 => "Fevereiro",
        3 => "Março",
        4 => "Abril",
        5 => "Maio",
        6 => "Junho",
        7 => "Julho",
        8 => "Agosto",
        9 => "Setembro",
        10 => "Outubro",
        11 => "Novembro",
        12 => "Dezembro",
        _ => "",
    }
}

/// Formats a date for the central date divider with an explicit 'today' reference date.
pub fn format_date_separator_with_today(date: NaiveDate, today: NaiveDate) -> String {
    let diff = today.signed_duration_since(date).num_days();

    if diff == 0 {
        "Hoje".to_string()
    } else if diff == 1 {
        "Ontem".to_string()
    } else {
        format!("{} de {} de {}", date.day(), pt_month_name(date.month()), date.year())
    }
}

/// Formats a date for the central date divider using local machine timezone.
pub fn format_date_separator(date: NaiveDate) -> String {
    let today = Local::now().date_naive();
    format_date_separator_with_today(date, today)
}

/// Formats message timestamp with explicit FixedOffset and reference local now time.
/// E.g. "Hoje às 14:32" or "Ontem às 14:32" or "22/09/2026 às 14:32".
pub fn format_message_timestamp_with_offset(
    dt: DateTime<Utc>,
    offset: FixedOffset,
    now: DateTime<FixedOffset>,
) -> String {
    let local_dt = dt.with_timezone(&offset);
    let today = now.date_naive();
    let msg_date = local_dt.date_naive();
    let diff = today.signed_duration_since(msg_date).num_days();
    let time_str = local_dt.format("%H:%M").to_string();

    if diff == 0 {
        format!("Hoje às {}", time_str)
    } else if diff == 1 {
        format!("Ontem às {}", time_str)
    } else {
        format!("{} às {}", local_dt.format("%d/%m/%Y"), time_str)
    }
}

/// Formats message timestamp: "Hoje às 14:32" or "Ontem às 14:32" or "22/09/2026 às 14:32" in local timezone.
pub fn format_message_timestamp(dt: DateTime<Utc>) -> String {
    let local_now = Local::now();
    let offset = *local_now.offset();
    format_message_timestamp_with_offset(dt, offset, local_now.with_timezone(&offset))
}

/// Returns weekday name in Portuguese.
fn pt_weekday_name(wd: chrono::Weekday) -> &'static str {
    match wd {
        chrono::Weekday::Mon => "segunda-feira",
        chrono::Weekday::Tue => "terça-feira",
        chrono::Weekday::Wed => "quarta-feira",
        chrono::Weekday::Thu => "quinta-feira",
        chrono::Weekday::Fri => "sexta-feira",
        chrono::Weekday::Sat => "sábado",
        chrono::Weekday::Sun => "domingo",
    }
}

/// Formats full message timestamp for hover tooltip with explicit FixedOffset (canonical Discord format: "domingo, 11 de outubro de 2026 às 15:30").
pub fn format_message_full_timestamp_with_offset(
    dt: DateTime<Utc>,
    offset: FixedOffset,
) -> String {
    let local_dt = dt.with_timezone(&offset);
    let weekday = pt_weekday_name(local_dt.weekday());
    let month = pt_month_name(local_dt.month()).to_lowercase();
    let time_str = local_dt.format("%H:%M").to_string();
    format!("{}, {} de {} de {} às {}", weekday, local_dt.day(), month, local_dt.year(), time_str)
}

/// Formats full message timestamp for hover tooltip in local timezone (canonical Discord format: "domingo, 11 de outubro de 2026 às 15:30").
pub fn format_message_full_timestamp(dt: DateTime<Utc>) -> String {
    let local_now = Local::now();
    let offset = *local_now.offset();
    format_message_full_timestamp_with_offset(dt, offset)
}

/// Formats compact gutter time with explicit FixedOffset ("HH:MM").
pub fn format_compact_gutter_time_with_offset(dt: DateTime<Utc>, offset: FixedOffset) -> String {
    dt.with_timezone(&offset).format("%H:%M").to_string()
}

/// Formats compact gutter time in local timezone ("HH:MM").
pub fn format_compact_gutter_time(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local).format("%H:%M").to_string()
}

/// Formats a file size in bytes to a human-readable string.
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ============================================================================
// Date Separator Component
// ============================================================================

/// Renders a horizontal divider with a centered date pill.
///
/// Geometry follows Discord's current `.divider_4TfzT5` / `.content_4TfzT5`:
/// a zero-height flex rule with a 12px semibold label on the chat background.
pub fn render_date_separator_element(date: NaiveDate) -> Div {
    let date_text = format_date_separator(date);

    div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h_px()
        .border_t_1()
        .border_color(border_subtle())
        .child(
            div()
                .relative()
                .bg(bg_chat())
                .px(space_2())
                .py(space_1())
                .pl(space_4())
                .rounded(radius_md())
                .text_size(font_size_xs())
                .font_weight(WEIGHT_SEMIBOLD)
                .line_height(px(13.0))
                .text_color(channels_default())
                .child(date_text),
        )
}

/// Renders Discord's unread divider: a red rule with the `NOVAS` end cap
/// aligned to the inline-end edge (`.isUnread_4TfzT5` / `.endCap_4TfzT5`).
///
/// The caller owns the unread boundary because the current `Message` model does
/// not carry a read-state marker. No divider is invented when that boundary is
/// unknown.
pub fn render_new_messages_divider() -> Div {
    div()
        .relative()
        .flex()
        .items_center()
        .justify_end()
        .w_full()
        .h_px()
        .border_t_1()
        .border_color(status_danger())
        .child(
            div()
                .absolute()
                .top(px(-7.0))
                .right(px(0.0))
                .h(px(13.0))
                .px(px(4.0))
                .pr(px(1.0))
                .bg(status_danger())
                .rounded_tl(radius_sm())
                .rounded_bl(radius_sm())
                .flex()
                .items_center()
                .justify_center()
                .text_size(font_size_xxs())
                .font_weight(WEIGHT_BOLD)
                .line_height(px(9.0))
                .text_color(white())
                .child("NOVAS"),
        )
}

// ============================================================================
// Hover Action Bar Component
// ============================================================================

/// Renders the floating action bar displayed on message hover (React, Reply, More).
///
/// Kept as the three-action compatibility entry point for callers that do not
/// expose an edit callback. Own messages should use
/// [`render_hover_action_bar_with_edit`] so the canonical Edit action is shown.
pub fn render_hover_action_bar(
    message_id: &str,
    on_react: Option<MessageActionCallback>,
    on_reply: Option<MessageActionCallback>,
    on_more: Option<MessageActionPosCallback>,
) -> Div {
    render_hover_action_bar_with_edit(message_id, on_react, on_reply, None, on_more)
}

/// Renders the canonical hover action order: React, Reply, Edit (own messages
/// only), More. The edit button is omitted when no callback is available.
pub fn render_hover_action_bar_with_edit(
    message_id: &str,
    on_react: Option<MessageActionCallback>,
    on_reply: Option<MessageActionCallback>,
    on_edit: Option<MessageActionCallback>,
    on_more: Option<MessageActionPosCallback>,
) -> Div {
    let msg_id_react = message_id.to_string();
    let msg_id_reply = message_id.to_string();
    let msg_id_edit = message_id.to_string();
    let msg_id_more = message_id.to_string();

    let mut react_btn = IconButton::with_icon(
        format!("hover-react-{}", message_id),
        |sz, col| icon_smile(sz, col).into_any_element(),
    )
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .tooltip("Adicionar reação");

    if let Some(handler) = on_react {
        react_btn = react_btn.on_click(move |_ev, window, cx| {
            handler(&msg_id_react, window, cx);
        });
    }

    let mut reply_btn = IconButton::with_icon(
        format!("hover-reply-{}", message_id),
        |sz, col| svg().path("icons/undo-2.svg").size(sz).text_color(col).into_any_element(),
    )
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .tooltip("Responder");

    if let Some(handler) = on_reply {
        reply_btn = reply_btn.on_click(move |_ev, window, cx| {
            handler(&msg_id_reply, window, cx);
        });
    }

    let mut action_bar = div()
        .absolute()
        .top(px(-16.0))
        .right(px(16.0))
        .bg(bg_channels())
        .border_1()
        .border_color(border_subtle())
        .rounded(radius_md())
        .shadow_md()
        .p(px(2.0))
        .flex()
        .flex_row()
        .items_center()
        .invisible()
        .group_hover("message-row", |s| s.visible())
        .child(react_btn)
        .child(reply_btn);

    if let Some(handler) = on_edit {
        let edit_btn = IconButton::with_icon(
            format!("hover-edit-{}", message_id),
            |sz, col| svg().path("icons/pencil.svg").size(sz).text_color(col).into_any_element(),
        )
        .size(px(32.0))
        .icon_size(px(20.0))
        .rounded(radius_sm())
        .tooltip("Editar")
        .on_click(move |_ev, window, cx| {
            handler(&msg_id_edit, window, cx);
        });
        action_bar = action_bar.child(edit_btn);
    }

    let mut more_btn = IconButton::with_icon(
        format!("hover-more-{}", message_id),
        |sz, col| svg().path("icons/ellipsis.svg").size(sz).text_color(col).into_any_element(),
    )
    .size(px(32.0))
    .icon_size(px(20.0))
    .rounded(radius_sm())
    .tooltip("Mais");

    if let Some(handler) = on_more {
        more_btn = more_btn.on_click(move |event, window, cx| {
            handler(&msg_id_more, event.position(), window, cx);
        });
    }

    action_bar.child(more_btn)
}

// ============================================================================
// Message Context Menu Component (Kit PopupMenu & ContextMenu)
// ============================================================================

/// Configuration for the message contextual dropdown menu.
pub struct MessageContextMenuConfig {
    pub message_id: String,
    pub message_content: String,
    pub is_pinned: bool,
    pub is_own: bool,
    pub on_add_reaction: Option<MessageActionCallback>,
    pub on_edit: Option<MessageActionCallback>,
    pub on_reply: Option<MessageActionCallback>,
    pub on_copy_text: Option<MessageActionCallback>,
    pub on_copy_link: Option<MessageActionCallback>,
    pub on_copy_id: Option<MessageActionCallback>,
    pub on_mark_unread: Option<MessageActionCallback>,
    pub on_pin: Option<MessageActionCallback>,
    pub on_delete: Option<MessageActionCallback>,
    pub on_delete_shift: Option<MessageActionCallback>,
    pub on_close: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
}

/// Builds kit PopupMenu with canonical Discord message actions.
pub fn build_message_popup_menu(
    config: MessageContextMenuConfig,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu {
    move |mut menu, _window, _cx| {
        let msg_id = config.message_id.clone();
        let is_pinned = config.is_pinned;
        let is_own = config.is_own;
        let pin_label = if is_pinned {
            "Desafixar Mensagem"
        } else {
            "Fixar Mensagem"
        };

        if let Some(act) = config.on_add_reaction.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Adicionar Reação")
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        if is_own {
            if let Some(act) = config.on_edit.clone() {
                let mid = msg_id.clone();
                menu = menu.item(
                    PopupMenuItem::new("Editar Mensagem")
                        .on_click(move |_ev, window, cx| act(&mid, window, cx)),
                );
            }
        }
        if let Some(act) = config.on_reply.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Responder")
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        menu = menu.separator();
        if let Some(act) = config.on_copy_text.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Copiar Texto")
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        if let Some(act) = config.on_copy_link.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Copiar Link da Mensagem")
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        if let Some(act) = config.on_mark_unread.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Marcar como Não Lida")
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        if let Some(act) = config.on_pin.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new(pin_label)
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        if is_own {
            menu = menu.separator();
            if let Some(act) = config.on_delete.clone() {
                let mid = msg_id.clone();
                menu = menu.item(
                    PopupMenuItem::new("Excluir Mensagem")
                        .on_click(move |_ev, window, cx| act(&mid, window, cx)),
                );
            }
        }
        menu = menu.separator();
        if let Some(act) = config.on_copy_id.clone() {
            let mid = msg_id.clone();
            menu = menu.item(
                PopupMenuItem::new("Copiar ID da Mensagem")
                    .on_click(move |_ev, window, cx| act(&mid, window, cx)),
            );
        }
        menu.min_w(px(222.0))
    }
}

/// Helper to render an item row inside the message context menu.
fn render_menu_item(
    label: &str,
    icon: &str,
    is_danger: bool,
    action: Option<MessageActionCallback>,
    action_shift: Option<MessageActionCallback>,
    message_id: &str,
    on_close: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync + 'static>>,
) -> Div {
    let msg_id = message_id.to_string();
    let label_str = label.to_string();
    let icon_str = icon.to_string();
    let is_disabled = action.is_none() && action_shift.is_none();

    let mut item = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(space_2())
        .py(px(6.0))
        .rounded(radius_xs());

    if is_disabled {
        item = item
            .text_color(text_muted())
            .opacity(0.4)
            .cursor_not_allowed();
    } else {
        item = item.cursor_pointer();
        if is_danger {
            item = item
                .text_color(control_critical_secondary_text_default())
                .hover(|s| s.bg(status_danger()).text_color(white()));
        } else {
            item = item
                .text_color(text_primary())
                .hover(|s| s.bg(blurple()).text_color(white()));
        }
    }

    item = item
        .child(
            div()
                .text_size(font_size_sm())
                .font_weight(WEIGHT_NORMAL)
                .child(label_str),
        )
        .child(
            div()
                .text_size(font_size_sm())
                .opacity(0.8)
                .child(icon_str),
        );

    if !is_disabled && (action.is_some() || action_shift.is_some() || on_close.is_some()) {
        item = item.on_mouse_down(MouseButton::Left, move |ev, window, cx| {
            cx.stop_propagation();
            if ev.modifiers.shift {
                if let Some(ref act_s) = action_shift {
                    act_s(&msg_id, window, cx);
                } else if let Some(ref act) = action {
                    act(&msg_id, window, cx);
                }
            } else if let Some(ref act) = action {
                act(&msg_id, window, cx);
            }
            if let Some(ref close) = on_close {
                close(window, cx);
            }
        });
    }

    item
}

/// Renders canonical Discord message context menu popup (222px width, 8px radius, floating background).
pub fn render_message_context_menu(config: MessageContextMenuConfig) -> Div {
    let pin_label = if config.is_pinned {
        "Desafixar Mensagem"
    } else {
        "Fixar Mensagem"
    };

    let mut menu = div()
        .w(px(222.0))
        .bg(bg_floating())
        .border_1()
        .border_color(bg_user_bar())
        .rounded(radius_md())
        .shadow_lg()
        .p(px(6.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        // 1. Reagir
        .child(render_menu_item(
            "Adicionar Reação",
            "☺",
            false,
            config.on_add_reaction,
            None,
            &config.message_id,
            config.on_close.clone(),
        ));

    // 2. Editar Mensagem (apenas mensagem própria)
    if config.is_own {
        menu = menu.child(render_menu_item(
            "Editar Mensagem",
            "✏",
            false,
            config.on_edit,
            None,
            &config.message_id,
            config.on_close.clone(),
        ));
    }

    // 3. Responder
    menu = menu
        .child(div().h_px().bg(bg_user_bar()).my(px(3.0)))
        .child(render_menu_item(
            "Responder",
            "↩",
            false,
            config.on_reply,
            None,
            &config.message_id,
            config.on_close.clone(),
        ))
        .child(div().h_px().bg(bg_user_bar()).my(px(3.0)))
        // 4. Copiar Texto
        .child(render_menu_item(
            "Copiar Texto",
            "📋",
            false,
            config.on_copy_text,
            None,
            &config.message_id,
            config.on_close.clone(),
        ));

    // 5. Copiar Link da Mensagem (se canal/guild conhecidos)
    if config.on_copy_link.is_some() {
        menu = menu.child(render_menu_item(
            "Copiar Link da Mensagem",
            "🔗",
            false,
            config.on_copy_link,
            None,
            &config.message_id,
            config.on_close.clone(),
        ));
    }

    // 6. Marcar como Não Lida (desabilitado até rota existir no backend)
    menu = menu.child(render_menu_item(
        "Marcar como Não Lida",
        "✉",
        false,
        config.on_mark_unread,
        None,
        &config.message_id,
        config.on_close.clone(),
    ));

    // 7. Fixar Mensagem (desabilitado se on_pin for None até rota existir no backend)
    menu = menu.child(render_menu_item(
        pin_label,
        "📌",
        false,
        config.on_pin,
        None,
        &config.message_id,
        config.on_close.clone(),
    ));

    // 8. Excluir Mensagem (apenas mensagem própria)
    if config.is_own {
        menu = menu
            .child(div().h_px().bg(bg_user_bar()).my(px(3.0)))
            .child(render_menu_item(
                "Excluir Mensagem",
                "🗑️",
                true,
                config.on_delete,
                config.on_delete_shift,
                &config.message_id,
                config.on_close.clone(),
            ));
    }

    // 9. Copiar ID da Mensagem
    menu = menu
        .child(div().h_px().bg(bg_user_bar()).my(px(3.0)))
        .child(render_menu_item(
            "Copiar ID da Mensagem",
            "🆔",
            false,
            config.on_copy_id,
            None,
            &config.message_id,
            config.on_close,
        ));

    menu
}

// ============================================================================
// Attachment & Embed Renderers
// ============================================================================

/// Renders message attachments (images or files).
pub fn render_attachments(attachments: &[Attachment]) -> Div {
    if attachments.is_empty() {
        return div();
    }

    let mut container = div()
        .flex()
        .flex_col()
        .gap(space_2())
        .my(space_2());

    for att in attachments {
        if att.is_image() {
            // Image attachment preview container
            container = container.child(
                div()
                    .flex()
                    .flex_col()
                    .max_w(px(400.0))
                    .bg(bg_channels())
                    .border_1()
                    .border_color(bg_server())
                    .rounded(radius_md())
                    .p(space_2())
                    .gap(space_1())
                    .child(
                        div()
                            .h(px(200.0))
                            .w_full()
                            .bg(bg_user_bar())
                            .rounded(radius_sm())
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(text_muted())
                            .text_size(font_size_sm())
                            .child(format!("🖼️ {}", att.filename)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .text_size(font_size_xs())
                            .text_color(text_muted())
                            .child(att.filename.clone())
                            .child(format_file_size(att.size)),
                    ),
            );
        } else {
            // General file attachment card
            container = container.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(space_3())
                    .max_w(px(400.0))
                    .p(space_3())
                    .bg(bg_channels())
                    .border_1()
                    .border_color(bg_server())
                    .rounded(radius_md())
                    .child(
                        div()
                            .text_size(px(24.0))
                            .child("📄"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(font_size_sm())
                                    .font_weight(WEIGHT_MEDIUM)
                                    .text_color(text_titles())
                                    .child(att.filename.clone()),
                            )
                            .child(
                                div()
                                    .text_size(font_size_xs())
                                    .text_color(text_muted())
                                    .child(format_file_size(att.size)),
                            ),
                    ),
            );
        }
    }

    container
}

/// Renders a clean fallback sticker card in chat history when image CDN is not available.
pub fn render_sticker_card(sticker_name: &str) -> Div {
    div()
        .my(space_2())
        .p(space_2())
        .max_w(px(220.0))
        .bg(bg_channels())
        .border_1()
        .border_color(bg_server())
        .rounded(radius_md())
        .flex()
        .items_center()
        .gap(space_2())
        .child(
            div()
                .text_size(px(18.0))
                .child("🏷️"),
        )
        .child(
            div()
                .text_size(font_size_sm())
                .font_weight(WEIGHT_MEDIUM)
                .text_color(text_titles())
                .child(sticker_name.to_string()),
        )
}

/// Renders a native sticker image (160x160 CDN URL) or a clean fallback card in chat history.
pub fn render_sticker(
    sticker_id: Option<&str>,
    sticker_name: &str,
    format_type: Option<u8>,
) -> Div {
    if let Some(id) = sticker_id {
        if !id.is_empty() {
            let is_animated = matches!(format_type, Some(2) | Some(4));
            let cdn_url = if is_animated {
                format!(
                    "https://media.discordapp.net/stickers/{}.gif?size=160",
                    id
                )
            } else {
                format!(
                    "https://media.discordapp.net/stickers/{}.webp?size=160&quality=lossless",
                    id
                )
            };
            return div()
                .my(space_2())
                .child(
                    img(cdn_url)
                        .id(ElementId::from(SharedString::from(format!("msg-sticker-img-{}", id))))
                        .size(px(160.0))
                        .rounded(radius_md())
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9)),
                );
        }
    }

    render_sticker_card(sticker_name)
}

/// Renders Discord rich embeds.
pub fn render_embeds(embeds: &[Embed]) -> Div {
    if embeds.is_empty() {
        return div();
    }

    let mut container = div()
        .flex()
        .flex_col()
        .gap(space_2())
        .my(space_2());

    for embed in embeds {
        let stripe_color = embed.color.map(from_hex).unwrap_or_else(blurple);

        let mut embed_card = div()
            .flex()
            .flex_col()
            .max_w(px(520.0))
            .bg(bg_channels())
            .border_l_4()
            .border_color(stripe_color)
            .rounded(radius_sm())
            .p(space_4())
            .gap(space_2());

        // Embed Author
        if let Some(author) = &embed.author {
            embed_card = embed_card.child(
                div()
                    .text_size(font_size_xs())
                    .font_weight(WEIGHT_MEDIUM)
                    .text_color(text_muted())
                    .child(author.name.clone()),
            );
        }

        // Embed Title
        if let Some(title) = &embed.title {
            embed_card = embed_card.child(
                div()
                    .text_size(font_size_base())
                    .font_weight(WEIGHT_BOLD)
                    .text_color(blurple())
                    .child(title.clone()),
            );
        }

        // Embed Description
        if let Some(desc) = &embed.description {
            embed_card = embed_card.child(render_markdown(desc));
        }

        // Embed Fields
        if !embed.fields.is_empty() {
            let mut fields_row = div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(space_4())
                .my(space_1());

            for field in &embed.fields {
                let w = if field.inline { px(150.0) } else { px(480.0) };
                fields_row = fields_row.child(
                    div()
                        .flex()
                        .flex_col()
                        .w(w)
                        .child(
                            div()
                                .text_size(font_size_xs())
                                .font_weight(WEIGHT_BOLD)
                                .text_color(text_muted())
                                .child(field.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(font_size_sm())
                                .text_color(text_primary())
                                .child(field.value.clone()),
                        ),
                );
            }
            embed_card = embed_card.child(fields_row);
        }

        // Embed Footer
        if let Some(footer) = &embed.footer {
            embed_card = embed_card.child(
                div()
                    .text_size(font_size_xxs())
                    .text_color(text_muted())
                    .child(footer.text.clone()),
            );
        }

        container = container.child(embed_card);
    }

    container
}

// ============================================================================
// Reply Preview
// ============================================================================

/// Returns real message content for a one-line preview. Empty-content media
/// messages fall back to metadata returned by Discord; no placeholder text is
/// fabricated when the payload has no displayable content.
fn reply_preview_text(message: &Message) -> Option<String> {
    let content = message.content.trim();
    if !content.is_empty() {
        return Some(content.to_string());
    }
    if let Some(attachment) = message.attachments.first() {
        return Some(attachment.filename.clone());
    }
    if let Some(sticker) = message.sticker_items.first() {
        return Some(sticker.name.clone());
    }
    message
        .embeds
        .iter()
        .find_map(|embed| embed.title.clone())
}

/// Reply references use a 16px avatar in the current Discord Cozy layout. The
/// shared Avatar component starts at 24px, so this small renderer keeps the
/// real CDN avatar/decoration without scaling or inventing an identity.
fn render_reply_avatar(reply: &Message) -> AnyElement {
    let inner = if let Some(url) = reply.author.avatar_url() {
        div()
            .size_full()
            .rounded_full()
            .overflow_hidden()
            .child(img(url).size_full().rounded_full())
    } else {
        let initial = reply
            .author
            .display_name()
            .chars()
            .next()
            .map(|ch| ch.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string());
        div()
            .size_full()
            .rounded_full()
            .bg(bg_channels())
            .flex()
            .items_center()
            .justify_center()
            .text_color(text_muted())
            .text_size(px(9.0))
            .font_weight(WEIGHT_BOLD)
            .child(initial)
    };

    let mut avatar = div()
        .relative()
        .w(px(16.0))
        .h(px(16.0))
        .flex_shrink_0()
        .rounded_full()
        .child(inner);

    if let Some(decoration) = &reply.author.avatar_decoration_data {
        avatar = avatar.child(
            div()
                .absolute()
                .top(px(-1.6))
                .left(px(-1.6))
                .w(px(19.2))
                .h(px(19.2))
                .child(img(decoration.url()).size_full()),
        );
    }

    avatar.into_any_element()
}

/// Draws the Cozy reply spine from the parent avatar center to the 16px reply
/// avatar. The 36px offset is `40px avatar / 2 + 16px gutter`, matching the
/// current Discord `--custom-message-margin-left-content-cozy` geometry.
fn render_reply_spine() -> AnyElement {
    div()
        .absolute()
        .left(px(-36.0))
        .top(px(8.0))
        .w(px(38.0))
        .h(px(10.0))
        .border_l_2()
        .border_t_2()
        .border_color(borders())
        .rounded_tl(px(6.0))
        .into_any_element()
}

/// Renders the compact reply reference shown above a message, including the
/// curved connector used by Discord's Cozy layout.
pub fn render_reply_preview(reply: &Message, role_color: Option<u32>) -> Div {
    let author_name = reply.author.display_name().to_string();
    let author_color = role_color
        .filter(|color| *color != 0)
        .map(from_hex)
        .unwrap_or_else(text_titles);
    let preview_text = reply_preview_text(reply);

    let mut summary = div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(space_2())
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .text_color(author_color)
                .font_weight(WEIGHT_SEMIBOLD)
                .text_size(font_size_sm())
                .line_height(line_height_sm())
                .opacity(0.64)
                .truncate()
                .child(author_name),
        );

    if let Some(text) = preview_text {
        summary = summary.child(
            div()
                .text_color(text_primary())
                .text_size(font_size_sm())
                .line_height(line_height_sm())
                .truncate()
                .child(text),
        );
    }

    div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(space_2())
        .w_full()
        .min_w(px(0.0))
        .mb(space_2())
        .child(render_reply_spine())
        .child(render_reply_avatar(reply))
        .child(summary)
}

// ============================================================================
// Single Message Row (Full & Grouped)
// ============================================================================

/// Configuration for rendering a message row.
pub struct MessageRowConfig {
    pub is_grouped: bool,
    pub role_color: Option<u32>,
    pub on_react: Option<MessageActionCallback>,
    pub on_reply: Option<MessageActionCallback>,
    pub on_more: Option<MessageActionPosCallback>,
    pub on_reaction_click: Option<ReactionActionCallback>,
    pub on_add_reaction: Option<AddReactionCallback>,
}

/// Renders an individual message item in Cozy format.
pub fn render_message_row(message: &Message, config: MessageRowConfig) -> Div {
    render_message_row_with_reply_and_edit(message, config, None, None)
}

/// Renders a Cozy message row with optional reply context and an own-message
/// edit callback. This entry point lets the chat view provide Discord's
/// `message_reference` data without changing the compatibility configuration
/// used by existing callers.
pub fn render_message_row_with_reply_and_edit(
    message: &Message,
    config: MessageRowConfig,
    reply: Option<&Message>,
    on_edit: Option<MessageActionCallback>,
) -> Div {
    let author_name = message.author.display_name().to_string();
    let name_color = if message.is_pending {
        text_muted()
    } else {
        config
            .role_color
            .filter(|color| *color != 0)
            .map(from_hex)
            .unwrap_or_else(text_titles)
    };

    let mut row = div()
        .group("message-row")
        .relative()
        .flex()
        .flex_row()
        .w_full()
        .min_w(px(0.0))
        .min_h(if config.is_grouped { px(22.0) } else { px(44.0) })
        .max_w_full()
        .pl(space_8())
        .pr(space_8())
        .py(px(2.0))
        .hover(|s| s.bg(bg_chat_hover()))
        .rounded_tr(radius_sm())
        .rounded_br(radius_sm());

    if message.is_pending {
        row = row.opacity(0.5).text_color(text_muted());
    }

    // 1. Left Gutter (avatar or hover timestamp; content starts at Cozy 72px)
    let left_gutter = if config.is_grouped {
        // Grouped mode: gutter displays timestamp only on hover
        let time_hh_mm = format_compact_gutter_time(message.timestamp);
        let full_timestamp = format_message_full_timestamp(message.timestamp);
        div()
            .w(px(56.0))
            .flex()
            .justify_end()
            .items_baseline()
            .pt(px(2.0))
            .flex_shrink_0()
            .child(
                div()
                    .id(ElementId::Name(format!("compact-time-{}", message.id).into()))
                    .text_size(px(11.0))
                    .line_height(line_height_base())
                    .text_color(text_muted())
                    .invisible()
                    .cursor_default()
                    .tooltip(move |window, cx| Tooltip::new(full_timestamp.clone()).build(window, cx))
                    .group_hover("message-row", |s| s.visible())
                    .child(time_hh_mm),
            )
    } else {
        // Full mode: displays canonical author avatar using crate::components::Avatar
        let author_id = message.author.id.clone();
        let author_avatar = Avatar::new(author_name.clone())
            .size(AvatarSize::Px40)
            .opt_src(message.author.avatar_url())
            .opt_decoration(message.author.avatar_decoration_data.clone());

        div()
            .w(px(56.0))
            .flex_shrink_0()
            .pt(px(2.0))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                let state_entity = cx.try_global::<crate::app::state::AppStateGlobal>().map(|g| g.0.clone());
                if let Some(state_entity) = state_entity {
                    let aid = author_id.clone();
                    let pos = ev.position;
                    state_entity.update(cx, |s, cx| {
                        s.open_user_profile(&aid, Some(pos));
                        cx.notify();
                    });
                }
            })
            .child(author_avatar)
    };

    // 2. Right Content Area
    let mut content_area = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0));

    if message.is_pending {
        content_area = content_area.text_color(text_muted());
    }

    if let Some(reply) = reply {
        content_area = content_area.child(render_reply_preview(reply, config.role_color));
    }

    if !config.is_grouped {
        // Author header row (Username, BOT tag, Timestamp)
        let author_id = message.author.id.clone();
        let mut header_row = div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap(space_2())
            .child(
                div()
                    .font_weight(WEIGHT_MEDIUM)
                    .text_size(font_size_base())
                    .line_height(line_height_base())
                    .text_color(name_color)
                    .cursor_pointer()
                    .hover(|s| s.border_b_1().border_color(name_color))
                    .on_mouse_down(MouseButton::Left, move |ev, _win, cx| {
                        let state_entity = cx.try_global::<crate::app::state::AppStateGlobal>().map(|g| g.0.clone());
                        if let Some(state_entity) = state_entity {
                            let aid = author_id.clone();
                            let pos = ev.position;
                            state_entity.update(cx, |s, cx| {
                                s.open_user_profile(&aid, Some(pos));
                                cx.notify();
                            });
                        }
                    })
                    .child(author_name),
            );

        if message.author.bot {
            header_row = header_row.child(
                div()
                    .bg(blurple())
                    .rounded(radius_xs())
                    .px(px(4.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .text_size(font_size_xxs())
                    .font_weight(WEIGHT_BOLD)
                    .text_color(white())
                    .child("BOT"),
            );
        }

        let full_timestamp = format_message_full_timestamp(message.timestamp);
        header_row = header_row.child(
            div()
                .id(ElementId::Name(format!("header-time-{}", message.id).into()))
                .text_size(font_size_xs())
                .font_weight(WEIGHT_MEDIUM)
                .line_height(line_height_base())
                .text_color(text_muted())
                .cursor_default()
                .tooltip(move |window, cx| Tooltip::new(full_timestamp.clone()).build(window, cx))
                .child(format_message_timestamp(message.timestamp)),
        );

        if message.edited_timestamp.is_some() {
            header_row = header_row.child(
                div()
                    .text_size(px(10.0))
                    .text_color(text_muted())
                    .child("(editado)"),
            );
        }

        content_area = content_area.child(header_row);
    }

    // Sticker or Markdown Message Content
    if message.is_sticker() {
        let sticker_name = message.sticker_name().unwrap_or_else(|| "Figurinha".to_string());
        let sticker_id = message.sticker_id();
        let format_type = message.sticker_format_type();
        content_area = content_area.child(render_sticker(sticker_id.as_deref(), &sticker_name, format_type));
    } else {
        content_area = content_area.child(render_markdown(&message.content));
    }

    // Attachments
    if !message.attachments.is_empty() {
        content_area = content_area.child(render_attachments(&message.attachments));
    }

    // Poll
    if let Some(poll) = &message.poll {
        content_area = content_area.child(render_poll_card(poll, chrono::Utc::now()));
    }

    // Embeds
    if !message.embeds.is_empty() {
        content_area = content_area.child(render_embeds(&message.embeds));
    }

    // Reactions
    if !message.reactions.is_empty() {
        content_area = content_area.child(render_reaction_bar(
            &message.reactions,
            &message.id,
            config.on_reaction_click,
            config.on_add_reaction,
        ));
    }

    // Floating action bar
    let mut row = row.child(left_gutter).child(content_area);

    if let Some(on_more) = config.on_more.clone() {
        let msg_id = message.id.clone();
        row = row.on_mouse_down(MouseButton::Right, move |event, window, cx| {
            on_more(&msg_id, event.position, window, cx);
        });
    }

    if !message.is_pending {
        let action_bar = render_hover_action_bar_with_edit(
            &message.id,
            config.on_react,
            config.on_reply,
            on_edit,
            config.on_more,
        );
        row = row.child(action_bar);
    }

    row
}

// ============================================================================
// Message List Container (Cozy Layout with 5-Minute Window & Date Dividers)
// ============================================================================

/// Determines if two consecutive messages should be grouped in Cozy mode (within 5 minutes by same author).
pub fn should_group_messages(prev: &Message, current: &Message) -> bool {
    let elapsed = (current.timestamp - prev.timestamp).num_seconds();
    prev.author.id == current.author.id
        && (0..300).contains(&elapsed)
        && current.timestamp.with_timezone(&Local).date_naive() == prev.timestamp.with_timezone(&Local).date_naive()
}

/// Flattened item representation for virtualized chat streams
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatListItem {
    /// Initial channel welcome banner
    WelcomeHeader,
    /// Date divider between chat message days
    DateSeparator(NaiveDate),
    /// Message item holding the index into the channel's messages slice
    Message {
        message_idx: usize,
        is_grouped: bool,
    },
    /// Bottom anchor spacer
    BottomSpacer,
}

/// Builds the flattened, virtualizable list of chat items in O(N) time with zero message cloning.
pub fn build_chat_list_items(messages: &[Message]) -> Vec<ChatListItem> {
    let mut items = Vec::with_capacity(messages.len() + 8);
    items.push(ChatListItem::WelcomeHeader);
    let mut last_date: Option<NaiveDate> = None;
    let mut prev_message: Option<&Message> = None;

    for (idx, msg) in messages.iter().enumerate() {
        let msg_date = msg.timestamp.with_timezone(&Local).date_naive();

        // 1. Date Divider if date has changed
        if last_date.map_or(true, |d| d != msg_date) {
            items.push(ChatListItem::DateSeparator(msg_date));
            last_date = Some(msg_date);
            prev_message = None; // Reset grouping across dates
        }

        // 2. Check 5-minute Cozy grouping
        let is_grouped = prev_message.map_or(false, |prev| should_group_messages(prev, msg));
        items.push(ChatListItem::Message {
            message_idx: idx,
            is_grouped,
        });

        prev_message = Some(msg);
    }

    items.push(ChatListItem::BottomSpacer);
    items
}

/// Renders the channel or DM welcome banner displayed at the beginning of chat history.
pub fn render_welcome_banner(
    channel_name: &str,
    channel_topic: &str,
    is_dm: bool,
    avatar_src: Option<SharedString>,
    avatar_decoration: Option<crate::domain::user::AvatarDecorationData>,
) -> Div {
    let title_text = if is_dm {
        channel_name.to_string()
    } else {
        format!("Bem-vindo(a) a #{}!", channel_name)
    };

    let icon_element = if is_dm {
        Avatar::new(channel_name.to_string())
            .size(AvatarSize::Px80)
            .opt_src(avatar_src)
            .opt_decoration(avatar_decoration)
            .into_any_element()
    } else {
        div()
            .size(px(68.0))
            .rounded(radius_full())
            .bg(channel_active())
            .flex()
            .items_center()
            .justify_center()
            .text_color(white())
            .text_size(px(36.0))
            .font_weight(WEIGHT_BOLD)
            .child("#")
            .into_any_element()
    };

    div()
        .flex()
        .flex_col()
        .gap(space_2())
        .px(space_8())
        .pt(space_8())
        .pb(space_4())
        .child(icon_element)
        .child(
            div()
                .text_color(text_titles())
                .font_weight(WEIGHT_BOLD)
                .text_size(font_size_2xl())
                .child(title_text),
        )
        .child(
            div()
                .text_color(text_muted())
                .text_size(font_size_sm())
                .child(channel_topic.to_string()),
        )
}

/// Renders a complete message history stream with 5-minute Cozy grouping and date separators.
/// Uses a bounded dynamic buffer window (latest 60 messages) if called on large datasets
/// without virtualization, preventing layout and frame-rate bottlenecks.
pub fn render_message_list(
    messages: &[Message],
    on_react: Option<MessageActionCallback>,
    on_reply: Option<MessageActionCallback>,
    on_more: Option<MessageActionPosCallback>,
    on_reaction_click: Option<ReactionActionCallback>,
    on_add_reaction: Option<AddReactionCallback>,
) -> Div {
    render_message_list_inner(
        messages,
        on_react,
        on_reply,
        on_more,
        on_reaction_click,
        on_add_reaction,
        None,
    )
}

/// Renders the bounded non-virtualized history with an explicit first-unread
/// message boundary. Virtualized chat should map the same boundary to a list
/// item once read-state data is available in the chat view model.
pub fn render_message_list_with_unread(
    messages: &[Message],
    on_react: Option<MessageActionCallback>,
    on_reply: Option<MessageActionCallback>,
    on_more: Option<MessageActionPosCallback>,
    on_reaction_click: Option<ReactionActionCallback>,
    on_add_reaction: Option<AddReactionCallback>,
    first_unread_message_id: Option<&str>,
) -> Div {
    render_message_list_inner(
        messages,
        on_react,
        on_reply,
        on_more,
        on_reaction_click,
        on_add_reaction,
        first_unread_message_id,
    )
}

fn render_message_list_inner(
    messages: &[Message],
    on_react: Option<MessageActionCallback>,
    on_reply: Option<MessageActionCallback>,
    on_more: Option<MessageActionPosCallback>,
    on_reaction_click: Option<ReactionActionCallback>,
    on_add_reaction: Option<AddReactionCallback>,
    first_unread_message_id: Option<&str>,
) -> Div {
    let mut list = div()
        .flex()
        .flex_col()
        .w_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .max_w_full()
        .overflow_hidden()
        .py(space_4());

    // When called directly without gpui::list, use a bounded dynamic buffer (latest 60 messages)
    let (rendered_messages, is_truncated) = if messages.len() > 60 {
        (&messages[messages.len() - 60..], true)
    } else {
        (messages, false)
    };

    if is_truncated {
        list = list.child(
            div()
                .w_full()
                .py(space_2())
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(space_3())
                        .py(space_1())
                        .rounded(radius_pill())
                        .bg(bg_channels())
                        .text_size(font_size_xs())
                        .text_color(text_muted())
                        .child(format!("Exibindo últimas 60 de {} mensagens", messages.len())),
                ),
        );
    }

    let mut prev_message: Option<&Message> = None;
    let mut last_date: Option<NaiveDate> = None;

    for msg in rendered_messages {
        let msg_date = msg.timestamp.with_timezone(&Local).date_naive();

        // 1. Date Divider if date has changed
        if last_date.map_or(true, |d| d != msg_date) {
            list = list.child(render_date_separator_element(msg_date));
            last_date = Some(msg_date);
            prev_message = None; // Reset grouping across dates
        }

        if first_unread_message_id == Some(msg.id.as_str()) {
            list = list.child(render_new_messages_divider());
            prev_message = None; // Unread messages start a fresh visual group
        }

        // 2. Check 5-minute Cozy grouping
        let is_grouped = prev_message.map_or(false, |prev| should_group_messages(prev, msg));

        list = list.child(render_message_row(
            msg,
            MessageRowConfig {
                is_grouped,
                role_color: None,
                on_react: on_react.clone(),
                on_reply: on_reply.clone(),
                on_more: on_more.clone(),
                on_reaction_click: on_reaction_click.clone(),
                on_add_reaction: on_add_reaction.clone(),
            },
        ));

        prev_message = Some(msg);
    }

    list
}

// ============================================================================
// GPUI View Component for Message List
// ============================================================================

/// GPUI View component wrapping a message list.
pub struct MessageListView {
    pub messages: Vec<Message>,
}

impl MessageListView {
    pub fn new(messages: Vec<Message>) -> Self {
        Self { messages }
    }
}

impl Render for MessageListView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg_chat())
            .overflow_hidden()
            .child(render_message_list(
                &self.messages,
                None,
                None,
                None,
                None,
                None,
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::message::Author;

    #[::core::prelude::v1::test]
    fn test_5min_grouping_logic() {
        let author = Author::new("1001", "Alice");
        let t0 = Utc::now();
        let t1 = t0 + chrono::Duration::seconds(120); // 2 minutes later
        let t2 = t0 + chrono::Duration::seconds(400); // 6.6 minutes later

        let msg1 = Message::new("1", "c1", author.clone(), "Hello").with_timestamp(t0);
        let msg2 = Message::new("2", "c1", author.clone(), "World").with_timestamp(t1);
        let msg3 = Message::new("3", "c1", author.clone(), "After 6min").with_timestamp(t2);

        // msg1 and msg2 should be grouped (within 5 minutes by same author)
        assert!(should_group_messages(&msg1, &msg2));

        // msg1 and msg3 should NOT be grouped (> 5 minutes)
        assert!(!should_group_messages(&msg1, &msg3));
    }

    #[::core::prelude::v1::test]
    fn test_build_chat_list_items_virtualization() {
        let author = Author::new("1001", "Bob");
        let t0 = DateTime::from_timestamp(1700000000, 0).unwrap();
        let mut messages = Vec::new();

        for i in 0..100 {
            let msg = Message::new(
                format!("msg-{}", i),
                "chan-1",
                author.clone(),
                format!("Mensagem de teste {}", i),
            )
            .with_timestamp(t0 + chrono::Duration::seconds(i as i64 * 10));
            messages.push(msg);
        }

        let items = build_chat_list_items(&messages);
        // Should have WelcomeHeader + DateSeparator + 100 Messages + BottomSpacer = 103 items
        assert_eq!(items.len(), 103);
        assert!(matches!(items[0], ChatListItem::WelcomeHeader));
        assert!(matches!(items[1], ChatListItem::DateSeparator(_)));
        assert!(matches!(items[102], ChatListItem::BottomSpacer));
    }

    #[::core::prelude::v1::test]
    fn test_markdown_ast_cache_reuse() {
        let text = "Olá **GPUI**! Este texto tem *formatação* e `código`.";
        crate::chat::markdown::prewarm_markdown_cache(text);

        // Verify that the AST is now present in the cache
        let in_cache = {
            let cache = crate::chat::markdown::MARKDOWN_AST_CACHE.lock().unwrap();
            cache.contains_key(text)
        };
        assert!(in_cache, "MARKDOWN_AST_CACHE should contain prewarmed markdown text");
    }

    #[::core::prelude::v1::test]
    fn test_sticker_message_rendering_detection() {
        let author = Author::new("1001", "Alice");
        let msg = Message::new("s1", "c1", author.clone(), ":sticker[Wumpus Wave]:");
        assert!(msg.is_sticker());
        assert_eq!(msg.sticker_name(), Some("Wumpus Wave".to_string()));
        assert_eq!(msg.sticker_id(), None);

        let sticker_item = crate::domain::MessageStickerItem::new("897123", "Wumpus Dance", 1);
        let native_sticker_msg = Message::new("s2", "c1", author.clone(), "")
            .with_sticker_item(sticker_item);
        assert!(native_sticker_msg.is_sticker());
        assert_eq!(native_sticker_msg.sticker_name(), Some("Wumpus Dance".to_string()));
        assert_eq!(native_sticker_msg.sticker_id(), Some("897123".to_string()));

        let normal_msg = Message::new("m1", "c1", author, "Olá mundo");
        assert!(!normal_msg.is_sticker());
        assert_eq!(normal_msg.sticker_name(), None);
        assert_eq!(normal_msg.sticker_id(), None);

        // Verify elements construct cleanly
        let _card = render_sticker_card("Wumpus Wave");
        let _img_sticker = render_sticker(Some("897123"), "Wumpus Dance", Some(1));
        let _anim_sticker = render_sticker(Some("897123"), "Wumpus Dance", Some(2));
        let _fallback_sticker = render_sticker(None, "Wumpus Wave", None);
    }

    #[::core::prelude::v1::test]
    fn test_avatar_dimensions_and_rendering() {
        use crate::components::avatar::{Avatar, AvatarSize};
        assert_eq!(avatar_md(), px(40.0));
        assert_eq!(avatar_sm(), px(32.0));
        assert_eq!(AvatarSize::Px40.pixels(), px(40.0));
        assert_eq!(AvatarSize::Px32.pixels(), px(32.0));

        // Test Avatar with URL and without URL
        let _avatar_initials = Avatar::new("Rustacean").size(AvatarSize::Px40);
        let _avatar_url = Avatar::new("Rustacean")
            .size(AvatarSize::Px40)
            .src("https://cdn.discordapp.com/avatars/1/test.png");
    }

    #[::core::prelude::v1::test]
    fn test_render_hover_action_bar_and_context_menu() {
        let _bar = render_hover_action_bar("msg-101", None, None, None);

        let menu = render_message_context_menu(MessageContextMenuConfig {
            message_id: "msg-101".into(),
            message_content: "Hello world".into(),
            is_pinned: false,
            is_own: true,
            on_add_reaction: None,
            on_edit: None,
            on_reply: None,
            on_copy_text: None,
            on_copy_link: None,
            on_copy_id: None,
            on_mark_unread: None,
            on_pin: None,
            on_delete: None,
            on_delete_shift: None,
            on_close: None,
        });
        let _pinned_menu = render_message_context_menu(MessageContextMenuConfig {
            message_id: "msg-101".into(),
            message_content: "Hello world".into(),
            is_pinned: true,
            is_own: false,
            on_add_reaction: None,
            on_edit: None,
            on_reply: None,
            on_copy_text: None,
            on_copy_link: None,
            on_copy_id: None,
            on_mark_unread: None,
            on_pin: None,
            on_delete: None,
            on_delete_shift: None,
            on_close: None,
        });
        drop(menu);
    }

    #[::core::prelude::v1::test]
    fn test_render_welcome_banner_dm_and_guild() {
        let dm_banner = render_welcome_banner(
            "Usuário B",
            "Este é o início do seu histórico de mensagens diretas com Usuário B.",
            true,
            Some("https://cdn.discordapp.com/avatars/123/abc.png".into()),
            Some(crate::domain::user::AvatarDecorationData {
                asset: "synth_decor".to_string(),
                sku_id: None,
            }),
        );
        drop(dm_banner);

        let guild_banner = render_welcome_banner(
            "geral",
            "Este é o início do canal #geral.",
            false,
            None,
            None,
        );
        drop(guild_banner);
    }

    #[::core::prelude::v1::test]
    fn test_grouping_rejects_backwards_and_exact_five_minute_boundaries() {
        let author = Author::new("1001", "Alice");
        let t0 = DateTime::from_timestamp(1700000000, 0).unwrap();
        let first = Message::new("1", "c1", author.clone(), "first").with_timestamp(t0);
        let backwards =
            Message::new("2", "c1", author.clone(), "backwards").with_timestamp(t0 - chrono::Duration::seconds(1));
        let exact_five =
            Message::new("3", "c1", author.clone(), "five").with_timestamp(t0 + chrono::Duration::minutes(5));

        assert!(!should_group_messages(&first, &backwards));
        assert!(!should_group_messages(&first, &exact_five));
    }

    #[::core::prelude::v1::test]
    fn test_render_reply_unread_divider_and_edit_hover_action() {
        let author = Author::new("1001", "Alice");
        let reply = Message::new("reply-1", "c1", author.clone(), "Mensagem respondida");
        let message = Message::new("message-1", "c1", author, "Resposta");

        drop(render_reply_preview(&reply, Some(0x2ecc71)));
        drop(render_new_messages_divider());
        drop(render_hover_action_bar_with_edit(
            "message-1",
            None,
            None,
            Some(Arc::new(|_id, _window, _cx| {})),
            None,
        ));
        drop(render_message_row_with_reply_and_edit(
            &message,
            MessageRowConfig {
                is_grouped: false,
                role_color: Some(0),
                on_react: None,
                on_reply: None,
                on_more: None,
                on_reaction_click: None,
                on_add_reaction: None,
            },
            Some(&reply),
            None,
        ));
        drop(render_message_list_with_unread(
            &[reply, message],
            None,
            None,
            None,
            None,
            None,
            Some("message-1"),
        ));
    }
}
