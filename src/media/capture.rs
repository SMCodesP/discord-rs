use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::media::codec::{OpusAudioEncoder, OPUS_CHANNELS, OPUS_SAMPLE_RATE, STEREO_FRAME_SAMPLES};
use crate::media::resample::{convert_to_stereo, AudioFrameAccumulator, LinearResampler};
use crate::media::AudioError;

/// Voice activity speaking event transitions emitted to the transport layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakingEvent {
    SpeakingStarted,
    SpeakingStopped,
}

/// Pipeline processor for microphone capture.
/// Performs channel conversion, stateful resampling, frame accumulation,
/// VAD with 200 ms hysteresis, silence suppression, Opus encoding,
/// and dispatch to outgoing channels.
pub struct CaptureProcessor {
    dev_channels: usize,
    resampler: LinearResampler,
    accumulator: AudioFrameAccumulator,
    encoder: OpusAudioEncoder,
    outgoing_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    speaking_tx: Option<tokio::sync::mpsc::Sender<SpeakingEvent>>,

    is_muted: Arc<AtomicBool>,
    audio_level_bits: Arc<AtomicU32>,
    is_speaking: Arc<AtomicBool>,
    vad_threshold_bits: Arc<AtomicU32>,

    speaking_state: bool,
    hangover_frames_remaining: usize,
}

impl CaptureProcessor {
    pub const HANGOVER_FRAMES_200MS: usize = 10; // 10 frames * 20 ms = 200 ms

    pub fn new(
        dev_rate: u32,
        dev_channels: usize,
        outgoing_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        speaking_tx: Option<tokio::sync::mpsc::Sender<SpeakingEvent>>,
        is_muted: Arc<AtomicBool>,
        audio_level_bits: Arc<AtomicU32>,
        is_speaking: Arc<AtomicBool>,
        vad_threshold_bits: Arc<AtomicU32>,
    ) -> Result<Self, AudioError> {
        let resampler = LinearResampler::new(dev_rate, OPUS_SAMPLE_RATE, OPUS_CHANNELS);
        let accumulator = AudioFrameAccumulator::new(STEREO_FRAME_SAMPLES);
        let encoder = OpusAudioEncoder::new()?;

        Ok(Self {
            dev_channels,
            resampler,
            accumulator,
            encoder,
            outgoing_tx,
            speaking_tx,
            is_muted,
            audio_level_bits,
            is_speaking,
            vad_threshold_bits,
            speaking_state: false,
            hangover_frames_remaining: 0,
        })
    }

    /// Feeds incoming device PCM samples into the capture pipeline.
    /// Emits Opus packets into `outgoing_tx` (and returns produced packets).
    /// Emits `SpeakingEvent` transitions on speaking start and end.
    pub fn process_input_samples(&mut self, samples: &[f32]) -> Result<Vec<Vec<u8>>, AudioError> {
        let stereo = convert_to_stereo(samples, self.dev_channels);
        let resampled = self.resampler.resample(&stereo);
        self.accumulator.push_samples(&resampled);

        let mut produced_packets = Vec::new();

        while let Some(frame) = self.accumulator.pop_frame() {
            let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
            let rms = (sum_sq / frame.len() as f32).sqrt();

            if self.is_muted.load(Ordering::Relaxed) {
                self.audio_level_bits.store(0f32.to_bits(), Ordering::Relaxed);
                if self.speaking_state {
                    self.speaking_state = false;
                    self.is_speaking.store(false, Ordering::Relaxed);
                    self.hangover_frames_remaining = 0;
                    if let Some(tx) = &self.speaking_tx {
                        let _ = tx.try_send(SpeakingEvent::SpeakingStopped);
                    }
                }
                // Do not transmit when muted
                continue;
            }

            self.audio_level_bits.store(rms.to_bits(), Ordering::Relaxed);
            let threshold = f32::from_bits(self.vad_threshold_bits.load(Ordering::Relaxed));
            let voice_active = rms > threshold;

            if voice_active {
                if !self.speaking_state {
                    self.speaking_state = true;
                    self.is_speaking.store(true, Ordering::Relaxed);
                    if let Some(tx) = &self.speaking_tx {
                        let _ = tx.try_send(SpeakingEvent::SpeakingStarted);
                    }
                }
                self.hangover_frames_remaining = Self::HANGOVER_FRAMES_200MS;
            } else if self.hangover_frames_remaining > 0 {
                self.hangover_frames_remaining -= 1;
            } else if self.speaking_state {
                self.speaking_state = false;
                self.is_speaking.store(false, Ordering::Relaxed);
                if let Some(tx) = &self.speaking_tx {
                    let _ = tx.try_send(SpeakingEvent::SpeakingStopped);
                }
            }

            // Only transmit while speaking (including hysteresis hangover frames)
            if self.speaking_state {
                let packet = self.encoder.encode_frame(&frame)?;
                let _ = self.outgoing_tx.try_send(packet.clone());
                produced_packets.push(packet);
            }
        }

        Ok(produced_packets)
    }

    pub fn is_speaking_active(&self) -> bool {
        self.speaking_state
    }

    pub fn hangover_remaining(&self) -> usize {
        self.hangover_frames_remaining
    }
}
