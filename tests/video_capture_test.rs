use std::time::Duration;
use discord_rs::capture::{
    bgra_to_rgba, bgr_to_rgba, detect_session, detect_session_from_env, i420_to_rgba,
    nv12_to_rgba, rgb_to_rgba, yuyv_to_rgba, CameraCaptureConfig, CameraManager,
    CaptureError, PixelFormat, ScreenCaptureConfig, ScreenCaptureManager, ScreenCaptureMode,
    ScreenSourceType, SessionType, VideoFrame,
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

