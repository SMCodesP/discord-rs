use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::media::AudioError;
use crate::media::codec::{
    OPUS_CHANNELS, OPUS_SAMPLE_RATE, OpusAudioEncoder, STEREO_FRAME_SAMPLES,
};
use crate::media::resample::{AudioFrameAccumulator, LinearResampler, convert_to_stereo};

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
        if dev_rate == 0 {
            return Err(AudioError::InvalidSampleRate(dev_rate));
        }
        if dev_channels == 0 {
            return Err(AudioError::InvalidChannels(0));
        }

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

        while let Some(mut frame) = self.accumulator.pop_frame() {
            // cpal f32 data is nominally normalized, but drivers can expose values outside
            // [-1, 1]. Opus float input must be finite and bounded to avoid corrupted packets.
            for sample in &mut frame {
                *sample = if sample.is_finite() {
                    sample.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
            }

            let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
            let rms = (sum_sq / frame.len() as f32).sqrt();

            if self.is_muted.load(Ordering::Relaxed) {
                self.audio_level_bits
                    .store(0f32.to_bits(), Ordering::Relaxed);
                if self.speaking_state {
                    self.speaking_state = false;
                    self.is_speaking.store(false, Ordering::Relaxed);
                    self.hangover_frames_remaining = 0;
                    if let Some(tx) = &self.speaking_tx {
                        let _ = tx.try_send(SpeakingEvent::SpeakingStopped);
                    }
                }
                // Do not transmit when muted.
                continue;
            }

            self.audio_level_bits
                .store(rms.to_bits(), Ordering::Relaxed);
            let configured_threshold =
                f32::from_bits(self.vad_threshold_bits.load(Ordering::Relaxed));
            let threshold = if configured_threshold.is_finite() {
                configured_threshold.max(0.0)
            } else {
                0.01
            };
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

            // Only transmit while speaking (including hysteresis hangover frames).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::codec::OpusAudioDecoder;

    fn processor(rate: u32, channels: usize, threshold: f32) -> CaptureProcessor {
        let (outgoing_tx, _) = tokio::sync::mpsc::channel(128);
        CaptureProcessor::new(
            rate,
            channels,
            outgoing_tx,
            None,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU32::new(0)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU32::new(threshold.to_bits())),
        )
        .unwrap()
    }

    #[test]
    fn rejects_invalid_device_rate_and_channel_count() {
        let (tx, _) = tokio::sync::mpsc::channel(1);
        let args = |rate, channels| {
            CaptureProcessor::new(
                rate,
                channels,
                tx.clone(),
                None,
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicU32::new(0)),
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicU32::new(0.01f32.to_bits())),
            )
        };
        assert!(matches!(args(0, 2), Err(AudioError::InvalidSampleRate(0))));
        assert!(matches!(
            args(48_000, 0),
            Err(AudioError::InvalidChannels(0))
        ));
    }

    #[test]
    fn mono_44100_capture_produces_48k_stereo_opus_frames() {
        let mut capture = processor(44_100, 1, 0.01);
        let mut decoder = OpusAudioDecoder::new().unwrap();
        let mut packets = Vec::new();

        // 250 ms gives the streaming resampler enough tail input to emit twelve full frames.
        for start in (0..11_025).step_by(173) {
            let end = (start + 173).min(11_025);
            let samples: Vec<f32> = (start..end)
                .map(|i| 0.4 * (std::f32::consts::TAU * 440.0 * i as f32 / 44_100.0).sin())
                .collect();
            packets.extend(capture.process_input_samples(&samples).unwrap());
        }

        assert_eq!(packets.len(), 12);
        for packet in packets {
            let decoded = decoder.decode_frame(&packet).unwrap();
            assert_eq!(decoded.len(), STEREO_FRAME_SAMPLES);
            for frame in decoded.chunks_exact(2) {
                assert!(
                    (frame[0] - frame[1]).abs() < 1e-5,
                    "mono input must be duplicated to L/R"
                );
            }
        }
    }

    #[test]
    fn irregular_callback_chunks_still_produce_exact_20ms_frames() {
        let mut capture = processor(48_000, 2, 0.01);
        let mut decoder = OpusAudioDecoder::new().unwrap();
        let mut packets = Vec::new();
        let chunk_sizes = [1, 17, 240, 13, 960, 333, 1920];
        let signal: Vec<f32> = (0..10_000)
            .flat_map(|index| {
                let sample = 0.3 * (std::f32::consts::TAU * 330.0 * index as f32 / 48_000.0).sin();
                [sample, sample]
            })
            .collect();

        let mut offset = 0;
        let mut chunk_index = 0;
        while offset < signal.len() {
            let end = (offset + chunk_sizes[chunk_index % chunk_sizes.len()]).min(signal.len());
            packets.extend(capture.process_input_samples(&signal[offset..end]).unwrap());
            offset = end;
            chunk_index += 1;
        }

        assert_eq!(packets.len(), 10);
        for packet in packets {
            assert_eq!(
                decoder.decode_frame(&packet).unwrap().len(),
                STEREO_FRAME_SAMPLES
            );
        }
    }

    #[test]
    fn non_finite_and_overscaled_driver_samples_are_sanitized_before_encoding() {
        let mut capture = processor(48_000, 2, 0.0);
        let mut decoder = OpusAudioDecoder::new().unwrap();
        let mut frame = vec![0.5; STEREO_FRAME_SAMPLES];
        frame[0] = f32::NAN;
        frame[1] = f32::INFINITY;
        frame[2] = 2.0;
        frame[3] = -2.0;

        let packets = capture.process_input_samples(&frame).unwrap();
        assert_eq!(packets.len(), 1);
        let decoded = decoder.decode_frame(&packets[0]).unwrap();
        assert!(decoded.iter().all(|sample| sample.is_finite()));
        assert!(decoded.iter().all(|sample| sample.abs() <= 1.0));
    }
}
