//! Discord RS - Guild Item Component
//! Server icon in vertical rail (72px) with white indicator pill on left and squircle morph on hover.

use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::components::badge::MentionBadge;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// Server navigation item in the 72px vertical guild rail
#[derive(IntoElement)]
pub struct GuildItem {
    id: ElementId,
    name: SharedString,
    icon_url: Option<SharedString>,
    initials: SharedString,
    selected: bool,
    unread: bool,
    mention_count: u32,
    is_home: bool,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl GuildItem {
    /// Creates a new guild rail item
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        let name_str: SharedString = name.into();
        let initials = Self::compute_initials(&name_str);
        Self {
            id: id.into(),
            name: name_str,
            icon_url: None,
            initials,
            selected: false,
            unread: false,
            mention_count: 0,
            is_home: false,
            on_click: None,
        }
    }

    /// Sets explicit icon image URL
    pub fn icon_url(mut self, url: impl Into<SharedString>) -> Self {
        self.icon_url = Some(url.into());
        self
    }

    /// Sets optional icon image URL
    pub fn opt_icon_url(mut self, url: Option<impl Into<SharedString>>) -> Self {
        self.icon_url = url.map(|u| u.into());
        self
    }

    /// Sets selected / active server state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets unread state
    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    /// Sets unread mention count
    pub fn mention_count(mut self, count: u32) -> Self {
        self.mention_count = count;
        self
    }

    /// Marks as top DM / Home button
    pub fn is_home(mut self, is_home: bool) -> Self {
        self.is_home = is_home;
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

    pub fn get_id(&self) -> &ElementId {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn get_icon_url(&self) -> Option<&str> {
        self.icon_url.as_ref().map(|s| s.as_ref())
    }

    pub fn get_initials(&self) -> &str {
        self.initials.as_ref()
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

    pub fn is_home_button(&self) -> bool {
        self.is_home
    }

    /// Derives 1-3 letter initials from a server name
    pub fn compute_initials(name: &str) -> SharedString {
        let words: Vec<&str> = name.split_whitespace().collect();
        if words.is_empty() {
            return "?".into();
        }
        if words.len() == 1 {
            let s: String = words[0].chars().take(3).collect();
            return s.into();
        }
        let initials: String = words.iter().filter_map(|w| w.chars().next()).take(3).collect();
        initials.into()
    }
}

impl RenderOnce for GuildItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Icon corner radius: Circle (24px) by default, Squircle (16px) when selected or hovered
        let icon_radius = if self.selected {
            radius_server_hover() // 16px squircle
        } else {
            radius_server_default() // 24px circle
        };

        let icon_bg = if self.selected {
            blurple()
        } else {
            bg_chat() // #313338
        };

        let _tooltip_name = self.name.clone();

        let mut item_container = div()
            .id(self.id)
            .group("guild-item")
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .w(rail_width()) // 72px
            .h(px(56.0))
            .cursor_pointer();

        if let Some(on_click) = self.on_click {
            item_container = item_container.on_click(move |ev, window, cx| {
                (on_click)(ev, window, cx);
            });
        }

        // Left white indicator pill
        let mut pill = div()
            .absolute()
            .left_0()
            .w(px(4.0))
            .bg(crate::tokens::colors::white())
            .rounded_r(radius_sm());

        if self.selected {
            pill = pill.h(px(40.0));
            item_container = item_container.child(pill);
        } else if self.unread {
            pill = pill.h(px(8.0)).group_hover("guild-item", |s| s.h(px(20.0)));
            item_container = item_container.child(pill);
        } else {
            pill = pill.h(px(0.0)).group_hover("guild-item", |s| s.h(px(20.0)));
            item_container = item_container.child(pill);
        }

        // Server Icon container (48x48px) with Squircle Morph on hover
        let mut icon_box = div()
            .flex()
            .items_center()
            .justify_center()
            .w(server_icon_size()) // 48px
            .h(server_icon_size()) // 48px
            .rounded(icon_radius)
            .bg(icon_bg)
            .overflow_hidden()
            .flex_shrink_0();

        if !self.selected {
            // Squircle morph on hover: morph radius from 24px circle to 16px squircle and bg to Blurple
            icon_box = icon_box
                .group_hover("guild-item", |s| s.rounded(radius_server_hover()).bg(blurple()))
                .hover(move |s| s.rounded(radius_server_hover()).bg(blurple()));
        }

        // Icon contents: Home Discord icon, server image, or initials text
        if self.is_home {
            icon_box = icon_box.child(
                icon_gamepad(px(24.0), crate::tokens::colors::white())
            );
        } else if let Some(url) = &self.icon_url {
            if !url.is_empty() {
                icon_box = icon_box.child(
                    img(url.clone())
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                );
            } else {
                icon_box = icon_box.child(
                    div()
                        .text_size(font_size_md())
                        .font_weight(FontWeight::BOLD)
                        .text_color(crate::tokens::colors::white())
                        .child(self.initials.clone()),
                );
            }
        } else {
            icon_box = icon_box.child(
                div()
                    .text_size(font_size_md())
                    .font_weight(FontWeight::BOLD)
                    .text_color(crate::tokens::colors::white())
                    .child(self.initials.clone()),
            );
        }

        let mut icon_wrapper = div()
            .relative()
            .child(icon_box);

        // Floating mention badge on bottom-right corner
        if self.mention_count > 0 {
            icon_wrapper = icon_wrapper.child(
                div()
                    .absolute()
                    .bottom(px(-4.0))
                    .right(px(-4.0))
                    .child(MentionBadge::new(self.mention_count).with_cutout(bg_server())),
            );
        }

        item_container.child(icon_wrapper)
    }
}

/// Child guild representation inside a folder
#[derive(Debug, Clone)]
pub struct GuildFolderChild {
    pub id: ElementId,
    pub name: SharedString,
    pub icon_url: Option<SharedString>,
    pub initials: SharedString,
    pub selected: bool,
    pub unread: bool,
    pub mention_count: u32,
}

impl GuildFolderChild {
    /// Creates a child guild entry for a folder
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        let name_str: SharedString = name.into();
        let initials = GuildItem::compute_initials(&name_str);
        Self {
            id: id.into(),
            name: name_str,
            icon_url: None,
            initials,
            selected: false,
            unread: false,
            mention_count: 0,
        }
    }

    /// Sets child icon image URL
    pub fn icon_url(mut self, url: impl Into<SharedString>) -> Self {
        self.icon_url = Some(url.into());
        self
    }

    /// Sets child selected state
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Sets child unread state
    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    /// Sets child mention count
    pub fn mention_count(mut self, count: u32) -> Self {
        self.mention_count = count;
        self
    }
}

/// Discord server folder component (GuildFolderItem)
/// Displays a group of servers with semitransparent background, rounded borders, and thumbnail previews.
#[derive(IntoElement)]
pub struct GuildFolderItem {
    id: ElementId,
    name: SharedString,
    color: Option<Rgba>,
    expanded: bool,
    guilds: Vec<GuildFolderChild>,
    unread: bool,
    mention_count: u32,
    on_toggle: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    on_child_click: Option<Box<dyn Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static>>,
}

impl GuildFolderItem {
    /// Creates a new guild folder item
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            color: None,
            expanded: false,
            guilds: Vec::new(),
            unread: false,
            mention_count: 0,
            on_toggle: None,
            on_child_click: None,
        }
    }

    /// Sets folder accent color (semitransparent opacity is applied automatically)
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets whether folder is expanded
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Adds a child guild to the folder
    pub fn child_guild(mut self, child: GuildFolderChild) -> Self {
        self.guilds.push(child);
        self
    }

    /// Sets full list of children
    pub fn guilds(mut self, guilds: Vec<GuildFolderChild>) -> Self {
        self.guilds = guilds;
        self
    }

    /// Sets unread state
    pub fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    /// Sets unread mention count
    pub fn mention_count(mut self, count: u32) -> Self {
        self.mention_count = count;
        self
    }

    /// Sets handler for expanding/collapsing folder
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    /// Sets handler when clicking a specific child guild inside expanded folder
    pub fn on_child_click(
        mut self,
        handler: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_child_click = Some(Box::new(handler));
        self
    }

    pub fn get_id(&self) -> &ElementId {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn get_color(&self) -> Option<Rgba> {
        self.color
    }

    pub fn get_guilds(&self) -> &[GuildFolderChild] {
        &self.guilds
    }

    pub fn is_unread(&self) -> bool {
        self.unread || self.guilds.iter().any(|g| g.unread)
    }

    pub fn get_mention_count(&self) -> u32 {
        if self.mention_count > 0 {
            self.mention_count
        } else {
            self.guilds.iter().map(|g| g.mention_count).sum()
        }
    }
}

impl RenderOnce for GuildFolderItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let has_selected_child = self.guilds.iter().any(|g| g.selected);
        let has_unread_child = self.is_unread();
        let total_mentions = self.get_mention_count();

        let base_color = self.color.unwrap_or_else(|| blurple());
        // Semitransparent background (~40% opacity)
        let folder_bg = Rgba {
            r: base_color.r,
            g: base_color.g,
            b: base_color.b,
            a: 0.40,
        };

        let _tooltip_text = self.name.clone();

        if !self.expanded {
            // =================================================================
            // COLLAPSED / CLOSED FOLDER (48x48px 2x2 thumbnail preview)
            // =================================================================
            let (pill_height, pill_visible) = if has_selected_child {
                (px(40.0), true)
            } else if has_unread_child {
                (px(8.0), true)
            } else {
                (px(0.0), false)
            };

            let mut container = div()
                .id(self.id.clone())
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .w(rail_width()) // 72px
                .h(px(56.0))
                .cursor_pointer();

            if let Some(on_toggle) = self.on_toggle {
                container = container.on_click(move |ev, window, cx| {
                    (on_toggle)(ev, window, cx);
                });
            }

            // Left white indicator pill
            if pill_visible {
                let pill = div()
                    .absolute()
                    .left_0()
                    .w(px(4.0))
                    .bg(crate::tokens::colors::white())
                    .rounded_r(radius_sm())
                    .h(pill_height);
                container = container.child(pill);
            }

            // Folder Icon Box with 2x2 Miniatures
            let mut folder_box = div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_center()
                .content_center()
                .w(server_icon_size()) // 48px
                .h(server_icon_size()) // 48px
                .p(px(4.0))
                .gap(px(2.0))
                .rounded(radius_server_default()) // 24px circle
                .bg(folder_bg)
                .border_1()
                .border_color(Rgba {
                    r: base_color.r,
                    g: base_color.g,
                    b: base_color.b,
                    a: 0.60,
                })
                .overflow_hidden()
                .hover(move |s| {
                    s.rounded(radius_server_hover()) // Squircle morph on hover!
                        .bg(Rgba {
                            r: base_color.r,
                            g: base_color.g,
                            b: base_color.b,
                            a: 0.60,
                        })
                });

            // Mini thumbnails (up to 4)
            for child in self.guilds.iter().take(4) {
                let mut thumb = div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded(px(4.0))
                    .bg(rgb(HEX_BG_SERVER))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center();

                if let Some(ref url) = child.icon_url {
                    if !url.is_empty() {
                        thumb = thumb.child(
                            img(url.clone())
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        );
                    } else {
                        let single_letter: String = child.initials.chars().take(1).collect();
                        thumb = thumb.child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(crate::tokens::colors::white())
                                .child(single_letter),
                        );
                    }
                } else {
                    let single_letter: String = child.initials.chars().take(1).collect();
                    thumb = thumb.child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(crate::tokens::colors::white())
                            .child(single_letter),
                    );
                }

                folder_box = folder_box.child(thumb);
            }

            let mut wrapper = div().relative().child(folder_box);

            if total_mentions > 0 {
                wrapper = wrapper.child(
                    div()
                        .absolute()
                        .bottom(px(-4.0))
                        .right(px(-4.0))
                        .child(MentionBadge::new(total_mentions).with_cutout(bg_server())),
                );
            }

            container.child(wrapper)
        } else {
            // =================================================================
            // EXPANDED FOLDER (Vertical capsule with child guilds)
            // =================================================================
            let folder_track_bg = Rgba {
                r: base_color.r,
                g: base_color.g,
                b: base_color.b,
                a: 0.20,
            };

            let mut expanded_container = div()
                .id(self.id.clone())
                .flex()
                .flex_col()
                .items_center()
                .w(rail_width())
                .py(px(4.0))
                .gap(px(6.0))
                .rounded(radius_lg())
                .bg(folder_track_bg);

            // Folder Header toggle button
            let mut folder_header = div()
                .id(ElementId::Name(format!("{}-header", self.id).into()))
                .flex()
                .items_center()
                .justify_center()
                .w(server_icon_size())
                .h(px(32.0))
                .rounded(radius_sm())
                .bg(folder_bg)
                .cursor_pointer()
                .hover(|s| s.bg(with_alpha(HEX_BG_CHAT, 0.8)))
                .child(
                    icon_folder(px(16.0), crate::tokens::colors::white()),
                );

            if let Some(on_toggle) = self.on_toggle {
                folder_header = folder_header.on_click(move |ev, window, cx| {
                    (on_toggle)(ev, window, cx);
                });
            }

            expanded_container = expanded_container.child(folder_header);

            // Render each child guild
            let on_child_click = self.on_child_click.map(Arc::new);
            for (idx, child) in self.guilds.into_iter().enumerate() {
                let mut child_item = GuildItem::new(child.id, child.name)
                    .selected(child.selected)
                    .unread(child.unread)
                    .mention_count(child.mention_count);

                if let Some(url) = child.icon_url {
                    child_item = child_item.icon_url(url);
                }

                if let Some(ref click_handler) = on_child_click {
                    let handler = click_handler.clone();
                    child_item = child_item.on_click(move |ev, window, cx| {
                        (handler)(idx, ev, window, cx);
                    });
                }

                expanded_container = expanded_container.child(child_item);
            }

            expanded_container
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_compute_initials() {
        assert_eq!(GuildItem::compute_initials("Rust Brasil"), "RB");
        assert_eq!(GuildItem::compute_initials("Rust"), "Rus");
        assert_eq!(GuildItem::compute_initials(""), "?");
        assert_eq!(
            GuildItem::compute_initials("Very Long Guild Name In Discord"),
            "VLG"
        );
    }

    #[::core::prelude::v1::test]
    fn test_guild_item_builder() {
        let item = GuildItem::new("item-1", "Rust Lang")
            .selected(true)
            .unread(true)
            .mention_count(5)
            .is_home(false)
            .icon_url("https://example.com/icon.png");

        assert_eq!(item.get_name(), "Rust Lang");
        assert_eq!(item.get_initials(), "RL");
        assert!(item.is_selected());
        assert!(item.is_unread());
        assert_eq!(item.get_mention_count(), 5);
        assert!(!item.is_home_button());
        assert_eq!(item.get_icon_url(), Some("https://example.com/icon.png"));
    }

    #[::core::prelude::v1::test]
    fn test_guild_folder_item() {
        let child1 = GuildFolderChild::new("child-1", "Child One")
            .unread(true)
            .mention_count(2);

        let child2 = GuildFolderChild::new("child-2", "Child Two")
            .selected(true)
            .mention_count(3);

        let folder = GuildFolderItem::new("folder-1", "My Folder")
            .color(blurple())
            .expanded(false)
            .child_guild(child1)
            .child_guild(child2);

        assert_eq!(folder.get_name(), "My Folder");
        assert!(!folder.is_expanded());
        assert_eq!(folder.get_guilds().len(), 2);
        assert!(folder.is_unread());
        assert_eq!(folder.get_mention_count(), 5);
    }

    #[::core::prelude::v1::test]
    fn test_guild_folder_explicit_mention_count() {
        let child1 = GuildFolderChild::new("c1", "G1").mention_count(2);
        let folder = GuildFolderItem::new("f1", "Folder")
            .mention_count(10)
            .child_guild(child1);

        assert_eq!(folder.get_mention_count(), 10);
    }
}
