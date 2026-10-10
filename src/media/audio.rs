use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

use crate::media::capture::{CaptureProcessor, SpeakingEvent};
use crate::media::codec::{OPUS_CHANNELS, OPUS_SAMPLE_RATE};
use crate::media::devices::MediaDeviceManager;
use crate::media::mixer::AudioMixer;
use crate::media::resample::{
    convert_stereo_to_channels, f32_to_i16, f32_to_u16, i16_to_f32, u16_to_f32, LinearResampler,
};
use crate::media::AudioError;

/// Core audio engine that implements real hardware capture/playback via `cpal`,
/// 48 kHz stereo VoIP encoding/decoding via Opus, multi-participant mixing,
/// voice activity detection (VAD), silence suppression, and atomic device switching.
pub struct AudioEngine {
    device_manager: Arc<RwLock<MediaDeviceManager>>,
    mixer: Arc<Mutex<AudioMixer>>,
    outgoing_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    outgoing_rx: Option<tokio::sync::mpsc::Receiver<Vec<u8>>>,
    speaking_tx: tokio::sync::mpsc::Sender<SpeakingEvent>,
    speaking_rx: Option<tokio::sync::mpsc::Receiver<SpeakingEvent>>,

    // Local user audio state
    is_muted: Arc<AtomicBool>,
    is_deafened: Arc<AtomicBool>,
    audio_level_bits: Arc<AtomicU32>,
    is_speaking: Arc<AtomicBool>,
    vad_threshold_bits: Arc<AtomicU32>,

    // Active streams
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    active_input_id: Option<String>,
    active_output_id: Option<String>,
    is_session_active: bool,
}

impl AudioEngine {
    /// Creates a new AudioEngine instance with transport contract channels.
    pub fn new() -> Self {
        let (out_tx, out_rx) = tokio::sync::mpsc::channel(128);
        let (spk_tx, spk_rx) = tokio::sync::mpsc::channel(32);

        Self {
            device_manager: Arc::new(RwLock::new(MediaDeviceManager::new())),
            mixer: Arc::new(Mutex::new(AudioMixer::new())),
            outgoing_tx: out_tx,
            outgoing_rx: Some(out_rx),
            speaking_tx: spk_tx,
            speaking_rx: Some(spk_rx),

            is_muted: Arc::new(AtomicBool::new(false)),
            is_deafened: Arc::new(AtomicBool::new(false)),
            audio_level_bits: Arc::new(AtomicU32::new(0f32.to_bits())),
            is_speaking: Arc::new(AtomicBool::new(false)),
            vad_threshold_bits: Arc::new(AtomicU32::new(0.01f32.to_bits())),

            input_stream: None,
            output_stream: None,
            active_input_id: None,
            active_output_id: None,
            is_session_active: false,
        }
    }

    /// Access the underlying `MediaDeviceManager`.
    pub fn device_manager(&self) -> Arc<RwLock<MediaDeviceManager>> {
        Arc::clone(&self.device_manager)
    }

    /// Access the multi-stream `AudioMixer`.
    pub fn mixer(&self) -> Arc<Mutex<AudioMixer>> {
        Arc::clone(&self.mixer)
    }

    /// Takes the outgoing Opus frame receiver for the transport layer (VoiceUDP).
    /// Emits `Vec<u8>` frames (one per 20 ms) strictly during active speech.
    pub fn take_outgoing_receiver(&mut self) -> Option<tokio::sync::mpsc::Receiver<Vec<u8>>> {
        self.outgoing_rx.take()
    }

    /// Takes the receiver for voice activity state transitions (SpeakingStarted / SpeakingStopped).
    /// Used by the transport layer to send Opcode 5 Speaking and silence frames to Discord.
    pub fn take_speaking_receiver(&mut self) -> Option<tokio::sync::mpsc::Receiver<SpeakingEvent>> {
        self.speaking_rx.take()
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
    /// If output or input fails, cleans up any partially opened stream and leaves state consistent.
    pub fn start_session(&mut self) -> Result<(), AudioError> {
        let selected_input = self.device_manager.read().unwrap().selected_input_id().map(String::from);
        let selected_output = self.device_manager.read().unwrap().selected_output_id().map(String::from);

        let (in_stream, in_id) = self.build_input_stream(selected_input.as_deref())?;

        let (out_stream, out_id) = match self.build_output_stream(selected_output.as_deref()) {
            Ok(res) => res,
            Err(err) => {
                // Rollback: drop input stream and leave state consistent
                drop(in_stream);
                self.stop_session();
                return Err(err);
            }
        };

        self.input_stream = Some(in_stream);
        self.active_input_id = Some(in_id);
        self.output_stream = Some(out_stream);
        self.active_output_id = Some(out_id);
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

    /// Switches the active microphone dynamically.
    /// Builds and starts the new stream first; if it fails, the previous stream and selection remain intact.
    pub fn switch_input_device(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        if self.is_session_active {
            // Build and play new stream FIRST
            let (new_stream, new_id) = self.build_input_stream(device_id)?;
            // Only update selection and replace stream on success
            self.device_manager.write().unwrap().select_input_device(device_id)?;
            self.input_stream = Some(new_stream);
            self.active_input_id = Some(new_id);
        } else {
            self.device_manager.write().unwrap().select_input_device(device_id)?;
        }
        Ok(())
    }

    /// Switches the active playback device dynamically.
    /// Builds and starts the new stream first; if it fails, the previous stream and selection remain intact.
    pub fn switch_output_device(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        if self.is_session_active {
            // Build and play new stream FIRST
            let (new_stream, new_id) = self.build_output_stream(device_id)?;
            // Only update selection and replace stream on success
            self.device_manager.write().unwrap().select_output_device(device_id)?;
            self.output_stream = Some(new_stream);
            self.active_output_id = Some(new_id);
        } else {
            self.device_manager.write().unwrap().select_output_device(device_id)?;
        }
        Ok(())
    }

    fn build_input_stream(&self, device_id: Option<&str>) -> Result<(Stream, String), AudioError> {
        let mgr = self.device_manager.read().unwrap();
        let dev = mgr.get_cpal_input_device(device_id)?;
        let dev_id_or_name = device_id.map(String::from).or_else(|| dev.name().ok()).unwrap_or_default();

        let supported_config = dev
            .default_input_config()
            .map_err(|e| AudioError::StreamError(format!("Failed to query default input config: {e:?}")))?;

        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        let dev_channels = stream_config.channels as usize;
        let dev_rate = stream_config.sample_rate.0;

        let processor = Arc::new(Mutex::new(CaptureProcessor::new(
            dev_rate,
            dev_channels,
            self.outgoing_tx.clone(),
            Some(self.speaking_tx.clone()),
            Arc::clone(&self.is_muted),
            Arc::clone(&self.audio_level_bits),
            Arc::clone(&self.is_speaking),
            Arc::clone(&self.vad_threshold_bits),
        )?));

        let err_fn = |err| {
            log::error!("Audio input stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let proc = Arc::clone(&processor);
                dev.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        if let Ok(mut p) = proc.lock() {
                            let _ = p.process_input_samples(data);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build f32 input stream: {e:?}")))?
            }

            SampleFormat::I16 => {
                let proc = Arc::clone(&processor);
                dev.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| i16_to_f32(s)).collect();
                        if let Ok(mut p) = proc.lock() {
                            let _ = p.process_input_samples(&f32_data);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build i16 input stream: {e:?}")))?
            }

            SampleFormat::U16 => {
                let proc = Arc::clone(&processor);
                dev.build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        let f32_data: Vec<f32> = data.iter().map(|&s| u16_to_f32(s)).collect();
                        if let Ok(mut p) = proc.lock() {
                            let _ = p.process_input_samples(&f32_data);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamError(format!("Failed to build u16 input stream: {e:?}")))?
            }

            other => {
                return Err(AudioError::UnsupportedFormat(format!(
                    "Unsupported input sample format: {other:?}"
                )))
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::StreamError(format!("Failed to play input stream: {e:?}")))?;

        Ok((stream, dev_id_or_name))
    }

    fn build_output_stream(&self, device_id: Option<&str>) -> Result<(Stream, String), AudioError> {
        let mgr = self.device_manager.read().unwrap();
        let dev = mgr.get_cpal_output_device(device_id)?;
        let dev_id_or_name = device_id.map(String::from).or_else(|| dev.name().ok()).unwrap_or_default();

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
        let mut resampler = LinearResampler::new(OPUS_SAMPLE_RATE, dev_rate, OPUS_CHANNELS);

        let mut fill_samples = move |out_samples: &mut [f32]| {
            let deafened = is_deafened.load(Ordering::Relaxed);

            // Refill output buffer with stateful resampling to avoid phase clicks
            while output_buffer.len() < out_samples.len() {
                let frame_48k_stereo = {
                    let mut m = mixer.lock().unwrap();
                    m.pull_mixed_frame(deafened)
                };

                let resampled = resampler.resample(&frame_48k_stereo);
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

        Ok((stream, dev_id_or_name))
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}
