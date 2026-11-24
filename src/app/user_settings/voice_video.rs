use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use gpui::component::scroll::ScrollableElement;
use gpui::prelude::*;
use gpui::*;

use crate::capture::CameraManager;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::media::AudioEngine;
use crate::media::devices::{MediaDeviceInfo, MediaDeviceManager};
use crate::tokens::kit_theme::DiscordTheme;

use super::components::{render_divider, render_section_header, render_slider_row};
use super::{DeviceMenu, UserSettingsModal};

pub(super) struct MicTestSession {
    stop_tx: Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MicTestSession {
    fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        // Dropping the JoinHandle detaches the worker. The worker owns the audio
        // engine and releases its streams immediately after observing stop.
        self.handle.take();
    }
}

impl Drop for MicTestSession {
    fn drop(&mut self) {
        self.stop();
    }
}

enum MicTestEvent {
    Level(f32),
    Error(String),
}

#[derive(Clone)]
struct DeviceOption {
    id: String,
    name: String,
    is_default: bool,
}

impl UserSettingsModal {
    pub(super) fn refresh_devices(&mut self) {
        self.stop_mic_test();
        let manager = MediaDeviceManager::new();
        let mut errors = Vec::new();

        let input_devices = match manager.list_input_devices() {
            Ok(devices) => devices,
            Err(error) => {
                errors.push(format!("Microfones: {error}"));
                Vec::new()
            }
        };
        let output_devices = match manager.list_output_devices() {
            Ok(devices) => devices,
            Err(error) => {
                errors.push(format!("Alto-falantes: {error}"));
                Vec::new()
            }
        };
        let cameras = match CameraManager::list_devices() {
            Ok(devices) => devices,
            Err(error) => {
                errors.push(format!("Câmeras: {error}"));
                Vec::new()
            }
        };

        self.selected_input_device =
            retain_or_default(self.selected_input_device.take(), &input_devices);
        self.selected_output_device =
            retain_or_default(self.selected_output_device.take(), &output_devices);
        if self
            .selected_camera_device
            .as_ref()
            .is_some_and(|id| !cameras.iter().any(|camera| &camera.id == id))
        {
            self.selected_camera_device = None;
        }

        self.input_devices = input_devices;
        self.output_devices = output_devices;
        self.cameras = cameras;
        self.device_error = if errors.is_empty() {
            None
        } else {
            Some(errors.join(" • "))
        };
        self.devices_loaded = true;
        self.device_menu = None;
    }

    pub(super) fn sync_active_call_devices(&mut self, cx: &mut Context<Self>) {
        if !self.devices_loaded || self.pending_device.is_some() {
            return;
        }

        let (session, current_input, current_output) = {
            let state = self.state.read(cx);
            let Some(call) = state.active_voice_call.as_ref() else {
                return;
            };
            (
                call.voice_session.clone(),
                call.selected_input_device.clone(),
                call.selected_output_device.clone(),
            )
        };
        let Some(session) = session else {
            return;
        };

        if let Some(id) = self.selected_input_device.clone() {
            if current_input.as_deref() != Some(id.as_str()) {
                self.select_input_device(id, cx);
                return;
            }
        }
        if let Some(id) = self.selected_output_device.clone() {
            if current_output.as_deref() != Some(id.as_str()) {
                self.select_output_device(id, cx);
            }
        }
    }

    pub(super) fn select_input_device(&mut self, id: String, cx: &mut Context<Self>) {
        if self.pending_device.is_some() {
            return;
        }
        if !self.input_devices.iter().any(|device| device.id == id) {
            self.device_error = Some("O microfone selecionado não está mais disponível.".into());
            cx.notify();
            return;
        }

        self.stop_mic_test();
        self.device_menu = None;
        let state_entity = self.state.clone();
        let session = state_entity
            .read(cx)
            .active_voice_call
            .as_ref()
            .and_then(|call| call.voice_session.clone());

        if let Some(session) = session {
            self.pending_device = Some(DeviceMenu::Input);
            let entity = cx.entity();
            self.device_task = Some(cx.spawn(move |_view, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let result = session.switch_input_device(Some(id.clone())).await;
                    let _ = cx.update(|cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.pending_device = None;
                            match result {
                                Ok(()) => {
                                    this.selected_input_device = Some(id.clone());
                                    this.device_error = None;
                                    state_entity.update(cx, |state, cx| {
                                        if let Some(call) = state.active_voice_call.as_mut() {
                                            call.selected_input_device = Some(id);
                                        }
                                        cx.notify();
                                    });
                                }
                                Err(error) => {
                                    this.device_error = Some(format!(
                                        "Não foi possível trocar o microfone: {error}"
                                    ));
                                }
                            }
                            cx.notify();
                        });
                    });
                }
            }));
        } else if state_entity.read(cx).active_voice_call.is_some() {
            self.device_error = Some(
                "A sessão de áudio da chamada ainda não está disponível; o dispositivo anterior foi mantido."
                    .into(),
            );
        } else {
            let mut manager = MediaDeviceManager::new();
            match manager.select_input_device(Some(&id)) {
                Ok(()) => {
                    self.selected_input_device = Some(id);
                    self.device_error = None;
                }
                Err(error) => {
                    self.device_error =
                        Some(format!("Não foi possível validar o microfone: {error}"));
                }
            }
        }
        cx.notify();
    }

    pub(super) fn select_output_device(&mut self, id: String, cx: &mut Context<Self>) {
        if self.pending_device.is_some() {
            return;
        }
        if !self.output_devices.iter().any(|device| device.id == id) {
            self.device_error = Some("O alto-falante selecionado não está mais disponível.".into());
            cx.notify();
            return;
        }

        self.device_menu = None;
        let state_entity = self.state.clone();
        let session = state_entity
            .read(cx)
            .active_voice_call
            .as_ref()
            .and_then(|call| call.voice_session.clone());

        if let Some(session) = session {
            self.pending_device = Some(DeviceMenu::Output);
            let entity = cx.entity();
            self.device_task = Some(cx.spawn(move |_view, cx: &mut AsyncApp| {
                let cx = cx.clone();
                async move {
                    let result = session.switch_output_device(Some(id.clone())).await;
                    let _ = cx.update(|cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.pending_device = None;
                            match result {
                                Ok(()) => {
                                    this.selected_output_device = Some(id.clone());
                                    this.device_error = None;
                                    state_entity.update(cx, |state, cx| {
                                        if let Some(call) = state.active_voice_call.as_mut() {
                                            call.selected_output_device = Some(id);
                                        }
                                        cx.notify();
                                    });
                                }
                                Err(error) => {
                                    this.device_error = Some(format!(
                                        "Não foi possível trocar o alto-falante: {error}"
                                    ));
                                }
                            }
                            cx.notify();
                        });
                    });
                }
            }));
        } else if state_entity.read(cx).active_voice_call.is_some() {
            self.device_error = Some(
                "A sessão de áudio da chamada ainda não está disponível; o dispositivo anterior foi mantido."
                    .into(),
            );
        } else {
            let mut manager = MediaDeviceManager::new();
            match manager.select_output_device(Some(&id)) {
                Ok(()) => {
                    self.selected_output_device = Some(id);
                    self.device_error = None;
                }
                Err(error) => {
                    self.device_error =
                        Some(format!("Não foi possível validar o alto-falante: {error}"));
                }
            }
        }
        cx.notify();
    }

    pub(super) fn select_camera_device(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.cameras.iter().any(|camera| camera.id == id) {
            self.device_error = Some("A câmera selecionada não está mais disponível.".into());
            cx.notify();
            return;
        }
        self.selected_camera_device = Some(id.clone());
        self.device_menu = None;
        self.device_error = None;
        self.state.update(cx, |state, cx| {
            state.set_camera_device(Some(id));
            cx.notify();
        });
        cx.notify();
    }

    pub(super) fn start_mic_test(&mut self, cx: &mut Context<Self>) {
        if self.mic_test.is_some() {
            self.stop_mic_test();
            cx.notify();
            return;
        }

        let selected_input = self
            .selected_input_device
            .clone()
            .or_else(|| default_device(&self.input_devices));
        if selected_input.is_none() {
            self.mic_test_error =
                Some("Nenhum microfone real está disponível para o teste.".into());
            cx.notify();
            return;
        }

        self.mic_test_error = None;
        self.mic_level = 0.0;
        let (stop_tx, stop_rx) = mpsc::channel();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        let handle = match std::thread::Builder::new()
            .name("discord-settings-mic-test".into())
            .spawn(move || {
                let mut engine = AudioEngine::new();
                if let Err(error) = engine.switch_input_device(selected_input.as_deref()) {
                    let _ = event_tx.send(MicTestEvent::Error(format!(
                        "Não foi possível selecionar o microfone: {error}"
                    )));
                    return;
                }
                if let Err(error) = engine.start_session() {
                    let _ = event_tx.send(MicTestEvent::Error(format!(
                        "Não foi possível iniciar a captura: {error}"
                    )));
                    return;
                }

                loop {
                    match stop_rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    let level = engine.audio_level().clamp(0.0, 1.0);
                    if event_tx.send(MicTestEvent::Level(level)).is_err() {
                        break;
                    }
                }
                engine.stop_session();
            }) {
            Ok(handle) => handle,
            Err(error) => {
                self.mic_test_error = Some(format!(
                    "Não foi possível iniciar a thread do teste de microfone: {error}"
                ));
                cx.notify();
                return;
            }
        };

        self.mic_test = Some(MicTestSession {
            stop_tx,
            handle: Some(handle),
        });

        let entity = cx.entity();
        self.mic_test_task = Some(cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Some(event) = event_rx.recv().await {
                    let _ = cx.update(|cx| {
                        let _ = entity.update(cx, |this, cx| {
                            match event {
                                MicTestEvent::Level(level) => this.mic_level = level,
                                MicTestEvent::Error(error) => {
                                    this.mic_level = 0.0;
                                    this.mic_test_error = Some(error);
                                    this.mic_test = None;
                                }
                            }
                            cx.notify();
                        });
                    });
                }
            }
        }));
        cx.notify();
    }
}

fn retain_or_default(selected: Option<String>, devices: &[MediaDeviceInfo]) -> Option<String> {
    if let Some(selected) = selected {
        if devices.iter().any(|device| device.id == selected) {
            return Some(selected);
        }
    }
    default_device(devices)
}

fn default_device(devices: &[MediaDeviceInfo]) -> Option<String> {
    devices
        .iter()
        .find(|device| device.is_default)
        .map(|device| device.id.clone())
}

pub fn render_voice_video_expanded(
    modal: Entity<UserSettingsModal>,
    modal_state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let input_options: Vec<_> = modal_state
        .input_devices
        .iter()
        .map(|device| DeviceOption {
            id: device.id.clone(),
            name: device.name.clone(),
            is_default: device.is_default,
        })
        .collect();
    let output_options: Vec<_> = modal_state
        .output_devices
        .iter()
        .map(|device| DeviceOption {
            id: device.id.clone(),
            name: device.name.clone(),
            is_default: device.is_default,
        })
        .collect();
    let camera_options: Vec<_> = modal_state
        .cameras
        .iter()
        .map(|camera| DeviceOption {
            id: camera.id.clone(),
            name: camera.name.clone(),
            is_default: false,
        })
        .collect();

    let is_testing = modal_state.mic_test.is_some();
    let mic_width = 440.0 * modal_state.mic_level.clamp(0.0, 1.0);

    div()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(render_voice_section(
            modal.clone(),
            modal_state,
            input_options,
            output_options,
            cx,
        ))
        .child(render_volume_section(modal.clone(), modal_state, cx))
        .child(render_mic_test_section(
            modal.clone(),
            modal_state,
            is_testing,
            mic_width,
            cx,
        ))
        .child(render_divider(cx))
        .child(render_video_section(modal, modal_state, camera_options, cx))
}

fn render_voice_section(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    input_options: Vec<DeviceOption>,
    output_options: Vec<DeviceOption>,
    cx: &App,
) -> Div {
    let input_toggle = modal.clone();
    let input_select = modal.clone();
    let output_toggle = modal.clone();
    let output_select = modal.clone();
    let refresh = modal.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(render_section_header("VOZ", cx))
                .child(
                    Button::new("btn-refresh-settings-devices", "Atualizar dispositivos")
                        .variant(ButtonVariant::Gray)
                        .size(ButtonSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            refresh.update(cx, |this, cx| {
                                this.refresh_devices();
                                cx.notify();
                            });
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(20.0))
                .child(render_device_selector(
                    "MICROFONE",
                    "Nenhum microfone encontrado",
                    input_options,
                    state.selected_input_device.as_deref(),
                    state.device_menu == Some(DeviceMenu::Input),
                    state.pending_device == Some(DeviceMenu::Input),
                    move |_window, cx| {
                        input_toggle.update(cx, |this, cx| {
                            this.device_menu = if this.device_menu == Some(DeviceMenu::Input) {
                                None
                            } else {
                                Some(DeviceMenu::Input)
                            };
                            cx.notify();
                        });
                    },
                    move |id, _window, cx| {
                        input_select.update(cx, |this, cx| {
                            this.select_input_device(id, cx);
                        });
                    },
                    cx,
                ))
                .child(render_device_selector(
                    "ALTO-FALANTE",
                    "Nenhum alto-falante encontrado",
                    output_options,
                    state.selected_output_device.as_deref(),
                    state.device_menu == Some(DeviceMenu::Output),
                    state.pending_device == Some(DeviceMenu::Output),
                    move |_window, cx| {
                        output_toggle.update(cx, |this, cx| {
                            this.device_menu = if this.device_menu == Some(DeviceMenu::Output) {
                                None
                            } else {
                                Some(DeviceMenu::Output)
                            };
                            cx.notify();
                        });
                    },
                    move |id, _window, cx| {
                        output_select.update(cx, |this, cx| {
                            this.select_output_device(id, cx);
                        });
                    },
                    cx,
                )),
        )
        .children(state.device_error.as_ref().map(|error| {
            div()
                .text_color(cx.discord().status_dnd)
                .text_size(px(12.0))
                .child(error.clone())
        }))
}

fn render_volume_section(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    cx: &App,
) -> Div {
    let input_modal = modal.clone();
    let output_modal = modal;

    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(render_slider_row(
            "VOLUME DO MICROFONE",
            state.input_volume,
            &state.input_volume_slider,
            move |value, _window, cx| {
                input_modal.update(cx, |this, cx| {
                    this.set_input_volume(value, cx);
                    cx.notify();
                });
            },
            cx,
        ))
        .child(render_slider_row(
            "VOLUME DO ALTO-FALANTE",
            state.output_volume,
            &state.output_volume_slider,
            move |value, _window, cx| {
                output_modal.update(cx, |this, cx| {
                    this.set_output_volume(value, cx);
                    cx.notify();
                });
            },
            cx,
        ))
        .children(state.volume_error.as_ref().map(|error| {
            div()
                .text_color(cx.discord().status_dnd)
                .text_size(px(12.0))
                .child(error.clone())
        }))
}

fn render_mic_test_section(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    is_testing: bool,
    mic_width: f32,
    cx: &App,
) -> Div {
    let test_modal = modal.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(render_section_header("TESTE DE MICROFONE", cx))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.0))
                .child(
                    Button::new(
                        "btn-mic-test",
                        if is_testing {
                            "Parar teste"
                        } else {
                            "Testar microfone"
                        },
                    )
                    .variant(if is_testing {
                        ButtonVariant::Danger
                    } else {
                        ButtonVariant::Blurple
                    })
                    .size(ButtonSize::Sm)
                    .disabled(state.selected_input_device.is_none())
                    .on_click(move |_event, _window, cx| {
                        test_modal.update(cx, |this, cx| this.start_mic_test(cx));
                    }),
                )
                .child(
                    div()
                        .w(px(440.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(cx.discord().borders)
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(px(mic_width))
                                .rounded_full()
                                .bg(cx.discord().status_online),
                        ),
                ),
        )
        .children(state.mic_test_error.as_ref().map(|error| {
            div()
                .text_color(cx.discord().status_dnd)
                .text_size(px(12.0))
                .child(error.clone())
        }))
}

fn render_video_section(
    modal: Entity<UserSettingsModal>,
    state: &UserSettingsModal,
    camera_options: Vec<DeviceOption>,
    cx: &App,
) -> Div {
    let camera_toggle = modal.clone();
    let camera_select = modal;

    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(render_section_header("VÍDEO", cx))
        .child(render_device_selector(
            "CÂMERA",
            "Nenhuma câmera encontrada",
            camera_options,
            state.selected_camera_device.as_deref(),
            state.device_menu == Some(DeviceMenu::Camera),
            false,
            move |_window, cx| {
                camera_toggle.update(cx, |this, cx| {
                    this.device_menu = if this.device_menu == Some(DeviceMenu::Camera) {
                        None
                    } else {
                        Some(DeviceMenu::Camera)
                    };
                    cx.notify();
                });
            },
            move |id, _window, cx| {
                camera_select.update(cx, |this, cx| {
                    this.select_camera_device(id, cx);
                });
            },
            cx,
        ))
        .child(render_camera_capabilities(state, cx))
        .child(
            div()
                .p(px(12.0))
                .rounded(px(6.0))
                .bg(cx.discord().bg_channels)
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(
                    "A prévia de vídeo e os perfis de processamento não são simulados. A transmissão de vídeo ainda não está disponível neste cliente.",
                ),
        )
}

fn render_camera_capabilities(state: &UserSettingsModal, cx: &App) -> Div {
    let selected = state
        .selected_camera_device
        .as_ref()
        .and_then(|id| state.cameras.iter().find(|camera| &camera.id == id));

    let description = selected.map(|camera| {
        let mut resolutions = Vec::new();
        let mut frame_rates = Vec::new();
        for capability in &camera.capabilities {
            let resolution = format!("{}×{}", capability.width, capability.height);
            if !resolutions.contains(&resolution) {
                resolutions.push(resolution);
            }
            for fps in &capability.available_fps {
                if !frame_rates.contains(fps) {
                    frame_rates.push(*fps);
                }
            }
        }
        frame_rates.sort_unstable();
        if resolutions.is_empty() {
            "O driver não informou resoluções para esta câmera.".to_string()
        } else {
            format!(
                "Resoluções: {} • FPS: {}",
                resolutions.join(", "),
                frame_rates
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    });

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_color(cx.discord().text_titles)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(13.0))
                .child("CAPACIDADES REAIS DA CÂMERA"),
        )
        .child(
            div()
                .text_color(cx.discord().text_secondary)
                .text_size(px(12.0))
                .child(description.unwrap_or_else(|| {
                    "Selecione uma câmera para ver as capacidades informadas pelo driver.".into()
                })),
        )
}

fn render_device_selector<F, G>(
    label: &'static str,
    empty_text: &'static str,
    options: Vec<DeviceOption>,
    selected: Option<&str>,
    is_open: bool,
    is_pending: bool,
    on_toggle: F,
    on_select: G,
    cx: &App,
) -> Div
where
    F: Fn(&mut Window, &mut App) + 'static,
    G: Fn(String, &mut Window, &mut App) + 'static + Clone,
{
    let selected_name = selected
        .and_then(|id| options.iter().find(|option| option.id == id))
        .map(|option| {
            if option.is_default {
                format!("{} (Padrão)", option.name)
            } else {
                option.name.clone()
            }
        });

    let mut selector = div()
        .relative()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(render_section_header(label, cx))
        .child(
            div()
                .relative()
                .h(px(40.0))
                .px(px(12.0))
                .rounded(px(4.0))
                .bg(cx.discord().bg_server)
                .border_1()
                .border_color(cx.discord().neutral_67)
                .flex()
                .items_center()
                .justify_between()
                .text_color(cx.discord().text_primary)
                .text_size(px(13.0))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    cx.stop_propagation();
                    on_toggle(window, cx);
                })
                .child(div().truncate().child(if is_pending {
                    "Aplicando...".to_string()
                } else {
                    selected_name.unwrap_or_else(|| empty_text.to_string())
                }))
                .child(if options.is_empty() { "" } else { "⌵" }),
        );

    if is_open && !options.is_empty() {
        let mut menu = div()
            .absolute()
            .top(px(60.0))
            .left(px(0.0))
            .w_full()
            .max_h(px(240.0))
            .overflow_y_scrollbar()
            .p(px(4.0))
            .rounded(px(6.0))
            .bg(cx.discord().bg_floating)
            .border_1()
            .border_color(cx.discord().borders)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(2.0));

        for option in options {
            let id = option.id.clone();
            let name = if option.is_default {
                format!("{} (Padrão)", option.name)
            } else {
                option.name
            };
            let is_selected = selected == Some(id.as_str());
            let on_select = on_select.clone();
            let hover = cx.discord().channel_hover;
            menu = menu.child(
                div()
                    .px(px(8.0))
                    .py(px(7.0))
                    .rounded(px(4.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .text_color(if is_selected {
                        cx.discord().text_titles
                    } else {
                        cx.discord().text_secondary
                    })
                    .text_size(px(13.0))
                    .hover(move |style| style.bg(hover))
                    .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        cx.stop_propagation();
                        on_select(id.clone(), window, cx);
                    })
                    .child(name)
                    .children(if is_selected {
                        Some(div().text_color(cx.discord().status_online).child("✓"))
                    } else {
                        None
                    }),
            );
        }
        selector = selector.child(menu);
    }

    selector
}
