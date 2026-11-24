use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_toggle_row};
use super::UserSettingsModal;

pub fn render_notifications(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let modal_read = modal_state;
    let desk = modal_read.desktop_notifications;
    let s_stream = modal_read.unread_badge;
    let s_anniv = modal_read.flash_taskbar;
    let s_online = modal_read.sound_message;
    let s_event = modal_read.sound_voice_join;
    let s_update = modal_read.sound_mute_toggle;

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        // -------------------------------------------------------------
        // Overview Section (settings-notifications.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("OVERVIEW", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-desk-notif",
                        "Enable Desktop Notifications",
                        "If you're looking for per-channel or per-server notifications, right-click the desired server icon and select Notification Settings.",
                        desk,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.desktop_notifications = !this.desktop_notifications;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        .child(render_divider(cx))
        // -------------------------------------------------------------
        // Notify me when... Section (settings-notifications.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(render_section_header("NOTIFY ME WHEN...", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-notif-stream",
                        "People I know start streaming in small servers",
                        "",
                        s_stream,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.unread_badge = !this.unread_badge;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-notif-anniv",
                        "A friend and I reach a friendship anniversary",
                        "",
                        s_anniv,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.flash_taskbar = !this.flash_taskbar;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-notif-online",
                        "Friends come online",
                        "",
                        s_online,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_message = !this.sound_message;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-notif-event",
                        "A server has an upcoming event",
                        "",
                        s_event,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_voice_join = !this.sound_voice_join;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-notif-update",
                        "Friends update their profile",
                        "",
                        s_update,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_mute_toggle = !this.sound_mute_toggle;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                // Someone reacts to my messages dropdown
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .py(px(10.0))
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child("Someone reacts to my messages"),
                        )
                        .child(
                            div()
                                .h(px(36.0))
                                .px(px(12.0))
                                .rounded(px(4.0))
                                .bg(cx.discord().bg_server)
                                .border_1()
                                .border_color(cx.discord().neutral_67)
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .text_color(cx.discord().text_primary)
                                .text_size(px(13.0))
                                .child("All Messages")
                                .child("⌵"),
                        ),
                ),
        )
}
