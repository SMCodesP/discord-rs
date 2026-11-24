use gpui::prelude::*;
use gpui::*;
use gpui::component::switch::Switch;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::tokens::kit_theme::DiscordTheme;

/// Uppercase section header in muted secondary text.
pub fn render_section_header(title: &'static str, cx: &App) -> Div {
    div()
        .text_color(cx.discord().text_secondary)
        .font_weight(FontWeight::BOLD)
        .text_size(px(11.0))
        .child(title)
}

/// Descriptive paragraph in muted secondary text.
pub fn render_section_description(desc: &'static str, cx: &App) -> Div {
    div()
        .text_color(cx.discord().text_secondary)
        .text_size(px(13.0))
        .child(desc)
}

/// 1px horizontal divider line.
pub fn render_divider(cx: &App) -> Div {
    div()
        .w_full()
        .h(px(1.0))
        .my(px(8.0))
        .bg(cx.discord().neutral_67)
}

/// Standard settings toggle row with title, description, and gpui-kit Switch.
pub fn render_toggle_row<F>(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    is_checked: bool,
    on_toggle: F,
    cx: &App,
) -> Div
where
    F: Fn(&bool, &mut Window, &mut App) + 'static,
{
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .py(px(10.0))
        .child(
            div()
                .flex_1()
                .pr(px(16.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_color(cx.discord().text_titles)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.0))
                        .child(title),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child(description),
                ),
        )
        .child(
            Switch::new(id)
                .checked(is_checked)
                .on_click(on_toggle)
        )
}

/// Account details row: label, value, and action button.
pub fn render_info_row<F>(
    label: &'static str,
    value: &str,
    action_label: &'static str,
    button_id: &'static str,
    on_action: F,
    cx: &App,
) -> Div
where
    F: Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child(label),
                )
                .child(
                    div()
                        .text_color(cx.discord().text_primary)
                        .text_size(px(13.0))
                        .child(value.to_string()),
                ),
        )
        .child(
            Button::new(button_id, action_label)
                .variant(ButtonVariant::Gray)
                .size(ButtonSize::Sm)
                .on_click(on_action)
        )
}

/// Slider control with percentage display and visual track.
pub fn render_slider_row<F>(
    label: &'static str,
    value: u32,
    set_action: F,
    cx: &App,
) -> Div
where
    F: Fn(u32, &mut Window, &mut App) + 'static + Clone,
{
    let fill_ratio = (value as f32 / 100.0).clamp(0.0, 1.0);
    let track_width_px = 440.0;
    let filled_px = track_width_px * fill_ratio;

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(11.0))
                        .child(label),
                )
                .child(
                    div()
                        .text_color(cx.discord().blurple)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(12.0))
                        .child(format!("{}%", value)),
                ),
        )
        .child(
            div()
                .w(px(track_width_px))
                .h(px(8.0))
                .rounded_full()
                .bg(cx.discord().borders)
                .relative()
                .flex()
                .items_center()
                .child(
                    div()
                        .h(px(8.0))
                        .w(px(filled_px))
                        .rounded_full()
                        .bg(cx.discord().blurple),
                )
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_full()
                        .bg(cx.discord().white)
                        .border_2()
                        .border_color(cx.discord().blurple)
                        .ml(px(-8.0)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .pt(px(2.0))
                .child(render_slider_preset_btn("0%", 0, set_action.clone(), cx))
                .child(render_slider_preset_btn("25%", 25, set_action.clone(), cx))
                .child(render_slider_preset_btn("50%", 50, set_action.clone(), cx))
                .child(render_slider_preset_btn("75%", 75, set_action.clone(), cx))
                .child(render_slider_preset_btn("100%", 100, set_action, cx))
        )
}

fn render_slider_preset_btn<F>(
    text: &'static str,
    val: u32,
    action: F,
    cx: &App,
) -> Div
where
    F: Fn(u32, &mut Window, &mut App) + 'static,
{
    let hover_bg = cx.discord().channel_hover;
    let hover_text = cx.discord().text_titles;

    div()
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(cx.discord().bg_channels)
        .text_color(cx.discord().text_secondary)
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(hover_text))
        .on_mouse_down(MouseButton::Left, move |_ev, win, cx| {
            action(val, win, cx);
        })
        .child(text)
}
