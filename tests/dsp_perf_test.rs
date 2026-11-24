use std::borrow::Cow;
use std::hint::black_box;
use std::time::{Duration, Instant};

use discord_rs::media::{
    f32_to_i16_into, fast_path_48k, i16_to_f32_into, limit_i16_into, process_48k_in_place,
    soft_limit, soft_limit_in_place, StreamFormat, FAST_PATH_SAMPLE_RATE, SOFT_LIMIT_KNEE,
};

const FRAME_SAMPLES: usize = 960 * 2;

fn sine_stereo_f32(freq_hz: f32, amplitude: f32, frames: usize) -> Vec<f32> {
    let mut pcm = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let t = frame as f32 / FAST_PATH_SAMPLE_RATE as f32;
        let sample = (std::f32::consts::TAU * freq_hz * t).sin() * amplitude;
        pcm.push(sample);
        pcm.push(sample);
    }
    pcm
}

fn sine_stereo_i16(freq_hz: f32, amplitude: f32, frames: usize) -> Vec<i16> {
    sine_stereo_f32(freq_hz, amplitude, frames)
        .into_iter()
        .map(|sample| (sample * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect()
}

#[test]
fn fast_path_48k_borrows_native_stereo_without_copy() {
    let input = sine_stereo_f32(440.0, 0.5, 960);
    let out = fast_path_48k(&input, StreamFormat::new(48_000, 2)).expect("valid format");

    match out {
        Cow::Borrowed(pcm) => {
            assert_eq!(pcm.as_ptr(), input.as_ptr());
            assert_eq!(pcm, input.as_slice());
        }
        Cow::Owned(_) => panic!("48 kHz stereo must take the zero-copy path"),
    }
}

#[test]
fn fast_path_48k_remaps_mono_at_native_rate() {
    let input = sine_stereo_f32(330.0, 0.25, 960)
        .into_iter()
        .step_by(2)
        .collect::<Vec<_>>();
    let out = fast_path_48k(&input, StreamFormat::new(48_000, 1)).expect("valid format");

    assert!(matches!(out, Cow::Owned(_)));
    assert_eq!(out.len(), input.len() * 2);
    for (frame, sample) in input.iter().enumerate() {
        assert_eq!(out[frame * 2], *sample);
        assert_eq!(out[frame * 2 + 1], *sample);
    }
}

#[test]
fn fast_path_48k_resamples_44k1_to_native_rate() {
    let input = sine_stereo_f32(440.0, 0.5, 441);
    let out = fast_path_48k(&input, StreamFormat::new(44_100, 2)).expect("valid format");

    assert!(matches!(out, Cow::Owned(_)));
    let expected = 480 * 2;
    assert!(
        out.len().abs_diff(expected) <= 4,
        "expected about {expected} samples, got {}",
        out.len()
    );
    assert!(out.iter().any(|sample| sample.abs() > 0.1));
    assert!(out.iter().all(|sample| sample.abs() <= 1.0));
}

#[test]
fn fast_path_48k_rejects_degenerate_formats() {
    let input = [0.0f32; 16];
    assert!(fast_path_48k(&input, StreamFormat::new(0, 2)).is_none());
    assert!(fast_path_48k(&input, StreamFormat::new(48_000, 0)).is_none());
}

#[test]
fn i16_to_f32_roundtrip_is_exact_for_every_sample_value() {
    let input: Vec<i16> = (i16::MIN..=i16::MAX).collect();
    let mut as_f32 = vec![0.0f32; input.len()];
    let mut back = vec![0i16; input.len()];

    assert_eq!(i16_to_f32_into(&input, &mut as_f32), input.len());
    assert_eq!(f32_to_i16_into(&as_f32, &mut back), input.len());
    assert_eq!(back, input);
}

#[test]
fn conversions_write_into_preallocated_buffers_only() {
    let input = sine_stereo_i16(440.0, 0.5, 960);
    let mut f32_buffer = vec![0.0f32; input.len()];
    let f32_ptr = f32_buffer.as_ptr();
    let f32_len = f32_buffer.len();

    assert_eq!(i16_to_f32_into(&input, &mut f32_buffer), input.len());
    assert_eq!(f32_buffer.as_ptr(), f32_ptr);
    assert_eq!(f32_buffer.len(), f32_len);

    let mut i16_buffer = vec![0i16; input.len()];
    let i16_ptr = i16_buffer.as_ptr();
    let i16_len = i16_buffer.len();

    assert_eq!(f32_to_i16_into(&f32_buffer, &mut i16_buffer), input.len());
    assert_eq!(i16_buffer.as_ptr(), i16_ptr);
    assert_eq!(i16_buffer.len(), i16_len);
    assert_eq!(i16_buffer, input);
}

#[test]
fn conversion_and_limiting_fit_the_20ms_voice_frame_budget() {
    let input = sine_stereo_i16(440.0, 1.5, 960);
    let mut f32_buffer = vec![0.0f32; FRAME_SAMPLES];
    let mut i16_buffer = vec![0i16; FRAME_SAMPLES];

    let iterations = 500u32;
    let start = Instant::now();
    for _ in 0..iterations {
        i16_to_f32_into(black_box(&input), black_box(&mut f32_buffer));
        soft_limit_in_place(black_box(&mut f32_buffer));
        f32_to_i16_into(black_box(&f32_buffer), black_box(&mut i16_buffer));
    }
    let per_frame = start.elapsed() / iterations;

    // A 20 ms frame at 48 kHz stereo is 1920 samples. Even a generous CI
    // allowance must stay far inside the real-time deadline.
    assert!(
        per_frame < Duration::from_millis(20),
        "per-frame DSP too slow: {per_frame:?} (budget 20 ms)"
    );
}

#[test]
fn limiter_is_bit_exact_below_knee() {
    let mut samples = sine_stereo_f32(440.0, SOFT_LIMIT_KNEE, 960);
    let original = samples.clone();

    soft_limit_in_place(&mut samples);

    for (after, before) in samples.iter().zip(original.iter()) {
        assert_eq!(after.to_bits(), before.to_bits());
    }
    for sample in [-SOFT_LIMIT_KNEE, -0.5, 0.0, 0.5, SOFT_LIMIT_KNEE] {
        assert_eq!(soft_limit(sample).to_bits(), sample.to_bits());
    }
}

#[test]
fn limiter_is_bounded_and_sign_preserving_on_overload() {
    let mut samples = sine_stereo_f32(1000.0, 8.0, 960);
    soft_limit_in_place(&mut samples);

    assert!(samples.iter().all(|sample| sample.is_finite()));
    assert!(samples.iter().all(|sample| sample.abs() <= 1.0));
    assert!(samples.iter().any(|sample| sample.abs() > SOFT_LIMIT_KNEE));

    for sample in [1.0f32, 1.5, 10.0, 1_000.0] {
        let limited = soft_limit(sample);
        assert!(limited > SOFT_LIMIT_KNEE && limited <= 1.0);
    }
    for sample in [-1.0f32, -1.5, -10.0, -1_000.0] {
        let limited = soft_limit(sample);
        assert!(limited < -SOFT_LIMIT_KNEE && limited >= -1.0);
    }
    assert_eq!(soft_limit(f32::NAN), 0.0);
    assert_eq!(soft_limit(f32::INFINITY), 0.0);
    assert_eq!(soft_limit(f32::NEG_INFINITY), 0.0);
}

#[test]
fn limiter_i16_path_is_transparent_for_normal_levels() {
    let input = sine_stereo_i16(440.0, 0.5, 960);
    let mut output = vec![0i16; input.len()];

    assert_eq!(limit_i16_into(&input, &mut output), input.len());
    assert_eq!(output, input);
}

#[test]
fn process_in_place_uses_fast_path_only_at_native_rate() {
    let mut samples = sine_stereo_f32(440.0, 1.5, 960);

    assert!(!process_48k_in_place(
        &mut samples,
        StreamFormat::new(44_100, 2)
    ));
    assert!(samples.iter().any(|sample| sample.abs() > 1.0));

    assert!(process_48k_in_place(
        &mut samples,
        StreamFormat::new(48_000, 2)
    ));
    assert!(samples.iter().all(|sample| sample.abs() <= 1.0));
}
