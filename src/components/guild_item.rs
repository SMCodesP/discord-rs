//! Discord RS - Guild Item Component
//! Server icon in the 72px vertical rail with the measured 40px squircle and
//! an 8px indicator pill on the left.
//! Visual sources:
//! - /tmp/opencode/discord-shell-measurements.md (Discord Desktop darker shell)
//! - docs/PARITY_MASTER_TASKS.md T-071
//!
//! The official icon is clipped by a 40x40 SVG squircle mask (curves at
//! (0,17.4545)->(17.4545,0)). GPUI has no image clip mask here, so rendering
//! keeps an explicit rounded-rectangle approximation while exposing the
//! measured squircle contour for tests/comparison.

use gpui::component::tooltip::Tooltip;
use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::components::badge::MentionBadge;
use crate::tokens::colors::{white, *};
use crate::tokens::icons::*;
use crate::tokens::spacing::*;
use crate::tokens::typography::*;

/// A cubic bezier segment of the guild icon squircle contour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SquircleCubic {
    pub from: (f32, f32),
    pub c1: (f32, f32),
    pub c2: (f32, f32),
    pub to: (f32, f32),
}

/// Returns the measured 40x40 Discord guild icon squircle contour as 4 cubic
/// bezier segments (one per corner), clockwise from the top-left corner.
///
/// Source: /tmp/opencode/discord-shell-measurements.md — SVG mask curves at
/// (0,17.4545)->(17.4545,0), symmetric across 40x40.
pub fn guild_icon_squircle_segments(size: f32) -> [SquircleCubic; 4] {
    let s = size;
    let c = 17.4545_f32 / 40.0_f32 * s;
    let k = 8.0_f32 / 40.0_f32 * s; // control handle depth approximation
    [
        // top-left: (0,c) -> (c,0)
        SquircleCubic {
            from: (0.0, c),
            c1: (0.0, k),
            c2: (k, 0.0),
            to: (c, 0.0),
        },
        // top-right: (s-c,0) -> (s,c)
        SquircleCubic {
            from: (s - c, 0.0),
            c1: (s - k, 0.0),
            c2: (s, k),
            to: (s, c),
        },
        // bottom-right: (s,s-c) -> (s-c,s)
        SquircleCubic {
            from: (s, s - c),
            c1: (s, s - k),
            c2: (s - k, s),
            to: (s - c, s),
        },
        // bottom-left: (c,s) -> (0,s-c)
        SquircleCubic {
            from: (c, s),
            c1: (k, s),
            c2: (0.0, s - k),
            to: (0.0, s - c),
        },
    ]
}

/// Builds a GPUI path for the measured guild icon squircle contour.
pub fn guild_icon_squircle_path(size: Pixels) -> gpui::Path<Pixels> {
    let s = f32::from(size);
    let segs = guild_icon_squircle_segments(s);
    let mut b = gpui::PathBuilder::fill();
    b.move_to(point(px(segs[0].from.0), px(segs[0].from.1)));
    b.cubic_bezier_to(
        point(px(segs[0].to.0), px(segs[0].to.1)),
        point(px(segs[0].c1.0), px(segs[0].c1.1)),
        point(px(segs[0].c2.0), px(segs[0].c2.1)),
    );
    b.line_to(point(px(segs[1].from.0), px(segs[1].from.1)));
    b.cubic_bezier_to(
        point(px(segs[1].to.0), px(segs[1].to.1)),
        point(px(segs[1].c1.0), px(segs[1].c1.1)),
        point(px(segs[1].c2.0), px(segs[1].c2.1)),
    );
    b.line_to(point(px(segs[2].from.0), px(segs[2].from.1)));
    b.cubic_bezier_to(
        point(px(segs[2].to.0), px(segs[2].to.1)),
        point(px(segs[2].c1.0), px(segs[2].c1.1)),
        point(px(segs[2].c2.0), px(segs[2].c2.1)),
    );
    b.line_to(point(px(segs[3].from.0), px(segs[3].from.1)));
    b.cubic_bezier_to(
        point(px(segs[3].to.0), px(segs[3].to.1)),
        point(px(segs[3].c1.0), px(segs[3].c1.1)),
        point(px(segs[3].c2.0), px(segs[3].c2.1)),
    );
    b.close();
    b.build().expect("guild icon squircle path")
}

/// Server navigation item in the 72px vertical guild rail (icon 40x40px)
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

    /// Uses the acronym supplied by the guild payload for the no-image state.
    pub fn initials(mut self, initials: impl Into<SharedString>) -> Self {
        let initials: SharedString = initials.into();
        if !initials.trim().is_empty() {
            self.initials = initials;
        }
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
        let initials: String = words
            .iter()
            .filter_map(|w| w.chars().next())
            .take(3)
            .collect();
        initials.into()
    }
}

impl RenderOnce for GuildItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        render_guild_item(self)
    }
}

fn guild_icon_image_source(url: &str) -> gpui::ImageSource {
    if url.starts_with('/') {
        std::path::PathBuf::from(url).into()
    } else if let Some(stripped) = url.strip_prefix("file://") {
        std::path::PathBuf::from(stripped).into()
    } else {
        url.to_string().into()
    }
}

fn guild_initials(initials: SharedString, text_size: Pixels) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(text_size)
        .font_weight(WEIGHT_MEDIUM)
        .line_height(line_height_sm())
        .text_color(white())
        .child(initials)
        .into_any_element()
}

fn guild_icon_radius(_selected: bool) -> Pixels {
    // The measured client uses the same 40px SVG squircle mask in every
    // state. GPUI has no equivalent image mask here, so 16px is the explicit
    // rounded-rectangle approximation already used by this component.
    radius_server_hover()
}

fn guild_pill_height(selected: bool, unread: bool) -> Pixels {
    if selected {
        guild_pill_selected_height()
    } else if unread {
        guild_pill_unread_height()
    } else {
        px(0.0)
    }
}

fn resolved_guild_pill_hover_height(selected: bool) -> Pixels {
    if selected {
        guild_pill_selected_height()
    } else {
        guild_pill_hover_height()
    }
}

fn render_guild_item(item: GuildItem) -> AnyElement {
    // Measured Discord Desktop: 40px server icon with the same squircle mask
    // at rest, hover and selected. Each item needs its own hover group so one
    // hovered server never changes every server in the rail.
    let icon_radius = guild_icon_radius(item.selected);
    let icon_bg = if item.selected { blurple() } else { bg_chat() };
    let tooltip_name = item.name.clone();
    let group_name: SharedString = format!("guild-item-{:?}", item.id).into();
    let image_id = format!("guild-icon-image-{:?}", item.id);

    let mut item_container = div()
        .id(item.id.clone())
        .group(group_name.clone())
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(rail_width())
        .h(guild_item_height())
        .cursor_pointer()
        .tooltip(move |window, cx| Tooltip::new(tooltip_name.clone()).build(window, cx));

    if let Some(on_click) = item.on_click {
        item_container = item_container.on_click(move |ev, window, cx| {
            (on_click)(ev, window, cx);
        });
    }

    // Measured Discord Desktop indicator: 8px wide, right corners rounded,
    // 8px unread, 20px hovered and 40px selected. The measured CSS offset is
    // normalized to the rail's left edge because the rail clips overflow.
    let pill = div()
        .absolute()
        .left_0()
        .w(guild_pill_width())
        .h(guild_pill_height(
            item.selected,
            item.unread || item.mention_count > 0,
        ))
        .bg(white())
        .rounded_r(radius_sm())
        .group_hover(group_name.clone(), |style| {
            style.h(resolved_guild_pill_hover_height(item.selected))
        });
    item_container = item_container.child(pill);

    let mut icon_box = div()
        .flex()
        .items_center()
        .justify_center()
        .w(server_icon_size())
        .h(server_icon_size())
        .rounded(icon_radius)
        .bg(icon_bg)
        .overflow_hidden()
        .flex_shrink_0();

    if !item.selected {
        icon_box = icon_box.group_hover(group_name.clone(), |style| {
            style.rounded(radius_server_hover()).bg(blurple())
        });
    }

    if item.is_home {
        icon_box = icon_box.child(icon_discord_logo(px(24.0), white()));
    } else if let Some(url) = &item.icon_url {
        if !url.is_empty() {
            let fallback = item.initials.clone();
            let loading_fallback = fallback.clone();
            let mut icon_img = img(guild_icon_image_source(url))
                .id(image_id)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .rounded(icon_radius)
                .with_loading(move || guild_initials(loading_fallback.clone(), font_size_sm()))
                .with_fallback(move || guild_initials(fallback.clone(), font_size_sm()));

            if !item.selected {
                icon_img = icon_img.group_hover(group_name.clone(), |style| {
                    style.rounded(radius_server_hover())
                });
            }
            icon_box = icon_box.child(icon_img);
        } else {
            icon_box = icon_box.child(guild_initials(item.initials.clone(), font_size_sm()));
        }
    } else {
        icon_box = icon_box.child(guild_initials(item.initials.clone(), font_size_sm()));
    }

    let mut icon_wrapper = div().relative().child(icon_box);

    if item.mention_count > 0 {
        icon_wrapper = icon_wrapper.child(
            div()
                .absolute()
                .bottom(px(-4.0))
                .right(px(-4.0))
                .child(MentionBadge::new(item.mention_count).with_cutout(bg_server())),
        );
    }

    item_container.child(icon_wrapper).into_any_element()
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

    /// Sets folder accent color
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
        self.unread || self.guilds.iter().any(|g| g.unread || g.mention_count > 0)
    }

    pub fn get_mention_count(&self) -> u32 {
        if self.mention_count > 0 {
            self.mention_count
        } else {
            self.guilds.iter().fold(0u32, |total, guild| {
                total.saturating_add(guild.mention_count)
            })
        }
    }
}

impl RenderOnce for GuildFolderItem {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if !self.expanded {
            render_folder_collapsed(self)
        } else {
            render_folder_expanded(self)
        }
    }
}

fn folder_preview_initials(letter: SharedString) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .font_weight(WEIGHT_MEDIUM)
        .text_color(white())
        .child(letter)
        .into_any_element()
}

fn render_folder_collapsed(folder: GuildFolderItem) -> AnyElement {
    let has_selected_child = folder.guilds.iter().any(|g| g.selected);
    let has_unread_child = folder.is_unread();
    let total_mentions = folder.get_mention_count();

    let base_color = folder.color.unwrap_or_else(bg_channels);
    let folder_bg = Rgba {
        r: base_color.r,
        g: base_color.g,
        b: base_color.b,
        a: 0.40,
    };

    let group_name: SharedString = format!("guild-folder-{:?}", folder.id).into();
    let mut container = div()
        .id(folder.id.clone())
        .group(group_name.clone())
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(rail_width())
        .h(guild_folder_size())
        .cursor_pointer();

    if !folder.name.is_empty() {
        let tooltip_text = folder.name.clone();
        container = container
            .tooltip(move |window, cx| Tooltip::new(tooltip_text.clone()).build(window, cx));
    }

    if let Some(on_toggle) = folder.on_toggle {
        container = container.on_click(move |ev, window, cx| {
            (on_toggle)(ev, window, cx);
        });
    }

    let pill = div()
        .absolute()
        .left_0()
        .w(guild_pill_width())
        .h(guild_pill_height(has_selected_child, has_unread_child))
        .bg(white())
        .rounded_r(radius_sm())
        .group_hover(group_name, |style| {
            style.h(resolved_guild_pill_hover_height(has_selected_child))
        });
    container = container.child(pill);

    let mut folder_box = div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_center()
        .content_center()
        .w(guild_folder_size())
        .h(guild_folder_size())
        .rounded(radius_lg())
        .bg(folder_bg)
        .overflow_hidden()
        .hover(move |s| {
            s.rounded(radius_lg()).bg(Rgba {
                r: base_color.r,
                g: base_color.g,
                b: base_color.b,
                a: 0.60,
            })
        });

    // 4px padding + 19px thumbnail + 2px gap + 19px thumbnail + 4px
    // padding = 48px. Using the 40px inner icon size here clipped the second
    // row and column of the measured four-up preview.
    let mut preview = div()
        .grid()
        .grid_cols(2)
        .grid_rows(2)
        .w(guild_folder_size())
        .h(guild_folder_size())
        .p(guild_folder_preview_padding())
        .gap(guild_folder_preview_gap())
        .rounded(radius_lg())
        .overflow_hidden();

    for (idx, child) in folder.guilds.iter().take(4).enumerate() {
        // Measured asymmetric corner radii from Discord DOM (.folderPreviewGuildIcon__48112)
        // Top-left, top-right, bottom-left, bottom-right corners
        let (tl, tr, bl, br) = match idx {
            0 => (px(13.0), px(4.0), px(4.0), px(4.0)),
            1 => (px(4.0), px(13.0), px(4.0), px(4.0)),
            2 => (px(4.0), px(4.0), px(4.0), px(13.0)),
            _ => (px(4.0), px(4.0), px(13.0), px(4.0)),
        };

        let mut thumb = div()
            .w(guild_folder_preview_size())
            .h(guild_folder_preview_size())
            .rounded_tl(tl)
            .rounded_tr(tr)
            .rounded_bl(bl)
            .rounded_br(br)
            .bg(bg_server())
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center();

        let single_letter: SharedString = child.initials.chars().take(1).collect::<String>().into();
        if let Some(ref url) = child.icon_url {
            if !url.is_empty() {
                let loading_fallback = single_letter.clone();
                let error_fallback = single_letter.clone();
                thumb = thumb.child(
                    img(guild_icon_image_source(url))
                        .id(format!("folder-preview-{:?}-{idx}", folder.id))
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .rounded_tl(tl)
                        .rounded_tr(tr)
                        .rounded_bl(bl)
                        .rounded_br(br)
                        .with_loading(move || folder_preview_initials(loading_fallback.clone()))
                        .with_fallback(move || folder_preview_initials(error_fallback.clone())),
                );
            } else {
                thumb = thumb.child(folder_preview_initials(single_letter));
            }
        } else {
            thumb = thumb.child(folder_preview_initials(single_letter));
        }

        preview = preview.child(thumb);
    }

    folder_box = folder_box.child(preview);
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

    container.child(wrapper).into_any_element()
}

fn render_folder_expanded(folder: GuildFolderItem) -> AnyElement {
    let base_color = folder.color.unwrap_or_else(bg_channels);
    let folder_bg = Rgba {
        r: base_color.r,
        g: base_color.g,
        b: base_color.b,
        a: 0.40,
    };
    let folder_track_bg = Rgba {
        r: base_color.r,
        g: base_color.g,
        b: base_color.b,
        a: 0.20,
    };

    let mut expanded_container = div()
        .id(folder.id.clone())
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .w(rail_width())
        .py(space_2())
        .gap(space_3());

    // The expanded folder track is 48px wide and centered inside the 72px
    // rail. Child items remain 72px wide so their selection pills stay at x=0.
    let track = div()
        .absolute()
        .left(px((RAW_RAIL_WIDTH - RAW_GUILD_FOLDER_SIZE) * 0.5))
        .top(space_2())
        .bottom(space_2())
        .w(guild_folder_size())
        .rounded(radius_server_hover())
        .border_1()
        .border_color(border_subtle())
        .bg(folder_track_bg);

    let folder_icon = div()
        .w(guild_folder_preview_container_size())
        .h(guild_folder_preview_container_size())
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius_lg())
        .bg(folder_bg)
        .hover(move |style| {
            style.bg(Rgba {
                r: base_color.r,
                g: base_color.g,
                b: base_color.b,
                a: 0.60,
            })
        })
        .child(icon_folder(px(24.0), white()));

    let mut folder_header = div()
        .id(ElementId::Name(format!("{:?}-header", folder.id).into()))
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(guild_folder_size())
        .h(guild_folder_size())
        .cursor_pointer()
        .child(folder_icon);

    if !folder.name.is_empty() {
        let tooltip_text = folder.name.clone();
        folder_header = folder_header
            .tooltip(move |window, cx| Tooltip::new(tooltip_text.clone()).build(window, cx));
    }

    if let Some(on_toggle) = folder.on_toggle {
        folder_header = folder_header.on_click(move |ev, window, cx| {
            (on_toggle)(ev, window, cx);
        });
    }

    expanded_container = expanded_container.child(track).child(folder_header);

    let on_child_click = folder.on_child_click.map(Arc::new);
    for (idx, child) in folder.guilds.into_iter().enumerate() {
        let mut child_item = GuildItem::new(child.id, child.name)
            .selected(child.selected)
            .unread(child.unread)
            .mention_count(child.mention_count)
            .initials(child.initials);

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

    expanded_container.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn test_guild_icon_radius_and_pill_states() {
        assert_eq!(guild_icon_radius(false), radius_server_hover());
        assert_eq!(guild_icon_radius(true), radius_server_hover());
        assert_eq!(guild_pill_height(false, false), px(0.0));
        assert_eq!(guild_pill_height(false, true), guild_pill_unread_height());
        assert_eq!(guild_pill_height(true, true), guild_pill_selected_height());
        assert_eq!(guild_pill_width(), px(8.0));
        assert_eq!(guild_pill_hover_height(), px(20.0));
        assert_eq!(resolved_guild_pill_hover_height(false), px(20.0));
        assert_eq!(resolved_guild_pill_hover_height(true), px(40.0));
        assert_eq!(server_icon_size(), px(40.0));
        assert_eq!(guild_folder_size(), px(48.0));
        assert_eq!(guild_folder_preview_container_size(), px(40.0));
        assert_eq!(guild_folder_preview_size(), px(19.0));
        assert_eq!(guild_separator_height(), px(1.0));
        assert_eq!(
            guild_folder_preview_padding() * 2.0
                + guild_folder_preview_size() * 2.0
                + guild_folder_preview_gap(),
            guild_folder_size(),
        );
    }

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

    #[::core::prelude::v1::test]
    fn test_mentions_imply_unread_for_guild_and_folder_pills() {
        let child = GuildFolderChild::new("c1", "G1").mention_count(1);
        let folder = GuildFolderItem::new("f1", "Folder").child_guild(child);

        assert!(folder.is_unread());
        assert_eq!(guild_pill_height(false, true), guild_pill_unread_height());
    }
}
