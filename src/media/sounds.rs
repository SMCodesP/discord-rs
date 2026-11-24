use std::collections::HashMap;
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

pub const SOUND_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundEffect {
    Message,
    Mention,
    CallCalling,
    CallRinging,
    UserJoin,
    UserLeave,
    UserMoved,
    Mute,
    Unmute,
    Deafen,
    Undeafen,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdsrEnvelope {
    pub attack: Duration,
    pub decay: Duration,
    pub sustain_level: f32,
    pub release: Duration,
}

impl AdsrEnvelope {
    pub fn new(attack: Duration, decay: Duration, sustain_level: f32, release: Duration) -> Self {
        Self {
            attack,
            decay,
            sustain_level: sustain_level.clamp(0.0, 1.0),
            release,
        }
    }

    pub fn level_at(&self, t: Duration, note_duration: Duration) -> f32 {
        let t = t.as_secs_f32();
        let note = note_duration.as_secs_f32();
        let attack = self.attack.as_secs_f32();
        let decay = self.decay.as_secs_f32();
        let release = self.release.as_secs_f32();
        let sustain = self.sustain_level.clamp(0.0, 1.0);

        if t < 0.0 || t > note + release {
            return 0.0;
        }

        if t <= note {
            if attack > 0.0 && t < attack {
                return (t / attack).clamp(0.0, 1.0);
            }
            let after_attack = t - attack;
            if decay > 0.0 && after_attack < decay {
                let progress = (after_attack / decay).clamp(0.0, 1.0);
                return 1.0 + (sustain - 1.0) * progress;
            }
            return sustain;
        }

        let release_t = t - note;
        if release <= 0.0 {
            return 0.0;
        }
        let release_start = if attack > 0.0 && note < attack {
            (note / attack).clamp(0.0, 1.0)
        } else {
            let after_attack = note - attack;
            if decay > 0.0 && after_attack < decay {
                let progress = (after_attack / decay).clamp(0.0, 1.0);
                1.0 + (sustain - 1.0) * progress
            } else {
                sustain
            }
        };
        let progress = (release_t / release).clamp(0.0, 1.0);
        (release_start * (1.0 - progress)).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ToneSegment {
    freq_hz: f32,
    duration: Duration,
    gain: f32,
}

const MESSAGE_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 523.25, duration: Duration::from_millis(70), gain: 0.22 },
    ToneSegment { freq_hz: 659.25, duration: Duration::from_millis(90), gain: 0.20 },
];
const MENTION_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 659.25, duration: Duration::from_millis(60), gain: 0.22 },
    ToneSegment { freq_hz: 880.0, duration: Duration::from_millis(70), gain: 0.24 },
    ToneSegment { freq_hz: 1046.5, duration: Duration::from_millis(90), gain: 0.20 },
];
const CALL_CALLING_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 440.0, duration: Duration::from_millis(320), gain: 0.18 },
    ToneSegment { freq_hz: 0.0, duration: Duration::from_millis(120), gain: 0.0 },
    ToneSegment { freq_hz: 554.37, duration: Duration::from_millis(320), gain: 0.18 },
];
const CALL_RINGING_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 740.0, duration: Duration::from_millis(180), gain: 0.20 },
    ToneSegment { freq_hz: 932.33, duration: Duration::from_millis(220), gain: 0.20 },
    ToneSegment { freq_hz: 0.0, duration: Duration::from_millis(80), gain: 0.0 },
    ToneSegment { freq_hz: 740.0, duration: Duration::from_millis(180), gain: 0.18 },
];
const USER_JOIN_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 392.0, duration: Duration::from_millis(70), gain: 0.18 },
    ToneSegment { freq_hz: 523.25, duration: Duration::from_millis(110), gain: 0.20 },
];
const USER_LEAVE_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 523.25, duration: Duration::from_millis(70), gain: 0.18 },
    ToneSegment { freq_hz: 392.0, duration: Duration::from_millis(110), gain: 0.20 },
];
const USER_MOVED_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 440.0, duration: Duration::from_millis(60), gain: 0.16 },
    ToneSegment { freq_hz: 493.88, duration: Duration::from_millis(80), gain: 0.16 },
];
const MUTE_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 330.0, duration: Duration::from_millis(90), gain: 0.18 },
];
const UNMUTE_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 440.0, duration: Duration::from_millis(90), gain: 0.18 },
];
const DEAFEN_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 294.0, duration: Duration::from_millis(80), gain: 0.17 },
    ToneSegment { freq_hz: 220.0, duration: Duration::from_millis(120), gain: 0.19 },
];
const UNDEAFEN_SEGMENTS: &[ToneSegment] = &[
    ToneSegment { freq_hz: 220.0, duration: Duration::from_millis(80), gain: 0.17 },
    ToneSegment { freq_hz: 294.0, duration: Duration::from_millis(120), gain: 0.19 },
];

impl SoundEffect {
    fn segments(self) -> &'static [ToneSegment] {
        match self {
            Self::Message => MESSAGE_SEGMENTS,
            Self::Mention => MENTION_SEGMENTS,
            Self::CallCalling => CALL_CALLING_SEGMENTS,
            Self::CallRinging => CALL_RINGING_SEGMENTS,
            Self::UserJoin => USER_JOIN_SEGMENTS,
            Self::UserLeave => USER_LEAVE_SEGMENTS,
            Self::UserMoved => USER_MOVED_SEGMENTS,
            Self::Mute => MUTE_SEGMENTS,
            Self::Unmute => UNMUTE_SEGMENTS,
            Self::Deafen => DEAFEN_SEGMENTS,
            Self::Undeafen => UNDEAFEN_SEGMENTS,
        }
    }

    fn envelope(self) -> AdsrEnvelope {
        match self {
            Self::CallCalling | Self::CallRinging => AdsrEnvelope::new(
                Duration::from_millis(18),
                Duration::from_millis(50),
                0.85,
                Duration::from_millis(60),
            ),
            Self::Mute | Self::Unmute | Self::Deafen | Self::Undeafen => AdsrEnvelope::new(
                Duration::from_millis(8),
                Duration::from_millis(35),
                0.75,
                Duration::from_millis(45),
            ),
            _ => AdsrEnvelope::new(
                Duration::from_millis(10),
                Duration::from_millis(40),
                0.80,
                Duration::from_millis(50),
            ),
        }
    }

    pub fn default_cooldown(self) -> Duration {
        match self {
            Self::Message | Self::Mention => Duration::from_millis(350),
            Self::CallCalling | Self::CallRinging => Duration::from_millis(1500),
            Self::UserJoin | Self::UserLeave | Self::UserMoved => Duration::from_millis(250),
            Self::Mute | Self::Unmute | Self::Deafen | Self::Undeafen => Duration::from_millis(120),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SoundSynthesizer {
    sample_rate: u32,
}

impl Default for SoundSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundSynthesizer {
    pub fn new() -> Self {
        Self {
            sample_rate: SOUND_SAMPLE_RATE,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn synthesize(&self, effect: SoundEffect, volume: f32) -> Vec<f32> {
        self.synthesize_with_envelope(effect, volume, effect.envelope())
    }

    pub fn synthesize_with_envelope(
        &self,
        effect: SoundEffect,
        volume: f32,
        envelope: AdsrEnvelope,
    ) -> Vec<f32> {
        let volume = sanitize_volume(volume);
        let mut output = Vec::new();
        let mut phase: f32 = 0.0;

        for segment in effect.segments() {
            let segment_samples = duration_to_samples(segment.duration, self.sample_rate);
            let release_samples = duration_to_samples(envelope.release, self.sample_rate);
            let total_samples = segment_samples.saturating_add(release_samples);
            let note_duration = segment.duration;

            for i in 0..total_samples {
                let t = samples_to_duration(i, self.sample_rate);
                let env = envelope.level_at(t, note_duration);
                let sample = if segment.freq_hz > 0.0 && segment.gain > 0.0 {
                    let wave = smooth_wave(phase);
                    let amp = (segment.gain * volume * env).clamp(0.0, 1.0);
                    phase = (phase + (TAU * segment.freq_hz / self.sample_rate as f32)) % TAU;
                    wave * amp
                } else {
                    0.0
                };
                output.push(sample.clamp(-1.0, 1.0));
            }
        }

        if output.is_empty() {
            output.push(0.0);
        }
        output
    }
}

fn smooth_wave(phase: f32) -> f32 {
    // Fundamental + gentle second harmonic for a soft, less buzzy tone.
    let fundamental = phase.sin();
    let harmonic = (phase * 2.0).sin() * 0.08;
    (fundamental + harmonic) * 0.92
}

fn sanitize_volume(volume: f32) -> f32 {
    if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn duration_to_samples(duration: Duration, sample_rate: u32) -> usize {
    let secs = duration.as_secs_f64();
    let samples = secs * sample_rate as f64;
    if samples.is_finite() && samples > 0.0 {
        samples.round() as usize
    } else {
        0
    }
}

fn samples_to_duration(samples: usize, sample_rate: u32) -> Duration {
    if sample_rate == 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(samples as f64 / sample_rate as f64)
}

#[derive(Debug, Clone)]
pub struct SoundPlaybackPolicy {
    volume: f32,
    dnd_suppressed: bool,
    cooldown: Duration,
    last_played: HashMap<SoundEffect, Instant>,
}

impl Default for SoundPlaybackPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundPlaybackPolicy {
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            dnd_suppressed: false,
            cooldown: Duration::from_millis(250),
            last_played: HashMap::new(),
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.set_volume(volume);
        self
    }

    pub fn with_dnd_suppressed(mut self, suppressed: bool) -> Self {
        self.dnd_suppressed = suppressed;
        self
    }

    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.cooldown = cooldown;
        self
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = sanitize_volume(volume);
    }

    pub fn dnd_suppressed(&self) -> bool {
        self.dnd_suppressed
    }

    pub fn set_dnd_suppressed(&mut self, suppressed: bool) {
        self.dnd_suppressed = suppressed;
    }

    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    pub fn set_cooldown(&mut self, cooldown: Duration) {
        self.cooldown = cooldown;
    }

    pub fn can_play_at(&self, effect: SoundEffect, now: Instant) -> bool {
        if self.dnd_suppressed || self.volume <= 0.0 {
            return false;
        }
        let cooldown = effective_cooldown(self.cooldown, effect);
        match self.last_played.get(&effect) {
            Some(last) => now.saturating_duration_since(*last) >= cooldown,
            None => true,
        }
    }

    pub fn record_play_at(&mut self, effect: SoundEffect, now: Instant) {
        self.last_played.insert(effect, now);
    }

    pub fn try_play_at(&mut self, effect: SoundEffect, now: Instant) -> bool {
        if !self.can_play_at(effect, now) {
            return false;
        }
        self.record_play_at(effect, now);
        true
    }
}

fn effective_cooldown(global: Duration, effect: SoundEffect) -> Duration {
    if global > effect.default_cooldown() {
        global
    } else {
        effect.default_cooldown()
    }
}

#[derive(Debug, Clone)]
pub struct ClientSoundService {
    synthesizer: SoundSynthesizer,
    policy: SoundPlaybackPolicy,
}

impl Default for ClientSoundService {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientSoundService {
    pub fn new() -> Self {
        Self {
            synthesizer: SoundSynthesizer::new(),
            policy: SoundPlaybackPolicy::new(),
        }
    }

    pub fn with_policy(policy: SoundPlaybackPolicy) -> Self {
        Self {
            synthesizer: SoundSynthesizer::new(),
            policy,
        }
    }

    pub fn policy(&self) -> &SoundPlaybackPolicy {
        &self.policy
    }

    pub fn policy_mut(&mut self) -> &mut SoundPlaybackPolicy {
        &mut self.policy
    }

    pub fn synthesizer(&self) -> &SoundSynthesizer {
        &self.synthesizer
    }

    pub fn play_at(&mut self, effect: SoundEffect, now: Instant) -> Option<Vec<f32>> {
        if !self.policy.try_play_at(effect, now) {
            return None;
        }
        Some(self.synthesizer.synthesize(effect, self.policy.volume()))
    }

    pub fn render(&self, effect: SoundEffect) -> Vec<f32> {
        self.synthesizer.synthesize(effect, self.policy.volume())
    }
}
