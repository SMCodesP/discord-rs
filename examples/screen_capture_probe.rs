use std::env;
use std::time::{Duration, Instant};

use discord_rs::capture::{
    ScreenCaptureConfig, ScreenCaptureManager, ScreenCaptureMode, ScreenSourceType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "screen_probe.png".to_string());

    println!("==================================================");
    println!(" discord-rs Screen Capture Probe (Auto Mode)");
    println!(" Output destination: {}", output_path);
    println!("==================================================");
    println!("Requesting screen capture session (Wayland Portal / X11)...");
    println!("NOTE: If on Wayland, please select a screen/window in the system dialog within 30s.");

    let config = ScreenCaptureConfig {
        source_type: ScreenSourceType::Monitor,
        target_fps: 30,
        mode: ScreenCaptureMode::Auto,
        screen_index: None,
    };

    let (mut session, mut rx) = ScreenCaptureManager::start_capture(config)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    println!("Capture session initialized. Waiting for first video frame (up to 30s)...");
    let start_wait = Instant::now();

    let first_frame = match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
        Ok(Some(frame)) => {
            println!("First frame received in {:.2?}!", start_wait.elapsed());
            frame
        }
        Ok(None) => {
            session.stop();
            return Err("Frame stream closed before delivering first frame".into());
        }
        Err(_) => {
            session.stop();
            return Err("Timed out waiting 30 seconds for first screen capture frame".into());
        }
    };

    let bpp = first_frame.format.bytes_per_pixel().unwrap_or(4);
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
    println!("Snapshot successfully saved!");

    println!("Measuring throughput for next 3 seconds...");
    let bench_start = Instant::now();
    let mut frames_received = 1; // already got the first frame

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
                println!("Frame channel closed during throughput measurement.");
                break;
            }
            Err(_) => {
                // Timeout reached
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

    println!("Stopping screen capture session...");
    session.stop();
    println!("Screen capture session closed cleanly.");

    Ok(())
}
