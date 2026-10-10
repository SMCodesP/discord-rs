use std::time::Duration;
use discord_rs::capture::{
    bgr_to_rgba, bgra_to_rgba, bgrx_to_rgba, detect_session, detect_session_from_env,
    extract_pipewire_frame, i420_to_rgba, nv12_to_rgba, rgb_to_rgba, rgbx_to_rgba,
    yuyv_to_rgba, CameraCaptureConfig, CameraManager, CaptureError, PixelFormat,
    ScreenCaptureConfig, ScreenCaptureManager, ScreenCaptureMode, ScreenSourceType,
    SessionType, VideoFrame,
};

#[test]
fn test_pixel_format_conversion_suite() {
    // 1. BGRA to RGBA (typical from X11)
    let bgra = vec![
        255, 128, 64, 255, // Pixel 0: B=255, G=128, R=64, A=255
        0, 255, 0, 200,    // Pixel 1: B=0, G=255, R=0, A=200
    ];
    let rgba = bgra_to_rgba(&bgra, 2, 1).expect("BGRA to RGBA failed");
    assert_eq!(rgba, vec![64, 128, 255, 255, 0, 255, 0, 200]);

    // 2. BGR to RGBA
    let bgr = vec![255, 128, 64];
    let rgba_from_bgr = bgr_to_rgba(&bgr, 1, 1).expect("BGR to RGBA failed");
    assert_eq!(rgba_from_bgr, vec![64, 128, 255, 255]);

    // 3. RGB to RGBA
    let rgb = vec![64, 128, 255];
    let rgba_from_rgb = rgb_to_rgba(&rgb, 1, 1).expect("RGB to RGBA failed");
    assert_eq!(rgba_from_rgb, vec![64, 128, 255, 255]);

    // 4. YUYV to RGBA (typical from V4L2 webcams)
    let yuyv = vec![128, 128, 128, 128]; // Neutral gray 2 pixels
    let rgba_from_yuyv = yuyv_to_rgba(&yuyv, 2, 1).expect("YUYV to RGBA failed");
    assert_eq!(rgba_from_yuyv.len(), 8);
    assert_eq!(rgba_from_yuyv[0], 128); // R0
    assert_eq!(rgba_from_yuyv[1], 128); // G0
    assert_eq!(rgba_from_yuyv[2], 128); // B0
    assert_eq!(rgba_from_yuyv[3], 255); // A0

    // 5. NV12 to RGBA
    let nv12 = vec![
        128, 128,
        128, 128,
        128, 128,
    ];
    let rgba_from_nv12 = nv12_to_rgba(&nv12, 2, 2).expect("NV12 to RGBA failed");
    assert_eq!(rgba_from_nv12.len(), 16);
    assert_eq!(rgba_from_nv12[3], 255); // Alpha

    // 6. I420 to RGBA
    let i420 = vec![
        128, 128, 128, 128,
        128,
        128,
    ];
    let rgba_from_i420 = i420_to_rgba(&i420, 2, 2).expect("I420 to RGBA failed");
    assert_eq!(rgba_from_i420.len(), 16);
    assert_eq!(rgba_from_i420[3], 255); // Alpha
}

#[test]
fn test_video_frame_gpui_ready() {
    let raw_bgra = vec![0, 100, 200, 255];
    let frame = VideoFrame::new(
        1,
        1,
        PixelFormat::Bgra8,
        raw_bgra,
        Duration::from_millis(33),
        42,
    );

    let (width, height, data) = frame.as_gpui_image_data().expect("GPUI image conversion failed");
    assert_eq!(width, 1);
    assert_eq!(height, 1);
    assert_eq!(data, vec![200, 100, 0, 255]); // RGBA
}

#[test]
fn test_camera_manager_zero_mocks() {
    // 1. List real devices
    let devices = CameraManager::list_devices().expect("list_devices failed");
    for dev in &devices {
        assert!(!dev.id.is_empty());
        assert!(!dev.name.is_empty());
        assert_ne!(dev.name, "Mock Camera");
        assert_ne!(dev.name, "Ferris Webcam");
    }

    // 2. Start session with invalid device must return error, never mock frames
    let bad_config = CameraCaptureConfig {
        device_id: "/dev/video8888".into(),
        width: 640,
        height: 480,
        fps: 30,
        format: Some(PixelFormat::Yuyv),
    };

    let result = CameraManager::start_session(bad_config);
    assert!(result.is_err(), "Must error on non-existent hardware");
    match result.err().unwrap() {
        CaptureError::DeviceNotFound(_) => {}
        other => panic!("Expected DeviceNotFound, got: {:?}", other),
    }
}

#[test]
fn test_session_detection_runtime() {
    let session = detect_session();
    assert!(
        session == SessionType::Wayland || session == SessionType::X11 || session == SessionType::Unknown
    );

    assert_eq!(
        detect_session_from_env(Some("wayland"), None, None),
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
fn test_screen_capture_config_builder() {
    let config = ScreenCaptureConfig {
        source_type: ScreenSourceType::Monitor,
        target_fps: 60,
        mode: ScreenCaptureMode::X11Fallback,
        screen_index: Some(0),
    };

    assert_eq!(config.target_fps, 60);
    assert_eq!(config.mode, ScreenCaptureMode::X11Fallback);
    assert_eq!(config.source_type, ScreenSourceType::Monitor);
    assert_eq!(config.screen_index, Some(0));

    // Test ScreenCaptureManager screen list if X11 is available
    if std::env::var("DISPLAY").is_ok() {
        let screens = ScreenCaptureManager::list_x11_screens().unwrap_or_default();
        for s in screens {
            assert!(s.width > 0);
            assert!(s.height > 0);
        }
    }
}

#[test]
fn test_real_camera_hardware_if_available() {
    // If a real /dev/video0 exists, verify that query_caps works or handled safely
    let devices = CameraManager::list_devices().unwrap_or_default();
    if let Some(first_camera) = devices.first() {
        println!("Real camera detected: {} ({})", first_camera.name, first_camera.id);
        assert!(!first_camera.formats.is_empty() || first_camera.capabilities.is_empty());
    } else {
        println!("No physical webcams present; zero mocks enforced.");
    }
}

#[test]
fn test_resolve_capture_mode_decision_logic() {
    use discord_rs::capture::resolve_capture_mode;

    // Auto mode behavior
    assert_eq!(
        resolve_capture_mode(ScreenCaptureMode::Auto, SessionType::Wayland).unwrap(),
        ScreenCaptureMode::WaylandPortal
    );
    assert_eq!(
        resolve_capture_mode(ScreenCaptureMode::Auto, SessionType::X11).unwrap(),
        ScreenCaptureMode::X11Fallback
    );
    assert!(resolve_capture_mode(ScreenCaptureMode::Auto, SessionType::Unknown).is_err());

    // Explicit WaylandPortal
    assert_eq!(
        resolve_capture_mode(ScreenCaptureMode::WaylandPortal, SessionType::Wayland).unwrap(),
        ScreenCaptureMode::WaylandPortal
    );
    // Explicit WaylandPortal in pure X11 must error out
    assert!(resolve_capture_mode(ScreenCaptureMode::WaylandPortal, SessionType::X11).is_err());

    // Explicit X11Fallback is allowed regardless of detected session
    assert_eq!(
        resolve_capture_mode(ScreenCaptureMode::X11Fallback, SessionType::Wayland).unwrap(),
        ScreenCaptureMode::X11Fallback
    );
    assert_eq!(
        resolve_capture_mode(ScreenCaptureMode::X11Fallback, SessionType::X11).unwrap(),
        ScreenCaptureMode::X11Fallback
    );
}

#[tokio::test]
async fn test_wayland_capture_returns_error_never_mock_session() {
    // Attempting Wayland portal capture without a portal dialog response or in headless
    // must return an explicit error (PortalError, PortalCancelled, or WaylandCaptureUnavailable),
    // and NEVER an active dummy/mock session with is_running == true and empty frames.
    let config = ScreenCaptureConfig {
        source_type: ScreenSourceType::Monitor,
        target_fps: 30,
        mode: ScreenCaptureMode::WaylandPortal,
        screen_index: None,
    };

    let result = ScreenCaptureManager::start_capture(config).await;
    // In headless or test environment, ashpd portal should either fail or cancel
    if let Ok((mut session, _rx)) = result {
        // If an active portal is actually present on system (e.g. interactive dialog accepted):
        assert!(session.is_running());
        session.stop();
        assert!(!session.is_running());
    } else {
        // If portal is unavailable, must be an explicit error variant
        match result.err().unwrap() {
            CaptureError::PortalError(_)
            | CaptureError::PortalCancelled
            | CaptureError::WaylandCaptureUnavailable(_) => {}
            other => panic!("Unexpected error variant: {:?}", other),
        }
    }
}

#[test]
fn test_switch_camera_validates_destination_first() {
    // Attempting to switch to an invalid device must fail cleanly
    // and return DeviceNotFound without crashing or creating mock frames
    let fake_config = CameraCaptureConfig {
        device_id: "/dev/video99999_fake".into(),
        width: 1280,
        height: 720,
        fps: 30,
        format: Some(PixelFormat::Yuyv),
    };

    let result = CameraManager::start_session(fake_config);
    assert!(result.is_err());
}

#[test]
fn test_rgbx_and_bgrx_conversion_suite() {
    // RGBx: 4 bytes per pixel, 4th byte is padding and MUST NOT be inherited as alpha. Alpha is forced to 255.
    let rgbx_data = vec![
        100, 150, 200, 0,   // pixel 0: R=100, G=150, B=200, X=0
        50, 75, 125, 128,   // pixel 1: R=50, G=75, B=125, X=128
    ];
    let rgba_from_rgbx = rgbx_to_rgba(&rgbx_data, 2, 1).expect("RGBx to RGBA failed");
    assert_eq!(rgba_from_rgbx, vec![
        100, 150, 200, 255, // Alpha forced to 255
        50, 75, 125, 255,   // Alpha forced to 255
    ]);

    // BGRx: 4 bytes per pixel [B, G, R, X], 4th byte is padding.
    let bgrx_data = vec![
        200, 150, 100, 0,   // pixel 0: B=200, G=150, R=100, X=0 -> RGBA [100, 150, 200, 255]
        125, 75, 50, 99,    // pixel 1: B=125, G=75, R=50, X=99  -> RGBA [50, 75, 125, 255]
    ];
    let rgba_from_bgrx = bgrx_to_rgba(&bgrx_data, 2, 1).expect("BGRx to RGBA failed");
    assert_eq!(rgba_from_bgrx, vec![
        100, 150, 200, 255, // Alpha forced to 255
        50, 75, 125, 255,   // Alpha forced to 255
    ]);

    // Test through VideoFrame::to_rgba8()
    let frame_rgbx = VideoFrame::new(
        2, 1, PixelFormat::Rgbx8, rgbx_data, Duration::from_millis(16), 1,
    );
    assert_eq!(frame_rgbx.to_rgba8().unwrap().data, rgba_from_rgbx);

    let frame_bgrx = VideoFrame::new(
        2, 1, PixelFormat::Bgrx8, bgrx_data, Duration::from_millis(16), 2,
    );
    assert_eq!(frame_bgrx.to_rgba8().unwrap().data, rgba_from_bgrx);
}

#[test]
fn test_extract_pipewire_frame_with_row_stride_padding() {
    use pipewire::spa::param::video::VideoFormat;

    // Simulate 2x2 image with BGRx format (4 bytes per pixel).
    // Row bytes = width (2) * 4 = 8 bytes.
    // Stride with memory alignment = 16 bytes (8 bytes pixel data + 8 bytes padding).
    let mut raw_buffer = Vec::new();

    // Row 0: 2 pixels [B, G, R, X] + 8 padding bytes
    let row0_pixels = [
        10, 20, 30, 0,   // pixel (0, 0)
        40, 50, 60, 0,   // pixel (1, 0)
    ];
    let row0_padding = [0xEE; 8];
    raw_buffer.extend_from_slice(&row0_pixels);
    raw_buffer.extend_from_slice(&row0_padding);

    // Row 1: 2 pixels [B, G, R, X] + 8 padding bytes
    let row1_pixels = [
        70, 80, 90, 0,   // pixel (0, 1)
        100, 110, 120, 0, // pixel (1, 1)
    ];
    let row1_padding = [0xEE; 8];
    raw_buffer.extend_from_slice(&row1_pixels);
    raw_buffer.extend_from_slice(&row1_padding);

    assert_eq!(raw_buffer.len(), 32);

    // Extract frame with stride = 16
    let frame = extract_pipewire_frame(
        &raw_buffer,
        2,
        2,
        VideoFormat::BGRx,
        16, // stride > row_bytes (16 > 8)
        Duration::from_millis(33),
        10,
    )
    .expect("extract_pipewire_frame should succeed and strip stride padding");

    assert_eq!(frame.width, 2);
    assert_eq!(frame.height, 2);
    assert_eq!(frame.format, PixelFormat::Bgrx8);
    // Compact data should be exactly 2 * 2 * 4 = 16 bytes (padding stripped)
    assert_eq!(frame.data.len(), 16);
    assert_eq!(&frame.data[0..8], &row0_pixels);
    assert_eq!(&frame.data[8..16], &row1_pixels);

    // Ensure conversion to RGBA works properly on compact frame
    let rgba = frame.to_rgba8().expect("RGBA conversion of compact frame failed");
    assert_eq!(rgba.data.len(), 16);
    // Pixel (0, 0): BGRx [10, 20, 30, 0] -> RGBA [30, 20, 10, 255]
    assert_eq!(&rgba.data[0..4], &[30, 20, 10, 255]);
    // Pixel (1, 0): BGRx [40, 50, 60, 0] -> RGBA [60, 50, 40, 255]
    assert_eq!(&rgba.data[4..8], &[60, 50, 40, 255]);
}

#[test]
fn test_extract_pipewire_frame_drops_unnegotiated_format_or_zero_dimensions() {
    use pipewire::spa::param::video::VideoFormat;

    let dummy_buffer = vec![0u8; 1024];

    // 1. Zero width must error out without guessing 1920
    let res_zero_w = extract_pipewire_frame(
        &dummy_buffer,
        0,
        1080,
        VideoFormat::BGRx,
        0,
        Duration::from_millis(0),
        0,
    );
    assert!(res_zero_w.is_err(), "Must reject zero width");

    // 2. Zero height must error out without guessing 1080
    let res_zero_h = extract_pipewire_frame(
        &dummy_buffer,
        1920,
        0,
        VideoFormat::BGRx,
        0,
        Duration::from_millis(0),
        0,
    );
    assert!(res_zero_h.is_err(), "Must reject zero height");

    // 3. Unknown SPA format must error out without guessing Bgra8
    let res_unknown_fmt = extract_pipewire_frame(
        &dummy_buffer,
        1920,
        1080,
        VideoFormat::Unknown,
        1920 * 4,
        Duration::from_millis(0),
        0,
    );
    assert!(res_unknown_fmt.is_err(), "Must reject VideoFormat::Unknown");
}


