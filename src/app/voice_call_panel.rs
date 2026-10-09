use gpui::*;
use super::state::AppState;
use crate::tokens::icons::*;

/// Dedicated voice call room / stage panel displaying:
/// - Connected channel status and RTC ping
/// - Participant grid with green speaking border indicators
/// - Audio and media controls (Mute, Deafen, Screen Share, Disconnect)
pub struct VoiceCallPanel {
    state: Entity<AppState>,
}

impl VoiceCallPanel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_this, _state, cx| {
            cx.notify();
        })
        .detach();
        Self { state }
    }
}

impl Render for VoiceCallPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let active_call = state.active_voice_call.clone();
        let app_entity = self.state.clone();

        if let Some(call) = active_call {
            let entity = app_entity.clone();
            let is_muted = call.is_muted;
            let is_deafened = call.is_deafened;
            let is_screen_sharing = call.is_screen_sharing;
            let participants = call.participants.clone();

            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(rgb(0x111214))
                .justify_between()
                .p(px(16.0))
                // =========================================================================
                // TOP BAR: Voice Status & Ping
                // =========================================================================
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(icon_speaker(px(20.0), rgb(0xf2f3f5)))
                                        .child(
                                            div()
                                                .text_color(rgb(0xf2f3f5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(18.0))
                                                .child(call.channel_name.clone()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_color(rgb(0x80848e))
                                        .text_size(px(13.0))
                                        .child(format!("• {}", call.guild_name)),
                                ),
                        )
                        // RTC Connection & Ping Badge
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .px(px(8.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .bg(rgb(0x1e1f22))
                                .child(icon_wifi(px(14.0), rgb(0x23a55a)))
                                .child(
                                    div()
                                        .text_color(rgb(0x23a55a))
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(12.0))
                                        .child(format!("RTC {}ms", call.ping_ms)),
                                ),
                        ),
                )
                // =========================================================================
                // CENTER: Participant Video/Avatar Tiles
                // =========================================================================
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .justify_center()
                                .gap(px(16.0))
                                .children(participants.into_iter().map(|participant| {
                                    let is_speaking = participant.is_speaking;
                                    let p_muted = participant.is_muted;

                                    div()
                                        .relative()
                                        .w(px(260.0))
                                        .h(px(170.0))
                                        .rounded(px(8.0))
                                        .bg(rgb(0x2b2d31))
                                        .border_2()
                                        .border_color(if is_speaking {
                                            rgb(0x23a55a)
                                        } else {
                                            rgb(0x2b2d31)
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        // Participant Avatar
                                        .child(
                                            div()
                                                .w(px(72.0))
                                                .h(px(72.0))
                                                .rounded_full()
                                                .bg(rgb(0x5865f2))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_color(rgb(0xffffff))
                                                .text_size(px(32.0))
                                                .font_weight(FontWeight::BOLD)
                                                .child(participant.avatar_text),
                                        )
                                        // Bottom name badge
                                        .child(
                                            div()
                                                .absolute()
                                                .bottom(px(8.0))
                                                .left(px(8.0))
                                                .px(px(8.0))
                                                .py(px(2.0))
                                                .rounded(px(4.0))
                                                .bg(rgb(0x111214))
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xf2f3f5))
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(participant.name),
                                                )
                                                .children(if p_muted {
                                                    Some(icon_mic_off(px(14.0), rgb(0xf23f43)))
                                                } else {
                                                    None
                                                })
                                        )
                                })
                        )
                    )
                )
                // =========================================================================
                // BOTTOM CONTROL BAR: Mute, Deafen, Screen Share, Disconnect
                // =========================================================================
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(12.0))
                                .p(px(8.0))
                                .rounded(px(12.0))
                                .bg(rgb(0x1e1f22))
                                // Mute Button
                                .child({
                                    let entity = entity.clone();
                                    div()
                                        .w(px(44.0))
                                        .h(px(44.0))
                                        .rounded_full()
                                        .bg(if is_muted {
                                            rgb(0xf23f43)
                                        } else {
                                            rgb(0x313338)
                                        })
                                        .text_color(rgb(0xffffff))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0x3f4248)))
                                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                            entity.update(cx, |state, cx| {
                                                state.toggle_mute();
                                                cx.notify();
                                            });
                                        })
                                        .child(if is_muted { icon_mic_off(px(20.0), rgb(0xffffff)) } else { icon_mic(px(20.0), rgb(0xffffff)) })
                                })
                                // Deafen Button
                                .child({
                                    let entity = entity.clone();
                                    div()
                                        .w(px(44.0))
                                        .h(px(44.0))
                                        .rounded_full()
                                        .bg(if is_deafened {
                                            rgb(0xf23f43)
                                        } else {
                                            rgb(0x313338)
                                        })
                                        .text_color(rgb(0xffffff))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0x3f4248)))
                                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                            entity.update(cx, |state, cx| {
                                                state.toggle_deafen();
                                                cx.notify();
                                            });
                                        })
                                        .child(if is_deafened { icon_headphones_off(px(20.0), rgb(0xffffff)) } else { icon_headphones(px(20.0), rgb(0xffffff)) })
                                })
                                // Screen Share Button
                                .child(
                                    div()
                                        .h(px(44.0))
                                        .px(px(16.0))
                                        .rounded_full()
                                        .bg(if is_screen_sharing {
                                            rgb(0x23a55a)
                                        } else {
                                            rgb(0x313338)
                                        })
                                        .text_color(rgb(0xffffff))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0x3f4248)))
                                        .child(icon_screen_share(px(20.0), rgb(0xffffff)))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child("Compartilhar Tela"),
                                        ),
                                )
                                // Disconnect Button (Red)
                                .child({
                                    let entity = entity.clone();
                                    div()
                                        .h(px(44.0))
                                        .px(px(18.0))
                                        .rounded_full()
                                        .bg(rgb(0xda373c))
                                        .text_color(rgb(0xffffff))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(8.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0xf23f43)))
                                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                                            entity.update(cx, |state, cx| {
                                                state.disconnect_voice();
                                                cx.notify();
                                            });
                                        })
                                        .child(icon_phone_hangup(px(20.0), rgb(0xffffff)))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::BOLD)
                                                .child("Desconectar"),
                                        )
                                }),
                        ),
                )
        } else {
            // Not connected to voice
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(0x80848e))
                .child("Nenhuma chamada de voz conectada.")
        }
    }
}
