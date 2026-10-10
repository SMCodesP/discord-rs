use opus::{Application, Channels, Decoder, Encoder};
use crate::media::AudioError;

pub const OPUS_SAMPLE_RATE: u32 = 48000;
pub const OPUS_CHANNELS: usize = 2;
pub const FRAME_DURATION_MS: u32 = 20;
pub const SAMPLES_PER_CHANNEL: usize = (OPUS_SAMPLE_RATE * FRAME_DURATION_MS / 1000) as usize; // 960
pub const STEREO_FRAME_SAMPLES: usize = SAMPLES_PER_CHANNEL * OPUS_CHANNELS; // 1920
pub const MAX_OPUS_PACKET_SIZE: usize = 1275;

/// Opus audio encoder configured for 48 kHz stereo VoIP frames (20 ms, 960 samples/channel).
pub struct OpusAudioEncoder {
    encoder: Encoder,
}

impl OpusAudioEncoder {
    pub fn new() -> Result<Self, AudioError> {
        let encoder = Encoder::new(OPUS_SAMPLE_RATE, Channels::Stereo, Application::Voip)
            .map_err(|e| AudioError::OpusEncodeError(format!("Failed to create Opus encoder: {e:?}")))?;
        Ok(Self { encoder })
    }

    /// Encodes a 20 ms frame (1920 interleaved stereo f32 samples) into an Opus packet.
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

/// Opus audio decoder configured for 48 kHz stereo VoIP frames.
pub struct OpusAudioDecoder {
    decoder: Decoder,
}

impl OpusAudioDecoder {
    pub fn new() -> Result<Self, AudioError> {
        let decoder = Decoder::new(OPUS_SAMPLE_RATE, Channels::Stereo)
            .map_err(|e| AudioError::OpusDecodeError(format!("Failed to create Opus decoder: {e:?}")))?;
        Ok(Self { decoder })
    }

    /// Decodes an Opus packet into 20 ms of interleaved stereo f32 samples (1920 samples).
    pub fn decode_frame(&mut self, opus_packet: &[u8]) -> Result<Vec<f32>, AudioError> {
        let mut out = vec![0.0f32; STEREO_FRAME_SAMPLES];
        let samples_per_channel = self
            .decoder
            .decode_float(opus_packet, &mut out, false)
            .map_err(|e| AudioError::OpusDecodeError(format!("Opus decoding failed: {e:?}")))?;

        out.truncate(samples_per_channel * OPUS_CHANNELS);
        Ok(out)
    }

    /// Decodes packet loss concealment (PLC) when a packet is dropped.
    pub fn decode_loss(&mut self) -> Result<Vec<f32>, AudioError> {
        let mut out = vec![0.0f32; STEREO_FRAME_SAMPLES];
        let samples_per_channel = self
            .decoder
            .decode_float(&[], &mut out, true)
            .map_err(|e| AudioError::OpusDecodeError(format!("Opus PLC decoding failed: {e:?}")))?;

        out.truncate(samples_per_channel * OPUS_CHANNELS);
        Ok(out)
    }
}
