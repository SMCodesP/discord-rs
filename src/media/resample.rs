use std::collections::VecDeque;

const PHASE_EPSILON: f64 = 1e-9;

/// Stateful linear resampler that maintains phase and fractional position across chunks.
/// Eliminates phase discontinuities, sample drift, and clicks at callback chunk boundaries.
#[derive(Debug, Clone)]
pub struct LinearResampler {
    in_rate: u32,
    out_rate: u32,
    channels: usize,
    phase: f64,
    buffer: VecDeque<f32>,
}

impl LinearResampler {
    pub fn new(in_rate: u32, out_rate: u32, channels: usize) -> Self {
        Self {
            in_rate,
            out_rate,
            channels,
            phase: 0.0,
            buffer: VecDeque::with_capacity(channels.saturating_mul(2048)),
        }
    }

    /// Returns whether this resampler has a usable rate and channel configuration.
    pub fn is_valid(&self) -> bool {
        self.in_rate > 0 && self.out_rate > 0 && self.channels > 0
    }

    /// Resamples an incoming chunk of interleaved PCM audio.
    /// Preserves fractional phase and sample continuity from previous chunks.
    pub fn resample(&mut self, input: &[f32]) -> Vec<f32> {
        if !self.is_valid() {
            self.reset();
            return Vec::new();
        }
        if self.in_rate == self.out_rate {
            return input.to_vec();
        }

        self.buffer.extend(input.iter().copied());
        self.resample_available(false)
    }

    /// Flushes the remaining samples, including the final fractional output sample.
    /// The last input frame is held constant while interpolating the tail.
    pub fn flush(&mut self) -> Vec<f32> {
        if !self.is_valid() {
            self.reset();
            return Vec::new();
        }
        if self.in_rate == self.out_rate {
            self.reset();
            return Vec::new();
        }

        let output = self.resample_available(true);
        self.reset();
        output
    }

    fn resample_available(&mut self, hold_last_frame: bool) -> Vec<f32> {
        let available_frames = self.buffer.len() / self.channels;
        if available_frames == 0 {
            return Vec::new();
        }

        let ratio = self.in_rate as f64 / self.out_rate as f64;
        let remaining_outputs = ((available_frames as f64 - self.phase) / ratio)
            .ceil()
            .max(0.0);
        let mut output =
            Vec::with_capacity((remaining_outputs as usize).saturating_mul(self.channels));

        while self.phase < available_frames as f64 - PHASE_EPSILON {
            let idx0 = self.phase.floor() as usize;
            if idx0 >= available_frames {
                break;
            }
            let idx1 = idx0 + 1;
            if idx1 >= available_frames && !hold_last_frame {
                break;
            }

            let frac = (self.phase - idx0 as f64) as f32;
            let base0 = idx0 * self.channels;
            let base1 = idx1.min(available_frames - 1) * self.channels;

            for ch in 0..self.channels {
                let s0 = self.buffer[base0 + ch];
                let s1 = self.buffer[base1 + ch];
                output.push(s0 * (1.0 - frac) + s1 * frac);
            }

            self.phase += ratio;
        }

        // Repeated f64 additions can land a few ulps below an exact frame boundary.
        // Snap those boundaries so chunked and one-shot resampling emit the same count.
        let nearest_boundary = self.phase.round();
        if (self.phase - nearest_boundary).abs() < PHASE_EPSILON {
            self.phase = nearest_boundary;
        }

        let consumed_frames = self.phase.floor() as usize;
        if consumed_frames > 0 {
            let drain_samples = consumed_frames.min(available_frames) * self.channels;
            self.buffer.drain(0..drain_samples);
            self.phase -= consumed_frames as f64;
        }

        output
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.buffer.clear();
    }
}

/// Convenience function for one-shot batch resampling.
pub fn resample_linear(input: &[f32], in_rate: u32, out_rate: u32, channels: usize) -> Vec<f32> {
    let mut resampler = LinearResampler::new(in_rate, out_rate, channels);
    let mut out = resampler.resample(input);
    out.extend(resampler.flush());
    out
}

/// Converts arbitrary channel count audio to stereo (2 channels).
pub fn convert_to_stereo(input: &[f32], in_channels: usize) -> Vec<f32> {
    if in_channels == 2 {
        return input.to_vec();
    }
    if in_channels == 1 {
        // Mono to stereo: duplicate to both L and R.
        let mut out = Vec::with_capacity(input.len() * 2);
        for &sample in input {
            out.push(sample);
            out.push(sample);
        }
        return out;
    }
    if in_channels == 0 {
        return Vec::new();
    }

    // Multichannel downmix: take front-left (ch 0) and front-right (ch 1).
    let frames = input.len() / in_channels;
    let mut out = Vec::with_capacity(frames * 2);
    for frame_idx in 0..frames {
        let base = frame_idx * in_channels;
        let left = input[base];
        let right = input[base + 1];
        out.push(left);
        out.push(right);
    }
    out
}

/// Converts stereo audio (2 channels) to the target channel count.
pub fn convert_stereo_to_channels(input_stereo: &[f32], out_channels: usize) -> Vec<f32> {
    if out_channels == 2 {
        return input_stereo.to_vec();
    }
    if out_channels == 1 {
        // Downmix stereo to mono: (L + R) * 0.5.
        let frames = input_stereo.len() / 2;
        let mut out = Vec::with_capacity(frames);
        for frame_idx in 0..frames {
            let left = input_stereo[frame_idx * 2];
            let right = input_stereo[frame_idx * 2 + 1];
            out.push((left + right) * 0.5);
        }
        return out;
    }
    if out_channels == 0 {
        return Vec::new();
    }

    // Upmix stereo to multichannel: L, R, then zeroes for remaining channels.
    let frames = input_stereo.len() / 2;
    let mut out = Vec::with_capacity(frames * out_channels);
    for frame_idx in 0..frames {
        out.push(input_stereo[frame_idx * 2]);
        out.push(input_stereo[frame_idx * 2 + 1]);
        for _ in 2..out_channels {
            out.push(0.0);
        }
    }
    out
}

pub fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / 32768.0
}

pub fn f32_to_i16(sample: f32) -> i16 {
    if !sample.is_finite() {
        return 0;
    }
    (sample * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

pub fn u16_to_f32(sample: u16) -> f32 {
    (sample as f32 - 32768.0) / 32768.0
}

pub fn f32_to_u16(sample: f32) -> u16 {
    if !sample.is_finite() {
        return 32768;
    }
    ((sample.clamp(-1.0, 1.0) * 32768.0) + 32768.0)
        .round()
        .clamp(0.0, 65535.0) as u16
}

/// Accumulator that collects streamed PCM audio samples and yields exact fixed-size frames.
#[derive(Debug, Default)]
pub struct AudioFrameAccumulator {
    buffer: VecDeque<f32>,
    frame_size: usize,
}

impl AudioFrameAccumulator {
    pub fn new(frame_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(frame_size.saturating_mul(4)),
            // A zero-sized frame would make `pop_frame` produce empty frames forever.
            frame_size: frame_size.max(1),
        }
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        self.buffer.extend(samples.iter().copied());
    }

    /// Pops a complete frame if at least `frame_size` samples are available.
    pub fn pop_frame(&mut self) -> Option<Vec<f32>> {
        if self.buffer.len() < self.frame_size {
            return None;
        }

        let mut frame = Vec::with_capacity(self.frame_size);
        for _ in 0..self.frame_size {
            if let Some(s) = self.buffer.pop_front() {
                frame.push(s);
            }
        }
        Some(frame)
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_resampler_configurations_do_not_hang_or_panic() {
        for (in_rate, out_rate, channels) in [(0, 48_000, 2), (44_100, 0, 2), (44_100, 48_000, 0)] {
            let mut resampler = LinearResampler::new(in_rate, out_rate, channels);
            assert!(!resampler.is_valid());
            assert!(resampler.resample(&[0.5; 128]).is_empty());
            assert!(resampler.flush().is_empty());
        }
        assert!(resample_linear(&[0.5; 128], 0, 48_000, 2).is_empty());
        assert!(convert_to_stereo(&[0.5; 128], 0).is_empty());
    }

    #[test]
    fn flush_preserves_exact_duration_for_fractional_ratios() {
        let input_frames = 44_100 / 2;
        let input: Vec<f32> = (0..input_frames)
            .map(|i| (i as f32 / 44_100.0 * 440.0 * std::f32::consts::TAU).sin())
            .collect();

        let output = resample_linear(&input, 44_100, 48_000, 1);
        assert_eq!(
            output.len(),
            24_000,
            "0.5 s at 44.1 kHz must produce 0.5 s at 48 kHz"
        );

        let mut streaming = LinearResampler::new(44_100, 48_000, 1);
        let mut chunked = Vec::new();
        for chunk in input.chunks(317) {
            chunked.extend(streaming.resample(chunk));
        }
        chunked.extend(streaming.flush());
        assert_eq!(chunked.len(), output.len());
        let max_diff = chunked
            .iter()
            .zip(&output)
            .map(|(chunked, batch)| (chunked - batch).abs())
            .fold(0.0f32, f32::max);
        assert!(max_diff < 1e-5, "max chunk/batch difference: {max_diff}");
    }

    #[test]
    fn equal_rate_resampling_is_lossless_across_chunks() {
        let input = vec![0.25; 1000];
        let mut resampler = LinearResampler::new(48_000, 48_000, 2);
        let mut output = resampler.resample(&input[..333]);
        output.extend(resampler.resample(&input[333..]));
        assert!(resampler.flush().is_empty());
        assert_eq!(output, input);
    }

    #[test]
    fn pcm_conversion_clips_and_maps_both_endpoints_symmetrically() {
        assert_eq!(f32_to_i16(-1.0), i16::MIN);
        assert_eq!(f32_to_i16(1.0), i16::MAX);
        assert_eq!(f32_to_u16(-1.0), 0);
        assert_eq!(f32_to_u16(1.0), u16::MAX);
        assert_eq!(f32_to_u16(0.0), 32768);
        assert_eq!(f32_to_i16(f32::NAN), 0);
        assert_eq!(f32_to_u16(f32::INFINITY), 32768);

        for sample in [i16::MIN, -12345, 0, 12345, i16::MAX] {
            let roundtrip = f32_to_i16(i16_to_f32(sample));
            assert!((roundtrip - sample).abs() <= 1);
        }
        for sample in [0, 1, 32767, 32768, 65534, u16::MAX] {
            let roundtrip = f32_to_u16(u16_to_f32(sample));
            assert!((roundtrip as i32 - sample as i32).abs() <= 1);
        }
    }

    #[test]
    fn channel_conversion_preserves_interleaving() {
        assert_eq!(
            convert_to_stereo(&[0.1, 0.2, 0.3], 1),
            vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3]
        );
        assert_eq!(
            convert_to_stereo(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 3),
            vec![0.1, 0.2, 0.4, 0.5]
        );
        for (actual, expected) in convert_stereo_to_channels(&[0.2, 0.4, 0.6, 0.8], 1)
            .iter()
            .zip([0.3f32, 0.7])
        {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert_eq!(
            convert_stereo_to_channels(&[0.2, 0.4, 0.6, 0.8], 4),
            vec![0.2, 0.4, 0.0, 0.0, 0.6, 0.8, 0.0, 0.0]
        );
    }

    #[test]
    fn zero_sized_accumulator_never_yields_empty_frames() {
        let mut accumulator = AudioFrameAccumulator::new(0);
        assert!(accumulator.pop_frame().is_none());
        accumulator.push_samples(&[1.0, 2.0]);
        let frame = accumulator.pop_frame().unwrap();
        assert_eq!(frame.len(), 1);
        assert!(accumulator.pop_frame().is_some());
    }
}
