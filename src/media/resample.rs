use std::collections::VecDeque;

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
            buffer: VecDeque::with_capacity(channels * 2048),
        }
    }

    /// Resamples an incoming chunk of interleaved PCM audio.
    /// Preserves fractional phase and sample continuity from previous chunks.
    pub fn resample(&mut self, input: &[f32]) -> Vec<f32> {
        if self.in_rate == self.out_rate || self.channels == 0 {
            return input.to_vec();
        }

        self.buffer.extend(input.iter().copied());

        let available_frames = self.buffer.len() / self.channels;
        if available_frames < 2 {
            return Vec::new();
        }

        let ratio = self.in_rate as f64 / self.out_rate as f64;
        let mut output = Vec::new();

        while (self.phase.floor() as usize + 1) < (self.buffer.len() / self.channels) {
            let idx0 = self.phase.floor() as usize;
            let frac = (self.phase - idx0 as f64) as f32;
            let idx1 = idx0 + 1;

            let base0 = idx0 * self.channels;
            let base1 = idx1 * self.channels;

            for ch in 0..self.channels {
                let s0 = self.buffer[base0 + ch];
                let s1 = self.buffer[base1 + ch];
                let interpolated = s0 * (1.0 - frac) + s1 * frac;
                output.push(interpolated);
            }

            self.phase += ratio;
        }

        let consumed_frames = self.phase.floor() as usize;
        if consumed_frames > 0 {
            let drain_samples = consumed_frames * self.channels;
            self.buffer.drain(0..drain_samples);
            self.phase -= consumed_frames as f64;
        }

        output
    }

    /// Flushes any remaining samples in the resampler buffer (if available).
    pub fn flush(&mut self) -> Vec<f32> {
        let available_frames = self.buffer.len() / self.channels;
        if available_frames < 2 || self.channels == 0 {
            self.reset();
            return Vec::new();
        }

        let ratio = self.in_rate as f64 / self.out_rate as f64;
        let mut output = Vec::new();

        while (self.phase.floor() as usize + 1) < available_frames {
            let idx0 = self.phase.floor() as usize;
            let frac = (self.phase - idx0 as f64) as f32;
            let idx1 = idx0 + 1;

            let base0 = idx0 * self.channels;
            let base1 = idx1 * self.channels;

            for ch in 0..self.channels {
                let s0 = self.buffer[base0 + ch];
                let s1 = self.buffer[base1 + ch];
                output.push(s0 * (1.0 - frac) + s1 * frac);
            }

            self.phase += ratio;
        }

        self.reset();
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
