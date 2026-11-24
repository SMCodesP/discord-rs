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
    let notify_streams = modal_read.notify_streams;
    let notify_anniversaries = modal_read.notify_anniversaries;
    let notify_online = modal_read.notify_friends_online;
    let notify_events = modal_read.notify_server_events;
    let notify_profile = modal_read.notify_profile_updates;
    let reaction_scope = modal_read.notify_reactions;
    let sound_message = modal_read.sound_message;
    let sound_voice_join = modal_read.sound_voice_join;
    let sound_mute = modal_read.sound_mute_toggle;
    let sound_deafen = modal_read.sound_deafen_toggle;

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
                        notify_streams,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.notify_streams = !this.notify_streams;
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
                        notify_anniversaries,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.notify_anniversaries = !this.notify_anniversaries;
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
                        notify_online,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.notify_friends_online = !this.notify_friends_online;
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
                        notify_events,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.notify_server_events = !this.notify_server_events;
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
                        notify_profile,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.notify_profile_updates = !this.notify_profile_updates;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                // Someone reacts to my messages
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
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
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(render_reaction_choice(
                                    modal.clone(),
                                    reaction_scope,
                                    0,
                                    "Todas as mensagens",
                                    cx,
                                ))
                                .child(render_reaction_choice(
                                    modal.clone(),
                                    reaction_scope,
                                    1,
                                    "@Menções",
                                    cx,
                                ))
                                .child(render_reaction_choice(
                                    modal.clone(),
                                    reaction_scope,
                                    2,
                                    "Nada",
                                    cx,
                                )),
                        ),
                ),
        )
        .child(render_divider(cx))
        // -------------------------------------------------------------
        // Sounds Section
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(render_section_header("SOUNDS", cx))
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-sound-message",
                        "Message sent",
                        "",
                        sound_message,
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
                        "toggle-sound-voice-join",
                        "User joins voice",
                        "",
                        sound_voice_join,
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
                        "toggle-sound-mute",
                        "Mute / Unmute",
                        "",
                        sound_mute,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_mute_toggle = !this.sound_mute_toggle;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-sound-deafen",
                        "Deafen / Undeafen",
                        "",
                        sound_deafen,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.sound_deafen_toggle = !this.sound_deafen_toggle;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
}

fn render_reaction_choice(
    modal: Entity<UserSettingsModal>,
    current: u8,
    value: u8,
    label: &'static str,
    cx: &App,
) -> Div {
    let is_selected = current == value;
    let bg = if is_selected {
        cx.discord().blurple
    } else {
        cx.discord().bg_channels
    };
    let text = if is_selected {
        cx.discord().white
    } else {
        cx.discord().text_secondary
    };
    let hover_bg = cx.discord().channel_hover;

    div()
        .px(px(10.0))
        .py(px(5.0))
        .rounded(px(4.0))
        .bg(bg)
        .text_color(text)
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            modal.update(cx, |this, cx| {
                this.notify_reactions = value;
                cx.notify();
            });
        })
        .child(label)
}
