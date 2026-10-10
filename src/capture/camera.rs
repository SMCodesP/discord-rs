use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use v4l::buffer::Type;
use v4l::format::fourcc::FourCC;
use v4l::frameinterval::FrameIntervalEnum;
use v4l::framesize::FrameSizeEnum;
use v4l::io::mmap::Stream;
use v4l::io::traits::CaptureStream;
use v4l::video::Capture;
use v4l::Device;

use super::error::CaptureError;
use super::frame::{PixelFormat, VideoFrame};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CameraResolution {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub max_fps: u32,
    pub available_fps: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub bus_info: String,
    pub formats: Vec<PixelFormat>,
    pub capabilities: Vec<CameraResolution>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraCaptureConfig {
    pub device_id: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: Option<PixelFormat>,
}

impl Default for CameraCaptureConfig {
    fn default() -> Self {
        Self {
            device_id: "/dev/video0".to_string(),
            width: 1280,
            height: 720,
            fps: 30,
            format: Some(PixelFormat::Yuyv),
        }
    }
}

pub struct CameraManager;

impl CameraManager {
    /// Lists real webcams on the system by querying V4L2 device nodes.
    /// Filters out non-video nodes (such as metadata/IR capture nodes).
    /// If no webcams are connected or permissions are denied, returns an empty list or handled error.
    /// ZERO MOCKS: never returns fake devices.
    pub fn list_devices() -> Result<Vec<CameraInfo>, CaptureError> {
        let nodes = v4l::context::enum_devices();
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let mut cameras = Vec::new();

        for node in nodes {
            let path = node.path();
            let dev_result = Device::with_path(path);
            let dev = match dev_result {
                Ok(d) => d,
                Err(e) => {
                    log::debug!("Unable to open device at {:?}: {}", path, e);
                    continue;
                }
            };

            let caps = match dev.query_caps() {
                Ok(c) => c,
                Err(e) => {
                    log::debug!("Unable to query capabilities for {:?}: {}", path, e);
                    continue;
                }
            };

            // Ensure the device supports actual video capture (filters out metadata nodes like /dev/video1)
            if !caps.capabilities.contains(v4l::capability::Flags::VIDEO_CAPTURE) {
                continue;
            }

            let mut pixel_formats = Vec::new();
            let mut capabilities = Vec::new();

            if let Ok(descs) = dev.enum_formats() {
                for desc in descs {
                    if let Some(pf) = PixelFormat::from_fourcc(&desc.fourcc.repr) {
                        if !pixel_formats.contains(&pf) {
                            pixel_formats.push(pf);
                        }

                        if let Ok(sizes) = dev.enum_framesizes(desc.fourcc) {
                            for sz in sizes {
                                if let FrameSizeEnum::Discrete(discrete) = sz.size {
                                    let mut available_fps = Vec::new();
                                    if let Ok(intervals) = dev.enum_frameintervals(
                                        desc.fourcc,
                                        discrete.width,
                                        discrete.height,
                                    ) {
                                        for interval in intervals {
                                            if let FrameIntervalEnum::Discrete(frac) = interval.interval {
                                                if frac.numerator > 0 {
                                                    let fps = frac.denominator / frac.numerator;
                                                    if !available_fps.contains(&fps) {
                                                        available_fps.push(fps);
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    available_fps.sort_unstable();
                                    let max_fps = available_fps.last().copied().unwrap_or(30);

                                    capabilities.push(CameraResolution {
                                        width: discrete.width,
                                        height: discrete.height,
                                        format: pf,
                                        max_fps,
                                        available_fps,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            cameras.push(CameraInfo {
                id: path.to_string_lossy().into_owned(),
                name: caps.card,
                driver: caps.driver,
                bus_info: caps.bus,
                formats: pixel_formats,
                capabilities,
            });
        }

        Ok(cameras)
    }

    /// Starts a camera capture session with the given configuration.
    /// Returns a `CameraSession` handle and a channel receiver delivering real `VideoFrame`s.
    pub fn start_session(
        config: CameraCaptureConfig,
    ) -> Result<(CameraSession, mpsc::Receiver<VideoFrame>), CaptureError> {
        let (tx, rx) = mpsc::channel(32);
        let session = CameraSession::start_with_sender(config, tx)?;
        Ok((session, rx))
    }
}

pub struct CameraSession {
    current_config: CameraCaptureConfig,
    stop_signal: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    frame_tx: mpsc::Sender<VideoFrame>,
    worker_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl CameraSession {
    fn start_with_sender(
        config: CameraCaptureConfig,
        tx: mpsc::Sender<VideoFrame>,
    ) -> Result<Self, CaptureError> {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let is_running = Arc::new(AtomicBool::new(false));

        let handle = spawn_capture_worker(
            config.clone(),
            Arc::clone(&stop_signal),
            Arc::clone(&is_running),
            tx.clone(),
        )?;

        Ok(Self {
            current_config: config,
            stop_signal,
            is_running,
            frame_tx: tx,
            worker_handle: Arc::new(Mutex::new(Some(handle))),
        })
    }

    pub fn current_config(&self) -> &CameraCaptureConfig {
        &self.current_config
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Stops the camera capture session and releases hardware resources.
    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.is_running.store(false, Ordering::SeqCst);

        if let Ok(mut handle_guard) = self.worker_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                let _ = handle.join();
            }
        }
    }

    /// Dynamically switches to a different camera device or configuration during an active session.
    /// The existing `mpsc::Receiver` channel remains valid and will receive frames from the new source.
    pub fn switch_camera(&mut self, new_config: CameraCaptureConfig) -> Result<(), CaptureError> {
        if new_config.device_id != self.current_config.device_id {
            // Case 1: Switching to a different device.
            // Prepare and spawn the new worker FIRST before stopping the old one.
            let new_stop_signal = Arc::new(AtomicBool::new(false));
            let new_is_running = Arc::new(AtomicBool::new(false));

            let handle = spawn_capture_worker(
                new_config.clone(),
                Arc::clone(&new_stop_signal),
                Arc::clone(&new_is_running),
                self.frame_tx.clone(),
            )?; // If this fails, the current active camera remains running undisturbed!

            // New camera successfully opened: stop and join old worker
            self.stop_signal.store(true, Ordering::SeqCst);
            if let Ok(mut handle_guard) = self.worker_handle.lock() {
                if let Some(old_handle) = handle_guard.take() {
                    let _ = old_handle.join();
                }
                *handle_guard = Some(handle);
            }

            self.stop_signal = new_stop_signal;
            self.is_running = new_is_running;
            self.current_config = new_config;
            Ok(())
        } else {
            // Case 2: Same device (format/fps reconfiguration).
            // V4L2 requires stopping active stream before re-negotiating buffers.
            let old_config = self.current_config.clone();

            // Stop old worker
            self.stop_signal.store(true, Ordering::SeqCst);
            let mut old_handle_opt = None;
            if let Ok(mut handle_guard) = self.worker_handle.lock() {
                old_handle_opt = handle_guard.take();
            }
            if let Some(old_handle) = old_handle_opt {
                let _ = old_handle.join();
            }

            let new_stop_signal = Arc::new(AtomicBool::new(false));
            let new_is_running = Arc::new(AtomicBool::new(false));

            match spawn_capture_worker(
                new_config.clone(),
                Arc::clone(&new_stop_signal),
                Arc::clone(&new_is_running),
                self.frame_tx.clone(),
            ) {
                Ok(new_handle) => {
                    self.stop_signal = new_stop_signal;
                    self.is_running = new_is_running;
                    self.current_config = new_config;
                    if let Ok(mut handle_guard) = self.worker_handle.lock() {
                        *handle_guard = Some(new_handle);
                    }
                    Ok(())
                }
                Err(err) => {
                    // Restore previous configuration so the session does not die
                    log::warn!(
                        "Failed to reconfigure camera on {}: {}. Restoring previous config.",
                        new_config.device_id,
                        err
                    );
                    let restore_stop = Arc::new(AtomicBool::new(false));
                    let restore_running = Arc::new(AtomicBool::new(false));
                    if let Ok(restore_handle) = spawn_capture_worker(
                        old_config.clone(),
                        Arc::clone(&restore_stop),
                        Arc::clone(&restore_running),
                        self.frame_tx.clone(),
                    ) {
                        self.stop_signal = restore_stop;
                        self.is_running = restore_running;
                        self.current_config = old_config;
                        if let Ok(mut handle_guard) = self.worker_handle.lock() {
                            *handle_guard = Some(restore_handle);
                        }
                    }
                    Err(err)
                }
            }
        }
    }

    /// Changes format or framerate without switching the device.
    pub fn change_format(
        &mut self,
        width: u32,
        height: u32,
        fps: u32,
        format: Option<PixelFormat>,
    ) -> Result<(), CaptureError> {
        let mut new_config = self.current_config.clone();
        new_config.width = width;
        new_config.height = height;
        new_config.fps = fps;
        if format.is_some() {
            new_config.format = format;
        }
        self.switch_camera(new_config)
    }
}

impl Drop for CameraSession {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn_capture_worker(
    config: CameraCaptureConfig,
    stop_signal: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    frame_tx: mpsc::Sender<VideoFrame>,
) -> Result<std::thread::JoinHandle<()>, CaptureError> {
    let path = Path::new(&config.device_id);
    if !path.exists() {
        return Err(CaptureError::DeviceNotFound(format!(
            "Video device {} does not exist",
            config.device_id
        )));
    }

    let dev = Device::with_path(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            CaptureError::PermissionDenied(format!(
                "Permission denied accessing device {}: {}",
                config.device_id, e
            ))
        } else if e.kind() == std::io::ErrorKind::NotFound {
            CaptureError::DeviceNotFound(format!(
                "Device {} not found: {}",
                config.device_id, e
            ))
        } else if e.raw_os_error() == Some(16) {
            // EBUSY (16)
            CaptureError::DeviceBusy(format!(
                "Device {} is currently busy: {}",
                config.device_id, e
            ))
        } else {
            CaptureError::InitializationFailed(format!(
                "Failed to open device {}: {}",
                config.device_id, e
            ))
        }
    })?;

    let caps = dev
        .query_caps()
        .map_err(|e| CaptureError::InitializationFailed(e.to_string()))?;
    if !caps.capabilities.contains(v4l::capability::Flags::VIDEO_CAPTURE) {
        return Err(CaptureError::FormatUnsupported(format!(
            "Device {} does not support VIDEO_CAPTURE",
            config.device_id
        )));
    }

    let target_fourcc_bytes = match config.format {
        Some(fmt) => fmt.to_fourcc().unwrap_or(*b"YUYV"),
        None => *b"YUYV",
    };

    let fourcc = FourCC::new(&target_fourcc_bytes);
    let mut requested_fmt = dev
        .format()
        .unwrap_or_else(|_| v4l::Format::new(config.width, config.height, fourcc));
    requested_fmt.width = config.width;
    requested_fmt.height = config.height;
    requested_fmt.fourcc = fourcc;

    let negotiated_fmt = dev.set_format(&requested_fmt).map_err(|e| {
        CaptureError::FormatUnsupported(format!(
            "Failed to set format on {}: {}",
            config.device_id, e
        ))
    })?;

    if config.fps > 0 {
        if let Ok(mut params) = dev.params() {
            params.interval = v4l::fraction::Fraction::new(1, config.fps);
            let _ = dev.set_params(&params);
        }
    }

    let actual_width = negotiated_fmt.width;
    let actual_height = negotiated_fmt.height;
    let actual_pixel_format =
        PixelFormat::from_fourcc(&negotiated_fmt.fourcc.repr).unwrap_or(PixelFormat::Yuyv);

    let dev_id = config.device_id.clone();
    let thread_name = format!("v4l-capture-{}", dev_id.replace('/', "-"));

    let handle = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            is_running.store(true, Ordering::SeqCst);

            let stream_result = Stream::with_buffers(&dev, Type::VideoCapture, 4);
            let mut stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to create mmap stream on {}: {}", dev_id, e);
                    is_running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            let _ = stream.set_timeout(Duration::from_millis(500));

            let start_time = std::time::Instant::now();
            let mut sequence = 0u64;

            while !stop_signal.load(Ordering::Relaxed) {
                match stream.next() {
                    Ok((buf, _meta)) => {
                        let frame = VideoFrame {
                            width: actual_width,
                            height: actual_height,
                            format: actual_pixel_format,
                            data: buf.to_vec(),
                            timestamp: start_time.elapsed(),
                            sequence,
                        };
                        sequence += 1;
                        match frame_tx.try_send(frame) {
                            Ok(()) => {}
                            Err(mpsc::error::TrySendError::Full(_)) => {
                                log::trace!("Camera frame dropped: consumer queue full");
                            }
                            Err(mpsc::error::TrySendError::Closed(_)) => {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::TimedOut {
                            continue;
                        }
                        if !stop_signal.load(Ordering::Relaxed) {
                            log::warn!("Camera frame capture error on {}: {}", dev_id, e);
                        }
                        break;
                    }
                }
            }

            is_running.store(false, Ordering::SeqCst);
        })
        .map_err(|e| {
            CaptureError::InitializationFailed(format!(
                "Failed to spawn capture worker thread: {}",
                e
            ))
        })?;

    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_device_not_found_zero_mocks() {
        let config = CameraCaptureConfig {
            device_id: "/dev/video9999_nonexistent".into(),
            width: 1280,
            height: 720,
            fps: 30,
            format: Some(PixelFormat::Yuyv),
        };

        let result = CameraManager::start_session(config);
        assert!(result.is_err(), "Nonexistent camera must return error");
        match result.err().unwrap() {
            CaptureError::DeviceNotFound(msg) => {
                assert!(msg.contains("does not exist") || msg.contains("not found"));
            }
            other => panic!("Expected DeviceNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_camera_list_devices_real_or_empty() {
        let devices = CameraManager::list_devices().expect("Listing devices should not panic");
        // Must either be empty (if no webcams) or have real devices
        for dev in &devices {
            assert!(!dev.id.is_empty());
            assert!(!dev.name.is_empty());
            assert!(!dev.driver.is_empty());
            // ZERO MOCKS: ensure name is not hardcoded fake mock
            assert_ne!(dev.name, "Mock Camera");
            assert_ne!(dev.name, "Ferris Cam");
        }
    }

    #[test]
    fn test_camera_config_default() {
        let config = CameraCaptureConfig::default();
        assert_eq!(config.device_id, "/dev/video0");
        assert_eq!(config.width, 1280);
        assert_eq!(config.height, 720);
        assert_eq!(config.fps, 30);
        assert_eq!(config.format, Some(PixelFormat::Yuyv));
    }
}
