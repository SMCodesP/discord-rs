use std::sync::Arc;
use std::time::Duration;

use gpui::component::Selectable;
use gpui::component::popover::Popover;
use gpui::component::scroll::ScrollableElement;
use gpui::component::tooltip::Tooltip;
use gpui::prelude::*;
use gpui::*;

use super::state::{ActiveVoiceCall, AppState, VoiceParticipantState};
use crate::capture::camera::{CameraInfo, CameraManager, CameraSession};
use crate::capture::screen::{
    ScreenCaptureManager, ScreenCaptureMode, ScreenSession, ScreenSourceType, SessionType,
    detect_session, resolve_capture_mode,
};
use crate::capture::{CaptureError, VideoFrame};
use crate::components::avatar::{Avatar, AvatarSize};
use crate::media::devices::MediaDeviceManager;
use crate::protocol::voice_session::VoiceSessionState;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::kit_theme::DiscordTheme;

const FIRST_PREVIEW_FRAME_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalPreviewSource {
    Camera,
    Screen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalPreviewStatus {
    #[allow(dead_code)]
    Idle,
    Starting,
    Live,
    Failed(String),
}

impl LocalPreviewStatus {
    fn is_active(&self) -> bool {
        matches!(self, Self::Starting | Self::Live)
    }
}

#[derive(Debug, Clone)]
struct LocalPreview {
    source: LocalPreviewSource,
    status: LocalPreviewStatus,
    frame: Option<Arc<RenderImage>>,
    real_source_label: String,
    expanded: bool,
}

impl LocalPreview {
    fn new(source: LocalPreviewSource, real_source_label: impl Into<String>) -> Self {
        Self {
            source,
            status: LocalPreviewStatus::Starting,
            frame: None,
            real_source_label: real_source_label.into(),
            expanded: false,
        }
    }

    #[cfg(test)]
    fn accept_frame(&mut self, frame: &VideoFrame) -> Result<(), CaptureError> {
        self.frame = Some(video_frame_to_render_image(frame)?);
        self.status = LocalPreviewStatus::Live;
        Ok(())
    }

    fn fail(&mut self, error: impl Into<String>) {
        self.frame = None;
        self.expanded = false;
        self.status = LocalPreviewStatus::Failed(error.into());
    }

    fn toggle_expanded(&mut self) -> bool {
        if self.status == LocalPreviewStatus::Live && self.frame.is_some() {
            self.expanded = !self.expanded;
            true
        } else {
            false
        }
    }
}

/// Dedicated voice call room / stage panel displaying:
/// - Connected channel status, real RTC latency (zero mocks), and connection state
/// - Responsive participant stage grid with Avatar component and green speaking ring
/// - Local-only camera/screen preview, spotlight, and media controls
pub struct VoiceCallPanel {
    state: Entity<AppState>,
    local_preview: Option<LocalPreview>,
    camera_session: Option<CameraSession>,
    screen_session: Option<ScreenSession>,
    preview_task: Option<gpui::Task<()>>,
    preview_generation: u64,
    preview_source_id: Option<String>,
    selected_screen_index: Option<usize>,
    cached_audio_device_warning: Option<String>,
    audio_devices_checked_at: Option<std::time::Instant>,
}

impl Drop for VoiceCallPanel {
    fn drop(&mut self) {
        let _ = self.preview_task.take();
        shutdown_preview_sessions(self.camera_session.take(), self.screen_session.take());
    }
}

fn shutdown_preview_sessions(
    camera_session: Option<CameraSession>,
    screen_session: Option<ScreenSession>,
) {
    if camera_session.is_none() && screen_session.is_none() {
        return;
    }
    // CameraSession/ScreenSession::stop can join a capture thread. Move that
    // work off the UI thread both for normal stop and for panel teardown.
    let _ = std::thread::Builder::new()
        .name("discord-preview-shutdown".into())
        .spawn(move || {
            drop(camera_session);
            drop(screen_session);
        });
}

fn enumerate_audio_device_warning() -> Option<String> {
    let device_manager = MediaDeviceManager::new();
    let input_devices = device_manager.list_input_devices();
    let output_devices = device_manager.list_output_devices();
    match (&input_devices, &output_devices) {
        (Err(input_error), Err(output_error)) => Some(format!(
            "Não foi possível enumerar os dispositivos de áudio: entrada: {input_error}; saída: {output_error}"
        )),
        (Err(input_error), _) => Some(format!(
            "Não foi possível enumerar os microfones: {input_error}"
        )),
        (_, Err(output_error)) => Some(format!(
            "Não foi possível enumerar as saídas de áudio: {output_error}"
        )),
        (Ok(inputs), Ok(outputs)) if inputs.is_empty() && outputs.is_empty() => {
            Some("Nenhum microfone ou saída de áudio detectados no sistema.".to_string())
        }
        (Ok(inputs), _) if inputs.is_empty() => {
            Some("Nenhum microfone de entrada detectado no sistema.".to_string())
        }
        (_, Ok(outputs)) if outputs.is_empty() => {
            Some("Nenhum dispositivo de saída de áudio detectado no sistema.".to_string())
        }
        _ => None,
    }
}

fn select_camera_device(
    devices: Vec<CameraInfo>,
    selected_device_id: Option<&str>,
) -> Result<CameraInfo, CaptureError> {
    if devices.is_empty() {
        return Err(CaptureError::DeviceNotFound(
            "Nenhuma câmera real disponível. Conecte um dispositivo e tente novamente.".into(),
        ));
    }

    let selected_device_id = selected_device_id.ok_or_else(|| {
        CaptureError::DeviceNotFound(
            "Nenhuma câmera selecionada; escolha um dispositivo real no seletor.".into(),
        )
    })?;
    devices
        .into_iter()
        .find(|device| device.id == selected_device_id)
        .ok_or_else(|| {
            CaptureError::DeviceNotFound(format!(
                "Câmera selecionada indisponível: {selected_device_id}"
            ))
        })
}

impl VoiceCallPanel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, state, cx| {
            let state = state.read(cx);
            let selected_camera = state
                .active_voice_call
                .as_ref()
                .and_then(|call| call.selected_camera_device.clone());
            let camera_preview_active = this.local_preview.as_ref().is_some_and(|preview| {
                preview.source == LocalPreviewSource::Camera && preview.status.is_active()
            });
            if camera_preview_active
                && this.preview_source_id.as_deref() != selected_camera.as_deref()
            {
                this.start_camera_preview(cx);
            }
            cx.notify();
        })
        .detach();
        Self {
            state,
            local_preview: None,
            camera_session: None,
            screen_session: None,
            preview_task: None,
            preview_generation: 0,
            preview_source_id: None,
            selected_screen_index: None,
            cached_audio_device_warning: None,
            audio_devices_checked_at: None,
        }
    }

    fn stop_local_preview(&mut self, cx: &mut Context<Self>) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let _ = self.preview_task.take();
        shutdown_preview_sessions(self.camera_session.take(), self.screen_session.take());
        clear_local_preview(&mut self.local_preview);
        self.preview_source_id = None;
        cx.notify();
    }

    fn toggle_preview_expanded(&mut self, cx: &mut Context<Self>) {
        if self
            .local_preview
            .as_mut()
            .is_some_and(|preview| preview.toggle_expanded())
        {
            cx.notify();
        }
    }

    fn audio_device_warning(&mut self) -> Option<String> {
        const AUDIO_DEVICE_CACHE_TTL: Duration = Duration::from_secs(5);
        let now = std::time::Instant::now();
        let fresh = self
            .audio_devices_checked_at
            .is_some_and(|checked| now.saturating_duration_since(checked) < AUDIO_DEVICE_CACHE_TTL);
        if !fresh {
            self.cached_audio_device_warning = enumerate_audio_device_warning();
            self.audio_devices_checked_at = Some(now);
        }
        self.cached_audio_device_warning.clone()
    }

    fn start_camera_preview(&mut self, cx: &mut Context<Self>) {
        self.stop_local_preview(cx);
        let generation = self.preview_generation;
        let selected_camera = self
            .state
            .read(cx)
            .active_voice_call
            .as_ref()
            .and_then(|call| call.selected_camera_device.clone());

        if selected_camera.is_none() {
            let mut preview = LocalPreview::new(LocalPreviewSource::Camera, "Câmera");
            preview.fail("Escolha uma câmera real no seletor antes de iniciar a prévia.");
            self.local_preview = Some(preview);
            cx.notify();
            return;
        }

        self.local_preview = Some(LocalPreview::new(LocalPreviewSource::Camera, "Câmera"));
        cx.notify();
        let this_entity = cx.entity().downgrade();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let opened = tokio::task::spawn_blocking(move || {
                    let devices = CameraManager::list_devices()?;
                    let device = select_camera_device(devices, selected_camera.as_deref())?;
                    let mut config = crate::capture::CameraCaptureConfig::default();
                    config.device_id = device.id.clone();
                    let (session, frames) = CameraManager::start_session(config)?;
                    Ok::<_, CaptureError>((session, frames, device.name, device.id))
                })
                .await
                .map_err(|error| {
                    CaptureError::InitializationFailed(format!("Falha ao abrir a câmera: {error}"))
                })
                .and_then(|result| result);

                let (session, frames, label, source_id) = match opened {
                    Ok(opened) => opened,
                    Err(error) => {
                        let _ = cx.update(|cx| {
                            if let Some(entity) = this_entity.upgrade() {
                                entity.update(cx, |panel, cx| {
                                    if panel.preview_generation == generation {
                                        panel.fail_local_preview(error.to_string(), cx);
                                    }
                                });
                            }
                        });
                        return;
                    }
                };

                let mut camera_session = Some(session);
                let accepted = cx.update(|cx| {
                    this_entity
                        .upgrade()
                        .map(|entity| {
                            entity.update(cx, |panel, cx| {
                                if panel.preview_generation != generation {
                                    return false;
                                }
                                panel.camera_session = camera_session.take();
                                panel.preview_source_id = Some(source_id);
                                if let Some(preview) = panel.local_preview.as_mut() {
                                    preview.real_source_label = label;
                                }
                                panel.begin_preview_frames(
                                    frames,
                                    LocalPreviewSource::Camera,
                                    generation,
                                    cx,
                                );
                                true
                            })
                        })
                        .unwrap_or(false)
                });
                if !accepted {
                    shutdown_preview_sessions(camera_session, None);
                }
            }
        })
        .detach();
    }

    fn start_screen_preview(&mut self, cx: &mut Context<Self>) {
        self.stop_local_preview(cx);
        let generation = self.preview_generation;
        let session_type = detect_session();
        let capture_mode = match resolve_capture_mode(ScreenCaptureMode::Auto, session_type) {
            Ok(mode) => mode,
            Err(error) => {
                let mut preview = LocalPreview::new(LocalPreviewSource::Screen, "Tela");
                preview.fail(error.to_string());
                self.local_preview = Some(preview);
                cx.notify();
                return;
            }
        };

        let selected_screen = self.selected_screen_index;
        self.local_preview = Some(LocalPreview::new(
            LocalPreviewSource::Screen,
            screen_preview_label(session_type, selected_screen),
        ));
        cx.notify();
        let this_entity = cx.entity().downgrade();
        cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let opened = async {
                    match session_type {
                        SessionType::Wayland => {
                            let config = crate::capture::ScreenCaptureConfig {
                                source_type: ScreenSourceType::Both,
                                target_fps: 30,
                                mode: capture_mode,
                                screen_index: None,
                            };
                            ScreenCaptureManager::start_capture(config).await.map(
                                |(session, frames)| {
                                    (
                                        session,
                                        frames,
                                        screen_preview_label(session_type, None),
                                        "wayland-portal".to_string(),
                                    )
                                },
                            )
                        }
                        SessionType::X11 => {
                            let screens =
                                tokio::task::spawn_blocking(ScreenCaptureManager::list_x11_screens)
                                    .await
                                    .map_err(|error| {
                                        CaptureError::X11Error(format!(
                                            "Falha ao listar monitores X11: {error}"
                                        ))
                                    })??;
                            let screen_index = selected_screen.unwrap_or(0);
                            if screens.get(screen_index).is_none() {
                                return Err(CaptureError::X11Error(format!(
                                    "Monitor X11 selecionado indisponível: {screen_index}"
                                )));
                            }
                            let label = screen_preview_label(session_type, Some(screen_index));
                            let config = crate::capture::ScreenCaptureConfig {
                                source_type: ScreenSourceType::Monitor,
                                target_fps: 30,
                                mode: capture_mode,
                                screen_index: Some(screen_index),
                            };
                            let (session, frames) =
                                ScreenCaptureManager::start_capture(config).await?;
                            Ok::<_, CaptureError>((
                                session,
                                frames,
                                label,
                                format!("x11-{screen_index}"),
                            ))
                        }
                        SessionType::Unknown => Err(CaptureError::SessionUnavailable(
                            "Sessão de exibição desconhecida".into(),
                        )),
                    }
                }
                .await;

                let (session, frames, label, source_id) = match opened {
                    Ok(opened) => opened,
                    Err(CaptureError::PortalCancelled) => {
                        let _ = cx.update(|cx| {
                            if let Some(entity) = this_entity.upgrade() {
                                entity.update(cx, |panel, cx| {
                                    if panel.preview_generation == generation {
                                        clear_local_preview(&mut panel.local_preview);
                                        panel.preview_source_id = None;
                                        cx.notify();
                                    }
                                });
                            }
                        });
                        return;
                    }
                    Err(error) => {
                        let _ = cx.update(|cx| {
                            if let Some(entity) = this_entity.upgrade() {
                                entity.update(cx, |panel, cx| {
                                    if panel.preview_generation == generation {
                                        panel.fail_local_preview(error.to_string(), cx);
                                    }
                                });
                            }
                        });
                        return;
                    }
                };

                let mut screen_session = Some(session);
                let accepted = cx.update(|cx| {
                    this_entity
                        .upgrade()
                        .map(|entity| {
                            entity.update(cx, |panel, cx| {
                                if panel.preview_generation != generation {
                                    return false;
                                }
                                panel.screen_session = screen_session.take();
                                panel.preview_source_id = Some(source_id);
                                if let Some(preview) = panel.local_preview.as_mut() {
                                    preview.real_source_label = label;
                                }
                                panel.begin_preview_frames(
                                    frames,
                                    LocalPreviewSource::Screen,
                                    generation,
                                    cx,
                                );
                                true
                            })
                        })
                        .unwrap_or(false)
                });
                if !accepted {
                    shutdown_preview_sessions(None, screen_session);
                }
            }
        })
        .detach();
    }

    fn set_screen_index(&mut self, screen_index: usize, cx: &mut Context<Self>) {
        self.selected_screen_index = Some(screen_index);
        let screen_preview_active = self.local_preview.as_ref().is_some_and(|preview| {
            preview.source == LocalPreviewSource::Screen && preview.status.is_active()
        });
        if screen_preview_active {
            self.start_screen_preview(cx);
        } else {
            cx.notify();
        }
    }

    fn begin_preview_frames(
        &mut self,
        mut frames: tokio::sync::mpsc::Receiver<VideoFrame>,
        _source: LocalPreviewSource,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let this_entity = cx.entity().downgrade();
        self.preview_task = Some(cx.spawn(move |_view, cx: &mut AsyncApp| {
            let cx = cx.clone();
            async move {
                let first_frame =
                    match tokio::time::timeout(FIRST_PREVIEW_FRAME_TIMEOUT, frames.recv()).await {
                        Ok(Some(frame)) => frame,
                        Ok(None) => {
                            let _ = cx.update(|cx| {
                                if let Some(entity) = this_entity.upgrade() {
                                    entity.update(cx, |panel, cx| {
                                        if panel.preview_generation == generation {
                                            panel.fail_local_preview(
                                            "O canal de captura encerrou antes do primeiro quadro."
                                                .to_string(),
                                            cx,
                                        );
                                        }
                                    });
                                }
                            });
                            return;
                        }
                        Err(_) => {
                            let _ = cx.update(|cx| {
                                if let Some(entity) = this_entity.upgrade() {
                                    entity.update(cx, |panel, cx| {
                                        if panel.preview_generation == generation {
                                            panel.fail_local_preview(
                                            "Tempo limite aguardando o primeiro quadro da captura."
                                                .to_string(),
                                            cx,
                                        );
                                        }
                                    });
                                }
                            });
                            return;
                        }
                    };

                let latest_frame = latest_queued_frame(first_frame, &mut frames);
                let converted = video_frame_to_render_image(&latest_frame);
                let conversion_failed = converted.is_err();
                let _ = cx.update(|cx| {
                    if let Some(entity) = this_entity.upgrade() {
                        entity.update(cx, |panel, cx| {
                            if panel.preview_generation != generation {
                                return;
                            }
                            match converted {
                                Ok(frame) => {
                                    if let Some(preview) = panel.local_preview.as_mut() {
                                        preview.frame = Some(frame);
                                        preview.status = LocalPreviewStatus::Live;
                                        cx.notify();
                                    }
                                }
                                Err(error) => {
                                    panel.fail_local_preview(error.to_string(), cx);
                                }
                            }
                        });
                    }
                });
                if conversion_failed {
                    return;
                }

                loop {
                    match frames.recv().await {
                        Some(frame) => {
                            let latest_frame = latest_queued_frame(frame, &mut frames);
                            let converted = video_frame_to_render_image(&latest_frame);
                            let conversion_failed = converted.is_err();
                            let _ = cx.update(|cx| {
                                if let Some(entity) = this_entity.upgrade() {
                                    entity.update(cx, |panel, cx| {
                                        if panel.preview_generation != generation {
                                            return;
                                        }
                                        match converted {
                                            Ok(frame) => {
                                                if let Some(preview) = panel.local_preview.as_mut()
                                                {
                                                    preview.frame = Some(frame);
                                                    preview.status = LocalPreviewStatus::Live;
                                                    cx.notify();
                                                }
                                            }
                                            Err(error) => {
                                                panel.fail_local_preview(error.to_string(), cx);
                                            }
                                        }
                                    });
                                }
                            });
                            if conversion_failed {
                                return;
                            }
                        }
                        None => {
                            let _ = cx.update(|cx| {
                                if let Some(entity) = this_entity.upgrade() {
                                    entity.update(cx, |panel, cx| {
                                        if panel.preview_generation == generation {
                                            panel.fail_local_preview(
                                                "O canal de captura encerrou inesperadamente."
                                                    .to_string(),
                                                cx,
                                            );
                                        }
                                    });
                                }
                            });
                            break;
                        }
                    }
                }
            }
        }));
    }

    fn fail_local_preview(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(preview) = self.local_preview.as_mut() {
            preview.fail(error);
        }
        self.preview_source_id = None;
        shutdown_preview_sessions(self.camera_session.take(), self.screen_session.take());
        cx.notify();
    }
}

fn screen_preview_label(session_type: SessionType, screen_index: Option<usize>) -> String {
    match session_type {
        SessionType::Wayland => "Tela · Wayland Portal".to_string(),
        SessionType::X11 => format!("Tela X11 · Monitor {}", screen_index.unwrap_or(0) + 1),
        SessionType::Unknown => "Tela".to_string(),
    }
}

fn video_frame_to_bgra(frame: &VideoFrame) -> Result<(u32, u32, Vec<u8>), CaptureError> {
    let rgba = frame.to_rgba8()?;
    if rgba.width == 0 || rgba.height == 0 {
        return Err(CaptureError::ConversionError(
            "Quadro de vídeo com dimensões inválidas".into(),
        ));
    }
    let expected_len = (rgba.width as usize)
        .checked_mul(rgba.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            CaptureError::ConversionError("Dimensões do quadro excedem o limite".into())
        })?;
    if rgba.data.len() < expected_len {
        return Err(CaptureError::ConversionError(format!(
            "Buffer RGBA incompleto: esperados {} bytes, recebidos {}",
            expected_len,
            rgba.data.len()
        )));
    }

    let mut bgra = Vec::with_capacity(expected_len);
    for pixel in rgba.data[..expected_len].chunks_exact(4) {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    Ok((rgba.width, rgba.height, bgra))
}

fn video_frame_to_render_image(frame: &VideoFrame) -> Result<Arc<RenderImage>, CaptureError> {
    let (width, height, bgra) = video_frame_to_bgra(frame)?;
    let buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(width, height, bgra)
            .ok_or_else(|| CaptureError::ConversionError("Falha ao criar buffer BGRA".into()))?;
    Ok(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

fn clear_local_preview(preview: &mut Option<LocalPreview>) {
    *preview = None;
}

fn latest_queued_frame(
    mut latest: VideoFrame,
    frames: &mut tokio::sync::mpsc::Receiver<VideoFrame>,
) -> VideoFrame {
    while let Ok(frame) = frames.try_recv() {
        latest = frame;
    }
    latest
}

impl Render for VoiceCallPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let active_call = state.active_voice_call.clone();
        let app_entity = self.state.clone();
        let panel_entity = cx.entity().clone();
        let local_preview = self.local_preview.clone();
        let viewport = window.viewport_size();
        let stage_width = call_stage_width(viewport.width);
        let stage_height = (viewport.height - px(180.0)).max(px(180.0));

        if let Some(call) = active_call {
            let device_warning = self.audio_device_warning();
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(cx.discord().bg_server)
                .justify_between()
                .p(px(16.0))
                .child(render_top_bar(&call, device_warning.as_deref(), cx))
                .child(render_stage_body(
                    app_entity.clone(),
                    panel_entity.clone(),
                    &call,
                    local_preview.as_ref(),
                    stage_width,
                    stage_height,
                    cx,
                ))
                .child(render_bottom_controls(
                    app_entity,
                    panel_entity,
                    &call,
                    local_preview.as_ref(),
                    cx,
                ))
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.discord().text_secondary)
                .child("Nenhuma chamada de voz conectada.")
        }
    }
}

/// Largura útil do palco após o trilho de servidores (72px), a sidebar de
/// canais/DMs (240px) e o padding do painel (16px em cada lado).
fn call_stage_width(viewport_width: Pixels) -> Pixels {
    px((viewport_width.as_f32() - 72.0 - 240.0 - 32.0).max(240.0))
}

fn grid_tile_size(
    available_width: Pixels,
    available_height: Pixels,
    participant_count: usize,
    focused: bool,
) -> (Pixels, Pixels) {
    if focused {
        return (px(480.0), px(300.0));
    }

    let desired_columns = match participant_count {
        0 | 1 => 1,
        2..=4 => 2,
        5..=9 => 3,
        _ => 4,
    };
    let columns = desired_columns
        .min((available_width.as_f32() / 200.0).floor() as usize)
        .max(1);
    let rows = participant_count.max(1).div_ceil(columns);
    let width = ((available_width.as_f32() - 8.0 * (columns.saturating_sub(1)) as f32)
        / columns as f32)
        .clamp(180.0, 480.0);
    let height = (width * 9.0 / 16.0)
        .min((available_height.as_f32() - 8.0 * rows.saturating_sub(1) as f32) / rows as f32)
        .clamp(80.0, 300.0);
    (px(width), px(height))
}

// ============================================================================
// Top Bar: Channel info, RTC status badge & fatal error banner
// ============================================================================

fn render_top_bar(call: &ActiveVoiceCall, device_warning: Option<&str>, cx: &App) -> AnyElement {
    let discord = cx.discord();
    let has_fatal_error = call.fatal_error.is_some();
    let call_warning = call.voice_session.as_ref().and_then(|s| s.call_warning());

    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .w_full()
                .overflow_hidden()
                .gap(px(8.0))
                .child(render_channel_identity(call, cx))
                .child(render_rtc_badge(call, cx)),
        )
        .children(has_fatal_error.then(|| render_fatal_error_banner(call, discord.white)))
        .children(call_warning.map(|msg| render_call_warning_banner(&msg, discord.white)))
        .children(device_warning.map(|msg| render_audio_device_warning_banner(msg, discord.white)))
        .into_any_element()
}

fn render_call_warning_banner(msg: &str, text_col: Rgba) -> AnyElement {
    div()
        .w_full()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(status_warning())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(icon_warning(px(16.0), text_col))
        .child(
            div()
                .text_color(text_col)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(12.0))
                .child(msg.to_string()),
        )
        .into_any_element()
}

fn render_audio_device_warning_banner(msg: &str, text_col: Rgba) -> AnyElement {
    div()
        .w_full()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(status_warning())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(icon_warning(px(16.0), text_col))
        .child(
            div()
                .text_color(text_col)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(12.0))
                .child(format!("Aviso de áudio: {}", msg)),
        )
        .into_any_element()
}

fn render_channel_identity(call: &ActiveVoiceCall, cx: &App) -> AnyElement {
    let discord = cx.discord();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .min_w_0()
        .overflow_hidden()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .min_w_0()
                .child(icon_speaker(px(20.0), discord.text_titles))
                .child(
                    div()
                        .text_color(discord.text_titles)
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(18.0))
                        .truncate()
                        .child(call.channel_name.clone()),
                ),
        )
        .child(
            div()
                .text_color(discord.text_secondary)
                .text_size(px(13.0))
                .flex_shrink_0()
                .child(format!("• {}", call.guild_name)),
        )
        .into_any_element()
}

fn render_rtc_badge(call: &ActiveVoiceCall, cx: &App) -> AnyElement {
    let discord = cx.discord();
    let (icon_col, label_text) = match call.connection_state {
        VoiceSessionState::Connected => {
            let label = if let Some(ms) = call.ping_ms {
                format!("RTC {}ms", ms)
            } else {
                "Voz Conectada".to_string()
            };
            (status_online(), label)
        }
        VoiceSessionState::Connecting => (status_idle(), "Conectando ao RTC...".to_string()),
        VoiceSessionState::Reconnecting => (status_idle(), "Reconectando...".to_string()),
        VoiceSessionState::Disconnected { ref reason } => {
            let label =
                if let Some(code) = crate::protocol::voice_gateway::extract_close_code(reason) {
                    format!("Desconectado (código {})", code)
                } else if reason.contains("4006") {
                    "Desconectado (código 4006)".to_string()
                } else if reason.contains("Opcode 2")
                    || reason.contains("Timed out")
                    || reason.contains("Tempo limite")
                {
                    "Desconectado (tempo limite)".to_string()
                } else if reason.contains("Missing Permissions") || reason.contains("50013") {
                    "Desconectado (sem permissão)".to_string()
                } else if reason.contains('{') && reason.contains('}') {
                    "Desconectado (erro de conexão)".to_string()
                } else if !reason.is_empty() && reason != "Sessão inicializada" {
                    format!("Desconectado ({})", reason)
                } else {
                    "Desconectado".to_string()
                };
            (status_dnd(), label)
        }
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .flex_shrink_0()
        .max_w(px(260.0))
        .overflow_hidden()
        .gap(px(6.0))
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(4.0))
        .bg(discord.bg_server)
        .child(icon_wifi(px(14.0), icon_col))
        .child(
            div()
                .text_color(icon_col)
                .font_weight(FontWeight::BOLD)
                .text_size(px(12.0))
                .truncate()
                .child(label_text),
        )
        .into_any_element()
}

fn sanitize_user_error_message(raw: &str) -> String {
    if let Some(code) = crate::protocol::voice_gateway::extract_close_code(raw) {
        format!("Não foi possível conectar à chamada (código {})", code)
    } else if raw.contains("4006") {
        "Não foi possível conectar à chamada (código 4006)".to_string()
    } else if raw.contains("Missing Permissions") || raw.contains("50013") {
        "Permissões insuficientes no canal".to_string()
    } else if raw.contains("Timed out waiting for VoiceReady") || raw.contains("Opcode 2") {
        "Tempo limite ao aguardar resposta do servidor de voz".to_string()
    } else if raw.contains('{') && raw.contains('}') {
        "Erro na comunicação com o servidor de voz".to_string()
    } else {
        raw.to_string()
    }
}

fn render_fatal_error_banner(call: &ActiveVoiceCall, text_col: Rgba) -> AnyElement {
    let raw_desc = call
        .fatal_error
        .clone()
        .unwrap_or_else(|| "Erro desconhecido na sessão de voz".into());
    let summary = sanitize_user_error_message(&raw_desc);
    let show_details = summary != raw_desc;

    div()
        .w_full()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .bg(status_dnd())
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(icon_warning(px(16.0), text_col))
                .child(
                    div()
                        .text_color(text_col)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(12.0))
                        .child(summary),
                ),
        )
        .children(show_details.then(|| {
            div()
                .text_color(text_col)
                .text_size(px(11.0))
                .child(raw_desc)
        }))
        .into_any_element()
}

// ============================================================================
// Stage Body: Responsive Grid or Screen Share / Focus Layout
// ============================================================================

fn render_stage_body(
    app_entity: Entity<AppState>,
    panel_entity: Entity<VoiceCallPanel>,
    call: &ActiveVoiceCall,
    local_preview: Option<&LocalPreview>,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    if let Some(preview) = local_preview.filter(|preview| preview.expanded) {
        return render_stage_body_spotlight(
            app_entity.clone(),
            panel_entity,
            preview,
            call,
            available_width,
            available_height,
            cx,
        );
    }

    let preview_reserved_height = if local_preview.is_some() {
        px(102.0)
    } else {
        px(0.0)
    };
    let participant_stage_height = (available_height - preview_reserved_height).max(px(120.0));
    let participant_stage = if call.focus_user_id.is_some() {
        render_focus_layout(
            app_entity.clone(),
            call,
            available_width,
            participant_stage_height,
            cx,
        )
    } else {
        render_grid_layout(
            app_entity.clone(),
            call,
            available_width,
            participant_stage_height,
            cx,
        )
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .size_full()
        .child(
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.0))
                .child(participant_stage),
        )
        .children(local_preview.map(|preview| {
            render_local_preview_tile(panel_entity, preview, px(140.0), px(90.0), cx)
        }))
        .into_any_element()
}

fn render_stage_body_spotlight(
    app_entity: Entity<AppState>,
    panel_entity: Entity<VoiceCallPanel>,
    preview: &LocalPreview,
    call: &ActiveVoiceCall,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .size_full()
        .child(
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .child(render_local_preview_tile(
                    panel_entity.clone(),
                    preview,
                    available_width,
                    (available_height - px(102.0)).max(px(120.0)),
                    cx,
                )),
        )
        .children((!call.participants.is_empty()).then(|| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .w_full()
                .overflow_x_scrollbar()
                .children(
                    call.participants
                        .iter()
                        .map(|participant| render_mini_tile(app_entity.clone(), participant, cx)),
                )
        }))
        .into_any_element()
}

fn render_grid_layout(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    let participants = &call.participants;
    let (tile_w, tile_h) =
        grid_tile_size(available_width, available_height, participants.len(), false);

    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_center()
        .p(px(12.0))
        .gap(px(8.0))
        .w_full()
        .h_full()
        .overflow_y_scrollbar()
        .children(participants.iter().map(|participant| {
            render_participant_tile(app_entity.clone(), participant, tile_w, tile_h, cx)
        }))
        .into_any_element()
}

fn render_focus_layout(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    available_width: Pixels,
    available_height: Pixels,
    cx: &App,
) -> AnyElement {
    let focus_uid = call.focus_user_id.clone().or_else(|| {
        call.participants
            .first()
            .map(|participant| participant.id.clone())
    });

    let focused_p = call
        .participants
        .iter()
        .find(|participant| Some(&participant.id) == focus_uid.as_ref())
        .cloned();

    let other_participants: Vec<_> = call
        .participants
        .iter()
        .filter(|participant| Some(&participant.id) != focus_uid.as_ref())
        .cloned()
        .collect();
    let (tile_w, tile_h) = grid_tile_size(
        available_width,
        (available_height - px(102.0)).max(px(120.0)),
        1,
        true,
    );
    let tile_w = tile_w.min(available_width);
    let tile_h = tile_h.min((available_height - px(102.0)).max(px(120.0)));

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .size_full()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .child(if let Some(ref participant) = focused_p {
                    // Não existe decodificador de vídeo remoto neste cliente.
                    // O estado real de stream é mostrado no tile, nunca um
                    // quadro de tela inventado.
                    render_participant_tile(app_entity.clone(), participant, tile_w, tile_h, cx)
                } else {
                    div().into_any_element()
                }),
        )
        .children((!other_participants.is_empty()).then(|| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(10.0))
                .w_full()
                .overflow_x_scrollbar()
                .children(
                    other_participants
                        .into_iter()
                        .map(|participant| render_mini_tile(app_entity.clone(), &participant, cx)),
                )
        }))
        .into_any_element()
}

fn render_local_preview_tile(
    panel_entity: Entity<VoiceCallPanel>,
    preview: &LocalPreview,
    tile_w: Pixels,
    tile_h: Pixels,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let has_frame = preview.frame.is_some();
    let mut tile = div()
        .id("local-preview-tile")
        .relative()
        .w(tile_w)
        .h(tile_h)
        .flex_shrink_0()
        .rounded(px(8.0))
        .bg(discord.bg_channels)
        .border_2()
        .border_color(transparent())
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden();

    if let Some(frame) = preview.frame.as_ref() {
        tile = tile.child(
            img(frame.clone())
                .object_fit(ObjectFit::Contain)
                .size_full(),
        );
    } else {
        tile = tile.child(render_preview_state(preview, cx));
    }

    if preview.status == LocalPreviewStatus::Live && has_frame {
        let expanded = preview.expanded;
        tile = tile
            .cursor_pointer()
            .tooltip(move |window, cx| {
                Tooltip::new(if expanded {
                    "Reduzir prévia local"
                } else {
                    "Expandir prévia local"
                })
                .build(window, cx)
            })
            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                panel_entity.update(cx, |panel, cx| {
                    panel.toggle_preview_expanded(cx);
                });
            });
    }

    tile.child(render_local_preview_badge(preview, discord.text_titles))
        .into_any_element()
}

fn render_preview_state(preview: &LocalPreview, cx: &App) -> AnyElement {
    let discord = cx.discord();
    let (icon, message) = match &preview.status {
        LocalPreviewStatus::Idle => (
            if preview.source == LocalPreviewSource::Camera {
                render_camera_glyph(px(20.0), discord.text_secondary)
            } else {
                icon_screen_share(px(20.0), discord.text_secondary).into_any_element()
            },
            "Nenhuma prévia local".to_string(),
        ),
        LocalPreviewStatus::Starting => (
            if preview.source == LocalPreviewSource::Camera {
                render_camera_glyph(px(20.0), discord.text_secondary)
            } else {
                icon_screen_share(px(20.0), discord.text_secondary).into_any_element()
            },
            "Aguardando o primeiro quadro real...".to_string(),
        ),
        LocalPreviewStatus::Live => (
            if preview.source == LocalPreviewSource::Camera {
                render_camera_glyph(px(20.0), discord.text_secondary)
            } else {
                icon_screen_share(px(20.0), discord.text_secondary).into_any_element()
            },
            "Prévia local ativa".to_string(),
        ),
        LocalPreviewStatus::Failed(error) => (
            icon_warning(px(20.0), status_dnd()).into_any_element(),
            error.clone(),
        ),
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .p(px(8.0))
        .max_w(px(220.0))
        .text_center()
        .child(icon)
        .child(
            div()
                .text_color(
                    if matches!(&preview.status, LocalPreviewStatus::Failed(_)) {
                        status_dnd()
                    } else {
                        discord.text_secondary
                    },
                )
                .text_size(px(11.0))
                .child(message),
        )
        .into_any_element()
}

fn render_local_preview_badge(preview: &LocalPreview, text_col: Rgba) -> AnyElement {
    let dimensions = preview.frame.as_ref().map(|frame| {
        let size = frame.size(0);
        format!("{}×{}", u32::from(size.width), u32::from(size.height))
    });
    div()
        .absolute()
        .bottom(px(8.0))
        .left(px(8.0))
        .max_w(px(220.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(bg_floating())
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_color(text_col)
                .text_size(px(10.0))
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child("Prévia local — não transmite"),
        )
        .child(
            div()
                .text_color(text_secondary())
                .text_size(px(9.0))
                .truncate()
                .child(match dimensions {
                    Some(dimensions) => {
                        format!("{} · {}", preview.real_source_label, dimensions)
                    }
                    None => preview.real_source_label.clone(),
                }),
        )
        .into_any_element()
}

fn render_participant_tile(
    app_entity: Entity<AppState>,
    participant: &VoiceParticipantState,
    tile_w: Pixels,
    tile_h: Pixels,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let is_speaking = participant.is_speaking;
    let uid = participant.id.clone();
    let entity = app_entity;

    div()
        .relative()
        .w(tile_w)
        .h(tile_h)
        .flex_shrink_0()
        .rounded(px(8.0))
        .bg(discord.bg_channels)
        .border_2()
        .border_color(if is_speaking {
            status_online()
        } else {
            transparent()
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let uid_clone = uid.clone();
            entity.update(cx, |state, cx| {
                if state
                    .active_voice_call
                    .as_ref()
                    .and_then(|c| c.focus_user_id.as_ref())
                    == Some(&uid_clone)
                {
                    state.set_focus_user(None);
                } else {
                    state.set_focus_user(Some(uid_clone));
                }
                cx.notify();
            });
        })
        .child(
            Avatar::new(participant.name.clone())
                .size(AvatarSize::Px80)
                .opt_src(participant.avatar_url.clone())
                .opt_decoration(participant.avatar_decoration_data.clone())
                .cutout_color(discord.bg_channels),
        )
        .child(render_tile_name_badge(participant, discord.text_titles))
        .into_any_element()
}

fn render_mini_tile(
    app_entity: Entity<AppState>,
    participant: &VoiceParticipantState,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let is_speaking = participant.is_speaking;
    let uid = participant.id.clone();
    let entity = app_entity;

    div()
        .relative()
        .w(px(140.0))
        .h(px(90.0))
        .flex_shrink_0()
        .rounded(px(6.0))
        .bg(discord.bg_channels)
        .border_2()
        .border_color(if is_speaking {
            status_online()
        } else {
            transparent()
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            let uid_clone = uid.clone();
            entity.update(cx, |state, cx| {
                state.set_focus_user(Some(uid_clone));
                cx.notify();
            });
        })
        .child(
            Avatar::new(participant.name.clone())
                .size(AvatarSize::Px40)
                .opt_src(participant.avatar_url.clone())
                .opt_decoration(participant.avatar_decoration_data.clone())
                .cutout_color(discord.bg_channels),
        )
        .child(render_tile_name_badge(participant, discord.text_titles))
        .into_any_element()
}

fn render_tile_name_badge(participant: &VoiceParticipantState, text_col: Rgba) -> AnyElement {
    div()
        .absolute()
        .bottom(px(8.0))
        .left(px(8.0))
        .max_w(px(180.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(bg_floating())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .children((!participant.name.is_empty()).then(|| {
            div()
                .text_color(text_col)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child(participant.name.clone())
        }))
        .children(participant.is_screen_sharing.then(|| {
            div()
                .px(px(4.0))
                .rounded_xs()
                .bg(status_dnd())
                .text_color(crate::tokens::colors::white())
                .text_size(px(9.0))
                .font_weight(FontWeight::BOLD)
                .child("AO VIVO")
        }))
        .children(
            participant
                .is_camera_on
                .then(|| render_camera_glyph(px(14.0), text_col)),
        )
        .children(
            participant
                .is_muted
                .then(|| icon_mic_off(px(14.0), status_dnd())),
        )
        .children(
            participant
                .is_deafened
                .then(|| icon_headphones_off(px(14.0), status_dnd())),
        )
        .into_any_element()
}

// ============================================================================
// Bottom Control Bar: Mute, Camera, Screen Share, Disconnect with device pickers
// ============================================================================

fn render_bottom_controls(
    app_entity: Entity<AppState>,
    panel_entity: Entity<VoiceCallPanel>,
    call: &ActiveVoiceCall,
    local_preview: Option<&LocalPreview>,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_center()
                .gap(px(12.0))
                .p(px(12.0))
                .rounded(px(12.0))
                .bg(discord.bg_server)
                .child(render_mute_control(app_entity.clone(), call, cx))
                .child(render_deafen_control(app_entity.clone(), call, cx))
                .child(render_camera_control(
                    app_entity.clone(),
                    panel_entity.clone(),
                    local_preview,
                    cx,
                ))
                .child(render_screen_share_control(panel_entity, local_preview, cx))
                .child(render_disconnect_control(app_entity, cx)),
        )
        .into_any_element()
}

/// Selectable trigger for popover arrow buttons
#[derive(IntoElement)]
#[allow(dead_code)]
pub struct ArrowDropupTrigger {
    pub is_open: bool,
    pub id: SharedString,
    pub bg_col: Rgba,
    pub hover_col: Rgba,
    pub tooltip: SharedString,
}

impl Selectable for ArrowDropupTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.is_open = selected;
        self
    }
    fn is_selected(&self) -> bool {
        self.is_open
    }
}

impl RenderOnce for ArrowDropupTrigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let discord = cx.discord();
        div()
            .id(self.id)
            .w(px(16.0))
            .h(px(32.0))
            .rounded(px(8.0))
            .bg(self.bg_col)
            .text_color(discord.white)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |s| s.bg(self.hover_col))
            .tooltip(move |window, cx| Tooltip::new(self.tooltip.clone()).build(window, cx))
            .child(
                div()
                    .text_color(discord.white)
                    .text_size(px(10.0))
                    .child("▲"),
            )
    }
}

fn render_mute_control(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let entity_btn = app_entity.clone();
    let is_muted = call.is_muted;

    let bg_col = if is_muted {
        control_critical_primary_bg_default()
    } else {
        control_secondary_bg_default()
    };

    let hover_col = if is_muted {
        control_critical_primary_bg_hover()
    } else {
        control_secondary_bg_hover()
    };

    let mic_btn = div()
        .id("call-mic-btn")
        .w(px(32.0))
        .h(px(32.0))
        .rounded(px(16.0))
        .bg(bg_col)
        .text_color(discord.white)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |s| s.bg(hover_col))
        .tooltip(move |window, cx| {
            Tooltip::new(if is_muted {
                "Ativar microfone"
            } else {
                "Desativar microfone"
            })
            .build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity_btn.update(cx, |state, cx| {
                state.toggle_mute();
                cx.notify();
            });
        })
        .child(if is_muted {
            icon_mic_off(px(20.0), discord.white)
        } else {
            icon_mic(px(20.0), discord.white)
        });

    let trigger = ArrowDropupTrigger {
        is_open: false,
        id: "call-mic-arrow".into(),
        bg_col,
        hover_col,
        tooltip: "Selecionar microfone".into(),
    };

    let mic_arrow = Popover::new("call-mic-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| render_input_device_menu(entity.clone(), cx)
        });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(mic_btn)
        .child(mic_arrow)
        .into_any_element()
}

fn render_deafen_control(
    app_entity: Entity<AppState>,
    call: &ActiveVoiceCall,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let entity_btn = app_entity.clone();
    let is_deafened = call.is_deafened;
    let bg_col = if is_deafened {
        control_critical_primary_bg_default()
    } else {
        control_secondary_bg_default()
    };
    let hover_col = if is_deafened {
        control_critical_primary_bg_hover()
    } else {
        control_secondary_bg_hover()
    };

    let deafen_btn = div()
        .id("call-deafen-btn")
        .w(px(32.0))
        .h(px(32.0))
        .rounded(px(16.0))
        .bg(bg_col)
        .text_color(discord.white)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(hover_col))
        .tooltip(move |window, cx| {
            Tooltip::new(if is_deafened {
                "Ativar áudio"
            } else {
                "Desativar áudio"
            })
            .build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity_btn.update(cx, |state, cx| {
                state.toggle_deafen();
                cx.notify();
            });
        })
        .child(if is_deafened {
            icon_headphones_off(px(20.0), discord.white)
        } else {
            icon_headphones(px(20.0), discord.white)
        });

    let trigger = ArrowDropupTrigger {
        is_open: false,
        id: "call-output-arrow".into(),
        bg_col,
        hover_col,
        tooltip: "Selecionar saída de áudio".into(),
    };
    let output_popover = Popover::new("call-output-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| render_output_device_menu(entity.clone(), cx)
        });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(deafen_btn)
        .child(output_popover)
        .into_any_element()
}

fn render_camera_control(
    app_entity: Entity<AppState>,
    panel_entity: Entity<VoiceCallPanel>,
    local_preview: Option<&LocalPreview>,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();

    let camera_preview_active = local_preview.is_some_and(|preview| {
        preview.source == LocalPreviewSource::Camera && preview.status.is_active()
    });
    let bg_col = if camera_preview_active {
        control_secondary_bg_active()
    } else {
        control_secondary_bg_default()
    };
    let entity_btn = panel_entity.clone();

    let cam_btn = div()
        .id("call-cam-btn")
        .w(px(32.0))
        .h(px(32.0))
        .rounded(px(16.0))
        .bg(bg_col)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|style| style.bg(control_secondary_bg_hover()))
        .tooltip(move |window, cx| {
            Tooltip::new(if camera_preview_active {
                "Encerrar prévia local"
            } else {
                "Iniciar prévia local da câmera — não transmite"
            })
            .build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity_btn.update(cx, |panel, cx| {
                if camera_preview_active {
                    panel.stop_local_preview(cx);
                } else {
                    panel.start_camera_preview(cx);
                }
            });
        })
        .child(render_camera_glyph(px(20.0), discord.white));

    let camera_trigger = ArrowDropupTrigger {
        is_open: false,
        id: "call-cam-arrow".into(),
        bg_col,
        hover_col: control_secondary_bg_hover(),
        tooltip: "Selecionar câmera (prévia local)".into(),
    };
    let cam_arrow = Popover::new("call-camera-device-popover")
        .anchor(Anchor::TopCenter)
        .offset(px(6.0))
        .trigger(camera_trigger)
        .content({
            let entity = app_entity.clone();
            move |_state, _window, cx| render_camera_device_menu(entity.clone(), cx)
        });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(cam_btn)
        .child(cam_arrow)
        .into_any_element()
}

fn render_screen_share_control(
    panel_entity: Entity<VoiceCallPanel>,
    local_preview: Option<&LocalPreview>,
    cx: &App,
) -> AnyElement {
    let discord = cx.discord();
    let screen_preview_active = local_preview.is_some_and(|preview| {
        preview.source == LocalPreviewSource::Screen && preview.status.is_active()
    });
    let bg_col = if screen_preview_active {
        control_secondary_bg_active()
    } else {
        control_secondary_bg_default()
    };
    let entity_btn = panel_entity.clone();

    let screen_btn = div()
        .id("call-screen-share-btn")
        .h(px(32.0))
        .px(px(16.0))
        .rounded(px(16.0))
        .bg(bg_col)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|style| style.bg(control_secondary_bg_hover()))
        .tooltip(move |window, cx| {
            Tooltip::new(if screen_preview_active {
                "Encerrar prévia local"
            } else {
                "Iniciar prévia local da tela — não transmite"
            })
            .build(window, cx)
        })
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            entity_btn.update(cx, |panel, cx| {
                if screen_preview_active {
                    panel.stop_local_preview(cx);
                } else {
                    panel.start_screen_preview(cx);
                }
            });
        })
        .child(icon_screen_share(px(20.0), discord.white))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Prévia da Tela"),
        );

    let source_control = match detect_session() {
        SessionType::X11 => {
            let trigger = ArrowDropupTrigger {
                is_open: false,
                id: "call-screen-source-arrow".into(),
                bg_col,
                hover_col: control_secondary_bg_hover(),
                tooltip: "Selecionar monitor X11".into(),
            };
            Popover::new("call-screen-source-popover")
                .anchor(Anchor::TopCenter)
                .offset(px(6.0))
                .trigger(trigger)
                .content({
                    let entity = panel_entity.clone();
                    move |_state, _window, cx| render_screen_source_menu(entity.clone(), cx)
                })
                .into_any_element()
        }
        SessionType::Wayland => render_disabled_source_control(
            bg_col,
            discord.white,
            "A seleção de monitor ou janela é feita pelo Portal",
        ),
        SessionType::Unknown => render_disabled_source_control(
            bg_col,
            discord.white,
            "Wayland ou X11 não foi detectado",
        ),
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .child(screen_btn)
        .child(source_control)
        .into_any_element()
}

fn render_disabled_source_control(
    bg_col: Rgba,
    text_col: Rgba,
    tooltip_text: &'static str,
) -> AnyElement {
    div()
        .id("call-screen-source-disabled")
        .w(px(18.0))
        .h(px(32.0))
        .rounded(px(8.0))
        .bg(bg_col)
        .opacity(0.5)
        .cursor_not_allowed()
        .flex()
        .items_center()
        .justify_center()
        .tooltip(move |window, cx| Tooltip::new(tooltip_text).build(window, cx))
        .child(div().text_color(text_col).text_size(px(10.0)).child("▲"))
        .into_any_element()
}

fn render_disconnect_control(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    let discord = cx.discord();

    div()
        .id("voice-call-disconnect-btn")
        .h(px(32.0))
        .px(px(16.0))
        .rounded(px(16.0))
        .bg(control_critical_primary_bg_default())
        .text_color(discord.white)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .cursor_pointer()
        .hover(|s| s.bg(control_critical_primary_bg_hover()))
        .tooltip(|window, cx| Tooltip::new("Desconectar da chamada").build(window, cx))
        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
            app_entity.update(cx, |state, cx| {
                state.disconnect_voice();
                cx.notify();
            });
        })
        .child(icon_phone_hangup(px(20.0), discord.white))
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .child("Desconectar"),
        )
        .into_any_element()
}

// ============================================================================
// Real Hardware Device Menus (MediaDeviceManager & CameraManager)
// ============================================================================

fn render_input_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    render_audio_device_menu(
        app_entity,
        crate::media::DeviceType::Input,
        "MICROFONES",
        "Nenhum microfone encontrado",
        cx,
    )
}

fn render_output_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    render_audio_device_menu(
        app_entity,
        crate::media::DeviceType::Output,
        "SAÍDAS DE ÁUDIO",
        "Nenhum alto-falante encontrado",
        cx,
    )
}

fn render_audio_device_menu(
    app_entity: Entity<AppState>,
    device_type: crate::media::DeviceType,
    title: &str,
    empty_message: &str,
    cx: &App,
) -> AnyElement {
    let selected_device = {
        let state = app_entity.read(cx);
        let call = state.active_voice_call.as_ref();
        match device_type {
            crate::media::DeviceType::Input => {
                call.and_then(|call| call.selected_input_device.clone())
            }
            crate::media::DeviceType::Output => {
                call.and_then(|call| call.selected_output_device.clone())
            }
        }
    };

    let device_manager = MediaDeviceManager::new();
    let devices = match device_type {
        crate::media::DeviceType::Input => device_manager.list_input_devices(),
        crate::media::DeviceType::Output => device_manager.list_output_devices(),
    };

    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(8.0))
        .min_w(px(240.0))
        .rounded(px(8.0))
        .bg(bg_floating())
        .border_1()
        .border_color(borders())
        .shadow_lg()
        .child(
            div()
                .text_color(text_secondary())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .px(px(6.0))
                .child(title.to_string()),
        );

    match devices {
        Err(error) => {
            menu = menu.child(
                div()
                    .text_color(status_dnd())
                    .text_size(px(12.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .child(format!("Não foi possível listar os dispositivos: {error}")),
            );
        }
        Ok(devices) if devices.is_empty() => {
            menu = menu.child(
                div()
                    .text_color(text_muted())
                    .text_size(px(12.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .child(empty_message.to_string()),
            );
        }
        Ok(devices) => {
            let default_entity = app_entity.clone();
            let default_selected = selected_device.is_none();
            menu = menu.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(px(4.0))
                    .hover(|style| style.bg(channel_hover()))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                        default_entity.update(cx, |state, cx| {
                            match device_type {
                                crate::media::DeviceType::Input => state.set_input_device(None),
                                crate::media::DeviceType::Output => state.set_output_device(None),
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_color(if default_selected {
                                text_primary()
                            } else {
                                text_secondary()
                            })
                            .text_size(px(13.0))
                            .child("Padrão do sistema"),
                    )
                    .children(
                        default_selected
                            .then(|| icon_check(px(14.0), status_online()).into_any_element()),
                    ),
            );

            for device in devices {
                let is_selected = selected_device.as_deref() == Some(device.id.as_str());
                let device_id = device.id.clone();
                let entity = app_entity.clone();
                menu = menu.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .hover(|style| style.bg(channel_hover()))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            let device_id = device_id.clone();
                            entity.update(cx, |state, cx| {
                                match device_type {
                                    crate::media::DeviceType::Input => {
                                        state.set_input_device(Some(device_id))
                                    }
                                    crate::media::DeviceType::Output => {
                                        state.set_output_device(Some(device_id))
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .text_color(if is_selected {
                                    text_primary()
                                } else {
                                    text_secondary()
                                })
                                .text_size(px(13.0))
                                .truncate()
                                .child(device.name),
                        )
                        .children(
                            is_selected
                                .then(|| icon_check(px(14.0), status_online()).into_any_element()),
                        ),
                );
            }
        }
    }

    menu.into_any_element()
}

fn render_screen_source_menu(panel_entity: Entity<VoiceCallPanel>, cx: &App) -> AnyElement {
    let selected_screen = panel_entity.read(cx).selected_screen_index;
    let screens = ScreenCaptureManager::list_x11_screens();
    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(8.0))
        .min_w(px(240.0))
        .rounded(px(8.0))
        .bg(bg_floating())
        .border_1()
        .border_color(borders())
        .shadow_lg()
        .child(
            div()
                .text_color(text_secondary())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .px(px(6.0))
                .child("MONITORES X11"),
        );

    match screens {
        Err(error) => {
            menu = menu.child(
                div()
                    .text_color(status_dnd())
                    .text_size(px(12.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .child(format!("Não foi possível listar os monitores: {error}")),
            );
        }
        Ok(screens) if screens.is_empty() => {
            menu = menu.child(
                div()
                    .text_color(text_muted())
                    .text_size(px(12.0))
                    .px(px(6.0))
                    .py(px(4.0))
                    .child("Nenhum monitor X11 encontrado"),
            );
        }
        Ok(screens) => {
            for screen in screens {
                let is_selected = selected_screen == Some(screen.index);
                let screen_index = screen.index;
                let entity = panel_entity.clone();
                menu = menu.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .hover(|style| style.bg(channel_hover()))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            entity.update(cx, |panel, cx| {
                                panel.set_screen_index(screen_index, cx);
                            });
                        })
                        .child(
                            div()
                                .text_color(if is_selected {
                                    text_primary()
                                } else {
                                    text_secondary()
                                })
                                .text_size(px(13.0))
                                .child(format!(
                                    "Monitor {} — {}×{}",
                                    screen.index + 1,
                                    screen.width,
                                    screen.height
                                )),
                        )
                        .children(
                            is_selected
                                .then(|| icon_check(px(14.0), status_online()).into_any_element()),
                        ),
                );
            }
        }
    }

    menu.into_any_element()
}

fn render_camera_device_menu(app_entity: Entity<AppState>, cx: &App) -> AnyElement {
    let selected_camera = app_entity
        .read(cx)
        .active_voice_call
        .as_ref()
        .and_then(|call| call.selected_camera_device.clone());
    let devices = CameraManager::list_devices();

    let mut menu = div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .p(px(8.0))
        .min_w(px(220.0))
        .rounded(px(8.0))
        .bg(bg_floating())
        .border_1()
        .border_color(borders())
        .shadow_lg()
        .child(
            div()
                .text_color(text_secondary())
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.0))
                .child("CÂMERAS DISPONÍVEIS"),
        );

    match devices {
        Err(error) => {
            menu = menu.child(
                div()
                    .text_color(status_dnd())
                    .text_size(px(12.0))
                    .py(px(4.0))
                    .child(format!("Não foi possível listar as câmeras: {error}")),
            );
        }
        Ok(devices) if devices.is_empty() => {
            menu = menu.child(
                div()
                    .text_color(text_muted())
                    .text_size(px(12.0))
                    .py(px(4.0))
                    .child("Nenhuma câmera encontrada"),
            );
        }
        Ok(devices) => {
            for device in devices {
                let is_selected = selected_camera.as_deref() == Some(device.id.as_str());
                let device_id = device.id.clone();
                let entity = app_entity.clone();
                menu = menu.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px(px(6.0))
                        .py(px(4.0))
                        .rounded(px(4.0))
                        .hover(|style| style.bg(channel_hover()))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                            let device_id = device_id.clone();
                            entity.update(cx, |state, cx| {
                                state.set_camera_device(Some(device_id));
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .text_color(if is_selected {
                                    text_primary()
                                } else {
                                    text_secondary()
                                })
                                .text_size(px(13.0))
                                .truncate()
                                .child(device.name),
                        )
                        .children(
                            is_selected
                                .then(|| icon_check(px(14.0), status_online()).into_any_element()),
                        ),
                );
            }
        }
    }

    menu.into_any_element()
}

fn render_camera_glyph(size: Pixels, color: Rgba) -> AnyElement {
    div()
        .size(size)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .child(div().w(px(12.0)).h(px(9.0)).rounded(px(1.5)).bg(color))
                .child(div().w(px(3.5)).h(px(6.5)).rounded(px(1.0)).bg(color)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_frame(
        width: u32,
        height: u32,
        format: crate::capture::PixelFormat,
        data: Vec<u8>,
        sequence: u64,
    ) -> VideoFrame {
        VideoFrame::new(
            width,
            height,
            format,
            data,
            Duration::from_millis(sequence),
            sequence,
        )
    }

    #[::core::prelude::v1::test]
    fn test_video_frame_to_render_image_converts_rgba_and_bgra_to_bgra() {
        let rgba = vec![
            10, 20, 30, 255, 40, 50, 60, 128, 70, 80, 90, 0, 100, 110, 120, 255,
        ];
        let expected = vec![
            30, 20, 10, 255, 60, 50, 40, 128, 90, 80, 70, 0, 120, 110, 100, 255,
        ];
        let rgba_frame = synthetic_frame(2, 2, crate::capture::PixelFormat::Rgba8, rgba, 1);
        let bgra_frame = synthetic_frame(2, 2, crate::capture::PixelFormat::Bgra8, expected.clone(), 2);

        for frame in [rgba_frame, bgra_frame] {
            let (width, height, bgra) = video_frame_to_bgra(&frame).expect("conversão sintética");
            assert_eq!((width, height), (2, 2));
            assert_eq!(bgra, expected);
        }
    }

    #[::core::prelude::v1::test]
    fn test_video_frame_to_render_image_normalizes_yuv_formats() {
        let cases = [
            (crate::capture::PixelFormat::Yuyv, vec![128; 16]),
            (crate::capture::PixelFormat::Nv12, vec![128; 6]),
            (crate::capture::PixelFormat::I420, vec![128; 6]),
        ];
        for (format, data) in cases {
            let frame = synthetic_frame(2, 2, format, data, 1);
            let (width, height, bgra) = video_frame_to_bgra(&frame).expect("conversão YUV sintética");
            assert_eq!((width, height), (2, 2));
            assert_eq!(bgra, [128, 128, 128, 255].repeat(4));
        }
    }

    #[::core::prelude::v1::test]
    fn test_video_frame_to_render_image_rejects_invalid_frames() {
        let zero = synthetic_frame(0, 2, crate::capture::PixelFormat::Rgba8, vec![], 1);
        assert!(video_frame_to_bgra(&zero).is_err());
        let truncated = synthetic_frame(2, 2, crate::capture::PixelFormat::Rgba8, vec![0; 4], 2);
        assert!(video_frame_to_bgra(&truncated).is_err());
    }

    #[::core::prelude::v1::test]
    fn test_preview_frame_coalescing_keeps_only_latest() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        for sequence in 1..=3 {
            tx.try_send(synthetic_frame(
                2,
                2,
                crate::capture::PixelFormat::Rgba8,
                vec![sequence as u8; 16],
                sequence,
            ))
            .expect("enviar quadro sintético");
        }
        let first = rx.try_recv().expect("primeiro quadro");
        let latest = latest_queued_frame(first, &mut rx);
        assert_eq!(latest.sequence, 3);
    }

    #[::core::prelude::v1::test]
    fn test_first_real_frame_moves_preview_from_starting_to_live() {
        let mut preview = LocalPreview::new(LocalPreviewSource::Camera, "Câmera sintética");
        assert_eq!(preview.status, LocalPreviewStatus::Starting);
        assert!(preview.frame.is_none());

        preview
            .accept_frame(&synthetic_frame(
                2,
                2,
                crate::capture::PixelFormat::Rgba8,
                [10, 20, 30, 255].repeat(4),
                1,
            ))
            .expect("primeiro quadro sintético");
        assert_eq!(preview.status, LocalPreviewStatus::Live);
        assert!(preview.frame.is_some());
    }

    #[::core::prelude::v1::test]
    fn test_preview_failure_clears_frame_and_expanded_state() {
        let mut preview = LocalPreview::new(LocalPreviewSource::Screen, "Tela sintética");
        preview
            .accept_frame(&synthetic_frame(
                2,
                2,
                crate::capture::PixelFormat::Rgba8,
                [10, 20, 30, 255].repeat(4),
                1,
            ))
            .expect("quadro sintético");
        preview.expanded = true;
        preview.fail("Permissão negada".to_string());

        assert!(matches!(preview.status, LocalPreviewStatus::Failed(_)));
        assert!(preview.frame.is_none());
        assert!(!preview.expanded);
    }

    #[::core::prelude::v1::test]
    fn test_portal_cancellation_clears_preview_without_live_badge() {
        let mut preview = Some(LocalPreview::new(
            LocalPreviewSource::Screen,
            "Tela sintética",
        ));
        clear_local_preview(&mut preview);
        assert!(preview.is_none());
    }

    #[::core::prelude::v1::test]
    fn test_local_spotlight_is_separate_from_participant_flags() {
        let participant = VoiceParticipantState {
            id: "100000000000000001".into(),
            name: "Participante sintético".into(),
            avatar_text: "P".into(),
            avatar_url: None,
            is_speaking: false,
            is_muted: false,
            is_deafened: false,
            is_camera_on: false,
            is_screen_sharing: false,
            avatar_decoration_data: None,
        };
        let original = participant.clone();
        let mut preview = LocalPreview::new(LocalPreviewSource::Camera, "Câmera sintética");
        preview.expanded = true;

        assert!(preview.expanded);
        assert_eq!(participant, original);
    }

    #[::core::prelude::v1::test]
    fn test_preview_failure_never_changes_broadcast_flags() {
        let mut state = AppState::default();
        state.current_user.id = "100000000000000001".into();
        state.connect_voice("200000000000000001", "Voz", "Guild");
        let call = state.active_voice_call.as_ref().unwrap();
        assert!(!call.is_camera_on);
        assert!(!call.is_screen_sharing);

        let mut preview = LocalPreview::new(LocalPreviewSource::Camera, "Câmera sintética");
        preview.fail("Dispositivo ocupado".to_string());
        let call = state.active_voice_call.as_ref().unwrap();
        assert!(!call.is_camera_on);
        assert!(!call.is_screen_sharing);
    }
}
