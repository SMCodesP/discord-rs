//! Discord RS - Channel Item Component
//! Channel row in sidebar: text, voice, announcement with selected, unread, and hover states.

use gpui::prelude::*;
use gpui::*;

use crate::components::badge::{MentionBadge, UnreadMarker, UnreadMarkerState};
use crate::domain::channel::ChannelType;
use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Channel row item in the channels sidebar (240px)
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
}

impl RenderOnce for ChannelItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (bg_color, text_color, icon_color, font_weight) = if self.selected {
            (
                channel_active(),
                crate::tokens::colors::white(),
                crate::tokens::colors::white(),
                FontWeight::SEMIBOLD,
            )
        } else if self.unread {
            (
                rgba(0x00000000),
                crate::tokens::colors::white(),
                crate::tokens::colors::white(),
                FontWeight::SEMIBOLD,
            )
        } else {
            (
                rgba(0x00000000),
                text_muted(),
                text_muted(),
                FontWeight::MEDIUM,
            )
        };

        let channel_icon_el = match self.channel_type {
            ChannelType::Text => icon_hash(px(18.0), icon_color),
            ChannelType::Voice => icon_speaker(px(18.0), icon_color),
            ChannelType::Announcement => icon_megaphone(px(18.0), icon_color),
            ChannelType::Dm => icon_user(px(18.0), icon_color),
            ChannelType::GroupDm => icon_members(px(18.0), icon_color),
            ChannelType::Category => icon_hash(px(18.0), icon_color),
        };

        // Indicator pill state on left edge
        let marker_state = if self.selected {
            UnreadMarkerState::Hidden // Selected row has full background highlight
        } else if self.unread {
            UnreadMarkerState::Unread
        } else {
            UnreadMarkerState::Hidden
        };

        let mut row = div()
            .id(self.id)
            .relative()
            .flex()
            .items_center()
            .h(px(34.0))
            .mx(space_2())
            .px(space_2())
            .rounded(radius_sm())
            .bg(bg_color)
            .cursor_pointer()
            .flex_shrink_0();

        if !self.selected {
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

        if let Some(on_click) = self.on_click {
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
                    .top(px(13.0)) // Center 8px indicator inside 34px height
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
                .child(channel_icon_el),
        );

        // Channel Name
        row = row.child(
            div()
                .flex_1()
                .text_size(font_size_sm())
                .font_weight(font_weight)
                .text_color(text_color)
                .overflow_hidden()
                .child(self.name),
        );

        // Lock icon if private channel
        if self.locked {
            row = row.child(
                div()
                    .mr(space_1())
                    .child(icon_lock(px(14.0), text_muted())),
            );
        }

        // Voice connected participants count
        if matches!(self.channel_type, ChannelType::Voice) && self.connected_users > 0 {
            row = row.child(
                div()
                    .px(px(6.0))
                    .py(px(1.0))
                    .rounded_xs()
                    .bg(rgba(0xffffff14))
                    .text_size(font_size_xxs())
                    .text_color(text_muted())
                    .child(format!("{}", self.connected_users)),
            );
        }

        // Red mention badge on right
        if self.mention_count > 0 {
            row = row.child(
                div()
                    .ml(space_2())
                    .child(MentionBadge::new(self.mention_count)),
            );
        }

        row
    }
}
