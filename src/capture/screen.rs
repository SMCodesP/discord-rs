use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;
use pipewire as pw;
use pw::spa;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use x11rb::connection::{Connection, RequestConnection};
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

/// Pure decision function determining the target capture mode based on configuration and detected session.
pub fn resolve_capture_mode(
    mode: ScreenCaptureMode,
    session_type: SessionType,
) -> Result<ScreenCaptureMode, CaptureError> {
    match mode {
        ScreenCaptureMode::Auto => match session_type {
            SessionType::Wayland => Ok(ScreenCaptureMode::WaylandPortal),
            SessionType::X11 => Ok(ScreenCaptureMode::X11Fallback),
            SessionType::Unknown => Err(CaptureError::SessionUnavailable(
                "Unable to determine display server session for Auto mode (neither Wayland nor X11 detected)".into(),
            )),
        },
        ScreenCaptureMode::WaylandPortal => {
            if session_type == SessionType::X11 {
                Err(CaptureError::SessionUnavailable(
                    "Cannot use WaylandPortal capture mode in an X11 session".into(),
                ))
            } else {
                Ok(ScreenCaptureMode::WaylandPortal)
            }
        }
        ScreenCaptureMode::X11Fallback => Ok(ScreenCaptureMode::X11Fallback),
    }
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
    pub pipewire_fd: OwnedFd,
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
    /// Manages the portal session lifecycle in a background task until the session is stopped.
    pub async fn request_portal_screencast(
        source_type: ScreenSourceType,
    ) -> Result<(PortalScreencastInfo, tokio::sync::oneshot::Sender<()>), CaptureError> {
        let (init_tx, init_rx) = tokio::sync::oneshot::channel();
        let (close_tx, close_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            let res = async {
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

                let info = PortalScreencastInfo {
                    node_id,
                    width: size.map(|s| s.0),
                    height: size.map(|s| s.1),
                    pipewire_fd,
                };

                Ok::<_, CaptureError>((session, info))
            }
            .await;

            match res {
                Ok((session, info)) => {
                    if init_tx.send(Ok(info)).is_ok() {
                        let _ = close_rx.await;
                        let _ = session.close().await;
                    }
                }
                Err(err) => {
                    let _ = init_tx.send(Err(err));
                }
            }
        });

        match init_rx.await {
            Ok(Ok(info)) => Ok((info, close_tx)),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(CaptureError::PortalError(
                "Portal screencast initiation task dropped unexpectedly".into(),
            )),
        }
    }

    /// Starts a screen capture session.
    pub async fn start_capture(
        config: ScreenCaptureConfig,
    ) -> Result<(ScreenSession, mpsc::Receiver<VideoFrame>), CaptureError> {
        let session_type = detect_session();
        let target_mode = resolve_capture_mode(config.mode, session_type)?;

        match target_mode {
            ScreenCaptureMode::WaylandPortal => {
                let (portal_info, portal_close_tx) =
                    Self::request_portal_screencast(config.source_type).await?;

                let (tx, rx) = mpsc::channel(32);
                let session = ScreenSession::start_wayland_pipewire(
                    portal_info,
                    portal_close_tx,
                    tx,
                )?;
                Ok((session, rx))
            }
            ScreenCaptureMode::X11Fallback => {
                let (tx, rx) = mpsc::channel(32);
                let session = ScreenSession::start_x11(config, tx, SessionType::X11)?;
                Ok((session, rx))
            }
            ScreenCaptureMode::Auto => unreachable!(),
        }
    }
}

pub struct ScreenSession {
    session_type: SessionType,
    stop_signal: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    worker_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pipewire_stop_fd: Arc<Mutex<Option<i32>>>,
    portal_close_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl ScreenSession {
    /// Starts PipeWire stream consumption for Wayland screencast
    fn start_wayland_pipewire(
        portal_info: PortalScreencastInfo,
        portal_close_tx: tokio::sync::oneshot::Sender<()>,
        tx: mpsc::Sender<VideoFrame>,
    ) -> Result<Self, CaptureError> {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let is_running = Arc::new(AtomicBool::new(false));

        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(CaptureError::WaylandCaptureUnavailable(
                "Failed to create PipeWire wakeup pipe".into(),
            ));
        }
        let read_fd = fds[0];
        let write_fd = fds[1];
        let pipewire_stop_fd = Arc::new(Mutex::new(Some(write_fd)));

        let stop_clone = Arc::clone(&stop_signal);
        let running_clone = Arc::clone(&is_running);

        let node_id = portal_info.node_id;
        let fd = portal_info.pipewire_fd;

        let (init_tx, init_rx) = std::sync::mpsc::channel();

        let handle = std::thread::Builder::new()
            .name("pipewire-screencast".into())
            .spawn(move || {
                use std::os::fd::FromRawFd;

                pw::init();

                let mainloop = match pw::main_loop::MainLoopBox::new(None) {
                    Ok(ml) => ml,
                    Err(e) => {
                        let _ = init_tx.send(Err(CaptureError::WaylandCaptureUnavailable(format!(
                            "Failed to create PipeWire main loop: {}",
                            e
                        ))));
                        unsafe { libc::close(read_fd); }
                        return;
                    }
                };

                let raw_mainloop_ptr = mainloop.as_raw_ptr() as usize;
                let read_file = unsafe { std::fs::File::from_raw_fd(read_fd) };
                let _io = mainloop.loop_().add_io(
                    read_file,
                    spa::support::system::IoFlags::IN,
                    move |_| {
                        unsafe {
                            pw::sys::pw_main_loop_quit(raw_mainloop_ptr as *mut pw::sys::pw_main_loop);
                        }
                    },
                );

                let context = match pw::context::ContextBox::new(&mainloop.loop_(), None) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        let _ = init_tx.send(Err(CaptureError::WaylandCaptureUnavailable(format!(
                            "Failed to create PipeWire context: {}",
                            e
                        ))));
                        return;
                    }
                };

                let core = match context.connect_fd(fd, None) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = init_tx.send(Err(CaptureError::WaylandCaptureUnavailable(format!(
                            "Failed to connect PipeWire to portal fd: {}",
                            e
                        ))));
                        return;
                    }
                };

                let props = pw::properties::properties! {
                    *pw::keys::MEDIA_TYPE => "Video",
                    *pw::keys::MEDIA_CATEGORY => "Capture",
                    *pw::keys::MEDIA_ROLE => "Screen",
                    "target.object" => node_id.to_string(),
                };

                let stream = match pw::stream::StreamBox::new(&core, "discord-screen-capture", props) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = init_tx.send(Err(CaptureError::WaylandCaptureUnavailable(format!(
                            "Failed to create PipeWire stream: {}",
                            e
                        ))));
                        return;
                    }
                };

                struct UserData {
                    format: spa::param::video::VideoInfoRaw,
                    frame_tx: mpsc::Sender<VideoFrame>,
                    start_time: Instant,
                    sequence: u64,
                }

                let data = UserData {
                    format: Default::default(),
                    frame_tx: tx,
                    start_time: Instant::now(),
                    sequence: 0,
                };

                let listener = match stream
                    .add_local_listener_with_user_data(data)
                    .param_changed(|_, user_data, id, param| {
                        if let Some(param) = param {
                            if id == spa::param::ParamType::Format.as_raw() {
                                let _ = user_data.format.parse(param);
                            }
                        }
                    })
                    .process(|stream, user_data| {
                        if let Some(mut buffer) = stream.dequeue_buffer() {
                            let datas = buffer.datas_mut();
                            if !datas.is_empty() {
                                let data = &mut datas[0];
                                let chunk = data.chunk();
                                let size = chunk.size() as usize;
                                let offset = chunk.offset() as usize;

                                if size > 0 {
                                    if let Some(bytes) = data.data() {
                                        if offset + size <= bytes.len() {
                                            let frame_data = bytes[offset..offset + size].to_vec();
                                            let rect = user_data.format.size();
                                            let width = rect.width;
                                            let height = rect.height;
                                            let spa_fmt = user_data.format.format();
                                            let format = if spa_fmt == spa::param::video::VideoFormat::RGBA {
                                                PixelFormat::Rgba8
                                            } else if spa_fmt == spa::param::video::VideoFormat::BGRA
                                                || spa_fmt == spa::param::video::VideoFormat::BGRx
                                            {
                                                PixelFormat::Bgra8
                                            } else if spa_fmt == spa::param::video::VideoFormat::RGBx
                                                || spa_fmt == spa::param::video::VideoFormat::RGB
                                            {
                                                PixelFormat::Rgb8
                                            } else if spa_fmt == spa::param::video::VideoFormat::BGR {
                                                PixelFormat::Bgr8
                                            } else if spa_fmt == spa::param::video::VideoFormat::YUY2 {
                                                PixelFormat::Yuyv
                                            } else if spa_fmt == spa::param::video::VideoFormat::NV12 {
                                                PixelFormat::Nv12
                                            } else if spa_fmt == spa::param::video::VideoFormat::I420 {
                                                PixelFormat::I420
                                            } else {
                                                PixelFormat::Bgra8
                                            };

                                            let frame = VideoFrame {
                                                width: if width > 0 { width } else { 1920 },
                                                height: if height > 0 { height } else { 1080 },
                                                format,
                                                data: frame_data,
                                                timestamp: user_data.start_time.elapsed(),
                                                sequence: user_data.sequence,
                                            };
                                            user_data.sequence += 1;

                                            match user_data.frame_tx.try_send(frame) {
                                                Ok(()) => {}
                                                Err(mpsc::error::TrySendError::Full(_)) => {
                                                    log::trace!(
                                                        "Wayland PipeWire frame dropped (slow consumer)"
                                                    );
                                                }
                                                Err(mpsc::error::TrySendError::Closed(_)) => {
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    })
                    .register()
                {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = init_tx.send(Err(CaptureError::WaylandCaptureUnavailable(format!(
                            "Failed to register PipeWire stream listener: {}",
                            e
                        ))));
                        return;
                    }
                };

                let mut params = [];
                if let Err(e) = stream.connect(
                    spa::utils::Direction::Input,
                    Some(node_id),
                    pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
                    &mut params,
                ) {
                    let _ = init_tx.send(Err(CaptureError::WaylandCaptureUnavailable(format!(
                        "Failed to connect PipeWire stream to node {}: {}",
                        node_id, e
                    ))));
                    return;
                }

                running_clone.store(true, Ordering::SeqCst);
                let _ = init_tx.send(Ok(()));

                // Loop runs until quit() is signaled
                while !stop_clone.load(Ordering::Relaxed) {
                    mainloop.run();
                    break;
                }

                drop(listener);
                drop(stream);
                running_clone.store(false, Ordering::SeqCst);
            })
            .map_err(|e| {
                CaptureError::InitializationFailed(format!(
                    "Failed to spawn PipeWire thread: {}",
                    e
                ))
            })?;

        // Wait for worker initialization result
        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                session_type: SessionType::Wayland,
                stop_signal,
                is_running,
                worker_handle: Arc::new(Mutex::new(Some(handle))),
                pipewire_stop_fd,
                portal_close_tx: Arc::new(Mutex::new(Some(portal_close_tx))),
            }),
            Ok(Err(err)) => {
                let _ = handle.join();
                Err(err)
            }
            Err(_) => {
                let _ = handle.join();
                Err(CaptureError::WaylandCaptureUnavailable(
                    "PipeWire initialization thread disconnected prematurely".into(),
                ))
            }
        }
    }

    /// Starts X11 screen capture using MIT-SHM with automatic get_image fallback
    fn start_x11(
        config: ScreenCaptureConfig,
        tx: mpsc::Sender<VideoFrame>,
        session_type: SessionType,
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

                // Check MIT-SHM extension availability
                let shm_available = conn
                    .extension_information(x11rb::protocol::shm::X11_EXTENSION_NAME)
                    .ok()
                    .flatten()
                    .is_some();

                // Allocate SHM segment if supported
                let shm_size = (width as usize) * (height as usize) * 4;
                let mut shm_info: Option<(u32, i32, *mut libc::c_void)> = None;

                if shm_available {
                    unsafe {
                        let shmid = libc::shmget(
                            libc::IPC_PRIVATE,
                            shm_size,
                            libc::IPC_CREAT | 0o600,
                        );
                        if shmid >= 0 {
                            let shmaddr = libc::shmat(shmid, std::ptr::null(), 0);
                            if shmaddr != libc::MAP_FAILED {
                                if let Ok(seg) = conn.generate_id() {
                                    use x11rb::protocol::shm::ConnectionExt as ShmExt;
                                    if conn.shm_attach(seg, shmid as u32, false).is_ok() {
                                        shm_info = Some((seg, shmid, shmaddr));
                                    } else {
                                        libc::shmdt(shmaddr);
                                        libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
                                    }
                                } else {
                                    libc::shmdt(shmaddr);
                                    libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
                                }
                            }
                        }
                    }
                }

                while !stop_signal_clone.load(Ordering::Relaxed) {
                    let loop_start = Instant::now();

                    let frame_data_opt = if let Some((seg, _, shmaddr)) = shm_info {
                        use x11rb::protocol::shm::ConnectionExt as ShmExt;
                        match conn.shm_get_image(
                            root,
                            0,
                            0,
                            width,
                            height,
                            !0,
                            ImageFormat::Z_PIXMAP.into(),
                            seg,
                            0,
                        ) {
                            Ok(cookie) => match cookie.reply() {
                                Ok(_) => unsafe {
                                    Some(std::slice::from_raw_parts(shmaddr as *const u8, shm_size).to_vec())
                                },
                                Err(_) => None,
                            },
                            Err(_) => None,
                        }
                    } else {
                        match conn.get_image(
                            ImageFormat::Z_PIXMAP,
                            root,
                            0,
                            0,
                            width,
                            height,
                            !0,
                        ) {
                            Ok(cookie) => cookie.reply().ok().map(|reply| reply.data),
                            Err(_) => None,
                        }
                    };

                    if let Some(data) = frame_data_opt {
                        let frame = VideoFrame {
                            width: width as u32,
                            height: height as u32,
                            format: PixelFormat::Bgra8,
                            data,
                            timestamp: start_time.elapsed(),
                            sequence,
                        };
                        sequence += 1;

                        // Non-blocking try_send: drop frame if consumer is slow
                        match tx.try_send(frame) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                log::trace!("X11 screen frame dropped (slow consumer)");
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                break;
                            }
                        }
                    }

                    let elapsed = loop_start.elapsed();
                    if elapsed < frame_delay {
                        std::thread::sleep(frame_delay - elapsed);
                    }
                }

                // Cleanup SHM resources if attached
                if let Some((seg, shmid, shmaddr)) = shm_info {
                    use x11rb::protocol::shm::ConnectionExt as ShmExt;
                    let _ = conn.shm_detach(seg);
                    unsafe {
                        libc::shmdt(shmaddr);
                        libc::shmctl(shmid, libc::IPC_RMID, std::ptr::null_mut());
                    }
                }

                is_running_clone.store(false, Ordering::SeqCst);
            })
            .map_err(|e| {
                CaptureError::InitializationFailed(format!(
                    "Failed to spawn screen capture thread: {}",
                    e
                ))
            })?;

        Ok(Self {
            session_type,
            stop_signal,
            is_running,
            worker_handle: Arc::new(Mutex::new(Some(handle))),
            pipewire_stop_fd: Arc::new(Mutex::new(None)),
            portal_close_tx: Arc::new(Mutex::new(None)),
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

        // Wake and quit PipeWire main loop if active by writing to wakeup pipe
        if let Ok(mut guard) = self.pipewire_stop_fd.lock() {
            if let Some(write_fd) = guard.take() {
                unsafe {
                    let _ = libc::write(write_fd, b"q".as_ptr() as *const libc::c_void, 1);
                    libc::close(write_fd);
                }
            }
        }

        if let Ok(mut handle_guard) = self.worker_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                let _ = handle.join();
            }
        }

        // Notify portal task to close session
        if let Ok(mut guard) = self.portal_close_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
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
    fn test_resolve_capture_mode_pure_logic() {
        // Auto mode
        assert_eq!(
            resolve_capture_mode(ScreenCaptureMode::Auto, SessionType::Wayland).unwrap(),
            ScreenCaptureMode::WaylandPortal
        );
        assert_eq!(
            resolve_capture_mode(ScreenCaptureMode::Auto, SessionType::X11).unwrap(),
            ScreenCaptureMode::X11Fallback
        );
        assert!(resolve_capture_mode(ScreenCaptureMode::Auto, SessionType::Unknown).is_err());

        // WaylandPortal explicit
        assert_eq!(
            resolve_capture_mode(ScreenCaptureMode::WaylandPortal, SessionType::Wayland).unwrap(),
            ScreenCaptureMode::WaylandPortal
        );
        assert!(resolve_capture_mode(ScreenCaptureMode::WaylandPortal, SessionType::X11).is_err());

        // X11Fallback explicit
        assert_eq!(
            resolve_capture_mode(ScreenCaptureMode::X11Fallback, SessionType::Wayland).unwrap(),
            ScreenCaptureMode::X11Fallback
        );
        assert_eq!(
            resolve_capture_mode(ScreenCaptureMode::X11Fallback, SessionType::X11).unwrap(),
            ScreenCaptureMode::X11Fallback
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
