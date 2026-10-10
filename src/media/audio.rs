use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

use crate::media::codec::{OpusAudioEncoder, OPUS_CHANNELS, OPUS_SAMPLE_RATE, STEREO_FRAME_SAMPLES};
use crate::media::devices::MediaDeviceManager;
use crate::media::mixer::AudioMixer;
use crate::media::resample::{
    convert_stereo_to_channels, convert_to_stereo, f32_to_i16, f32_to_u16, i16_to_f32,
    resample_linear, u16_to_f32, AudioFrameAccumulator,
};
use crate::media::AudioError;

/// Core audio engine that implements real hardware capture/playback via `cpal`,
/// 48 kHz stereo VoIP encoding/decoding via Opus, multi-participant mixing,
/// voice activity detection (VAD), and live device switching.
pub struct AudioEngine {
    device_manager: Arc<RwLock<MediaDeviceManager>>,
    mixer: Arc<Mutex<AudioMixer>>,
    outgoing_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    outgoing_rx: Option<tokio::sync::mpsc::Receiver<Vec<u8>>>,

    // Local user audio state
    is_muted: Arc<AtomicBool>,
    is_deafened: Arc<AtomicBool>,
    audio_level_bits: Arc<AtomicU32>,
    is_speaking: Arc<AtomicBool>,
    vad_threshold_bits: Arc<AtomicU32>,
    last_speech_time_ms: Arc<AtomicU64>,

    // Active streams
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    active_input_id: Option<String>,
    active_output_id: Option<String>,
    is_session_active: bool,
    start_time: Instant,
}

impl AudioEngine {
    /// Creates a new AudioEngine instance with transport contract channels.
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        Self {
            device_manager: Arc::new(RwLock::new(MediaDeviceManager::new())),
            mixer: Arc::new(Mutex::new(AudioMixer::new())),
            outgoing_tx: tx,
            outgoing_rx: Some(rx),

            is_muted: Arc::new(AtomicBool::new(false)),
            is_deafened: Arc::new(AtomicBool::new(false)),
            audio_level_bits: Arc::new(AtomicU32::new(0f32.to_bits())),
            is_speaking: Arc::new(AtomicBool::new(false)),
            vad_threshold_bits: Arc::new(AtomicU32::new(0.01f32.to_bits())),
            last_speech_time_ms: Arc::new(AtomicU64::new(0)),

            input_stream: None,
            output_stream: None,
            active_input_id: None,
            active_output_id: None,
            is_session_active: false,
            start_time: Instant::now(),
        }
    }

    /// Access the underlying `MediaDeviceManager`.
    pub fn device_manager(&self) -> Arc<RwLock<MediaDeviceManager>> {
        Arc::clone(&self.device_manager)
    }

    /// Takes the outgoing Opus frame receiver for the transport layer (VoiceUDP).
    /// One `Vec<u8>` is emitted per 20 ms frame.
    pub fn take_outgoing_receiver(&mut self) -> Option<tokio::sync::mpsc::Receiver<Vec<u8>>> {
        self.outgoing_rx.take()
    }

    /// Transport contract method: receives an Opus frame (`&[u8]`) identified by participant SSRC (`u32`).
    pub fn receive_opus_packet(&self, ssrc: u32, opus_data: &[u8]) -> Result<(), AudioError> {
        let mut mixer = self.mixer.lock().unwrap();
        mixer.feed_opus_packet(ssrc, opus_data)
    }

    /// Returns whether the user is locally muted.
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }

    /// Sets local microphone mute state.
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Relaxed);
        if muted {
            self.audio_level_bits.store(0f32.to_bits(), Ordering::Relaxed);
            self.is_speaking.store(false, Ordering::Relaxed);
        }
    }

    /// Returns whether output playback is deafened.
    pub fn is_deafened(&self) -> bool {
        self.is_deafened.load(Ordering::Relaxed)
    }

    /// Sets deafened state (mutes playback of all remote streams).
    pub fn set_deafened(&self, deafened: bool) {
        self.is_deafened.store(deafened, Ordering::Relaxed);
    }

    /// Returns current real microphone audio level (RMS, 0.0 to 1.0).
    pub fn audio_level(&self) -> f32 {
        f32::from_bits(self.audio_level_bits.load(Ordering::Relaxed))
    }

    /// Returns true if local voice activity is detected.
    pub fn is_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::Relaxed)
    }

    /// Configures the Voice Activity Detection (VAD) sensitivity threshold (default 0.01).
    pub fn set_vad_threshold(&self, threshold: f32) {
        self.vad_threshold_bits.store(threshold.max(0.0).to_bits(), Ordering::Relaxed);
        let mut mixer = self.mixer.lock().unwrap();
        mixer.set_vad_threshold(threshold);
    }

    pub fn vad_threshold(&self) -> f32 {
        f32::from_bits(self.vad_threshold_bits.load(Ordering::Relaxed))
    }

    /// Starts an active audio session with hardware capture and playback.
    /// If hardware is available, initializes the input and output streams.
    pub fn start_session(&mut self) -> Result<(), AudioError> {
        let selected_input = self.device_manager.read().unwrap().selected_input_id().map(String::from);
        let selected_output = self.device_manager.read().unwrap().selected_output_id().map(String::from);

        self.start_input_stream(selected_input.as_deref())?;
        self.start_output_stream(selected_output.as_deref())?;

        self.is_session_active = true;
        Ok(())
    }

    /// Stops the active audio session and releases hardware audio streams.
    pub fn stop_session(&mut self) {
        self.input_stream = None;
        self.output_stream = None;
        self.active_input_id = None;
        self.active_output_id = None;
        self.is_session_active = false;
        self.is_speaking.store(false, Ordering::Relaxed);
        self.audio_level_bits.store(0f32.to_bits(), Ordering::Relaxed);
    }

    pub fn is_session_active(&self) -> bool {
        self.is_session_active
    }

    pub fn active_input_id(&self) -> Option<&str> {
        self.active_input_id.as_deref()
    }

    pub fn active_output_id(&self) -> Option<&str> {
        self.active_output_id.as_deref()
    }

    /// Switches the active microphone dynamically WITHOUT tearing down the voice session.
    pub fn switch_input_device(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        self.device_manager.write().unwrap().select_input_device(device_id)?;

        if self.is_session_active {
            // Replace only the input stream while preserving the session and outgoing channel
            self.input_stream = None;
            self.start_input_stream(device_id)?;
        }
        Ok(())
    }

    /// Switches the active playback device dynamically WITHOUT tearing down the voice session.
    pub fn switch_output_device(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        self.device_manager.write().unwrap().select_output_device(device_id)?;

        if self.is_session_active {
            // Replace only the output stream while preserving all active participant decoders
            self.output_stream = None;
            self.start_output_stream(device_id)?;
        }
        Ok(())
    }

    fn start_input_stream(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        let mgr = self.device_manager.read().unwrap();
        let dev = mgr.get_cpal_input_device(device_id)?;

        let supported_config = dev
            .default_input_config()
            .map_err(|e| AudioError::StreamError(format!("Failed to query default input config: {e:?}")))?;

        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let dev_channels = stream_config.channels as usize;
        let dev_rate = stream_config.sample_rate.0;

        let mut accumulator = AudioFrameAccumulator::new(STEREO_FRAME_SAMPLES);
        let mut encoder = OpusAudioEncoder::new()?;
        let outgoing_tx = self.outgoing_tx.clone();

        let is_muted = Arc::clone(&self.is_muted);
        let audio_level_bits = Arc::clone(&self.audio_level_bits);
        let is_speaking = Arc::clone(&self.is_speaking);
        let vad_threshold_bits = Arc::clone(&self.vad_threshold_bits);
        let last_speech_time_ms = Arc::clone(&self.last_speech_time_ms);
        let start_time = self.start_time;

        let mut process_samples = move |samples_f32: &[f32]| {
            let stereo = convert_to_stereo(samples_f32, dev_channels);
            let resampled = resample_linear(&stereo, dev_rate, OPUS_SAMPLE_RATE, OPUS_CHANNELS);
            accumulator.push_samples(&resampled);

            while let Some(frame) = accumulator.pop_frame() {
                let muted = is_muted.load(Ordering::Relaxed);
                if muted {
                    audio_level_bits.store(0f32.to_bits(), Ordering::Relaxed);
                    is_speaking.store(false, Ordering::Relaxed);
                    continue;
                }

                // Calculate RMS level
                let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
                let rms = (sum_sq / frame.len() as f32).sqrt();
                audio_level_bits.store(rms.to_bits(), Ordering::Relaxed);

                let threshold = f32::from_bits(vad_threshold_bits.load(Ordering::Relaxed));
                let now_ms = Instant::now().duration_since(start_time).as_millis() as u64;

                if rms > threshold {
                    is_speaking.store(true, Ordering::Relaxed);
                    last_speech_time_ms.store(now_ms, Ordering::Relaxed);
                } else {
                    let last_ms = last_speech_time_ms.load(Ordering::Relaxed);
                    // 200 ms hangover to prevent flicker
                    if now_ms.saturating_sub(last_ms) > 200 {
                        is_speaking.store(false, Ordering::Relaxed);
                    }
                }

                // Encode with Opus and emit to transport channel
                if let Ok(packet) = encoder.encode_frame(&frame) {
                    let _ = outgoing_tx.try_send(packet);
                }
            }
        };

        let err_fn = |err| {
            log::error!("Audio input stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => dev
                .build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        process_samples(data);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build f32 input stream: {e:?}")))?,

            SampleFormat::I16 => dev
                .build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| i16_to_f32(s)).collect();
                        process_samples(&f32_data);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build i16 input stream: {e:?}")))?,

            SampleFormat::U16 => dev
                .build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| u16_to_f32(s)).collect();
                        process_samples(&f32_data);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build u16 input stream: {e:?}")))?,

            other => {
                return Err(AudioError::UnsupportedFormat(format!(
                    "Unsupported input sample format: {other:?}"
                )))
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::StreamError(format!("Failed to play input stream: {e:?}")))?;

        self.input_stream = Some(stream);
        self.active_input_id = device_id.map(String::from).or_else(|| dev.name().ok());
        Ok(())
    }

    fn start_output_stream(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        let mgr = self.device_manager.read().unwrap();
        let dev = mgr.get_cpal_output_device(device_id)?;

        let supported_config = dev
            .default_output_config()
            .map_err(|e| AudioError::StreamError(format!("Failed to query default output config: {e:?}")))?;

        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let dev_channels = stream_config.channels as usize;
        let dev_rate = stream_config.sample_rate.0;

        let mixer = Arc::clone(&self.mixer);
        let is_deafened = Arc::clone(&self.is_deafened);
        let mut output_buffer: VecDeque<f32> = VecDeque::with_capacity(dev_channels * 4096);

        let mut fill_samples = move |out_samples: &mut [f32]| {
            let deafened = is_deafened.load(Ordering::Relaxed);

            // Refill output buffer from mixer when needed
            while output_buffer.len() < out_samples.len() {
                let frame_48k_stereo = {
                    let mut m = mixer.lock().unwrap();
                    m.pull_mixed_frame(deafened)
                };

                let resampled = resample_linear(
                    &frame_48k_stereo,
                    OPUS_SAMPLE_RATE,
                    dev_rate,
                    OPUS_CHANNELS,
                );
                let target_channels = convert_stereo_to_channels(&resampled, dev_channels);
                output_buffer.extend(target_channels);
            }

            for sample in out_samples.iter_mut() {
                *sample = output_buffer.pop_front().unwrap_or(0.0);
            }
        };

        let err_fn = |err| {
            log::error!("Audio output stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => dev
                .build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _| {
                        fill_samples(data);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build f32 output stream: {e:?}")))?,

            SampleFormat::I16 => dev
                .build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _| {
                        let mut f32_buf = vec![0.0f32; data.len()];
                        fill_samples(&mut f32_buf);
                        for (dest, src) in data.iter_mut().zip(f32_buf.iter()) {
                            *dest = f32_to_i16(*src);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build i16 output stream: {e:?}")))?,

            SampleFormat::U16 => dev
                .build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _| {
                        let mut f32_buf = vec![0.0f32; data.len()];
                        fill_samples(&mut f32_buf);
                        for (dest, src) in data.iter_mut().zip(f32_buf.iter()) {
                            *dest = f32_to_u16(*src);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build u16 output stream: {e:?}")))?,

            other => {
                return Err(AudioError::UnsupportedFormat(format!(
                    "Unsupported output sample format: {other:?}"
                )))
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::StreamError(format!("Failed to play output stream: {e:?}")))?;

        self.output_stream = Some(stream);
        self.active_output_id = device_id.map(String::from).or_else(|| dev.name().ok());
        Ok(())
    }

    /// Processes a synthetic PCM stereo frame directly through the DSP, VAD, and Opus pipeline.
    /// Used for deterministic unit testing without requiring real hardware capture.
    pub fn process_synthetic_input_frame(&self, frame: &[f32]) -> Result<Option<Vec<u8>>, AudioError> {
        if frame.len() != STEREO_FRAME_SAMPLES {
            return Err(AudioError::InvalidFrameSize {
                expected: STEREO_FRAME_SAMPLES,
                actual: frame.len(),
            });
        }

        let muted = self.is_muted();
        if muted {
            self.audio_level_bits.store(0f32.to_bits(), Ordering::Relaxed);
            self.is_speaking.store(false, Ordering::Relaxed);
            return Ok(None);
        }

        let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame.len() as f32).sqrt();
        self.audio_level_bits.store(rms.to_bits(), Ordering::Relaxed);

        let threshold = self.vad_threshold();
        let is_speaking = rms > threshold;
        self.is_speaking.store(is_speaking, Ordering::Relaxed);

        let mut encoder = OpusAudioEncoder::new()?;
        let packet = encoder.encode_frame(frame)?;
        let _ = self.outgoing_tx.try_send(packet.clone());

        Ok(Some(packet))
    }

    /// Pulls a mixed frame directly from the mixer for testing.
    pub fn pull_mixed_frame_synthetic(&self) -> Vec<f32> {
        let mut mixer = self.mixer.lock().unwrap();
        mixer.pull_mixed_frame(self.is_deafened())
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}
