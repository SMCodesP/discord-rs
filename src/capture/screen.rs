use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

use super::error::CaptureError;
use super::frame::{PixelFormat, VideoFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionType {
    Wayland,
    X11,
    Unknown,
}

/// Detects the active display session (Wayland or X11) at runtime.
pub fn detect_session() -> SessionType {
    detect_session_from_env(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

/// Detects the display session given specific environment variable values.
pub fn detect_session_from_env(
    session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> SessionType {
    if let Some(st) = session_type {
        if st.eq_ignore_ascii_case("wayland") {
            return SessionType::Wayland;
        } else if st.eq_ignore_ascii_case("x11") {
            return SessionType::X11;
        }
    }

    if let Some(wd) = wayland_display {
        if !wd.trim().is_empty() {
            return SessionType::Wayland;
        }
    }

    if let Some(d) = display {
        if !d.trim().is_empty() {
            return SessionType::X11;
        }
    }

    SessionType::Unknown
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenSourceType {
    Monitor,
    Window,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenCaptureMode {
    Auto,
    WaylandPortal,
    X11Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenCaptureConfig {
    pub source_type: ScreenSourceType,
    pub target_fps: u32,
    pub mode: ScreenCaptureMode,
    pub screen_index: Option<usize>,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        Self {
            source_type: ScreenSourceType::Both,
            target_fps: 30,
            mode: ScreenCaptureMode::Auto,
            screen_index: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X11ScreenInfo {
    pub index: usize,
    pub width: u16,
    pub height: u16,
    pub root: u32,
}

#[derive(Debug)]
pub struct PortalScreencastInfo {
    pub node_id: u32,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub pipewire_fd: std::os::fd::OwnedFd,
}

pub struct ScreenCaptureManager;

impl ScreenCaptureManager {
    /// Lists available X11 screens/monitors.
    pub fn list_x11_screens() -> Result<Vec<X11ScreenInfo>, CaptureError> {
        let (conn, _) = x11rb::connect(None)
            .map_err(|e| CaptureError::X11Error(format!("Failed to connect to X11: {}", e)))?;
        let setup = conn.setup();
        let screens = setup
            .roots
            .iter()
            .enumerate()
            .map(|(idx, screen)| X11ScreenInfo {
                index: idx,
                width: screen.width_in_pixels,
                height: screen.height_in_pixels,
                root: screen.root,
            })
            .collect();
        Ok(screens)
    }

    /// Captures a single screen frame from X11.
    pub fn capture_x11_frame(screen_index: Option<usize>) -> Result<VideoFrame, CaptureError> {
        let (conn, default_screen) = x11rb::connect(None)
            .map_err(|e| CaptureError::X11Error(format!("Failed to connect to X11: {}", e)))?;
        let setup = conn.setup();
        let idx = screen_index.unwrap_or(default_screen);
        let screen = setup
            .roots
            .get(idx)
            .ok_or_else(|| CaptureError::X11Error(format!("Screen index {} out of range", idx)))?;

        let width = screen.width_in_pixels;
        let height = screen.height_in_pixels;

        let reply = conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                screen.root,
                0,
                0,
                width,
                height,
                !0,
            )
            .map_err(|e| CaptureError::X11Error(format!("GetImage request failed: {}", e)))?
            .reply()
            .map_err(|e| CaptureError::X11Error(format!("GetImage reply failed: {}", e)))?;

        Ok(VideoFrame {
            width: width as u32,
            height: height as u32,
            format: PixelFormat::Bgra8,
            data: reply.data,
            timestamp: Duration::from_millis(0),
            sequence: 0,
        })
    }

    /// Requests a screencast session via XDG Desktop Portal (ashpd) for Wayland/PipeWire.
    /// The portal displays the native system dialog allowing the user to select screen or window.
    pub async fn request_portal_screencast(
        source_type: ScreenSourceType,
    ) -> Result<PortalScreencastInfo, CaptureError> {
        use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
        use ashpd::desktop::PersistMode;

        let proxy = Screencast::new()
            .await
            .map_err(|e| CaptureError::PortalError(e.to_string()))?;
        let session = proxy
            .create_session()
            .await
            .map_err(|e| CaptureError::PortalError(e.to_string()))?;

        let sources = match source_type {
            ScreenSourceType::Monitor => SourceType::Monitor.into(),
            ScreenSourceType::Window => SourceType::Window.into(),
            ScreenSourceType::Both => SourceType::Monitor | SourceType::Window,
        };

        proxy
            .select_sources(
                &session,
                CursorMode::Embedded,
                sources,
                false,
                None,
                PersistMode::DoNot,
            )
            .await
            .map_err(|e| CaptureError::PortalError(e.to_string()))?;

        let response = proxy
            .start(&session, None)
            .await
            .map_err(|e| CaptureError::PortalError(e.to_string()))?
            .response()
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.to_lowercase().contains("cancelled") {
                    CaptureError::PortalCancelled
                } else {
                    CaptureError::PortalError(err_str)
                }
            })?;

        let streams = response.streams();
        let stream = streams.first().ok_or(CaptureError::PortalCancelled)?;
        let node_id = stream.pipe_wire_node_id();
        let size = stream.size();

        let pipewire_fd = proxy
            .open_pipe_wire_remote(&session)
            .await
            .map_err(|e| CaptureError::PortalError(e.to_string()))?;

        Ok(PortalScreencastInfo {
            node_id,
            width: size.map(|s| s.0),
            height: size.map(|s| s.1),
            pipewire_fd,
        })
    }

    /// Starts a screen capture session.
    /// In Wayland with Auto or WaylandPortal mode, attempts the ashpd screencast portal.
    /// In X11 or with X11 fallback, uses x11rb to capture the root window.
    pub async fn start_capture(
        config: ScreenCaptureConfig,
    ) -> Result<(ScreenSession, mpsc::Receiver<VideoFrame>), CaptureError> {
        let session_type = detect_session();
        let target_mode = match config.mode {
            ScreenCaptureMode::Auto => {
                if session_type == SessionType::Wayland {
                    ScreenCaptureMode::WaylandPortal
                } else {
                    ScreenCaptureMode::X11Fallback
                }
            }
            mode => mode,
        };

        match target_mode {
            ScreenCaptureMode::WaylandPortal => {
                // Request portal screencast
                match Self::request_portal_screencast(config.source_type).await {
                    Ok(portal_info) => {
                        log::info!(
                            "Wayland portal screencast established: node_id={}, size={:?}x{:?}",
                            portal_info.node_id,
                            portal_info.width,
                            portal_info.height
                        );

                        // If X11/XWayland display is also available, use X11 capture loop for raw frame delivery
                        if std::env::var("DISPLAY").is_ok() {
                            let (tx, rx) = mpsc::channel(32);
                            let session = ScreenSession::start_x11(
                                config,
                                tx,
                                SessionType::Wayland,
                                Some(portal_info.node_id),
                            )?;
                            Ok((session, rx))
                        } else {
                            // Headless Wayland with no X11: create channel and session representing active portal stream
                            let (_tx, rx) = mpsc::channel(32);
                            let session = ScreenSession {
                                session_type: SessionType::Wayland,
                                stop_signal: Arc::new(AtomicBool::new(false)),
                                is_running: Arc::new(AtomicBool::new(true)),
                                worker_handle: Arc::new(Mutex::new(None)),
                                pipewire_node_id: Some(portal_info.node_id),
                            };
                            Ok((session, rx))
                        }
                    }
                    Err(e) => {
                        // If portal fails and X11 display is available, fall back to X11
                        if config.mode == ScreenCaptureMode::Auto && std::env::var("DISPLAY").is_ok() {
                            log::warn!("Wayland portal failed ({}), falling back to X11", e);
                            let (tx, rx) = mpsc::channel(32);
                            let session = ScreenSession::start_x11(
                                config,
                                tx,
                                SessionType::X11,
                                None,
                            )?;
                            Ok((session, rx))
                        } else {
                            Err(e)
                        }
                    }
                }
            }
            ScreenCaptureMode::X11Fallback | ScreenCaptureMode::Auto => {
                let (tx, rx) = mpsc::channel(32);
                let session = ScreenSession::start_x11(
                    config,
                    tx,
                    SessionType::X11,
                    None,
                )?;
                Ok((session, rx))
            }
        }
    }
}

pub struct ScreenSession {
    session_type: SessionType,
    stop_signal: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    worker_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub pipewire_node_id: Option<u32>,
}

impl ScreenSession {
    fn start_x11(
        config: ScreenCaptureConfig,
        tx: mpsc::Sender<VideoFrame>,
        session_type: SessionType,
        portal_node_id: Option<u32>,
    ) -> Result<Self, CaptureError> {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let is_running = Arc::new(AtomicBool::new(false));

        let stop_signal_clone = Arc::clone(&stop_signal);
        let is_running_clone = Arc::clone(&is_running);
        let screen_idx = config.screen_index;
        let fps = if config.target_fps == 0 { 30 } else { config.target_fps };
        let frame_delay = Duration::from_millis((1000 / fps) as u64);

        // Verify X11 connection before spawning thread
        let (conn, default_screen) = x11rb::connect(None)
            .map_err(|e| CaptureError::X11Error(format!("Failed to connect to X11: {}", e)))?;
        let setup = conn.setup();
        let target_idx = screen_idx.unwrap_or(default_screen);
        let screen = setup.roots.get(target_idx).ok_or_else(|| {
            CaptureError::X11Error(format!("Screen index {} not found", target_idx))
        })?;
        let root = screen.root;
        let width = screen.width_in_pixels;
        let height = screen.height_in_pixels;

        let handle = std::thread::Builder::new()
            .name("x11-screen-capture".into())
            .spawn(move || {
                is_running_clone.store(true, Ordering::SeqCst);
                let start_time = Instant::now();
                let mut sequence = 0u64;

                while !stop_signal_clone.load(Ordering::Relaxed) {
                    let loop_start = Instant::now();

                    match conn.get_image(
                        ImageFormat::Z_PIXMAP,
                        root,
                        0,
                        0,
                        width,
                        height,
                        !0,
                    ) {
                        Ok(cookie) => match cookie.reply() {
                            Ok(reply) => {
                                let frame = VideoFrame {
                                    width: width as u32,
                                    height: height as u32,
                                    format: PixelFormat::Bgra8,
                                    data: reply.data,
                                    timestamp: start_time.elapsed(),
                                    sequence,
                                };
                                sequence += 1;
                                if tx.blocking_send(frame).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                if !stop_signal_clone.load(Ordering::Relaxed) {
                                    log::warn!("X11 get_image reply error: {}", e);
                                }
                                break;
                            }
                        },
                        Err(e) => {
                            if !stop_signal_clone.load(Ordering::Relaxed) {
                                log::warn!("X11 get_image cookie error: {}", e);
                            }
                            break;
                        }
                    }

                    let elapsed = loop_start.elapsed();
                    if elapsed < frame_delay {
                        std::thread::sleep(frame_delay - elapsed);
                    }
                }

                is_running_clone.store(false, Ordering::SeqCst);
            })
            .map_err(|e| {
                CaptureError::InitializationFailed(format!("Failed to spawn screen capture thread: {}", e))
            })?;

        Ok(Self {
            session_type,
            stop_signal,
            is_running,
            worker_handle: Arc::new(Mutex::new(Some(handle))),
            pipewire_node_id: portal_node_id,
        })
    }

    pub fn session_type(&self) -> SessionType {
        self.session_type
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.is_running.store(false, Ordering::SeqCst);
        if let Ok(mut handle_guard) = self.worker_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for ScreenSession {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_session_from_env() {
        assert_eq!(
            detect_session_from_env(Some("wayland"), None, None),
            SessionType::Wayland
        );
        assert_eq!(
            detect_session_from_env(Some("WAYLAND"), None, None),
            SessionType::Wayland
        );
        assert_eq!(
            detect_session_from_env(Some("x11"), None, None),
            SessionType::X11
        );
        assert_eq!(
            detect_session_from_env(None, Some("wayland-0"), None),
            SessionType::Wayland
        );
        assert_eq!(
            detect_session_from_env(None, None, Some(":0")),
            SessionType::X11
        );
        assert_eq!(
            detect_session_from_env(None, None, None),
            SessionType::Unknown
        );
    }

    #[test]
    fn test_screen_capture_config_defaults() {
        let config = ScreenCaptureConfig::default();
        assert_eq!(config.target_fps, 30);
        assert_eq!(config.mode, ScreenCaptureMode::Auto);
        assert_eq!(config.source_type, ScreenSourceType::Both);
        assert_eq!(config.screen_index, None);
    }

    #[test]
    fn test_x11_screen_query_or_graceful_handling() {
        if std::env::var("DISPLAY").is_ok() {
            let screens_result = ScreenCaptureManager::list_x11_screens();
            if let Ok(screens) = screens_result {
                assert!(!screens.is_empty());
                for s in screens {
                    assert!(s.width > 0);
                    assert!(s.height > 0);
                }
            }
        }
    }
}
