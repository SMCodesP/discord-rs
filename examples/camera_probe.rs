use std::env;
use std::time::{Duration, Instant};

use discord_rs::capture::{CameraCaptureConfig, CameraManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "camera_probe.png".to_string());
    let requested_device = env::args().nth(2);

    println!("==================================================");
    println!(" discord-rs Camera Probe");
    println!(" Output destination: {}", output_path);
    println!("==================================================");

    println!("Enumerating connected video capture devices...");
    let devices = CameraManager::list_devices()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    if devices.is_empty() {
        println!("No physical video capture devices detected on this system.");
        println!("Zero mocks enforced: aborting without creating fake frames.");
        return Ok(());
    }

    println!("Found {} camera device(s):", devices.len());
    for (i, dev) in devices.iter().enumerate() {
        println!("  [{}] {} ({})", i, dev.name, dev.id);
        for res in dev.capabilities.iter().take(4) {
            println!(
                "      {}x{} @ {} fps (format: {:?})",
                res.width, res.height, res.max_fps, res.format
            );
        }
    }

    let target_dev = if let Some(ref req_path) = requested_device {
        devices
            .iter()
            .find(|d| d.id == *req_path)
            .ok_or_else(|| format!("Requested device '{}' not found", req_path))?
    } else {
        &devices[0]
    };

    println!("Selecting camera: {} ({})", target_dev.name, target_dev.id);

    // Pick suitable resolution (prefer 1280x720 or 640x480 if available, else first)
    let chosen_res = target_dev
        .capabilities
        .iter()
        .find(|r| r.width == 1280 && r.height == 720)
        .or_else(|| target_dev.capabilities.iter().find(|r| r.width == 640 && r.height == 480))
        .or_else(|| target_dev.capabilities.first());

    let (width, height, fps, format) = if let Some(r) = chosen_res {
        (r.width, r.height, r.max_fps.min(30).max(1), Some(r.format))
    } else {
        (1280, 720, 30, None)
    };

    let config = CameraCaptureConfig {
        device_id: target_dev.id.clone(),
        width,
        height,
        fps,
        format,
    };

    println!(
        "Starting capture session: {}x{} @ {} fps ({:?})...",
        config.width, config.height, config.fps, config.format
    );

    let (mut session, mut rx) = CameraManager::start_session(config)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    println!("Waiting for first frame (up to 5s)...");
    let start_wait = Instant::now();

    let first_frame = match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
        Ok(Some(frame)) => {
            println!("First frame received in {:.2?}!", start_wait.elapsed());
            frame
        }
        Ok(None) => {
            session.stop();
            return Err("Camera frame channel closed before delivering first frame".into());
        }
        Err(_) => {
            session.stop();
            return Err("Timed out waiting 5 seconds for camera frame".into());
        }
    };

    let bpp = first_frame.format.bytes_per_pixel().unwrap_or(2);
    let stride = (first_frame.width as usize) * bpp;

    println!("--------------------------------------------------");
    println!("First Frame Metadata:");
    println!("  Width:     {} px", first_frame.width);
    println!("  Height:    {} px", first_frame.height);
    println!("  Format:    {:?}", first_frame.format);
    println!("  Stride:    {} bytes/line (compact row)", stride);
    println!("  Sequence:  #{}", first_frame.sequence);
    println!("  Timestamp: {:.3?}", first_frame.timestamp);
    println!("--------------------------------------------------");

    println!("Converting frame to RGBA8...");
    let rgba_frame = first_frame
        .to_rgba8()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    println!("Saving snapshot to '{}'...", output_path);
    image::save_buffer(
        &output_path,
        &rgba_frame.data,
        rgba_frame.width,
        rgba_frame.height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    println!("Camera snapshot successfully saved!");

    println!("Measuring throughput for next 3 seconds...");
    let bench_start = Instant::now();
    let mut frames_received = 1;

    while bench_start.elapsed() < Duration::from_secs(3) {
        let remaining = Duration::from_secs(3).saturating_sub(bench_start.elapsed());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(_frame)) => {
                frames_received += 1;
            }
            Ok(None) => {
                println!("Camera frame channel closed during benchmark.");
                break;
            }
            Err(_) => {
                break;
            }
        }
    }

    let elapsed = bench_start.elapsed();
    let fps = (frames_received as f64) / elapsed.as_secs_f64();
    println!("==================================================");
    println!("Throughput Result:");
    println!("  Total frames in 3s window: {}", frames_received);
    println!("  Measured FPS:              {:.2}", fps);
    println!("==================================================");

    println!("Stopping camera session...");
    session.stop();
    println!("Camera session closed cleanly.");

    Ok(())
}
