use gpui::prelude::*;
use gpui::*;

use crate::tokens::icons::icon_info;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::render_section_description;
use super::UserSettingsModal;

pub fn render_keybinds(
    _modal: Entity<UserSettingsModal>,
    _modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        // Section: Custom Keybinds
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(20.0))
                        .child("Custom Keybinds"),
                )
                .child(render_section_description(
                    "Keybinds are custom keyboard shortcuts that let you perform specific actions in Discord without needing to click through menus.",
                    cx,
                ))
                .child(
                    div()
                        .w_full()
                        .p(px(12.0))
                        .rounded(px(6.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().borders)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.0))
                        .child(icon_info(px(18.0), cx.discord().blurple))
                        .child(
                            div()
                                .w(px(580.0))
                                .text_color(cx.discord().text_primary)
                                .text_size(px(13.0))
                                .child("Custom keybinds are not currently supported in the browser. Download the desktop application to use custom keybinds."),
                        ),
                ),
        )
        // Section: Default Keybinds
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Default Keybinds"),
                )
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .child(render_keybind_row("Show Keyboard Shortcuts List", &["CTRL", "/"], cx)),
                ),
        )
        // Section: Messages
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(16.0))
                        .child("Messages"),
                )
                .child(render_section_description(
                    "These shortcuts work when focused on a message.",
                    cx,
                ))
                .child(
                    div()
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .child(render_keybind_row("Edit Message", &["E"], cx))
                        .child(render_keybind_row("Delete Message", &["BACKSPACE"], cx)),
                ),
        )
}

fn render_keybind_row(
    action: &'static str,
    keys: &[&'static str],
    cx: &App,
) -> Div {
    let hover_bg = cx.discord().channel_hover;

    div()
        .h(px(44.0))
        .px(px(16.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .hover(move |s| s.bg(hover_bg))
        // Action Label
        .child(
            div()
                .text_color(cx.discord().text_titles)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(14.0))
                .child(action),
        )
        // Key Pills
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .children(keys.iter().map(|key| {
                    div()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .bg(cx.discord().bg_server)
                        .border_1()
                        .border_color(cx.discord().borders)
                        .text_color(cx.discord().text_primary)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child(*key)
                })),
        )
}
