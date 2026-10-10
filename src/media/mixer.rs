use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::media::codec::{OpusAudioDecoder, STEREO_FRAME_SAMPLES};
use crate::media::AudioError;

pub struct ParticipantStream {
    pub ssrc: u32,
    pub decoder: OpusAudioDecoder,
    pub queue: VecDeque<Vec<f32>>,
    pub last_active: Instant,
    pub is_speaking: bool,
    pub audio_level: f32,
}

pub struct AudioMixer {
    participants: HashMap<u32, ParticipantStream>,
    vad_threshold: f32,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioMixer {
    pub fn new() -> Self {
        Self {
            participants: HashMap::new(),
            vad_threshold: 0.01,
        }
    }

    /// Feeds an incoming Opus packet for a remote participant identified by SSRC.
    pub fn feed_opus_packet(&mut self, ssrc: u32, opus_data: &[u8]) -> Result<(), AudioError> {
        let participant = match self.participants.get_mut(&ssrc) {
            Some(p) => p,
            None => {
                let decoder = OpusAudioDecoder::new()?;
                let stream = ParticipantStream {
                    ssrc,
                    decoder,
                    queue: VecDeque::with_capacity(16),
                    last_active: Instant::now(),
                    is_speaking: false,
                    audio_level: 0.0,
                };
                self.participants.insert(ssrc, stream);
                self.participants.get_mut(&ssrc).unwrap()
            }
        };

        let decoded = participant.decoder.decode_frame(opus_data)?;

        // Calculate RMS audio level and speech detection
        let sum_sq: f32 = decoded.iter().map(|&x| x * x).sum();
        let rms = if decoded.is_empty() {
            0.0
        } else {
            (sum_sq / decoded.len() as f32).sqrt()
        };

        participant.audio_level = rms;
        participant.is_speaking = rms > self.vad_threshold;
        participant.last_active = Instant::now();

        // Limit jitter buffer queue to prevent excessive delay/latency
        if participant.queue.len() >= 10 {
            participant.queue.pop_front();
        }
        participant.queue.push_back(decoded);

        Ok(())
    }

    /// Pulls a 20 ms mixed frame (1920 interleaved stereo f32 samples) from all active participants.
    /// If deafened, produces a silent frame.
    pub fn pull_mixed_frame(&mut self, is_deafened: bool) -> Vec<f32> {
        self.cleanup_stale_participants(Duration::from_secs(10));

        if is_deafened {
            return vec![0.0f32; STEREO_FRAME_SAMPLES];
        }

        let mut mixed = vec![0.0f32; STEREO_FRAME_SAMPLES];
        let mut active_streams = 0;

        for participant in self.participants.values_mut() {
            if let Some(frame) = participant.queue.pop_front() {
                active_streams += 1;
                let len = STEREO_FRAME_SAMPLES.min(frame.len());
                for i in 0..len {
                    mixed[i] += frame[i];
                }
            }
        }

        if active_streams > 0 {
            // Apply soft-limiting curve to prevent harsh digital clipping
            for sample in &mut mixed {
                *sample = soft_clip(*sample);
            }
        }

        mixed
    }

    pub fn is_participant_speaking(&self, ssrc: u32) -> bool {
        self.participants.get(&ssrc).map(|p| p.is_speaking).unwrap_or(false)
    }

    pub fn participant_audio_level(&self, ssrc: u32) -> f32 {
        self.participants.get(&ssrc).map(|p| p.audio_level).unwrap_or(0.0)
    }

    pub fn list_active_ssrcs(&self) -> Vec<u32> {
        self.participants.keys().copied().collect()
    }

    pub fn set_vad_threshold(&mut self, threshold: f32) {
        self.vad_threshold = threshold.max(0.0);
    }

    pub fn vad_threshold(&self) -> f32 {
        self.vad_threshold
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn cleanup_stale_participants(&mut self, timeout: Duration) {
        let now = Instant::now();
        self.participants.retain(|_, p| {
            now.checked_duration_since(p.last_active)
                .map(|dur| dur < timeout)
                .unwrap_or(true)
        });
    }

    pub fn clear(&mut self) {
        self.participants.clear();
    }
}

/// Smooth cubic polynomial saturation limiter.
pub fn soft_clip(x: f32) -> f32 {
    if x > 1.25 {
        1.0
    } else if x < -1.25 {
        -1.0
    } else {
        (x - (x * x * x) / 5.0).clamp(-1.0, 1.0)
    }
}
