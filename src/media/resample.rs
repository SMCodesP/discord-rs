use std::collections::VecDeque;

/// Resamples interleaved PCM audio from `in_rate` to `out_rate` using linear interpolation.
pub fn resample_linear(input: &[f32], in_rate: u32, out_rate: u32, channels: usize) -> Vec<f32> {
    if in_rate == out_rate || input.is_empty() || channels == 0 {
        return input.to_vec();
    }

    let in_frames = input.len() / channels;
    if in_frames == 0 {
        return Vec::new();
    }

    let out_frames = ((in_frames as u64 * out_rate as u64 + (in_rate as u64 / 2)) / in_rate as u64) as usize;
    if out_frames == 0 {
        return Vec::new();
    }

    let ratio = in_rate as f64 / out_rate as f64;
    let mut output = Vec::with_capacity(out_frames * channels);

    for frame_idx in 0..out_frames {
        let src_pos = frame_idx as f64 * ratio;
        let idx0 = src_pos.floor() as usize;
        let frac = (src_pos - idx0 as f64) as f32;
        let idx1 = (idx0 + 1).min(in_frames.saturating_sub(1));

        for ch in 0..channels {
            let s0 = input[idx0 * channels + ch];
            let s1 = input[idx1 * channels + ch];
            let interpolated = s0 * (1.0 - frac) + s1 * frac;
            output.push(interpolated);
        }
    }

    output
}

/// Converts arbitrary channel count audio to stereo (2 channels).
pub fn convert_to_stereo(input: &[f32], in_channels: usize) -> Vec<f32> {
    if in_channels == 2 {
        return input.to_vec();
    }
    if in_channels == 1 {
        // Mono to stereo: duplicate to both L and R
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

    // Multichannel downmix: take front-left (ch 0) and front-right (ch 1)
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
        // Downmix stereo to mono: (L + R) * 0.5
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

    // Upmix stereo to multichannel: L, R, then zeroes for remaining channels
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
    (sample * 32767.0).clamp(-32768.0, 32767.0) as i16
}

pub fn u16_to_f32(sample: u16) -> f32 {
    (sample as f32 - 32768.0) / 32768.0
}

pub fn f32_to_u16(sample: f32) -> u16 {
    ((sample.clamp(-1.0, 1.0) * 32767.0) + 32768.0) as u16
}

/// Accumulator that collects streamed PCM audio samples, converts them,
/// and yields exact 20 ms fixed-size frames.
#[derive(Debug, Default)]
pub struct AudioFrameAccumulator {
    buffer: VecDeque<f32>,
    frame_size: usize,
}

impl AudioFrameAccumulator {
    pub fn new(frame_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(frame_size * 4),
            frame_size,
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
