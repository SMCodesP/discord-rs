//! Fast-path DSP for the 48 kHz voice pipeline.
//!
//! Discord voice runs at 48 kHz stereo. When the hardware already delivers that
//! format there is no reason to resample or copy: [`fast_path_48k`] borrows the
//! caller's buffer. Sample format conversion writes into caller-provided output
//! slices and never allocates; the contiguous loops are written so LLVM can
//! auto-vectorize them (no explicit SIMD intrinsics or extra scratch buffers).
//!
//! The limiter is transparent below [`SOFT_LIMIT_KNEE`]: samples at normal
//! listening levels pass through bit-for-bit unchanged.

use std::borrow::Cow;

use crate::media::resample::{convert_to_stereo, resample_linear};

/// Native sample rate of the Discord voice pipeline.
pub const FAST_PATH_SAMPLE_RATE: u32 = 48_000;

/// Level below which the soft limiter is a bit-exact passthrough.
pub const SOFT_LIMIT_KNEE: f32 = 0.95;

const I16_SCALE: f32 = 1.0 / 32768.0;
const I16_SCALE_INV: f32 = 32768.0;

/// Interleaved PCM stream format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl StreamFormat {
    pub const fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    /// True when the stream is already at the voice pipeline's native rate, so
    /// no resampling step is needed.
    pub const fn is_fast_path(&self) -> bool {
        self.sample_rate == FAST_PATH_SAMPLE_RATE
    }

    /// True for the fully zero-copy case: native rate and stereo.
    pub const fn is_zero_copy_stereo(&self) -> bool {
        self.is_fast_path() && self.channels == 2
    }
}

/// Normalizes PCM to 48 kHz stereo.
///
/// When the input is already 48 kHz stereo the result borrows `input` with no
/// copy, resampling, or allocation. Other channel layouts at 48 kHz only get a
/// channel remap; other sample rates are resampled. Returns `None` for a
/// degenerate format (zero rate or zero channels).
pub fn fast_path_48k<'a>(input: &'a [f32], format: StreamFormat) -> Option<Cow<'a, [f32]>> {
    if format.sample_rate == 0 || format.channels == 0 {
        return None;
    }

    if format.is_zero_copy_stereo() {
        return Some(Cow::Borrowed(input));
    }

    if format.is_fast_path() {
        return Some(Cow::Owned(convert_to_stereo(
            input,
            format.channels as usize,
        )));
    }

    let channels = format.channels as usize;
    let resampled = resample_linear(
        input,
        format.sample_rate,
        FAST_PATH_SAMPLE_RATE,
        channels,
    );
    Some(Cow::Owned(convert_to_stereo(&resampled, channels)))
}

/// Applies the transparent soft limiter in place when the stream is already at
/// 48 kHz, avoiding a resample and an output allocation.
///
/// Returns `false` when the caller still has to resample to 48 kHz.
pub fn process_48k_in_place(samples: &mut [f32], format: StreamFormat) -> bool {
    if !format.is_fast_path() {
        return false;
    }
    soft_limit_in_place(samples);
    true
}

/// Transparent soft limiter for a single sample.
///
/// Samples at or below [`SOFT_LIMIT_KNEE`] are returned unchanged. Above the
/// knee the magnitude is mapped asymptotically into `(knee, 1.0]`, preserving
/// the sign. Non-finite samples collapse to silence.
#[inline]
pub fn soft_limit(sample: f32) -> f32 {
    if !sample.is_finite() {
        return 0.0;
    }

    let magnitude = sample.abs();
    if magnitude <= SOFT_LIMIT_KNEE {
        return sample;
    }

    let headroom = 1.0 - SOFT_LIMIT_KNEE;
    let excess = (magnitude - SOFT_LIMIT_KNEE) / headroom;
    let limited = SOFT_LIMIT_KNEE + headroom * excess.tanh();
    limited.copysign(sample)
}

/// Applies [`soft_limit`] to every sample in place.
pub fn soft_limit_in_place(samples: &mut [f32]) {
    for sample in samples {
        *sample = soft_limit(*sample);
    }
}

/// Converts interleaved `i16` PCM into `f32` in `[-1.0, 1.0)`.
///
/// Writes into the caller's `output` slice and returns the number of samples
/// converted (`min(input.len(), output.len())`). Performs no allocation.
#[inline]
pub fn i16_to_f32_into(input: &[i16], output: &mut [f32]) -> usize {
    let len = input.len().min(output.len());
    let input = &input[..len];
    let output = &mut output[..len];

    for (dest, &sample) in output.iter_mut().zip(input) {
        *dest = sample as f32 * I16_SCALE;
    }

    len
}

/// Converts interleaved `f32` PCM into `i16` with rounding and clamping.
///
/// Writes into the caller's `output` slice and returns the number of samples
/// converted (`min(input.len(), output.len())`). Non-finite samples become `0`.
/// Performs no allocation.
#[inline]
pub fn f32_to_i16_into(input: &[f32], output: &mut [i16]) -> usize {
    let len = input.len().min(output.len());
    let input = &input[..len];
    let output = &mut output[..len];

    for (dest, &sample) in output.iter_mut().zip(input) {
        *dest = if sample.is_finite() {
            (sample * I16_SCALE_INV)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16
        } else {
            0
        };
    }

    len
}

/// Applies the limiter to `i16` input, writing `i16` output without allocating.
///
/// Returns the number of samples written.
#[inline]
pub fn limit_i16_into(input: &[i16], output: &mut [i16]) -> usize {
    let len = input.len().min(output.len());
    let input = &input[..len];
    let output = &mut output[..len];

    for (dest, &sample) in output.iter_mut().zip(input) {
        *dest = (soft_limit(sample as f32 * I16_SCALE) * I16_SCALE_INV)
            .round()
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    }

    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_path_borrows_native_stereo() {
        let input = vec![0.25f32; 1920];
        let out = fast_path_48k(&input, StreamFormat::new(48_000, 2)).expect("format");
        match out {
            Cow::Borrowed(slice) => assert_eq!(slice.as_ptr(), input.as_ptr()),
            Cow::Owned(_) => panic!("native 48 kHz stereo must not be copied"),
        }
    }

    #[test]
    fn fast_path_rejects_empty_format() {
        let input = [0.0f32; 8];
        assert!(fast_path_48k(&input, StreamFormat::new(0, 2)).is_none());
        assert!(fast_path_48k(&input, StreamFormat::new(48_000, 0)).is_none());
    }

    #[test]
    fn limiter_is_transparent_below_knee() {
        for sample in [-0.95f32, -0.5, -0.001, 0.0, 0.001, 0.5, 0.95] {
            assert_eq!(soft_limit(sample).to_bits(), sample.to_bits());
        }
    }

    #[test]
    fn limiter_bounds_overload_and_handles_non_finite() {
        for sample in [1.0f32, 1.5, 10.0, 1_000.0] {
            let limited = soft_limit(sample);
            assert!(limited > SOFT_LIMIT_KNEE && limited <= 1.0);
            assert!(limited.is_sign_positive());
        }
        for sample in [-1.0f32, -1.5, -10.0, -1_000.0] {
            let limited = soft_limit(sample);
            assert!(limited < -SOFT_LIMIT_KNEE && limited >= -1.0);
            assert!(limited.is_sign_negative());
        }
        assert_eq!(soft_limit(f32::NAN), 0.0);
        assert_eq!(soft_limit(f32::INFINITY), 0.0);
        assert_eq!(soft_limit(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn i16_f32_roundtrip_is_exact_for_every_value() {
        let input: Vec<i16> = (i16::MIN..=i16::MAX).collect();
        let mut as_f32 = vec![0.0f32; input.len()];
        let mut back = vec![0i16; input.len()];

        assert_eq!(i16_to_f32_into(&input, &mut as_f32), input.len());
        assert_eq!(f32_to_i16_into(&as_f32, &mut back), input.len());
        assert_eq!(back, input);
    }

    #[test]
    fn conversions_respect_short_output_slice() {
        let input = [0i16, 1000, -1000, 2000];
        let mut out = [9.0f32; 2];
        assert_eq!(i16_to_f32_into(&input, &mut out), 2);
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], 1000.0 / 32768.0);

        let input = [0.0f32, 0.5, 1.0, -1.0];
        let mut out = [7i16; 3];
        assert_eq!(f32_to_i16_into(&input, &mut out), 3);
        assert_eq!(out, [0, 16384, i16::MAX]);
    }

    #[test]
    fn process_in_place_only_on_native_rate() {
        let mut samples = [0.5f32, 1.5];
        assert!(!process_48k_in_place(
            &mut samples,
            StreamFormat::new(44_100, 2)
        ));
        assert_eq!(samples, [0.5, 1.5]);

        assert!(process_48k_in_place(
            &mut samples,
            StreamFormat::new(48_000, 2)
        ));
        assert_eq!(samples[0], 0.5);
        assert!(samples[1] > 0.95 && samples[1] <= 1.0);
    }
}
