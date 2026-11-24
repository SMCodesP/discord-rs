use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::media::AudioError;
use crate::media::codec::{
    OpusAudioDecoder, SAMPLES_PER_CHANNEL, STEREO_FRAME_SAMPLES, validate_opus_packet,
};

const FRAME_DURATION: Duration = Duration::from_millis(20);
// One complete Opus frame is the minimum reorder window. The repository documents
// conflicting E2E goals (<15 ms and <25 ms); this mixer budget deliberately does
// not claim those goals are met. Network, hardware and capture latency are separate.
const MIXER_LATENCY_BUDGET: Duration = FRAME_DURATION;
const MAX_JITTER_FRAMES: usize = 20;
const MAX_RECENT_PACKETS: usize = MAX_JITTER_FRAMES * 2;
const MAX_PLC_FRAMES: usize = 10;
const FADE_FRAMES: usize = 240; // 5 ms at 48 kHz, applied per channel.

type AudioClock = Arc<dyn Fn() -> Instant + Send + Sync>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParticipantStats {
    pub received: u64,
    pub played: u64,
    pub duplicates: u64,
    pub late: u64,
    pub reordered: u64,
    pub conflicts: u64,
    pub lost: u64,
    pub plc: u64,
    pub dropped: u64,
    pub decode_errors: u64,
    pub jitter: Duration,
    pub target_delay: Duration,
    pub queue_len: usize,
    pub oldest_queue_age: Duration,
    pub mixer_latency: Duration,
    pub last_played_sequence: Option<u16>,
    pub last_played_timestamp: Option<u32>,
}

#[derive(Debug, Clone)]
struct JitterPacket {
    timestamp: i64,
    opus_data: Vec<u8>,
    arrived_at: Instant,
}

pub struct ParticipantStream {
    pub ssrc: u32,
    pub decoder: OpusAudioDecoder,
    packets: BTreeMap<i64, JitterPacket>,
    recent_packets: BTreeMap<i64, i64>,
    pub last_active: Instant,
    pub is_speaking: bool,
    pub audio_level: f32,

    highest_sequence: Option<i64>,
    highest_timestamp: Option<i64>,
    next_sequence: Option<i64>,
    next_timestamp: Option<i64>,
    last_played_sequence: Option<i64>,
    last_played_timestamp: Option<i64>,
    last_in_order_arrival: Option<Instant>,
    playout_deadline: Option<Instant>,
    playout_head_sequence: Option<i64>,
    jitter_estimate: Duration,
    target_delay: Duration,
    has_received_packet: bool,
    is_playing: bool,
    plc_frames: usize,
    fade_in_frames: usize,
    resync_after_loss: bool,
    consecutive_decode_errors: usize,
    failed: bool,
    stats: ParticipantStats,
}

impl ParticipantStream {
    fn new(ssrc: u32, decoder: OpusAudioDecoder, now: Instant) -> Self {
        Self {
            ssrc,
            decoder,
            packets: BTreeMap::new(),
            recent_packets: BTreeMap::new(),
            last_active: now,
            is_speaking: false,
            audio_level: 0.0,
            highest_sequence: None,
            highest_timestamp: None,
            next_sequence: None,
            next_timestamp: None,
            last_played_sequence: None,
            last_played_timestamp: None,
            last_in_order_arrival: None,
            playout_deadline: None,
            playout_head_sequence: None,
            jitter_estimate: Duration::ZERO,
            target_delay: FRAME_DURATION,
            has_received_packet: false,
            is_playing: false,
            plc_frames: 0,
            fade_in_frames: 0,
            resync_after_loss: false,
            consecutive_decode_errors: 0,
            failed: false,
            stats: ParticipantStats::default(),
        }
    }

    fn update_jitter_target(&mut self, now: Instant, sequence_delta: i64, timestamp_delta: i64) {
        if sequence_delta == 1 {
            if let Some(last) = self.last_in_order_arrival {
                let arrival_delta = now.saturating_duration_since(last);
                let rtp_delta = Duration::from_secs_f64(timestamp_delta as f64 / 48_000.0);
                if arrival_delta <= Duration::from_secs(1) {
                    let deviation = if arrival_delta >= rtp_delta {
                        arrival_delta - rtp_delta
                    } else {
                        rtp_delta - arrival_delta
                    };
                    self.jitter_estimate =
                        self.jitter_estimate.mul_f64(0.875) + deviation.mul_f64(0.125);
                }
            }
            self.last_in_order_arrival = Some(now);
        }

        self.target_delay = (FRAME_DURATION + self.jitter_estimate).min(MIXER_LATENCY_BUDGET);
    }

    fn prepare_playout_head(&mut self, now: Instant) {
        if self.is_playing {
            return;
        }

        let Some((&head_sequence, head)) = self.packets.first_key_value() else {
            return;
        };

        if self.resync_after_loss {
            if let Some(next) = self.next_sequence {
                if head_sequence > next {
                    self.stats.lost += (head_sequence - next) as u64;
                }
            }
            self.next_sequence = Some(head_sequence);
            self.next_timestamp = Some(head.timestamp);
            self.resync_after_loss = false;
        } else if self.next_sequence.is_none()
            || head_sequence < self.next_sequence.expect("sequence initialized above")
        {
            self.next_sequence = Some(head_sequence);
            self.next_timestamp = Some(head.timestamp);
        }

        // Anchor the prebuffer deadline to the current head. Later packets must
        // not keep postponing the first slot under a continuous arrival stream.
        // An earlier reordered head gets its own full reorder window.
        if self.playout_deadline.is_none() || self.playout_head_sequence != Some(head_sequence) {
            self.playout_deadline = Some(now + self.target_delay);
        }
        self.playout_head_sequence = Some(head_sequence);
    }

    fn record_recent_packet(&mut self, sequence: i64, timestamp: i64) {
        self.recent_packets.insert(sequence, timestamp);
        while self.recent_packets.len() > MAX_RECENT_PACKETS {
            self.recent_packets.pop_first();
        }
    }

    fn advance_playback(&mut self) {
        if let (Some(sequence), Some(timestamp)) = (self.next_sequence, self.next_timestamp) {
            self.last_played_sequence = Some(sequence);
            self.last_played_timestamp = Some(timestamp);
            self.next_sequence = Some(sequence + 1);
            self.next_timestamp = Some(timestamp + i64::from(SAMPLES_PER_CHANNEL as u32));
        }
    }

    fn update_audio_level(&mut self, frame: &[f32], vad_threshold: f32) {
        let sum_sq: f32 = frame.iter().map(|sample| sample * sample).sum();
        self.audio_level = if frame.is_empty() {
            0.0
        } else {
            (sum_sq / frame.len() as f32).sqrt()
        };
        self.is_speaking = self.audio_level > vad_threshold;
    }

    fn next_frame(&mut self, now: Instant, vad_threshold: f32) -> Option<Vec<f32>> {
        if self.failed || !self.has_received_packet {
            return None;
        }

        loop {
            let head = self
                .packets
                .first_key_value()
                .map(|(&sequence, packet)| (sequence, packet.timestamp, packet.arrived_at));

            if !self.is_playing {
                let (_, _, arrived_at) = head?;
                self.prepare_playout_head(now);
                let deadline = self
                    .playout_deadline
                    .unwrap_or(arrived_at + self.target_delay);
                if now < deadline {
                    return None;
                }
            }

            let Some((head_sequence, _, _)) = head else {
                if !self.is_playing {
                    self.audio_level = 0.0;
                    self.is_speaking = false;
                    return None;
                }
                return self.conceal_frame(now, vad_threshold, true);
            };

            if head_sequence < self.next_sequence.expect("playback clock initialized") {
                self.packets.pop_first();
                self.stats.late += 1;
                continue;
            }

            if head_sequence == self.next_sequence.expect("playback clock initialized") {
                let packet = self.packets.pop_first().expect("head packet exists").1;
                match self.decoder.decode_frame(&packet.opus_data) {
                    Ok(mut frame) => {
                        sanitize_frame(&mut frame);
                        self.update_audio_level(&frame, vad_threshold);
                        self.stats.played += 1;
                        self.advance_playback();
                        self.is_playing = true;
                        self.plc_frames = 0;
                        self.resync_after_loss = false;
                        self.consecutive_decode_errors = 0;
                        self.playout_deadline = Some(now + FRAME_DURATION);
                        apply_fade_in(&mut frame, &mut self.fade_in_frames);
                        return Some(frame);
                    }
                    Err(_) => {
                        self.stats.decode_errors += 1;
                        self.stats.lost += 1;
                        self.consecutive_decode_errors += 1;
                        if self.consecutive_decode_errors >= MAX_PLC_FRAMES {
                            self.failed = true;
                        }
                        return self.conceal_frame(now, vad_threshold, false);
                    }
                }
            }

            if now < self.playout_deadline.unwrap_or(now) {
                return None;
            }
            return self.conceal_frame(now, vad_threshold, true);
        }
    }

    fn conceal_frame(
        &mut self,
        now: Instant,
        vad_threshold: f32,
        count_loss: bool,
    ) -> Option<Vec<f32>> {
        if self.plc_frames >= MAX_PLC_FRAMES {
            self.is_playing = false;
            self.resync_after_loss = true;
            self.prepare_playout_head(now);
            return None;
        }

        self.plc_frames += 1;
        self.stats.plc += 1;
        if count_loss {
            self.stats.lost += 1;
        }

        let mut frame = match self.decoder.decode_loss() {
            Ok(frame) => frame,
            Err(_) => {
                self.stats.decode_errors += 1;
                self.consecutive_decode_errors += 1;
                if self.consecutive_decode_errors >= MAX_PLC_FRAMES {
                    self.failed = true;
                }
                vec![0.0; STEREO_FRAME_SAMPLES]
            }
        };
        sanitize_frame(&mut frame);
        self.update_audio_level(&frame, vad_threshold);
        self.advance_playback();
        self.playout_deadline = Some(now + FRAME_DURATION);

        if self.plc_frames == MAX_PLC_FRAMES {
            apply_fade_out(&mut frame);
            self.is_playing = false;
            self.resync_after_loss = true;
            self.prepare_playout_head(now);
        }

        Some(frame)
    }

    fn reset_playback(&mut self) {
        self.packets.clear();
        self.is_playing = false;
        self.plc_frames = 0;
        self.fade_in_frames = 0;
        self.resync_after_loss = false;
        self.playout_deadline = None;
        self.playout_head_sequence = None;
        self.last_in_order_arrival = None;
        self.consecutive_decode_errors = 0;
        self.audio_level = 0.0;
        self.is_speaking = false;
        if let (Some(sequence), Some(timestamp)) = (self.highest_sequence, self.highest_timestamp) {
            self.next_sequence = Some(sequence + 1);
            self.next_timestamp = Some(timestamp + i64::from(SAMPLES_PER_CHANNEL as u32));
        }
    }

    fn snapshot(&self, now: Instant) -> ParticipantStats {
        let mut stats = self.stats.clone();
        stats.queue_len = self.packets.len();
        stats.jitter = self.jitter_estimate;
        stats.target_delay = self.target_delay;
        if let Some((_, packet)) = self.packets.first_key_value() {
            stats.oldest_queue_age = now.saturating_duration_since(packet.arrived_at);
        }
        stats.mixer_latency = stats.target_delay.saturating_add(stats.oldest_queue_age);
        stats.last_played_sequence = self.last_played_sequence.map(|sequence| sequence as u16);
        stats.last_played_timestamp = self.last_played_timestamp.map(|timestamp| timestamp as u32);
        stats
    }
}

pub struct AudioMixer {
    participants: HashMap<u32, ParticipantStream>,
    vad_threshold: f32,
    clock: AudioClock,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioMixer {
    pub fn new() -> Self {
        Self::with_clock(Instant::now)
    }

    /// Test seam: all deadlines and arrival times come from this injected clock.
    pub fn with_clock(clock: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Self {
            participants: HashMap::new(),
            vad_threshold: 0.01,
            clock: Arc::new(clock),
        }
    }

    fn now(&self) -> Instant {
        (self.clock)()
    }

    /// Queues an RTP-identified Opus packet. Decoding is intentionally deferred
    /// until playout so reordered packets cannot advance the decoder state.
    pub fn feed_opus_packet(
        &mut self,
        ssrc: u32,
        sequence: u16,
        timestamp: u32,
        opus_data: &[u8],
    ) -> Result<(), AudioError> {
        validate_opus_packet(opus_data)?;
        let now = self.now();
        let participant = match self.participants.get_mut(&ssrc) {
            Some(participant) => participant,
            None => {
                let decoder = OpusAudioDecoder::new()?;
                self.participants
                    .insert(ssrc, ParticipantStream::new(ssrc, decoder, now));
                self.participants
                    .get_mut(&ssrc)
                    .expect("participant inserted")
            }
        };

        participant.stats.received += 1;
        participant.last_active = now;

        let (extended_sequence, extended_timestamp) =
            if let (Some(previous_sequence), Some(previous_timestamp)) =
                (participant.highest_sequence, participant.highest_timestamp)
            {
                (
                    extend_u16(previous_sequence, sequence),
                    extend_u32(previous_timestamp, timestamp),
                )
            } else {
                participant.highest_sequence = Some(i64::from(sequence));
                participant.highest_timestamp = Some(i64::from(timestamp));
                participant.next_sequence = Some(i64::from(sequence));
                participant.next_timestamp = Some(i64::from(timestamp));
                participant.has_received_packet = true;
                participant.fade_in_frames = FADE_FRAMES;
                participant.last_in_order_arrival = Some(now);
                participant.playout_deadline = Some(now + participant.target_delay);
                participant.playout_head_sequence = Some(i64::from(sequence));
                participant.record_recent_packet(i64::from(sequence), i64::from(timestamp));
                participant.packets.insert(
                    i64::from(sequence),
                    JitterPacket {
                        timestamp: i64::from(timestamp),
                        opus_data: opus_data.to_vec(),
                        arrived_at: now,
                    },
                );
                return Ok(());
            };

        if let Some(&recent_timestamp) = participant.recent_packets.get(&extended_sequence) {
            if recent_timestamp == extended_timestamp {
                participant.stats.duplicates += 1;
            } else {
                participant.stats.conflicts += 1;
            }
            return Ok(());
        }

        let previous_sequence = participant.highest_sequence.expect("clock initialized");
        let previous_timestamp = participant.highest_timestamp.expect("clock initialized");
        let sequence_delta = extended_sequence - previous_sequence;
        let timestamp_delta = extended_timestamp - previous_timestamp;
        if timestamp_delta != sequence_delta * i64::from(SAMPLES_PER_CHANNEL as u32) {
            participant.stats.conflicts += 1;
            return Ok(());
        }

        if extended_sequence < participant.next_sequence.expect("playback clock initialized") {
            participant.stats.late += 1;
            participant.record_recent_packet(extended_sequence, extended_timestamp);
            return Ok(());
        }

        if extended_sequence < previous_sequence {
            participant.stats.reordered += 1;
        }
        participant.update_jitter_target(now, sequence_delta, timestamp_delta);
        if extended_sequence > previous_sequence {
            participant.highest_sequence = Some(extended_sequence);
            participant.highest_timestamp = Some(extended_timestamp);
        }

        participant.record_recent_packet(extended_sequence, extended_timestamp);
        if participant.packets.len() >= MAX_JITTER_FRAMES {
            participant.packets.pop_first();
            participant.stats.dropped += 1;
        }
        participant.packets.insert(
            extended_sequence,
            JitterPacket {
                timestamp: extended_timestamp,
                opus_data: opus_data.to_vec(),
                arrived_at: now,
            },
        );

        if !participant.is_playing {
            participant.prepare_playout_head(now);
            participant.fade_in_frames = FADE_FRAMES;
        }

        Ok(())
    }

    /// Pulls one logical 20 ms playout slot from all contributing SSRCs.
    /// Gain is based on frames actually returned, not predicted readiness.
    pub fn pull_mixed_frame(&mut self, is_deafened: bool) -> Vec<f32> {
        let now = self.now();
        self.cleanup_stale_participants(Duration::from_secs(10));

        if is_deafened {
            for participant in self.participants.values_mut() {
                participant.reset_playback();
            }
            return vec![0.0f32; STEREO_FRAME_SAMPLES];
        }

        let mut frames = Vec::new();
        for participant in self.participants.values_mut() {
            if let Some(frame) = participant.next_frame(now, self.vad_threshold) {
                frames.push(frame);
            }
        }
        self.participants
            .retain(|_, participant| !participant.failed);

        if frames.is_empty() {
            return vec![0.0f32; STEREO_FRAME_SAMPLES];
        }

        let gain = 1.0 / (frames.len() as f32).sqrt();
        let mut mixed = vec![0.0f32; STEREO_FRAME_SAMPLES];
        for frame in frames {
            let len = STEREO_FRAME_SAMPLES.min(frame.len());
            for index in 0..len {
                mixed[index] += frame[index] * gain;
            }
        }
        for sample in &mut mixed {
            *sample = soft_clip(*sample);
        }
        mixed
    }

    pub fn participant_stats(&self, ssrc: u32) -> Option<ParticipantStats> {
        self.participants
            .get(&ssrc)
            .map(|participant| participant.snapshot(self.now()))
    }

    pub fn is_participant_speaking(&self, ssrc: u32) -> bool {
        self.participants
            .get(&ssrc)
            .map(|participant| participant.is_speaking)
            .unwrap_or(false)
    }

    pub fn participant_audio_level(&self, ssrc: u32) -> f32 {
        self.participants
            .get(&ssrc)
            .map(|participant| participant.audio_level)
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
        let now = self.now();
        self.participants.retain(|_, participant| {
            now.checked_duration_since(participant.last_active)
                .map(|duration| duration < timeout)
                .unwrap_or(true)
        });
    }

    pub fn clear(&mut self) {
        self.participants.clear();
    }
}

fn extend_u16(previous: i64, raw: u16) -> i64 {
    previous + i64::from(raw.wrapping_sub(previous as u16) as i16)
}

fn extend_u32(previous: i64, raw: u32) -> i64 {
    previous + i64::from(raw.wrapping_sub(previous as u32) as i32)
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
        let gain = 1.0 - (index + 1) as f32 / frames as f32;
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
    use std::sync::atomic::{AtomicU64, Ordering};

    struct ManualClock {
        base: Instant,
        offset_micros: AtomicU64,
    }

    impl ManualClock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                base: Instant::now(),
                offset_micros: AtomicU64::new(0),
            })
        }

        fn set(&self, offset: Duration) {
            self.offset_micros
                .store(offset.as_micros() as u64, Ordering::Release);
        }

        fn advance(&self, delta: Duration) {
            let previous = self.offset_micros.load(Ordering::Relaxed);
            self.offset_micros
                .store(previous + delta.as_micros() as u64, Ordering::Release);
        }

        fn now(&self) -> Instant {
            self.base + Duration::from_micros(self.offset_micros.load(Ordering::Acquire))
        }
    }

    fn test_mixer() -> (AudioMixer, Arc<ManualClock>) {
        let clock = ManualClock::new();
        let mixer = AudioMixer::with_clock({
            let clock = Arc::clone(&clock);
            move || clock.now()
        });
        (mixer, clock)
    }

    fn packet(frequency: f32, amplitude: f32) -> Vec<u8> {
        let frame: Vec<f32> = (0..SAMPLES_PER_CHANNEL)
            .flat_map(|index| {
                let sample =
                    amplitude * (std::f32::consts::TAU * frequency * index as f32 / 48_000.0).sin();
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

    fn dominant_tone(frame: &[f32]) -> usize {
        [300.0f32, 500.0, 700.0]
            .iter()
            .enumerate()
            .map(|(index, &frequency)| {
                let mut cosine = 0.0;
                let mut sine = 0.0;
                for (sample_index, sample) in frame.iter().step_by(2).enumerate() {
                    let phase = std::f32::consts::TAU * frequency * sample_index as f32 / 48_000.0;
                    cosine += sample * phase.cos();
                    sine += sample * phase.sin();
                }
                (index, cosine.hypot(sine))
            })
            .max_by(|left, right| left.1.total_cmp(&right.1))
            .unwrap()
            .0
    }

    fn feed(
        mixer: &mut AudioMixer,
        clock: &ManualClock,
        at: Duration,
        ssrc: u32,
        sequence: u16,
        timestamp: u32,
        packet: &[u8],
    ) {
        clock.set(at);
        mixer
            .feed_opus_packet(ssrc, sequence, timestamp, packet)
            .unwrap();
    }

    fn pull(mixer: &mut AudioMixer, clock: &ManualClock, at: Duration) -> Vec<f32> {
        clock.set(at);
        mixer.pull_mixed_frame(false)
    }

    #[test]
    fn modular_extension_handles_u16_and_u32_wraps() {
        let mut sequence = 65_534i64;
        for raw in [65_534u16, 65_535, 0, 1] {
            sequence = extend_u16(sequence, raw);
        }
        assert_eq!(sequence, 65_537);

        let mut timestamp = i64::from(0xFFFF_FFC0u32);
        for raw in [0xFFFF_FFC0u32, 0, 960, 1_920] {
            timestamp = extend_u32(timestamp, raw);
        }
        assert_eq!(timestamp, i64::from(0xFFFF_FFC0u32) + 3 * 960);
    }

    #[test]
    fn continuous_arrivals_do_not_postpone_the_first_playout_slot() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.4);
        for sequence in 1..=5u16 {
            feed(
                &mut mixer,
                &clock,
                Duration::from_millis(u64::from(sequence - 1) * 5),
                15,
                sequence,
                u32::from(sequence) * 960,
                &opus,
            );
        }

        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(20))) > 0.2);
        let stats = mixer.participant_stats(15).unwrap();
        assert_eq!(stats.played, 1);
        assert_eq!(stats.plc, 0);
    }

    #[test]
    fn reordered_packet_arriving_before_the_gap_deadline_avoids_plc() {
        let (mut mixer, clock) = test_mixer();
        let first = packet(300.0, 0.4);
        let second = packet(500.0, 0.4);
        let third = packet(700.0, 0.4);
        feed(&mut mixer, &clock, Duration::ZERO, 16, 1, 960, &first);
        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(20))) > 0.2);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(21),
            16,
            3,
            2_880,
            &third,
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(25),
            16,
            2,
            1_920,
            &second,
        );

        assert_eq!(
            dominant_tone(&pull(&mut mixer, &clock, Duration::from_millis(40))),
            1
        );
        assert_eq!(
            dominant_tone(&pull(&mut mixer, &clock, Duration::from_millis(60))),
            2
        );
        let stats = mixer.participant_stats(16).unwrap();
        assert_eq!((stats.played, stats.plc, stats.reordered), (3, 0, 1));
    }

    #[test]
    fn timestamp_divergence_on_a_new_sequence_does_not_corrupt_the_clock() {
        let (mut mixer, clock) = test_mixer();
        let first = packet(300.0, 0.4);
        let second = packet(500.0, 0.4);
        feed(&mut mixer, &clock, Duration::ZERO, 17, 1, 960, &first);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(5),
            17,
            2,
            2_880,
            &second,
        );
        assert_eq!(mixer.participant_stats(17).unwrap().conflicts, 1);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(10),
            17,
            2,
            1_920,
            &second,
        );

        assert_eq!(
            dominant_tone(&pull(&mut mixer, &clock, Duration::from_millis(20))),
            0
        );
        assert_eq!(
            dominant_tone(&pull(&mut mixer, &clock, Duration::from_millis(40))),
            1
        );
        let stats = mixer.participant_stats(17).unwrap();
        assert_eq!((stats.received, stats.played, stats.conflicts), (3, 2, 1));
    }

    #[test]
    fn packet_for_a_concealed_slot_is_late_not_a_duplicate() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.4);
        feed(&mut mixer, &clock, Duration::ZERO, 18, 1, 960, &opus);
        pull(&mut mixer, &clock, Duration::from_millis(20));
        pull(&mut mixer, &clock, Duration::from_millis(40));
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(41),
            18,
            2,
            1_920,
            &opus,
        );

        let stats = mixer.participant_stats(18).unwrap();
        assert_eq!((stats.late, stats.duplicates, stats.plc), (1, 0, 1));
    }

    #[test]
    fn overflow_dropped_packet_cannot_be_reinserted_as_a_duplicate_slot() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.3);
        for sequence in 1..=25u16 {
            feed(
                &mut mixer,
                &clock,
                Duration::ZERO,
                19,
                sequence,
                u32::from(sequence) * 960,
                &opus,
            );
        }
        feed(&mut mixer, &clock, Duration::ZERO, 19, 5, 4_800, &opus);

        let stats = mixer.participant_stats(19).unwrap();
        assert_eq!(stats.queue_len, MAX_JITTER_FRAMES);
        assert_eq!(stats.dropped, 5);
        assert_eq!(stats.duplicates, 1);
    }

    #[test]
    fn buffering_ssrc_does_not_attenuate_the_only_contributing_speaker() {
        let packet = packet(300.0, 0.4);
        let (mut mixed, clock) = test_mixer();
        feed(&mut mixed, &clock, Duration::ZERO, 20, 1, 960, &packet);
        feed(
            &mut mixed,
            &clock,
            Duration::from_millis(10),
            21,
            1,
            960,
            &packet,
        );
        let actual = pull(&mut mixed, &clock, Duration::from_millis(20));

        let (mut reference, reference_clock) = test_mixer();
        feed(
            &mut reference,
            &reference_clock,
            Duration::ZERO,
            20,
            1,
            960,
            &packet,
        );
        let expected = pull(&mut reference, &reference_clock, Duration::from_millis(20));
        assert!((rms(&actual) - rms(&expected)).abs() < 1e-6);
        assert_eq!(mixed.participant_stats(21).unwrap().played, 0);
    }

    #[test]
    fn reordered_packets_across_both_wraps_play_in_rtp_order() {
        let (mut mixer, clock) = test_mixer();
        let base_timestamp = 0xFFFF_FF00u32;
        let packets = [packet(300.0, 0.4), packet(500.0, 0.4), packet(700.0, 0.4)];
        feed(
            &mut mixer,
            &clock,
            Duration::ZERO,
            1,
            65_535,
            base_timestamp.wrapping_add(960),
            &packets[0],
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(5),
            1,
            1,
            base_timestamp.wrapping_add(2_880),
            &packets[2],
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(10),
            1,
            0,
            base_timestamp.wrapping_add(1_920),
            &packets[1],
        );

        assert!(
            pull(&mut mixer, &clock, Duration::from_millis(19))
                .iter()
                .all(|sample| *sample == 0.0)
        );
        let first = pull(&mut mixer, &clock, Duration::from_millis(20));
        let second = pull(&mut mixer, &clock, Duration::from_millis(40));
        let third = pull(&mut mixer, &clock, Duration::from_millis(60));
        assert_eq!(
            [
                dominant_tone(&first),
                dominant_tone(&second),
                dominant_tone(&third)
            ],
            [0, 1, 2],
            "distinguishable signals played out of order: {:?}",
            mixer.participant_stats(1).unwrap()
        );

        let stats = mixer.participant_stats(1).unwrap();
        assert_eq!(stats.played, 3);
        assert_eq!(stats.plc, 0);
        assert_eq!(stats.reordered, 1);
        assert_eq!(stats.last_played_sequence, Some(1));
    }

    #[test]
    fn sequence_2_1_3_is_reordered_without_plc() {
        let (mut mixer, clock) = test_mixer();
        let packets = [packet(300.0, 0.4), packet(500.0, 0.4), packet(700.0, 0.4)];
        feed(&mut mixer, &clock, Duration::ZERO, 2, 2, 1_920, &packets[1]);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(5),
            2,
            1,
            960,
            &packets[0],
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(10),
            2,
            3,
            2_880,
            &packets[2],
        );

        pull(&mut mixer, &clock, Duration::from_millis(19));
        let first = pull(&mut mixer, &clock, Duration::from_millis(25));
        let second = pull(&mut mixer, &clock, Duration::from_millis(45));
        let third = pull(&mut mixer, &clock, Duration::from_millis(65));
        let stats = mixer.participant_stats(2).unwrap();
        assert_eq!(
            [
                dominant_tone(&first),
                dominant_tone(&second),
                dominant_tone(&third)
            ],
            [0, 1, 2],
            "distinguishable signals played out of order: {stats:?}"
        );
        assert_eq!((stats.played, stats.plc, stats.reordered), (3, 0, 1));
    }

    #[test]
    fn duplicates_and_timestamp_conflicts_do_not_create_extra_slots() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.4);
        for _ in 0..3 {
            feed(&mut mixer, &clock, Duration::ZERO, 3, 1, 960, &opus);
        }
        feed(&mut mixer, &clock, Duration::ZERO, 3, 1, 1_920, &opus);
        pull(&mut mixer, &clock, Duration::from_millis(20));
        let stats = mixer.participant_stats(3).unwrap();
        assert_eq!(
            (
                stats.received,
                stats.played,
                stats.duplicates,
                stats.conflicts
            ),
            (4, 1, 2, 1)
        );
    }

    #[test]
    fn isolated_loss_produces_normal_plc_normal() {
        let (mut mixer, clock) = test_mixer();
        let first = packet(300.0, 0.5);
        let third = packet(700.0, 0.5);
        feed(&mut mixer, &clock, Duration::ZERO, 4, 1, 960, &first);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(20),
            4,
            3,
            2_880,
            &third,
        );

        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(20))) > 0.2);
        let concealed = pull(&mut mixer, &clock, Duration::from_millis(40));
        assert_eq!(concealed.len(), STEREO_FRAME_SAMPLES);
        assert!(concealed.iter().all(|sample| sample.is_finite()));
        assert!(rms(&concealed) > 0.01);
        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(60))) > 0.2);
        let stats = mixer.participant_stats(4).unwrap();
        assert_eq!((stats.played, stats.lost, stats.plc), (2, 1, 1));
    }

    #[test]
    fn burst_loss_is_limited_to_ten_plc_slots_then_resumes_with_fade_in() {
        let (mut mixer, clock) = test_mixer();
        let first = packet(440.0, 0.5);
        feed(&mut mixer, &clock, Duration::ZERO, 5, 1, 960, &first);
        pull(&mut mixer, &clock, Duration::from_millis(20));
        let mut previous_plc: Option<Vec<f32>> = None;
        for slot in 1..=MAX_PLC_FRAMES {
            let frame = pull(
                &mut mixer,
                &clock,
                Duration::from_millis((20 * (slot + 1)) as u64),
            );
            assert_eq!(frame.len(), STEREO_FRAME_SAMPLES);
            assert!(frame.iter().all(|sample| sample.is_finite()));
            assert!(rms(&frame) > 0.0);
            if let Some(previous) = &previous_plc {
                assert_ne!(previous, &frame, "each PLC slot must advance decoder state");
            }
            previous_plc = Some(frame);
        }
        let last_plc = previous_plc.unwrap();
        assert_eq!(&last_plc[last_plc.len() - 2..], &[0.0, 0.0]);
        assert!(
            pull(&mut mixer, &clock, Duration::from_millis(220))
                .iter()
                .all(|sample| *sample == 0.0)
        );

        let later = packet(660.0, 0.5);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(300),
            5,
            20,
            19_200,
            &later,
        );
        assert!(
            pull(&mut mixer, &clock, Duration::from_millis(319))
                .iter()
                .all(|sample| *sample == 0.0)
        );
        let resumed = pull(&mut mixer, &clock, Duration::from_millis(320));
        assert_eq!(resumed[0], 0.0);
        assert!(rms(&resumed) > 0.01);
        let stats = mixer.participant_stats(5).unwrap();
        assert_eq!(stats.plc, MAX_PLC_FRAMES as u64);
        assert_eq!(stats.lost, 18);
    }

    #[test]
    fn late_packets_are_dropped_after_playout() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.4);
        feed(&mut mixer, &clock, Duration::ZERO, 6, 1, 960, &opus);
        feed(&mut mixer, &clock, Duration::ZERO, 6, 2, 1_920, &opus);
        pull(&mut mixer, &clock, Duration::from_millis(20));
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(21),
            6,
            0,
            0,
            &opus,
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(22),
            6,
            1,
            960,
            &opus,
        );
        let stats = mixer.participant_stats(6).unwrap();
        assert_eq!((stats.late, stats.duplicates, stats.played), (1, 1, 1));
    }

    #[test]
    fn ssrc_clocks_and_decoders_are_independent() {
        let (mut mixer, clock) = test_mixer();
        let a = packet(300.0, 0.4);
        let b = packet(700.0, 0.4);
        feed(&mut mixer, &clock, Duration::ZERO, 10, 1, 960, &a);
        feed(&mut mixer, &clock, Duration::ZERO, 10, 3, 2_880, &a);
        feed(&mut mixer, &clock, Duration::ZERO, 11, 1, 960, &b);
        feed(&mut mixer, &clock, Duration::ZERO, 11, 2, 1_920, &b);
        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(20))) > 0.2);
        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(40))) > 0.2);
        assert_eq!(mixer.participant_stats(10).unwrap().plc, 1);
        assert_eq!(mixer.participant_stats(11).unwrap().plc, 0);
    }

    #[test]
    fn mixer_latency_reports_target_plus_queue_age_separately_from_e2e() {
        let (mut mixer, clock) = test_mixer();
        feed(
            &mut mixer,
            &clock,
            Duration::ZERO,
            14,
            1,
            960,
            &packet(440.0, 0.2),
        );
        clock.set(Duration::from_millis(5));
        let stats = mixer.participant_stats(14).unwrap();
        assert_eq!(stats.target_delay, FRAME_DURATION);
        assert_eq!(stats.oldest_queue_age, Duration::from_millis(5));
        assert_eq!(stats.mixer_latency, Duration::from_millis(25));
    }

    #[test]
    fn rtp_jitter_is_estimated_from_arrival_vs_timestamp_cadence() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.3);
        feed(&mut mixer, &clock, Duration::ZERO, 13, 1, 960, &opus);
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(25),
            13,
            2,
            1_920,
            &opus,
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(45),
            13,
            3,
            2_880,
            &opus,
        );
        let stats = mixer.participant_stats(13).unwrap();
        assert!(stats.jitter > Duration::ZERO);
        assert!(stats.target_delay <= MIXER_LATENCY_BUDGET);
    }

    #[test]
    fn queue_is_bounded_and_overflow_is_counted() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.3);
        for sequence in 1..=25u16 {
            feed(
                &mut mixer,
                &clock,
                Duration::ZERO,
                12,
                sequence,
                u32::from(sequence) * 960,
                &opus,
            );
        }
        let stats = mixer.participant_stats(12).unwrap();
        assert!(stats.queue_len <= MAX_JITTER_FRAMES);
        assert_eq!(stats.dropped, 5);
        assert!(stats.target_delay <= MIXER_LATENCY_BUDGET);
    }

    #[test]
    fn deterministic_multi_ssrc_load_preserves_bounds_and_counters() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.2);
        let mut random = 0x1234_5678u64;
        let mut last_played: HashMap<u32, u16> = HashMap::new();
        for slot in 0..200u32 {
            for ssrc in 20..24u32 {
                random = random
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let sequence = slot as u16;
                let timestamp = slot * 960;
                match random % 10 {
                    0 => {}
                    1 => {
                        let _ = mixer.feed_opus_packet(ssrc, sequence, timestamp, &opus);
                        let _ = mixer.feed_opus_packet(ssrc, sequence, timestamp, &opus);
                    }
                    2 => {
                        let _ = mixer.feed_opus_packet(
                            ssrc,
                            sequence.wrapping_add(1),
                            timestamp + 960,
                            &opus,
                        );
                        let _ = mixer.feed_opus_packet(ssrc, sequence, timestamp, &opus);
                    }
                    _ => {
                        let _ = mixer.feed_opus_packet(ssrc, sequence, timestamp, &opus);
                    }
                }
            }
            clock.advance(FRAME_DURATION);
            mixer.pull_mixed_frame(false);
            for ssrc in 20..24 {
                if let Some(stats) = mixer.participant_stats(ssrc) {
                    assert!(stats.queue_len <= MAX_JITTER_FRAMES);
                    assert!(stats.plc <= stats.lost);
                    assert!(stats.dropped <= stats.received);
                    assert!(stats.target_delay <= MIXER_LATENCY_BUDGET);
                    if let Some(sequence) = stats.last_played_sequence {
                        if let Some(previous) = last_played.insert(ssrc, sequence) {
                            assert!(
                                sequence > previous,
                                "playout moved backwards for SSRC {ssrc}: {previous} -> {sequence}"
                            );
                        }
                    }
                    assert_eq!(
                        stats.received,
                        stats.played
                            + stats.duplicates
                            + stats.late
                            + stats.conflicts
                            + stats.dropped
                            + stats.decode_errors
                            + stats.queue_len as u64,
                        "packet accounting must balance for SSRC {ssrc}: {stats:?}"
                    );
                }
            }
        }
        for ssrc in 20..24 {
            let stats = mixer.participant_stats(ssrc).unwrap();
            assert!(stats.played > 0);
        }
    }

    #[test]
    fn invalid_first_packet_does_not_create_a_ghost_ssrc() {
        let (mut mixer, _clock) = test_mixer();
        assert!(mixer.feed_opus_packet(30, 1, 960, &[0xFF]).is_err());
        assert_eq!(mixer.participant_count(), 0);
    }

    #[test]
    fn deafen_discards_packets_and_resumes_after_the_highest_received_sequence() {
        let (mut mixer, clock) = test_mixer();
        let opus = packet(440.0, 0.4);
        feed(&mut mixer, &clock, Duration::ZERO, 31, 1, 960, &opus);
        feed(&mut mixer, &clock, Duration::ZERO, 31, 2, 1_920, &opus);
        assert!(
            mixer
                .pull_mixed_frame(true)
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            pull(&mut mixer, &clock, Duration::from_millis(20))
                .iter()
                .all(|sample| *sample == 0.0)
        );
        feed(
            &mut mixer,
            &clock,
            Duration::from_millis(40),
            31,
            3,
            2_880,
            &opus,
        );
        assert!(rms(&pull(&mut mixer, &clock, Duration::from_millis(60))) > 0.2);
    }

    #[test]
    fn low_level_mix_is_linear_and_does_not_distort() {
        assert_eq!(soft_clip(0.01), 0.01);
        assert_eq!(soft_clip(-0.01), -0.01);
        let (mut mixer, clock) = test_mixer();
        let quiet = packet(440.0, 0.005);
        feed(&mut mixer, &clock, Duration::ZERO, 40, 1, 960, &quiet);
        feed(&mut mixer, &clock, Duration::ZERO, 40, 2, 1_920, &quiet);
        let first = pull(&mut mixer, &clock, Duration::from_millis(20));
        assert!(first.iter().all(|sample| sample.abs() < 0.015));
        let second = pull(&mut mixer, &clock, Duration::from_millis(40));
        assert!(second.iter().all(|sample| sample.abs() < 0.015));
        assert!(rms(&second) > 0.001);
    }

    #[test]
    fn limiter_is_bounded_only_above_the_linear_region() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.5), -0.5);
        assert!(soft_clip(10.0) <= 1.0);
        assert!(soft_clip(-10.0) >= -1.0);
        assert_eq!(soft_clip(f32::NAN), 0.0);
    }
}
