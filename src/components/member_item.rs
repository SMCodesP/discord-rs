//! Discord RS - Member Item Component
//! Member row in members sidebar (240px) with avatar, status, role color, and bot tag.

use gpui::prelude::*;
use gpui::*;

use crate::components::avatar::{Avatar, AvatarSize};
use crate::domain::user::{AvatarDecorationData, PresenceStatus, User};
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Member item row in the 240px member sidebar
#[derive(IntoElement)]
pub struct MemberItem {
    id: ElementId,
    name: SharedString,
    avatar_url: Option<SharedString>,
    avatar_decoration: Option<AvatarDecorationData>,
    status: PresenceStatus,
    role_color: Option<Rgba>,
    is_bot: bool,
    activity_text: Option<SharedString>,
    selected: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl MemberItem {
    /// Creates a new member item with an ID and display name
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            avatar_url: None,
            avatar_decoration: None,
            status: PresenceStatus::Offline,
            role_color: None,
            is_bot: false,
            activity_text: None,
            selected: false,
            on_click: None,
        }
    }

    /// Convenience constructor from a domain `User`
    pub fn from_user(id: impl Into<ElementId>, user: &User) -> Self {
        let name_str: String = user.display_name().to_string();
        let mut item = Self::new(id, name_str)
            .status(user.presence)
            .is_bot(user.bot)
            .opt_avatar_decoration(user.avatar_decoration_data.clone());

        if let Some(avatar_url) = user.avatar_url() {
            item = item.avatar_url(avatar_url);
        }

        if let Some(act) = &user.activity {
            let desc = if let Some(details) = &act.details {
                format!("{}: {}", act.name, details)
            } else {
                format!("{} {}", act.activity_type.label(), act.name)
            };
            item = item.activity_text(desc);
        }

        item
    }

    /// Sets avatar image URL
    pub fn avatar_url(mut self, url: impl Into<SharedString>) -> Self {
        self.avatar_url = Some(url.into());
        self
    }

    /// Sets optional avatar image URL
    pub fn opt_avatar_url(mut self, url: Option<impl Into<SharedString>>) -> Self {
        self.avatar_url = url.map(|u| u.into());
        self
    }

    /// Sets avatar decoration data
    pub fn avatar_decoration(mut self, decor: AvatarDecorationData) -> Self {
        self.avatar_decoration = Some(decor);
        self
    }

    /// Sets optional avatar decoration data
    pub fn opt_avatar_decoration(mut self, decor: Option<AvatarDecorationData>) -> Self {
        self.avatar_decoration = decor;
        self
    }

    /// Sets presence status
    pub fn status(mut self, status: PresenceStatus) -> Self {
        self.status = status;
        self
    }

    /// Sets role color
    pub fn role_color(mut self, color: Rgba) -> Self {
        self.role_color = Some(color);
        self
    }

    /// Sets role color from u32 RGB (e.g. from Role::color)
    pub fn role_color_rgb(mut self, color_rgb: u32) -> Self {
        if color_rgb != 0 {
            self.role_color = Some(rgb(color_rgb));
        }
        self
    }

    /// Sets whether member is a bot
    pub fn is_bot(mut self, is_bot: bool) -> Self {
        self.is_bot = is_bot;
        self
    }

    /// Sets rich activity or custom status text
    pub fn activity_text(mut self, text: impl Into<SharedString>) -> Self {
        self.activity_text = Some(text.into());
        self
    }

    /// Sets selected state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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

impl RenderOnce for MemberItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let name_color = if let Some(role_color) = self.role_color {
            role_color
        } else if self.status == PresenceStatus::Offline {
            text_muted()
        } else {
            text_normal()
        };

        let bg_color = if self.selected {
            channel_active()
        } else {
            rgba(0x00000000)
        };

        let mut row = div()
            .id(self.id)
            .flex()
            .items_center()
            .h(px(44.0))
            .mx(space_2())
            .px(space_2())
            .rounded(radius_sm())
            .bg(bg_color)
            .cursor_pointer()
            .gap(space_3())
            .flex_shrink_0();

        if !self.selected {
            row = row
                .hover(|s| s.bg(channel_hover()))
                .active(|s| s.bg(channel_active()));
        }

        if let Some(on_click) = self.on_click {
            row = row.on_click(move |ev, window, cx| {
                (on_click)(ev, window, cx);
            });
        }

        // 32px Avatar with status indicator and optional decoration
        let mut avatar = Avatar::new(self.name.clone())
            .size(AvatarSize::Px32)
            .status(self.status)
            .cutout_color(bg_channels())
            .opt_decoration(self.avatar_decoration);

        if let Some(url) = &self.avatar_url {
            avatar = avatar.src(url.clone());
        }

        row = row.child(avatar);

        // Name and Activity column
        let mut text_col = div()
            .flex()
            .flex_col()
            .justify_center()
            .flex_1()
            .overflow_hidden();

        // Top line: Name + Bot Tag
        let mut name_line = div()
            .flex()
            .items_center()
            .gap(space_1());

        name_line = name_line.child(
            div()
                .text_size(font_size_sm())
                .font_weight(FontWeight::MEDIUM)
                .text_color(name_color)
                .overflow_hidden()
                .child(self.name),
        );

        if self.is_bot {
            name_line = name_line.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .px(px(4.0))
                    .py(px(1.0))
                    .rounded_xs()
                    .bg(blurple())
                    .text_color(crate::tokens::colors::white())
                    .text_size(font_size_xxs())
                    .font_weight(FontWeight::BOLD)
                    .child(icon_check(px(10.0), crate::tokens::colors::white()))
                    .child("BOT"),
            );
        }

        text_col = text_col.child(name_line);

        // Bottom line: Activity text (if present)
        if let Some(act) = self.activity_text {
            text_col = text_col.child(
                div()
                    .text_size(font_size_xs())
                    .text_color(text_muted())
                    .overflow_hidden()
                    .child(act),
            );
        }

        row.child(text_col)
    }
}
