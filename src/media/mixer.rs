use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::media::AudioError;
use crate::media::codec::{OpusAudioDecoder, STEREO_FRAME_SAMPLES};

const FRAME_DURATION: Duration = Duration::from_millis(20);
const MIN_JITTER_FRAMES: usize = 1;
const MAX_JITTER_FRAMES: usize = 20;
const MAX_PLC_FRAMES: usize = 10;
const FADE_FRAMES: usize = 240; // 5 ms at 48 kHz, applied per channel.

pub struct ParticipantStream {
    pub ssrc: u32,
    pub decoder: OpusAudioDecoder,
    pub queue: VecDeque<Vec<f32>>,
    pub last_active: Instant,
    pub is_speaking: bool,
    pub audio_level: f32,

    target_frames: usize,
    last_packet_at: Option<Instant>,
    jitter_estimate: Duration,
    has_received_packet: bool,
    is_playing: bool,
    plc_frames: usize,
    fade_in_frames: usize,
}

impl ParticipantStream {
    fn new(ssrc: u32, decoder: OpusAudioDecoder) -> Self {
        Self {
            ssrc,
            decoder,
            queue: VecDeque::with_capacity(MAX_JITTER_FRAMES),
            last_active: Instant::now(),
            is_speaking: false,
            audio_level: 0.0,
            target_frames: MIN_JITTER_FRAMES,
            last_packet_at: None,
            jitter_estimate: Duration::ZERO,
            has_received_packet: false,
            is_playing: false,
            plc_frames: 0,
            fade_in_frames: 0,
        }
    }

    fn has_frame_ready(&self) -> bool {
        if self.queue.is_empty() {
            self.is_playing && self.plc_frames < MAX_PLC_FRAMES
        } else {
            self.is_playing || self.queue.len() >= self.target_frames
        }
    }

    fn update_jitter_target(&mut self, now: Instant) {
        if let Some(last) = self.last_packet_at {
            let interval = now.saturating_duration_since(last);
            // Ignore bursts/backlog delivery; only regular packet cadence informs jitter.
            if interval >= Duration::from_millis(5) && interval <= Duration::from_millis(100) {
                let deviation = if interval >= FRAME_DURATION {
                    interval - FRAME_DURATION
                } else {
                    FRAME_DURATION - interval
                };
                self.jitter_estimate =
                    self.jitter_estimate.mul_f64(0.875) + deviation.mul_f64(0.125);
            }
        }
        self.last_packet_at = Some(now);

        let jitter_frames =
            (self.jitter_estimate.as_secs_f64() / FRAME_DURATION.as_secs_f64()).ceil() as usize;
        self.target_frames =
            (MIN_JITTER_FRAMES + jitter_frames).clamp(MIN_JITTER_FRAMES, MAX_JITTER_FRAMES);
    }

    fn next_frame(&mut self, vad_threshold: f32) -> Option<Vec<f32>> {
        if !self.is_playing && self.queue.len() < self.target_frames {
            return None;
        }
        if let Some(mut frame) = self.queue.pop_front() {
            self.is_playing = true;
            self.plc_frames = 0;
            apply_fade_in(&mut frame, &mut self.fade_in_frames);
            return Some(frame);
        }

        // Before the first utterance, wait for the jitter buffer to fill instead of
        // starting immediately on the first packet. Once playback has started, an
        // empty queue is real packet loss and must be concealed by Opus PLC.
        if !self.is_playing || !self.has_received_packet || self.plc_frames >= MAX_PLC_FRAMES {
            self.is_playing = false;
            self.audio_level = 0.0;
            self.is_speaking = false;
            return None;
        }

        self.plc_frames += 1;
        let mut frame = self.decoder.decode_loss().ok()?;
        sanitize_frame(&mut frame);
        let sum_sq: f32 = frame.iter().map(|sample| sample * sample).sum();
        self.audio_level = (sum_sq / frame.len() as f32).sqrt();
        self.is_speaking = self.audio_level > vad_threshold;
        if self.plc_frames == MAX_PLC_FRAMES {
            apply_fade_out(&mut frame);
            self.is_playing = false;
        }
        Some(frame)
    }

    fn reset_playback(&mut self) {
        self.queue.clear();
        self.is_playing = false;
        self.plc_frames = 0;
        self.fade_in_frames = 0;
    }
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
        let is_new = !self.participants.contains_key(&ssrc);
        let participant = match self.participants.get_mut(&ssrc) {
            Some(p) => p,
            None => {
                let decoder = OpusAudioDecoder::new()?;
                let stream = ParticipantStream::new(ssrc, decoder);
                self.participants.insert(ssrc, stream);
                self.participants.get_mut(&ssrc).unwrap()
            }
        };

        let mut decoded = match participant.decoder.decode_frame(opus_data) {
            Ok(frame) => frame,
            Err(err) => {
                if is_new {
                    self.participants.remove(&ssrc);
                }
                return Err(err);
            }
        };
        sanitize_frame(&mut decoded);

        let now = Instant::now();
        participant.update_jitter_target(now);
        let restarting =
            !participant.has_received_packet || participant.plc_frames >= MAX_PLC_FRAMES;
        participant.has_received_packet = true;
        if restarting {
            participant.fade_in_frames = FADE_FRAMES;
        }

        let sum_sq: f32 = decoded.iter().map(|&x| x * x).sum();
        participant.audio_level = (sum_sq / decoded.len() as f32).sqrt();
        participant.is_speaking = participant.audio_level > self.vad_threshold;
        participant.last_active = now;

        // A full queue means the receiver is behind. Drop the oldest frame so the
        // mixer catches up to live audio instead of accumulating unbounded latency.
        if participant.queue.len() >= MAX_JITTER_FRAMES {
            participant.queue.pop_front();
        }
        participant.queue.push_back(decoded);

        Ok(())
    }

    /// Pulls a 20 ms mixed frame (1920 interleaved stereo f32 samples) from all active participants.
    /// If deafened, discards queued playback and produces a silent frame.
    pub fn pull_mixed_frame(&mut self, is_deafened: bool) -> Vec<f32> {
        self.cleanup_stale_participants(Duration::from_secs(10));

        if is_deafened {
            for participant in self.participants.values_mut() {
                participant.reset_playback();
            }
            return vec![0.0f32; STEREO_FRAME_SAMPLES];
        }

        let active_streams = self
            .participants
            .values()
            .filter(|p| p.has_frame_ready())
            .count();
        if active_streams == 0 {
            return vec![0.0f32; STEREO_FRAME_SAMPLES];
        }

        // Equal-power normalization keeps a single speaker at unity gain while
        // avoiding a linear level explosion when several SSRCs are summed.
        let gain = 1.0 / (active_streams as f32).sqrt();
        let mut mixed = vec![0.0f32; STEREO_FRAME_SAMPLES];

        for participant in self.participants.values_mut() {
            if let Some(frame) = participant.next_frame(self.vad_threshold) {
                let len = STEREO_FRAME_SAMPLES.min(frame.len());
                for i in 0..len {
                    mixed[i] += frame[i] * gain;
                }
            }
        }

        for sample in &mut mixed {
            *sample = soft_clip(*sample);
        }
        mixed
    }

    pub fn is_participant_speaking(&self, ssrc: u32) -> bool {
        self.participants
            .get(&ssrc)
            .map(|p| p.is_speaking)
            .unwrap_or(false)
    }

    pub fn participant_audio_level(&self, ssrc: u32) -> f32 {
        self.participants
            .get(&ssrc)
            .map(|p| p.audio_level)
            .unwrap_or(0.0)
    }

    pub fn list_active_ssrcs(&self) -> Vec<u32> {
        self.participants.keys().copied().collect()
    }

    pub fn set_vad_threshold(&mut self, threshold: f32) {
        self.vad_threshold = if threshold.is_finite() {
            threshold.max(0.0)
        } else {
            0.01
        };
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

fn sanitize_frame(frame: &mut [f32]) {
    for sample in frame {
        *sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
}

fn apply_fade_in(frame: &mut [f32], remaining: &mut usize) {
    if *remaining == 0 {
        return;
    }
    for pair in frame.chunks_exact_mut(2) {
        let gain = 1.0 - (*remaining as f32 / FADE_FRAMES as f32).min(1.0);
        pair[0] *= gain;
        pair[1] *= gain;
        *remaining = remaining.saturating_sub(1);
        if *remaining == 0 {
            break;
        }
    }
}

fn apply_fade_out(frame: &mut [f32]) {
    let frames = frame.len() / 2;
    for (index, pair) in frame.chunks_exact_mut(2).enumerate() {
        let gain = 1.0 - index as f32 / frames as f32;
        pair[0] *= gain;
        pair[1] *= gain;
    }
}

/// Smooth limiter: unity gain below -6 dBFS and asymptotic saturation near full scale.
pub fn soft_clip(x: f32) -> f32 {
    if !x.is_finite() {
        return 0.0;
    }
    let magnitude = x.abs();
    if magnitude <= 0.95 {
        return x;
    }
    let limited = 0.95 + 0.05 * ((magnitude - 0.95) / 0.05).tanh();
    limited.copysign(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::codec::OpusAudioEncoder;

    fn packet(amplitude: f32) -> Vec<u8> {
        let frame: Vec<f32> = (0..crate::media::codec::SAMPLES_PER_CHANNEL)
            .flat_map(|index| {
                let sample =
                    amplitude * (std::f32::consts::TAU * 440.0 * index as f32 / 48_000.0).sin();
                [sample, sample]
            })
            .collect();
        OpusAudioEncoder::new()
            .unwrap()
            .encode_frame(&frame)
            .unwrap()
    }

    fn rms(frame: &[f32]) -> f32 {
        (frame.iter().map(|sample| sample * sample).sum::<f32>() / frame.len() as f32).sqrt()
    }

    #[test]
    fn jitter_buffer_starts_on_first_packet_and_keeps_following_frames() {
        let mut mixer = AudioMixer::new();
        mixer.feed_opus_packet(1, &packet(0.4)).unwrap();
        assert!(rms(&mixer.pull_mixed_frame(false)) > 0.18);

        mixer.feed_opus_packet(1, &packet(0.4)).unwrap();
        assert!(rms(&mixer.pull_mixed_frame(false)) > 0.18);
    }

    #[test]
    fn packet_loss_uses_opus_plc_then_fades_to_silence() {
        let mut mixer = AudioMixer::new();
        let packet = packet(0.5);
        mixer.feed_opus_packet(7, &packet).unwrap();
        mixer.feed_opus_packet(7, &packet).unwrap();
        assert!(rms(&mixer.pull_mixed_frame(false)) > 0.18);
        // Drain the second prebuffered packet before simulating actual packet loss.
        assert!(rms(&mixer.pull_mixed_frame(false)) > 0.18);

        for _ in 0..MAX_PLC_FRAMES {
            assert!(rms(&mixer.pull_mixed_frame(false)) > 0.01);
        }
        assert!(
            mixer
                .pull_mixed_frame(false)
                .iter()
                .all(|sample| *sample == 0.0)
        );
    }

    #[test]
    fn jitter_target_grows_with_measured_arrival_variation() {
        let mut mixer = AudioMixer::new();
        mixer.feed_opus_packet(51, &packet(0.3)).unwrap();
        let now = Instant::now();
        let participant = mixer.participants.get_mut(&51).unwrap();
        participant.last_packet_at = Some(now - Duration::from_millis(80));
        participant.jitter_estimate = Duration::from_millis(80);
        participant.update_jitter_target(now);
        assert_eq!(participant.target_frames, 5);
    }

    #[test]
    fn buffering_participant_is_not_counted_as_an_active_speaker() {
        let mut mixer = AudioMixer::new();
        mixer.feed_opus_packet(52, &packet(0.3)).unwrap();
        let participant = mixer.participants.get_mut(&52).unwrap();
        participant.target_frames = 5;
        assert!(!participant.has_frame_ready());

        participant.is_playing = true;
        assert!(participant.has_frame_ready());
    }

    #[test]
    fn jitter_queue_is_bounded_and_drops_oldest_frames() {
        let mut mixer = AudioMixer::new();
        let packet = packet(0.2);
        for _ in 0..(MAX_JITTER_FRAMES + 5) {
            mixer.feed_opus_packet(9, &packet).unwrap();
        }
        assert_eq!(mixer.participants[&9].queue.len(), MAX_JITTER_FRAMES);
    }

    #[test]
    fn deafening_discards_old_playback_instead_of_resuming_it_later() {
        let mut mixer = AudioMixer::new();
        let packet = packet(0.4);
        mixer.feed_opus_packet(11, &packet).unwrap();
        mixer.feed_opus_packet(11, &packet).unwrap();
        assert!(
            mixer
                .pull_mixed_frame(true)
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            mixer
                .pull_mixed_frame(false)
                .iter()
                .all(|sample| *sample == 0.0)
        );

        mixer.feed_opus_packet(11, &packet).unwrap();
        mixer.feed_opus_packet(11, &packet).unwrap();
        assert!(rms(&mixer.pull_mixed_frame(false)) > 0.18);
    }

    #[test]
    fn single_speaker_keeps_unity_gain_and_multi_speaker_mix_does_not_clip() {
        let mut mixer = AudioMixer::new();
        let packet = packet(0.3);
        let mut decoder = crate::media::codec::OpusAudioDecoder::new().unwrap();
        let reference_rms = rms(&decoder.decode_frame(&packet).unwrap());
        mixer.feed_opus_packet(21, &packet).unwrap();
        mixer.feed_opus_packet(21, &packet).unwrap();
        let single = mixer.pull_mixed_frame(false);
        assert!(
            rms(&single) > reference_rms * 0.9,
            "single speaker must not be attenuated: {} vs {reference_rms}",
            rms(&single)
        );

        for ssrc in 22..30 {
            mixer.feed_opus_packet(ssrc, &packet).unwrap();
            mixer.feed_opus_packet(ssrc, &packet).unwrap();
        }
        let mixed = mixer.pull_mixed_frame(false);
        assert!(
            mixed
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0)
        );
        assert!(rms(&mixed) > 0.18);
    }

    #[test]
    fn utterance_start_is_faded_in_to_avoid_a_click() {
        let mut mixer = AudioMixer::new();
        let packet = packet(0.5);
        mixer.feed_opus_packet(31, &packet).unwrap();
        mixer.feed_opus_packet(31, &packet).unwrap();
        let frame = mixer.pull_mixed_frame(false);
        assert_eq!(frame[0], 0.0);
        assert_eq!(frame[1], 0.0);
        assert!(rms(&frame) > 0.18);
    }

    #[test]
    fn invalid_packet_does_not_create_a_ghost_participant() {
        let mut mixer = AudioMixer::new();
        assert!(mixer.feed_opus_packet(41, &[0xFF]).is_err());
        assert_eq!(mixer.participant_count(), 0);
    }

    #[test]
    fn limiter_is_linear_for_normal_levels_and_bounded_for_overload() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.5), -0.5);
        assert!(soft_clip(10.0) <= 1.0);
        assert!(soft_clip(-10.0) >= -1.0);
        assert_eq!(soft_clip(f32::NAN), 0.0);
    }
}
