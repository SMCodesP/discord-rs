use gpui::prelude::*;
use gpui::*;

use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_toggle_row};
use super::UserSettingsModal;

pub fn render_privacy(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let modal_read = modal_state;
    let tel_improve = modal_read.telemetry_improve;
    let tel_person = modal_read.telemetry_personalize;
    let allow_dm = modal_read.allow_dm_from_members;
    let allow_req = modal_read.allow_friend_requests_from_members;

    div()
        .flex()
        .flex_col()
        .gap(px(16.0))
        // -------------------------------------------------------------
        // How Discord Uses My Data (settings-data-privacy.png)
        // -------------------------------------------------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(render_section_header("HOW DISCORD USES MY DATA", cx))
                // Info block: Use data to make Discord work
                .child(
                    div()
                        .p(px(14.0))
                        .rounded(px(8.0))
                        .bg(cx.discord().bg_channels)
                        .border_1()
                        .border_color(cx.discord().neutral_67)
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_color(cx.discord().text_titles)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(14.0))
                                .child("Use data to make Discord work"),
                        )
                        .child(
                            div()
                                .text_color(cx.discord().text_secondary)
                                .text_size(px(12.0))
                                .child("Discord needs to store and process some data in order to provide you the basic Discord service, such as your messages, what servers you’re in and your Direct Messages. By using Discord, you allow Discord to provide this basic service. You can stop this by Disabling or Deleting your account."),
                        ),
                )
                // Toggle: Use data to improve Discord
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-tel-improve",
                        "Use data to improve Discord",
                        "Allow Discord to use and process my information to understand and improve Discord’s services.",
                        tel_improve,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.telemetry_improve = !this.telemetry_improve;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                // Toggle: Use data to personalize my Discord experience
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-tel-person",
                        "Use data to personalize my Discord experience",
                        "Allows Discord to use information, such as who you talk to and what games you play, to personalize Discord for you.",
                        tel_person,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.telemetry_personalize = !this.telemetry_personalize;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                // Toggle: Use my Discord activity to personalize Sponsored Content
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-sponsored-content",
                        "Use my Discord activity to personalize Sponsored Content",
                        "Allows Discord to personalize Sponsored Content, like Quests, using your Discord activity, such as the games you play. If you opt out you may still see Quests, but they won't be personalized.",
                        allow_dm,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.allow_dm_from_members = !this.allow_dm_from_members;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                })
                // Toggle: Allow my voice to be recorded in Clips
                .child({
                    let modal_c = modal.clone();
                    render_toggle_row(
                        "toggle-clips-voice",
                        "Allow my voice to be recorded in Clips",
                        "Allows your voice to be included when someone in the same voice channel uses Clips.",
                        allow_req,
                        move |_checked, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.allow_friend_requests_from_members = !this.allow_friend_requests_from_members;
                                cx.notify();
                            });
                        },
                        cx,
                    )
                }),
        )
        .child(render_divider(cx))
}
