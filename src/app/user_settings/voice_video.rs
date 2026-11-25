use gpui::prelude::*;
use gpui::*;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::media::MediaDeviceInfo;
use crate::tokens::icons::icon_check;
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_slider_row};
use super::UserSettingsModal;

pub fn render_voice_video_expanded(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let in_vol = modal_state.input_volume;
    let out_vol = modal_state.output_volume;
    let is_testing = modal_state.is_testing_mic;
    let devices_error = modal_state.devices_error.clone();

    let mut root = div().flex().flex_col().gap(px(20.0));

    if let Some(error) = devices_error {
        root = root.child(render_devices_warning(&error, cx));
    }

    root.child(
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(render_section_header("VOICE", cx))
                    .child({
                        let modal_c = modal.clone();
                        Button::new("btn-refresh-devices", "Atualizar dispositivos")
                            .variant(ButtonVariant::Gray)
                            .size(ButtonSize::Sm)
                            .on_click(move |_ev, _win, cx| {
                                modal_c.update(cx, |this, cx| {
                                    this.refresh_devices();
                                    cx.notify();
                                });
                            })
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(20.0))
                    .child(render_input_device_selector(modal.clone(), modal_state, cx))
                    .child(render_output_device_selector(modal.clone(), modal_state, cx)),
            ),
    )
    .child(
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child({
                let modal_c = modal.clone();
                render_slider_row(
                    "MICROPHONE VOLUME",
                    in_vol,
                    move |v, _win, cx| {
                        modal_c.update(cx, |this, cx| {
                            this.input_volume = v;
                            cx.notify();
                        });
                    },
                    cx,
                )
            })
            .child({
                let modal_c = modal.clone();
                render_slider_row(
                    "SPEAKER VOLUME",
                    out_vol,
                    move |v, _win, cx| {
                        modal_c.update(cx, |this, cx| {
                            this.output_volume = v;
                            cx.notify();
                        });
                    },
                    cx,
                )
            }),
    )
    .child(
        div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(render_section_header("MIC TEST", cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.0))
                    .child({
                        let modal_c = modal.clone();
                        Button::new(
                            "btn-mic-test",
                            if is_testing { "Stop Testing" } else { "Mic Test" },
                        )
                        .variant(if is_testing { ButtonVariant::Danger } else { ButtonVariant::Blurple })
                        .size(ButtonSize::Sm)
                        .on_click(move |_ev, _win, cx| {
                            modal_c.update(cx, |this, cx| {
                                this.is_testing_mic = !this.is_testing_mic;
                                cx.notify();
                            });
                        })
                    })
                    // Empty meter: no fabricated audio level. A real RMS meter is
                    // shown only by the voice call panel once capture is active.
                    .child(
                        div()
                            .flex_1()
                            .h(px(8.0))
                            .rounded_full()
                            .bg(cx.discord().borders),
                    ),
            )
            .children(if is_testing {
                Some(
                    div()
                        .text_color(cx.discord().text_secondary)
                        .text_size(px(12.0))
                        .child("O nível do microfone é exibido durante uma chamada com captura de áudio ativa."),
                )
            } else {
                None
            }),
    )
    .child(render_divider(cx))
    .child(render_video_section(modal, modal_state, cx))
}

fn render_devices_warning(error: &str, cx: &App) -> Div {
    div()
        .p(px(10.0))
        .rounded(px(6.0))
        .bg(cx.discord().bg_channels)
        .border_1()
        .border_color(cx.discord().status_dnd)
        .text_color(cx.discord().status_dnd)
        .text_size(px(12.0))
        .child(format!("Falha ao enumerar hardware: {error}"))
}

fn render_input_device_selector(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    render_device_selector(
        modal,
        "MICROPHONE",
        "Nenhum microfone encontrado",
        true,
        modal_state.input_devices.clone(),
        modal_state.selected_input_device.clone(),
        modal_state.input_device_menu_open,
        cx,
    )
}

fn render_output_device_selector(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    render_device_selector(
        modal,
        "SPEAKER",
        "Nenhuma saída de áudio encontrada",
        false,
        modal_state.output_devices.clone(),
        modal_state.selected_output_device.clone(),
        modal_state.output_device_menu_open,
        cx,
    )
}

fn render_device_selector(
    modal: Entity<UserSettingsModal>,
    title: &'static str,
    empty_message: &'static str,
    is_input: bool,
    devices: Vec<MediaDeviceInfo>,
    selected: Option<String>,
    open: bool,
    cx: &App,
) -> Div {
    let selected_label = selected
        .as_deref()
        .and_then(|id| devices.iter().find(|d| d.id == id).map(|d| d.name.clone()))
        .or_else(|| selected.clone())
        .unwrap_or_else(|| "Padrão do sistema".to_string());

    let toggle_modal = modal.clone();
    let menu_modal = modal.clone();

    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(render_section_header(title, cx))
        .child(
            div()
                .h(px(40.0))
                .px(px(12.0))
                .rounded(px(4.0))
                .bg(cx.discord().bg_server)
                .border_1()
                .border_color(if open { cx.discord().blurple } else { cx.discord().bg_server })
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .text_color(cx.discord().text_primary)
                .text_size(px(13.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                    toggle_modal.update(cx, |this, cx| {
                        if is_input {
                            this.input_device_menu_open = !this.input_device_menu_open;
                        } else {
                            this.output_device_menu_open = !this.output_device_menu_open;
                        }
                        cx.notify();
                    });
                })
                .child(div().truncate().child(selected_label))
                .child("⌵"),
        )
        .children(open.then(|| {
            render_audio_device_menu(
                menu_modal,
                devices,
                selected,
                is_input,
                empty_message,
                cx,
            )
        }))
}

fn render_audio_device_menu(
    modal: Entity<UserSettingsModal>,
    devices: Vec<MediaDeviceInfo>,
    selected: Option<String>,
    is_input: bool,
    empty_message: &'static str,
    cx: &App,
) -> Div {
    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .p(px(6.0))
        .rounded(px(6.0))
        .bg(cx.discord().bg_floating)
        .border_1()
        .border_color(cx.discord().borders);

    if devices.is_empty() {
        menu = menu.child(
            div()
                .px(px(6.0))
                .py(px(4.0))
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(empty_message),
        );
    }

    let default_modal = modal.clone();
    menu = menu.child(
        render_device_option(
            "Padrão do sistema",
            selected.is_none(),
            move |cx| {
                default_modal.update(cx, |this, cx| {
                    if is_input {
                        this.input_device_menu_open = false;
                        this.selected_input_device = None;
                        this.state.update(cx, |state, cx| {
                            state.set_input_device(None);
                            cx.notify();
                        });
                    } else {
                        this.output_device_menu_open = false;
                        this.selected_output_device = None;
                        this.state.update(cx, |state, cx| {
                            state.set_output_device(None);
                            cx.notify();
                        });
                    }
                    cx.notify();
                });
            },
            cx,
        ),
    );

    for device in devices {
        let is_selected = selected.as_deref() == Some(device.id.as_str());
        let device_id = device.id.clone();
        let device_name = device.name.clone();
        let option_modal = modal.clone();
        menu = menu.child(render_device_option(
            device_name,
            is_selected,
            move |cx| {
                let id = device_id.clone();
                option_modal.update(cx, |this, cx| {
                    if is_input {
                        this.input_device_menu_open = false;
                        this.selected_input_device = Some(id.clone());
                        this.state.update(cx, |state, cx| {
                            state.set_input_device(Some(id));
                            cx.notify();
                        });
                    } else {
                        this.output_device_menu_open = false;
                        this.selected_output_device = Some(id.clone());
                        this.state.update(cx, |state, cx| {
                            state.set_output_device(Some(id));
                            cx.notify();
                        });
                    }
                    cx.notify();
                });
            },
            cx,
        ));
    }

    menu
}

fn render_device_option<F>(
    label: impl Into<SharedString>,
    is_selected: bool,
    on_select: F,
    cx: &App,
) -> Div
where
    F: Fn(&mut App) + 'static,
{
    let label: SharedString = label.into();
    let hover_bg = cx.discord().channel_hover;
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(px(6.0))
        .py(px(5.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            on_select(cx);
        })
        .child(
            div()
                .truncate()
                .text_color(if is_selected {
                    cx.discord().text_primary
                } else {
                    cx.discord().text_secondary
                })
                .text_size(px(13.0))
                .child(label),
        )
        .children(is_selected.then(|| icon_check(px(14.0), cx.discord().status_online)))
}

fn render_video_section(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let selected = modal_state.selected_camera_device.clone();
    let devices = modal_state.camera_devices.clone();
    let open = modal_state.camera_device_menu_open;

    let selected_label = selected
        .as_deref()
        .and_then(|id| devices.iter().find(|d| d.id == id).map(|d| d.name.clone()))
        .or_else(|| selected.clone())
        .unwrap_or_else(|| "Padrão do sistema".to_string());

    let toggle_modal = modal.clone();
    let menu_modal = modal.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(render_section_header("VIDEO", cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(render_section_header("CAMERA", cx))
                .child(
                    div()
                        .h(px(40.0))
                        .px(px(12.0))
                        .rounded(px(4.0))
                        .bg(cx.discord().bg_server)
                        .border_1()
                        .border_color(if open {
                            cx.discord().blurple
                        } else {
                            cx.discord().bg_server
                        })
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .text_color(cx.discord().text_primary)
                        .text_size(px(13.0))
                        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                            toggle_modal.update(cx, |this, cx| {
                                this.camera_device_menu_open = !this.camera_device_menu_open;
                                cx.notify();
                            });
                        })
                        .child(div().truncate().child(selected_label))
                        .child("⌵"),
                )
                .children(open.then(|| {
                    render_camera_device_menu(menu_modal, devices, selected, cx)
                })),
        )
}

fn render_camera_device_menu(
    modal: Entity<UserSettingsModal>,
    devices: Vec<crate::capture::CameraInfo>,
    selected: Option<String>,
    cx: &App,
) -> Div {
    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .p(px(6.0))
        .rounded(px(6.0))
        .bg(cx.discord().bg_floating)
        .border_1()
        .border_color(cx.discord().borders);

    if devices.is_empty() {
        menu = menu.child(
            div()
                .px(px(6.0))
                .py(px(4.0))
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child("Nenhuma câmera encontrada"),
        );
    }

    let default_modal = modal.clone();
    menu = menu.child(render_device_option(
        "Padrão do sistema",
        selected.is_none(),
        move |cx| {
            default_modal.update(cx, |this, cx| {
                this.camera_device_menu_open = false;
                this.selected_camera_device = None;
                this.state.update(cx, |state, cx| {
                    state.set_camera_device(None);
                    cx.notify();
                });
                cx.notify();
            });
        },
        cx,
    ));

    for camera in devices {
        let is_selected = selected.as_deref() == Some(camera.id.as_str());
        let camera_id = camera.id.clone();
        let camera_name = camera.name.clone();
        let option_modal = modal.clone();
        menu = menu.child(render_device_option(
            camera_name,
            is_selected,
            move |cx| {
                let id = camera_id.clone();
                option_modal.update(cx, |this, cx| {
                    this.camera_device_menu_open = false;
                    this.selected_camera_device = Some(id.clone());
                    this.state.update(cx, |state, cx| {
                        state.set_camera_device(Some(id));
                        cx.notify();
                    });
                    cx.notify();
                });
            },
            cx,
        ));
    }

    menu
}
