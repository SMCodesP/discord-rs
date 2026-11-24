use opus::{Application, Channels, Decoder, Encoder};

use crate::media::AudioError;

pub const OPUS_SAMPLE_RATE: u32 = 48_000;
pub const OPUS_CHANNELS: usize = 2;
pub const FRAME_DURATION_MS: u32 = 20;
pub const SAMPLES_PER_CHANNEL: usize = (OPUS_SAMPLE_RATE * FRAME_DURATION_MS / 1000) as usize;
pub const STEREO_FRAME_SAMPLES: usize = SAMPLES_PER_CHANNEL * OPUS_CHANNELS;
pub const MAX_OPUS_PACKET_SIZE: usize = 1275;

/// Opus audio encoder configured for Discord's 48 kHz stereo VoIP transport.
pub struct OpusAudioEncoder {
    encoder: Encoder,
}

impl OpusAudioEncoder {
    pub fn new() -> Result<Self, AudioError> {
        let encoder =
            Encoder::new(OPUS_SAMPLE_RATE, Channels::Stereo, Application::Voip).map_err(|e| {
                AudioError::OpusEncodeError(format!("Failed to create Opus encoder: {e:?}"))
            })?;
        Ok(Self { encoder })
    }

    /// Encodes one 20 ms frame (960 samples per channel, interleaved as 1920 f32 samples).
    pub fn encode_frame(&mut self, pcm: &[f32]) -> Result<Vec<u8>, AudioError> {
        if pcm.len() != STEREO_FRAME_SAMPLES {
            return Err(AudioError::InvalidFrameSize {
                expected: STEREO_FRAME_SAMPLES,
                actual: pcm.len(),
            });
        }

        let mut out = vec![0u8; MAX_OPUS_PACKET_SIZE];
        let bytes_written = self
            .encoder
            .encode_float(pcm, &mut out)
            .map_err(|e| AudioError::OpusEncodeError(format!("Opus encoding failed: {e:?}")))?;

        out.truncate(bytes_written);
        Ok(out)
    }

    pub fn set_bitrate(&mut self, bitrate: i32) -> Result<(), AudioError> {
        self.encoder
            .set_bitrate(opus::Bitrate::Bits(bitrate))
            .map_err(|e| AudioError::OpusEncodeError(format!("Failed to set bitrate: {e:?}")))
    }
}

/// Opus audio decoder configured for Discord's 48 kHz stereo VoIP transport.
pub struct OpusAudioDecoder {
    decoder: Decoder,
}

impl OpusAudioDecoder {
    pub fn new() -> Result<Self, AudioError> {
        let decoder = Decoder::new(OPUS_SAMPLE_RATE, Channels::Stereo).map_err(|e| {
            AudioError::OpusDecodeError(format!("Failed to create Opus decoder: {e:?}"))
        })?;
        Ok(Self { decoder })
    }

    /// Decodes a packet into exactly 20 ms of interleaved stereo f32 samples.
    pub fn decode_frame(&mut self, opus_packet: &[u8]) -> Result<Vec<f32>, AudioError> {
        let mut out = vec![0.0f32; STEREO_FRAME_SAMPLES];
        let samples_per_channel = self
            .decoder
            .decode_float(opus_packet, &mut out, false)
            .map_err(|e| AudioError::OpusDecodeError(format!("Opus decoding failed: {e:?}")))?;

        if samples_per_channel != SAMPLES_PER_CHANNEL {
            return Err(AudioError::InvalidFrameSize {
                expected: STEREO_FRAME_SAMPLES,
                actual: samples_per_channel * OPUS_CHANNELS,
            });
        }

        Ok(out)
    }

    /// Decodes packet loss concealment (PLC) when a packet is dropped.
    pub fn decode_loss(&mut self) -> Result<Vec<f32>, AudioError> {
        let mut out = vec![0.0f32; STEREO_FRAME_SAMPLES];
        let samples_per_channel = self
            .decoder
            .decode_float(&[], &mut out, true)
            .map_err(|e| AudioError::OpusDecodeError(format!("Opus PLC decoding failed: {e:?}")))?;

        if samples_per_channel != SAMPLES_PER_CHANNEL {
            return Err(AudioError::InvalidFrameSize {
                expected: STEREO_FRAME_SAMPLES,
                actual: samples_per_channel * OPUS_CHANNELS,
            });
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_frame(frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..SAMPLES_PER_CHANNEL)
            .flat_map(|i| {
                let sample = amplitude
                    * (std::f32::consts::TAU * frequency * i as f32 / OPUS_SAMPLE_RATE as f32)
                        .sin();
                [sample, sample]
            })
            .collect()
    }

    #[test]
    fn discord_voice_contract_is_48k_stereo_20ms() {
        assert_eq!(OPUS_SAMPLE_RATE, 48_000);
        assert_eq!(OPUS_CHANNELS, 2);
        assert_eq!(FRAME_DURATION_MS, 20);
        assert_eq!(SAMPLES_PER_CHANNEL, 960);
        assert_eq!(STEREO_FRAME_SAMPLES, 1920);
    }

    #[test]
    fn roundtrip_preserves_stereo_interleaving() {
        let mut frame = sine_frame(300.0, 0.4);
        let right = sine_frame(700.0, 0.4);
        for (index, sample) in frame.iter_mut().enumerate().skip(1).step_by(2) {
            *sample = right[index];
        }

        let mut encoder = OpusAudioEncoder::new().unwrap();
        let mut decoder = OpusAudioDecoder::new().unwrap();
        let packet = encoder.encode_frame(&frame).unwrap();
        let decoded = decoder.decode_frame(&packet).unwrap();

        assert_eq!(decoded.len(), STEREO_FRAME_SAMPLES);
        assert!(rms(decoded.iter().step_by(2)) > 0.15);
        assert!(rms(decoded.iter().skip(1).step_by(2)) > 0.15);

        let left_crossings = zero_crossings(decoded.iter().step_by(2));
        let right_crossings = zero_crossings(decoded.iter().skip(1).step_by(2));
        assert!(
            right_crossings > left_crossings * 3 / 2,
            "L/R channels were swapped or mixed: {left_crossings} vs {right_crossings} crossings"
        );
    }

    #[test]
    fn plc_returns_a_complete_20ms_frame() {
        let mut encoder = OpusAudioEncoder::new().unwrap();
        let mut decoder = OpusAudioDecoder::new().unwrap();
        decoder
            .decode_frame(&encoder.encode_frame(&sine_frame(440.0, 0.5)).unwrap())
            .unwrap();

        let concealed = decoder.decode_loss().unwrap();
        assert_eq!(concealed.len(), STEREO_FRAME_SAMPLES);
        assert!(concealed.iter().all(|sample| sample.is_finite()));
        assert!(
            rms(concealed.iter()) > 0.01,
            "PLC should continue the previous signal"
        );
    }

    #[test]
    fn rejects_mono_sized_frames_instead_of_sending_malformed_opus() {
        let mut encoder = OpusAudioEncoder::new().unwrap();
        assert!(matches!(
            encoder.encode_frame(&vec![0.0; SAMPLES_PER_CHANNEL]),
            Err(AudioError::InvalidFrameSize {
                expected: STEREO_FRAME_SAMPLES,
                actual: SAMPLES_PER_CHANNEL
            })
        ));
    }

    fn zero_crossings<'a>(mut samples: impl Iterator<Item = &'a f32>) -> usize {
        let mut previous = *samples.next().unwrap();
        let mut crossings = 0;
        for sample in samples {
            if (previous < 0.0) != (*sample < 0.0) {
                crossings += 1;
            }
            previous = *sample;
        }
        crossings
    }

    fn rms<'a>(samples: impl Iterator<Item = &'a f32>) -> f32 {
        let mut sum = 0.0;
        let mut count = 0;
        for sample in samples {
            sum += sample * sample;
            count += 1;
        }
        (sum / count as f32).sqrt()
    }
}
