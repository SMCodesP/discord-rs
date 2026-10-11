//! Discord RS - Member Item Component
//! Member row in members sidebar (248x44px r8) with avatar, status, role color, and bot tag.
//! Visual reference (member-sidebar.md):
//! - linha de membro: 248x44 | padding vertical 1px | radius 8px | 16px/16px w400
//! - avatar (svg com status): 40x40

use gpui::prelude::*;
use gpui::*;

use crate::components::avatar::{Avatar, AvatarSize};
use crate::domain::user::{AvatarDecorationData, PresenceStatus, User};
use crate::tokens::colors::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Member item row in the member sidebar (248x44px, radius 8px, avatar 40px)
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

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn get_status(&self) -> PresenceStatus {
        self.status
    }
}

impl RenderOnce for MemberItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        render_member_row(self)
    }
}

fn render_bot_tag() -> AnyElement {
    div()
        .flex()
        .items_center()
        .px(px(4.0))
        .py(px(1.0))
        .rounded_xs()
        .bg(blurple())
        .text_color(crate::tokens::colors::white())
        .text_size(font_size_xxs())
        .font_weight(FontWeight::BOLD)
        .child("BOT")
        .into_any_element()
}

fn render_member_row(item: MemberItem) -> AnyElement {
    let name_color = if let Some(role_color) = item.role_color {
        role_color
    } else if item.status == PresenceStatus::Offline {
        text_muted()
    } else {
        text_normal()
    };

    let mut row = div()
        .id(item.id)
        .w_full()
        .min_w(px(0.0))
        .overflow_hidden()
        .flex()
        .items_center()
        .h(px(44.0))
        .mx(space_2())
        .px(px(8.0))
        .py(px(1.0))
        .rounded(radius_md()) // 8px radius per member-sidebar.md
        .cursor_pointer()
        .gap(space_3())
        .flex_shrink_0();

    if item.selected {
        row = row.bg(channel_active());
    } else {
        row = row
            .hover(|s| s.bg(channel_hover()))
            .active(|s| s.bg(channel_active()));
    }

    if let Some(on_click) = item.on_click {
        row = row.on_click(move |ev, window, cx| {
            (on_click)(ev, window, cx);
        });
    }

    // 32px Avatar with status indicator and optional decoration (per member-sidebar.md)
    let mut avatar = Avatar::new(item.name.clone())
        .size(AvatarSize::Px32)
        .status(item.status)
        .cutout_color(bg_channels())
        .opt_decoration(item.avatar_decoration);

    if let Some(url) = &item.avatar_url {
        avatar = avatar.src(url.clone());
    }

    row = row.child(avatar);

    // Name and Activity column
    let mut text_col = div()
        .flex()
        .flex_col()
        .justify_center()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden();

    // Top line: Name + Bot Tag
    let mut name_line = div()
        .flex()
        .items_center()
        .min_w(px(0.0))
        .gap(space_1());

    name_line = name_line.child(
        div()
            .text_size(font_size_sm())
            .font_weight(FontWeight::MEDIUM)
            .text_color(name_color)
            .truncate()
            .child(item.name),
    );

    if item.is_bot {
        name_line = name_line.child(render_bot_tag());
    }

    text_col = text_col.child(name_line);

    // Bottom line: Activity text (if present)
    if let Some(act) = item.activity_text {
        text_col = text_col.child(
            div()
                .text_size(font_size_xs())
                .text_color(text_muted())
                .w_full()
                .min_w(px(0.0))
                .truncate()
                .child(act),
        );
    }

    row.child(text_col).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_member_item_builder() {
        let m = MemberItem::new("m-1", "Alice")
            .status(PresenceStatus::Online)
            .is_bot(false)
            .activity_text("Coding Rust");

        assert_eq!(m.get_name(), "Alice");
        assert_eq!(m.get_status(), PresenceStatus::Online);
        assert!(!m.is_bot);
        assert_eq!(m.activity_text.as_deref(), Some("Coding Rust"));
    }

    #[::core::prelude::v1::test]
    fn test_member_item_bot() {
        let bot = MemberItem::new("bot-1", "MusicBot")
            .status(PresenceStatus::Idle)
            .is_bot(true);

        assert!(bot.is_bot);
        assert_eq!(bot.get_status(), PresenceStatus::Idle);
    }
}
