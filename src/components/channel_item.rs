//! Discord RS - Channel Item Component
//! Channel row in sidebar: text, voice, announcement with selected, unread, and hover states.
//! Visual reference (channel-sidebar.md):
//! - linha de canal: 286x32 | padding 4px 8px | radius 8px | 16px/16px w400

use gpui::prelude::*;
use gpui::*;

use crate::components::badge::{MentionBadge, UnreadMarker, UnreadMarkerState};
use crate::domain::channel::ChannelType;
use crate::tokens::colors::{channel_active, channel_hover, opacity_white_8, text_muted, text_normal, white};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Channel row item in the channels sidebar (286x32px, radius 8px)
#[derive(IntoElement)]
pub struct ChannelItem {
    id: ElementId,
    name: SharedString,
    channel_type: ChannelType,
    selected: bool,
    unread: bool,
    mention_count: u32,
    locked: bool,
    connected_users: u32,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl ChannelItem {
    /// Creates a new channel item
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            channel_type: ChannelType::Text,
            selected: false,
            unread: false,
            mention_count: 0,
            locked: false,
            connected_users: 0,
            on_click: None,
        }
    }

    /// Sets channel type (Text, Voice, Announcement)
    pub fn channel_type(mut self, channel_type: ChannelType) -> Self {
        self.channel_type = channel_type;
        self
    }

    /// Sets selected / active state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets unread indicator state
    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    /// Sets unread mention count
    pub fn mention_count(mut self, count: u32) -> Self {
        self.mention_count = count;
        self
    }

    /// Marks channel as locked / private
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Number of users connected in a voice channel
    pub fn connected_users(mut self, count: u32) -> Self {
        self.connected_users = count;
        self
    }

    /// Sets click handler
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn is_unread(&self) -> bool {
        self.unread
    }

    pub fn get_mention_count(&self) -> u32 {
        self.mention_count
    }
}

impl RenderOnce for ChannelItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        render_channel_row(self)
    }
}

fn render_channel_icon(channel_type: ChannelType, color: Rgba) -> AnyElement {
    match channel_type {
        ChannelType::Text => icon_hash(px(18.0), color).into_any_element(),
        ChannelType::Voice => icon_speaker(px(18.0), color).into_any_element(),
        ChannelType::Announcement => icon_megaphone(px(18.0), color).into_any_element(),
        ChannelType::Dm => icon_user(px(18.0), color).into_any_element(),
        ChannelType::GroupDm => icon_members(px(18.0), color).into_any_element(),
        ChannelType::Category => icon_hash(px(18.0), color).into_any_element(),
    }
}

fn render_channel_row(item: ChannelItem) -> AnyElement {
    let (bg_color, text_color, icon_color, font_weight) = if item.selected {
        (
            Some(channel_active()),
            white(),
            white(),
            FontWeight::SEMIBOLD,
        )
    } else if item.unread {
        (
            None,
            white(),
            white(),
            FontWeight::SEMIBOLD,
        )
    } else {
        (
            None,
            text_muted(),
            text_muted(),
            FontWeight::MEDIUM,
        )
    };

    let marker_state = if item.selected {
        UnreadMarkerState::Hidden
    } else if item.unread {
        UnreadMarkerState::Unread
    } else {
        UnreadMarkerState::Hidden
    };

    let mut row = div()
        .id(item.id)
        .relative()
        .flex()
        .items_center()
        .h(px(32.0))
        .mx(space_2())
        .px(px(8.0))
        .py(px(4.0))
        .rounded(radius_md()) // 8px radius per channel-sidebar.md
        .cursor_pointer()
        .flex_shrink_0();

    if let Some(bg) = bg_color {
        row = row.bg(bg);
    }

    if !item.selected {
        row = row
            .hover(move |s| {
                s.bg(channel_hover())
                    .text_color(text_normal())
            })
            .active(move |s| {
                s.bg(channel_active())
                    .text_color(white())
            });
    }

    if let Some(on_click) = item.on_click {
        row = row.on_click(move |ev, window, cx| {
            (on_click)(ev, window, cx);
        });
    }

    // Unread white pill indicator positioned at left edge
    if marker_state.is_visible() {
        row = row.child(
            div()
                .absolute()
                .left(px(-8.0))
                .top(px(12.0)) // Center 8px indicator inside 32px height
                .child(UnreadMarker::new(marker_state)),
        );
    }

    // Channel Icon
    row = row.child(
        div()
            .flex()
            .items_center()
            .justify_center()
            .w(px(20.0))
            .h(px(20.0))
            .mr(space_2())
            .child(render_channel_icon(item.channel_type, icon_color)),
    );

    // Channel Name
    row = row.child(
        div()
            .flex_1()
            .text_size(font_size_sm())
            .font_weight(font_weight)
            .text_color(text_color)
            .overflow_hidden()
            .child(item.name),
    );

    // Lock icon if private channel
    if item.locked {
        row = row.child(
            div()
                .mr(space_1())
                .child(icon_lock(px(14.0), text_muted())),
        );
    }

    // Voice connected participants count
    if matches!(item.channel_type, ChannelType::Voice) && item.connected_users > 0 {
        row = row.child(
            div()
                .px(px(6.0))
                .py(px(1.0))
                .rounded_xs()
                .bg(opacity_white_8())
                .text_size(font_size_xxs())
                .text_color(text_muted())
                .child(format!("{}", item.connected_users)),
        );
    }

    // Red mention badge on right
    if item.mention_count > 0 {
        row = row.child(
            div()
                .ml(space_2())
                .child(MentionBadge::new(item.mention_count)),
        );
    }

    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_channel_item_builder() {
        let ch = ChannelItem::new("ch-1", "general")
            .channel_type(ChannelType::Text)
            .selected(true)
            .unread(false)
            .mention_count(3)
            .locked(true)
            .connected_users(0);

        assert_eq!(ch.get_name(), "general");
        assert!(ch.is_selected());
        assert!(!ch.is_unread());
        assert_eq!(ch.get_mention_count(), 3);
    }

    #[::core::prelude::v1::test]
    fn test_channel_item_voice_users() {
        let voice = ChannelItem::new("vc-1", "Voice Lounge")
            .channel_type(ChannelType::Voice)
            .connected_users(5);

        assert_eq!(voice.connected_users, 5);
        assert!(!voice.is_selected());
    }
}
