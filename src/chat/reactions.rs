//! Discord RS - Reaction Bar & Reaction Badges
//! Implements reaction list, individual reaction counters, active user indicator,
//! and the add reaction button (+ / smiley) with kit IconButton and zero fixed colors.

use gpui::*;
use std::sync::Arc;

use crate::components::button::IconButton;
use crate::domain::message::Reaction;
use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

// ============================================================================
// Callback Type Aliases
// ============================================================================

pub type ReactionActionCallback = Arc<dyn Fn(&str, &str, &mut Window, &mut App) + Send + Sync + 'static>;
pub type AddReactionCallback = Arc<dyn Fn(&str, &mut Window, &mut App) + Send + Sync + 'static>;

// ============================================================================
// Popular Default Reaction Emojis
// ============================================================================

pub const DEFAULT_REACTION_EMOJIS: &[&str] = &["👍", "❤️", "🔥", "🎉", "🦀", "🚀", "👀", "😂"];

// ============================================================================
// Reaction Badge Component
// ============================================================================

/// Renders an individual reaction badge with emoji, count, and active (me) state.
pub fn render_reaction_badge(
    reaction: &Reaction,
    message_id: &str,
    on_click: Option<ReactionActionCallback>,
) -> Div {
    let emoji = reaction.emoji.clone();
    let msg_id = message_id.to_string();
    let count = reaction.count;
    let is_me = reaction.me;

    let (bg_color, border_col, text_col, hover_bg, hover_border) = if is_me {
        (
            with_alpha(HEX_BLURPLE, 0.15),
            blurple(),
            // TOKEN FALTANDO: --mention-foreground / COR ANTIGA SEM ORIGEM: 0xc9cdfb
            rgb(0xc9cdfb),
            with_alpha(HEX_BLURPLE, 0.25),
            blurple(),
        )
    } else {
        (
            bg_channels(),
            bg_chat_hover(),
            // TOKEN FALTANDO: --text-muted / COR ANTIGA SEM ORIGEM: 0xb5bac1
            rgb(0xb5bac1),
            channel_hover(),
            borders(),
        )
    };

    let mut badge = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(26.0))
        .px(px(8.0))
        .py(px(2.0))
        .bg(bg_color)
        .border_1()
        .border_color(border_col)
        .rounded(px(8.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).border_color(hover_border))
        .child(
            div()
                .text_size(px(14.0))
                .child(emoji.clone()),
        )
        .child(
            div()
                .text_size(font_size_xs())
                .font_weight(WEIGHT_BOLD)
                .text_color(text_col)
                .child(count.to_string()),
        );

    if let Some(handler) = on_click {
        badge = badge.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(&msg_id, &emoji, window, cx);
        });
    }

    badge
}

// ============================================================================
// Add Reaction Button
// ============================================================================

/// Renders the "+" / smiley add-reaction button at the end of the reaction bar.
pub fn render_add_reaction_button(
    message_id: &str,
    on_add_click: Option<AddReactionCallback>,
) -> Div {
    let msg_id = message_id.to_string();

    let mut btn = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .h(px(26.0))
        .px(space_3())
        .bg(bg_channels())
        .border_1()
        .border_color(bg_chat_hover())
        .rounded(radius_md())
        .cursor_pointer()
        .hover(|s| s.bg(channel_hover()).border_color(borders()))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .text_color(text_muted())
                .text_size(font_size_xs())
                .font_weight(WEIGHT_MEDIUM)
                .child("☺")
                .child("+"),
        );

    if let Some(handler) = on_add_click {
        btn = btn.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
            handler(&msg_id, window, cx);
        });
    }

    btn
}

// ============================================================================
// Reaction Bar (List of Reactions + Add Button)
// ============================================================================

/// Renders the complete reaction list for a message.
pub fn render_reaction_bar(
    reactions: &[Reaction],
    message_id: &str,
    on_reaction_click: Option<ReactionActionCallback>,
    on_add_click: Option<AddReactionCallback>,
) -> Div {
    if reactions.is_empty() {
        return div();
    }

    let mut bar = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(space_2())
        .mt(space_2());

    for rx in reactions {
        bar = bar.child(render_reaction_badge(rx, message_id, on_reaction_click.clone()));
    }

    bar = bar.child(render_add_reaction_button(message_id, on_add_click));

    bar
}

// ============================================================================
// Quick Reaction Picker Overlay (Hover Toolbar / Popover)
// ============================================================================

/// Renders a row of quick emoji reaction shortcuts with kit IconButton.
pub fn render_quick_reaction_picker(
    message_id: &str,
    on_select_emoji: Option<ReactionActionCallback>,
) -> Div {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(space_1())
        .bg(bg_user_bar())
        .border_1()
        .border_color(borders())
        .rounded(radius_md())
        .p(space_1())
        .shadow_md();

    for &emoji in DEFAULT_REACTION_EMOJIS {
        let msg_id = message_id.to_string();
        let em = emoji.to_string();

        let mut item = IconButton::new(format!("quick-rx-{}-{}", message_id, emoji), emoji)
            .size(px(28.0))
            .icon_size(px(16.0))
            .tooltip(format!("Reagir com {}", emoji));

        if let Some(handler) = on_select_emoji.as_ref() {
            let h = handler.clone();
            item = item.on_click(move |_event, window, cx| {
                h(&msg_id, &em, window, cx);
            });
        }

        row = row.child(item);
    }

    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_reaction_emojis_available() {
        assert_eq!(DEFAULT_REACTION_EMOJIS.len(), 8);
        assert!(DEFAULT_REACTION_EMOJIS.contains(&"🦀"));
    }

    #[::core::prelude::v1::test]
    fn test_reaction_badge_and_button_rendering() {
        let rx_me = Reaction {
            emoji: "👍".into(),
            count: 3,
            me: true,
        };
        let _badge_me = render_reaction_badge(&rx_me, "m1", None);

        let rx_other = Reaction {
            emoji: "🔥".into(),
            count: 1,
            me: false,
        };
        let _badge_other = render_reaction_badge(&rx_other, "m1", None);

        let _add_btn = render_add_reaction_button("m1", None);
        let _bar = render_reaction_bar(&[rx_me, rx_other], "m1", None, None);
        let _picker = render_quick_reaction_picker("m1", None);
    }
}
