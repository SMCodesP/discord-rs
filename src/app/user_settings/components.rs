use gpui::component::slider::Slider;
use gpui::component::slider::SliderState;
use gpui::component::switch::Switch;
use gpui::prelude::*;
use gpui::*;

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
        .child(Switch::new(id).checked(is_checked).on_click(on_toggle))
}

/// Account details row: label and authoritative value.
pub fn render_info_row(label: &'static str, value: &str, cx: &App) -> Div {
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
}

/// Slider control bound to a real `SliderState` with percentage display and presets.
pub fn render_slider_row<F>(
    label: &'static str,
    value: Option<u32>,
    slider: &Entity<SliderState>,
    set_action: F,
    cx: &App,
) -> Div
where
    F: Fn(u32, &mut Window, &mut App) + 'static + Clone,
{
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
                        .child(value.map_or_else(
                            || "Não configurado".to_string(),
                            |value| format!("{value}%"),
                        )),
                ),
        )
        .child(
            Slider::new(slider)
                .w(px(440.0))
                .h(px(24.0))
                .bg(cx.discord().blurple)
                .text_color(cx.discord().white)
                .disabled(value.is_none()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.0))
                .pt(px(2.0))
                .child(render_slider_preset_btn(
                    "0%",
                    0,
                    slider,
                    set_action.clone(),
                    cx,
                ))
                .child(render_slider_preset_btn(
                    "25%",
                    25,
                    slider,
                    set_action.clone(),
                    cx,
                ))
                .child(render_slider_preset_btn(
                    "50%",
                    50,
                    slider,
                    set_action.clone(),
                    cx,
                ))
                .child(render_slider_preset_btn(
                    "75%",
                    75,
                    slider,
                    set_action.clone(),
                    cx,
                ))
                .child(render_slider_preset_btn(
                    "100%", 100, slider, set_action, cx,
                )),
        )
}

fn render_slider_preset_btn<F>(
    text: &'static str,
    val: u32,
    slider: &Entity<SliderState>,
    action: F,
    cx: &App,
) -> Div
where
    F: Fn(u32, &mut Window, &mut App) + 'static,
{
    let hover_bg = cx.discord().channel_hover;
    let hover_text = cx.discord().text_titles;
    let slider = slider.clone();

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
            slider.update(cx, |state, cx| {
                state.set_value(val as f32, win, cx);
            });
            action(val, win, cx);
        })
        .child(text)
}
