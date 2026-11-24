use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

use crate::media::AudioError;
use crate::media::capture::{CaptureProcessor, SpeakingEvent};
use crate::media::codec::{
    FRAME_DURATION_MS, OPUS_CHANNELS, OPUS_SAMPLE_RATE, STEREO_FRAME_SAMPLES,
};
use crate::media::devices::MediaDeviceManager;
use crate::media::mixer::{AudioMixer, soft_clip};
use crate::media::resample::{
    LinearResampler, convert_stereo_to_channels, f32_to_i16, f32_to_u16, i16_to_f32, u16_to_f32,
};

const CAPTURE_RING_MS: usize = 500;
const PLAYBACK_RING_MS: usize = 250;
const PLAYBACK_PREBUFFER_FRAMES: usize = 3;
const CAPTURE_WORKER_POLL: Duration = Duration::from_millis(2);
const CALLBACK_SCRATCH_SAMPLES: usize = 4096;
const MAX_AUDIO_GAIN: f32 = 2.0;

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

    // Local user audio state.
    is_muted: Arc<AtomicBool>,
    is_deafened: Arc<AtomicBool>,
    audio_level_bits: Arc<AtomicU32>,
    input_gain_bits: Arc<AtomicU32>,
    output_gain_bits: Arc<AtomicU32>,
    is_speaking: Arc<AtomicBool>,
    vad_threshold_bits: Arc<AtomicU32>,

    // Active streams and their non-blocking workers. cpal callbacks only touch
    // atomic SPSC rings; resampling, Opus, and mixer locks live on worker threads.
    input_stream: Option<Stream>,
    output_stream: Option<Stream>,
    capture_ring: Option<Arc<AudioSampleRing>>,
    playback_ring: Option<Arc<AudioSampleRing>>,
    capture_stop: Option<Arc<AtomicBool>>,
    capture_worker: Option<JoinHandle<()>>,
    playback_worker: Option<PlaybackWorker>,
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
            input_gain_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            output_gain_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            is_speaking: Arc::new(AtomicBool::new(false)),
            vad_threshold_bits: Arc::new(AtomicU32::new(0.01f32.to_bits())),
            input_stream: None,
            output_stream: None,
            capture_ring: None,
            playback_ring: None,
            capture_stop: None,
            capture_worker: None,
            playback_worker: None,
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
    /// Decoding happens on the audio-engine thread, never on a cpal callback.
    pub fn receive_opus_packet(
        &self,
        ssrc: u32,
        sequence: u16,
        timestamp: u32,
        opus_data: &[u8],
    ) -> Result<(), AudioError> {
        let mut mixer = self
            .mixer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        mixer.feed_opus_packet(ssrc, sequence, timestamp, opus_data)
    }

    /// Returns whether the user is locally muted.
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Relaxed)
    }

    /// Sets local microphone mute state.
    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted.store(muted, Ordering::Relaxed);
        if muted {
            self.audio_level_bits
                .store(0f32.to_bits(), Ordering::Relaxed);
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

    /// Shared real RMS handle for UI/meter polling without fabricating a value.
    pub fn audio_level_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.audio_level_bits)
    }

    pub fn input_gain_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.input_gain_bits)
    }

    pub fn output_gain_handle(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.output_gain_bits)
    }

    /// Sets microphone gain in the inclusive range 0.0..=2.0.
    /// Non-finite values reset to unity; the capture limiter remains active.
    pub fn set_input_gain(&self, gain: f32) {
        self.input_gain_bits
            .store(normalize_gain(gain).to_bits(), Ordering::Relaxed);
    }

    pub fn input_gain(&self) -> f32 {
        f32::from_bits(self.input_gain_bits.load(Ordering::Relaxed))
    }

    /// Sets remote playback gain in the inclusive range 0.0..=2.0.
    /// Non-finite values reset to unity; the output limiter remains active.
    pub fn set_output_gain(&self, gain: f32) {
        self.output_gain_bits
            .store(normalize_gain(gain).to_bits(), Ordering::Relaxed);
    }

    pub fn output_gain(&self) -> f32 {
        f32::from_bits(self.output_gain_bits.load(Ordering::Relaxed))
    }

    /// Returns true if local voice activity is detected.
    pub fn is_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::Relaxed)
    }

    /// Configures the Voice Activity Detection (VAD) sensitivity threshold (default 0.01).
    pub fn set_vad_threshold(&self, threshold: f32) {
        let threshold = if threshold.is_finite() {
            threshold.max(0.0)
        } else {
            0.01
        };
        self.vad_threshold_bits
            .store(threshold.to_bits(), Ordering::Relaxed);
        let mut mixer = self
            .mixer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        mixer.set_vad_threshold(threshold);
    }

    pub fn vad_threshold(&self) -> f32 {
        f32::from_bits(self.vad_threshold_bits.load(Ordering::Relaxed))
    }

    /// Starts an active audio session with hardware capture and playback.
    /// If output or input fails, cleans up any partially opened stream and leaves state consistent.
    pub fn start_session(&mut self) -> Result<(), AudioError> {
        if self.is_session_active {
            return Ok(());
        }

        let (selected_input, selected_output) = {
            let manager = self.device_manager.read().unwrap();
            (
                manager.selected_input_id().map(String::from),
                manager.selected_output_id().map(String::from),
            )
        };

        let mut input = self.build_input_stream(selected_input.as_deref())?;
        let mut output = match self.build_output_stream(selected_output.as_deref()) {
            Ok(output) => output,
            Err(err) => {
                drop(input);
                self.stop_session();
                return Err(err);
            }
        };

        let capture_worker = match spawn_capture_worker(
            input
                .processor
                .take()
                .expect("capture processor must be present"),
            Arc::clone(&input.ring),
            Arc::clone(&input.stop),
            input.channels,
        ) {
            Ok(worker) => worker,
            Err(err) => {
                output.worker.stop();
                drop(output.stream);
                drop(input);
                self.stop_session();
                return Err(AudioError::StreamError(format!(
                    "Failed to start capture worker: {err}"
                )));
            }
        };

        self.input_stream = Some(input.stream);
        self.active_input_id = Some(input.id);
        self.capture_ring = Some(input.ring);
        self.capture_stop = Some(input.stop);
        self.capture_worker = Some(capture_worker);

        self.output_stream = Some(output.stream);
        self.active_output_id = Some(output.id);
        self.playback_ring = Some(output.ring);
        self.playback_worker = Some(output.worker);
        self.is_session_active = true;
        Ok(())
    }

    /// Stops the active audio session and releases hardware audio streams.
    pub fn stop_session(&mut self) {
        // Drop streams first so callbacks stop writing to their rings.
        self.input_stream = None;
        self.output_stream = None;
        self.stop_capture_worker();
        self.stop_playback_worker();
        self.capture_ring = None;
        self.playback_ring = None;
        self.active_input_id = None;
        self.active_output_id = None;
        self.is_session_active = false;
        self.is_speaking.store(false, Ordering::Relaxed);
        self.audio_level_bits
            .store(0f32.to_bits(), Ordering::Relaxed);
        self.mixer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
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
            let mut input = self.build_input_stream(device_id)?;
            let worker = spawn_capture_worker(
                input
                    .processor
                    .take()
                    .expect("capture processor must be present"),
                Arc::clone(&input.ring),
                Arc::clone(&input.stop),
                input.channels,
            )
            .map_err(|err| {
                AudioError::StreamError(format!("Failed to start capture worker: {err}"))
            })?;
            if let Err(err) = self
                .device_manager
                .write()
                .unwrap()
                .select_input_device(device_id)
            {
                input.stop.store(true, Ordering::Release);
                let _ = worker.join();
                return Err(err);
            }

            self.input_stream = None;
            self.stop_capture_worker();
            self.input_stream = Some(input.stream);
            self.active_input_id = Some(input.id);
            self.capture_ring = Some(input.ring);
            self.capture_stop = Some(input.stop);
            self.capture_worker = Some(worker);
        } else {
            self.device_manager
                .write()
                .unwrap()
                .select_input_device(device_id)?;
        }
        Ok(())
    }

    /// Switches the active playback device dynamically.
    /// Builds and starts the new stream first; if it fails, the previous stream and selection remain intact.
    pub fn switch_output_device(&mut self, device_id: Option<&str>) -> Result<(), AudioError> {
        if self.is_session_active {
            let mut output = self.build_output_stream(device_id)?;
            if let Err(err) = self
                .device_manager
                .write()
                .unwrap()
                .select_output_device(device_id)
            {
                output.worker.stop();
                return Err(err);
            }

            self.stop_playback_worker();
            self.output_stream = None;
            self.output_stream = Some(output.stream);
            self.active_output_id = Some(output.id);
            self.playback_ring = Some(output.ring);
            self.playback_worker = Some(output.worker);
        } else {
            self.device_manager
                .write()
                .unwrap()
                .select_output_device(device_id)?;
        }
        Ok(())
    }

    fn build_input_stream(
        &self,
        device_id: Option<&str>,
    ) -> Result<InputStreamResources, AudioError> {
        let manager = self.device_manager.read().unwrap();
        let device = manager.get_cpal_input_device(device_id)?;
        let device_name = device_id
            .map(String::from)
            .or_else(|| device.name().ok())
            .unwrap_or_default();

        let supported_config = device.default_input_config().map_err(|err| {
            AudioError::StreamError(format!("Failed to query default input config: {err:?}"))
        })?;
        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        validate_stream_config(stream_config.sample_rate.0, stream_config.channels)?;
        let device_channels = stream_config.channels as usize;
        let device_rate = stream_config.sample_rate.0;

        let ring = Arc::new(AudioSampleRing::new(ring_capacity(
            device_rate,
            device_channels,
            CAPTURE_RING_MS,
        )));
        let stop = Arc::new(AtomicBool::new(false));
        let processor = CaptureProcessor::new(
            device_rate,
            device_channels,
            self.outgoing_tx.clone(),
            Some(self.speaking_tx.clone()),
            Arc::clone(&self.is_muted),
            Arc::clone(&self.audio_level_bits),
            Arc::clone(&self.input_gain_bits),
            Arc::clone(&self.is_speaking),
            Arc::clone(&self.vad_threshold_bits),
        )?;

        log::info!(
            "[AudioEngine] Input '{}': {device_rate} Hz, {device_channels} channel(s), {sample_format:?} -> Opus 48000 Hz stereo",
            device_name
        );

        let err_fn = |err| log::error!("Audio input stream error: {err}");
        let stream = match sample_format {
            SampleFormat::F32 => {
                let callback_ring = Arc::clone(&ring);
                device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[f32], _| {
                            callback_ring.push(data);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|err| {
                        AudioError::StreamError(format!(
                            "Failed to build f32 input stream: {err:?}"
                        ))
                    })?
            }
            SampleFormat::I16 => {
                let callback_ring = Arc::clone(&ring);
                let callback_chunk_samples = aligned_scratch_samples(device_channels);
                let mut scratch = [0.0f32; CALLBACK_SCRATCH_SAMPLES];
                device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[i16], _| {
                            push_i16_input(
                                &callback_ring,
                                data,
                                &mut scratch,
                                callback_chunk_samples,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|err| {
                        AudioError::StreamError(format!(
                            "Failed to build i16 input stream: {err:?}"
                        ))
                    })?
            }
            SampleFormat::U16 => {
                let callback_ring = Arc::clone(&ring);
                let callback_chunk_samples = aligned_scratch_samples(device_channels);
                let mut scratch = [0.0f32; CALLBACK_SCRATCH_SAMPLES];
                device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[u16], _| {
                            push_u16_input(
                                &callback_ring,
                                data,
                                &mut scratch,
                                callback_chunk_samples,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|err| {
                        AudioError::StreamError(format!(
                            "Failed to build u16 input stream: {err:?}"
                        ))
                    })?
            }
            other => {
                return Err(AudioError::UnsupportedFormat(format!(
                    "Unsupported input sample format: {other:?}"
                )));
            }
        };

        stream.play().map_err(|err| {
            AudioError::StreamError(format!("Failed to play input stream: {err:?}"))
        })?;

        Ok(InputStreamResources {
            stream,
            id: device_name,
            processor: Some(processor),
            ring,
            stop,
            channels: device_channels,
        })
    }

    fn build_output_stream(
        &self,
        device_id: Option<&str>,
    ) -> Result<OutputStreamResources, AudioError> {
        let manager = self.device_manager.read().unwrap();
        let device = manager.get_cpal_output_device(device_id)?;
        let device_name = device_id
            .map(String::from)
            .or_else(|| device.name().ok())
            .unwrap_or_default();

        let supported_config = device.default_output_config().map_err(|err| {
            AudioError::StreamError(format!("Failed to query default output config: {err:?}"))
        })?;
        let sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        validate_stream_config(stream_config.sample_rate.0, stream_config.channels)?;
        let device_channels = stream_config.channels as usize;
        let device_rate = stream_config.sample_rate.0;

        let ring = Arc::new(AudioSampleRing::new(ring_capacity(
            device_rate,
            device_channels,
            PLAYBACK_RING_MS,
        )));
        let is_deafened = Arc::clone(&self.is_deafened);

        log::info!(
            "[AudioEngine] Output '{}': Opus 48000 Hz stereo -> {device_rate} Hz, {device_channels} channel(s), {sample_format:?}",
            device_name
        );

        let err_fn = |err| log::error!("Audio output stream error: {err}");
        let stream = match sample_format {
            SampleFormat::F32 => {
                let callback_ring = Arc::clone(&ring);
                let callback_deafened = Arc::clone(&is_deafened);
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [f32], _| {
                            fill_f32_output(data, &callback_ring, &callback_deafened);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|err| {
                        AudioError::StreamError(format!(
                            "Failed to build f32 output stream: {err:?}"
                        ))
                    })?
            }
            SampleFormat::I16 => {
                let callback_ring = Arc::clone(&ring);
                let callback_deafened = Arc::clone(&is_deafened);
                let callback_chunk_samples = aligned_scratch_samples(device_channels);
                let mut scratch = [0.0f32; CALLBACK_SCRATCH_SAMPLES];
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [i16], _| {
                            fill_i16_output(
                                data,
                                &callback_ring,
                                &callback_deafened,
                                &mut scratch,
                                callback_chunk_samples,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|err| {
                        AudioError::StreamError(format!(
                            "Failed to build i16 output stream: {err:?}"
                        ))
                    })?
            }
            SampleFormat::U16 => {
                let callback_ring = Arc::clone(&ring);
                let callback_deafened = Arc::clone(&is_deafened);
                let callback_chunk_samples = aligned_scratch_samples(device_channels);
                let mut scratch = [0.0f32; CALLBACK_SCRATCH_SAMPLES];
                device
                    .build_output_stream(
                        &stream_config,
                        move |data: &mut [u16], _| {
                            fill_u16_output(
                                data,
                                &callback_ring,
                                &callback_deafened,
                                &mut scratch,
                                callback_chunk_samples,
                            );
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|err| {
                        AudioError::StreamError(format!(
                            "Failed to build u16 output stream: {err:?}"
                        ))
                    })?
            }
            other => {
                return Err(AudioError::UnsupportedFormat(format!(
                    "Unsupported output sample format: {other:?}"
                )));
            }
        };

        let mut worker = spawn_playback_worker(
            Arc::clone(&self.mixer),
            is_deafened,
            Arc::clone(&self.output_gain_bits),
            device_rate,
            device_channels,
            Arc::clone(&ring),
        )?;
        if let Err(err) = stream.play() {
            worker.stop();
            return Err(AudioError::StreamError(format!(
                "Failed to play output stream: {err:?}"
            )));
        }

        Ok(OutputStreamResources {
            stream,
            id: device_name,
            ring,
            worker,
        })
    }

    fn stop_capture_worker(&mut self) {
        if let Some(stop) = self.capture_stop.take() {
            stop.store(true, Ordering::Release);
        }
        if let Some(worker) = self.capture_worker.take() {
            let _ = worker.join();
        }
    }

    fn stop_playback_worker(&mut self) {
        if let Some(mut worker) = self.playback_worker.take() {
            worker.stop();
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

struct InputStreamResources {
    stream: Stream,
    id: String,
    processor: Option<CaptureProcessor>,
    ring: Arc<AudioSampleRing>,
    stop: Arc<AtomicBool>,
    channels: usize,
}

struct OutputStreamResources {
    stream: Stream,
    id: String,
    ring: Arc<AudioSampleRing>,
    worker: PlaybackWorker,
}

struct PlaybackWorker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for PlaybackWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

impl PlaybackWorker {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn spawn_capture_worker(
    mut processor: CaptureProcessor,
    ring: Arc<AudioSampleRing>,
    stop: Arc<AtomicBool>,
    device_channels: usize,
) -> Result<JoinHandle<()>, std::io::Error> {
    thread::Builder::new()
        .name("discord-audio-capture".into())
        .spawn(move || {
            let scratch_len = aligned_scratch_samples(device_channels);
            let mut scratch = vec![0.0f32; scratch_len];
            while !stop.load(Ordering::Acquire) {
                let count = ring.pop(&mut scratch);
                if count == 0 {
                    thread::sleep(CAPTURE_WORKER_POLL);
                    continue;
                }
                if let Err(err) = processor.process_input_samples(&scratch[..count]) {
                    log::error!("Audio capture processing error: {err}");
                }
            }
        })
}

struct PlaybackRenderer {
    mixer: Arc<Mutex<AudioMixer>>,
    is_deafened: Arc<AtomicBool>,
    output_gain: Arc<AtomicU32>,
    resampler: LinearResampler,
    output_channels: usize,
    ring: Arc<AudioSampleRing>,
}

impl PlaybackRenderer {
    fn new(
        mixer: Arc<Mutex<AudioMixer>>,
        is_deafened: Arc<AtomicBool>,
        output_gain: Arc<AtomicU32>,
        device_rate: u32,
        output_channels: usize,
        ring: Arc<AudioSampleRing>,
    ) -> Self {
        Self {
            mixer,
            is_deafened,
            output_gain,
            resampler: LinearResampler::new(OPUS_SAMPLE_RATE, device_rate, OPUS_CHANNELS),
            output_channels,
            ring,
        }
    }

    fn render_frame(&mut self) {
        let mixed = {
            let deafened = self.is_deafened.load(Ordering::Relaxed);
            let mut mixer = self
                .mixer
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            mixer.pull_mixed_frame(deafened)
        };
        self.render_pcm(mixed);
    }

    fn render_pcm(&mut self, mut mixed: Vec<f32>) {
        let gain = f32::from_bits(self.output_gain.load(Ordering::Relaxed));
        let gain = if gain.is_finite() {
            gain.clamp(0.0, 2.0)
        } else {
            1.0
        };
        for sample in &mut mixed {
            *sample = soft_clip(*sample * gain);
        }
        let resampled = self.resampler.resample(&mixed);
        let converted = convert_stereo_to_channels(&resampled, self.output_channels);

        // Never split a device frame across callbacks. If the consumer stalled,
        // drop this whole 20 ms frame rather than writing a partial/corrupt frame.
        if converted.len() > self.ring.available() {
            return;
        }
        let written = self.ring.push(&converted);
        debug_assert_eq!(written, converted.len());
    }

    fn prebuffer(&mut self) {
        // Prebuffer device silence without consuming logical RTP playout slots.
        for _ in 0..PLAYBACK_PREBUFFER_FRAMES {
            self.render_pcm(vec![0.0; STEREO_FRAME_SAMPLES]);
        }
    }
}

fn spawn_playback_worker(
    mixer: Arc<Mutex<AudioMixer>>,
    is_deafened: Arc<AtomicBool>,
    output_gain: Arc<AtomicU32>,
    device_rate: u32,
    output_channels: usize,
    ring: Arc<AudioSampleRing>,
) -> Result<PlaybackWorker, AudioError> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let mut renderer = PlaybackRenderer::new(
        mixer,
        is_deafened,
        output_gain,
        device_rate,
        output_channels,
        ring,
    );
    renderer.prebuffer();

    let thread = thread::Builder::new()
        .name("discord-audio-playback".into())
        .spawn(move || {
            let interval = Duration::from_millis(u64::from(FRAME_DURATION_MS));
            let mut next_frame = Instant::now();
            while !worker_stop.load(Ordering::Acquire) {
                let now = Instant::now();
                if now < next_frame {
                    thread::sleep(next_frame - now);
                    continue;
                }

                renderer.render_frame();
                next_frame += interval;
                // Do not burst stale frames after a scheduler stall.
                if Instant::now().saturating_duration_since(next_frame) > interval {
                    next_frame = Instant::now();
                }
            }
        })
        .map_err(|err| {
            AudioError::StreamError(format!("Failed to start playback worker: {err}"))
        })?;

    Ok(PlaybackWorker {
        stop,
        thread: Some(thread),
    })
}

fn normalize_gain(gain: f32) -> f32 {
    if gain.is_finite() {
        gain.clamp(0.0, MAX_AUDIO_GAIN)
    } else {
        1.0
    }
}

fn validate_stream_config(sample_rate: u32, channels: u16) -> Result<(), AudioError> {
    if sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate(sample_rate));
    }
    if channels == 0 || channels as usize > CALLBACK_SCRATCH_SAMPLES {
        return Err(AudioError::InvalidChannels(channels));
    }
    Ok(())
}

fn ring_capacity(sample_rate: u32, channels: usize, duration_ms: usize) -> usize {
    let samples = sample_rate as usize * channels * duration_ms / 1000;
    samples.max(1).next_power_of_two()
}

fn fill_f32_output(output: &mut [f32], ring: &AudioSampleRing, is_deafened: &AtomicBool) {
    if is_deafened.load(Ordering::Relaxed) {
        ring.clear();
        output.fill(0.0);
        return;
    }

    let count = ring.pop(output);
    output[count..].fill(0.0);
}

fn aligned_scratch_samples(device_channels: usize) -> usize {
    let channels = device_channels.max(1);
    (CALLBACK_SCRATCH_SAMPLES / channels).max(1) * channels
}

fn push_i16_input(
    ring: &AudioSampleRing,
    data: &[i16],
    scratch: &mut [f32; CALLBACK_SCRATCH_SAMPLES],
    chunk_samples: usize,
) {
    for chunk in data.chunks(chunk_samples) {
        for (destination, &source) in scratch.iter_mut().zip(chunk) {
            *destination = i16_to_f32(source);
        }
        ring.push(&scratch[..chunk.len()]);
    }
}

fn push_u16_input(
    ring: &AudioSampleRing,
    data: &[u16],
    scratch: &mut [f32; CALLBACK_SCRATCH_SAMPLES],
    chunk_samples: usize,
) {
    for chunk in data.chunks(chunk_samples) {
        for (destination, &source) in scratch.iter_mut().zip(chunk) {
            *destination = u16_to_f32(source);
        }
        ring.push(&scratch[..chunk.len()]);
    }
}

fn fill_i16_output(
    output: &mut [i16],
    ring: &AudioSampleRing,
    is_deafened: &AtomicBool,
    scratch: &mut [f32; CALLBACK_SCRATCH_SAMPLES],
    chunk_samples: usize,
) {
    for chunk in output.chunks_mut(chunk_samples) {
        let len = chunk.len();
        fill_f32_output(&mut scratch[..len], ring, is_deafened);
        for (destination, &source) in chunk.iter_mut().zip(&scratch[..len]) {
            *destination = f32_to_i16(source);
        }
    }
}

fn fill_u16_output(
    output: &mut [u16],
    ring: &AudioSampleRing,
    is_deafened: &AtomicBool,
    scratch: &mut [f32; CALLBACK_SCRATCH_SAMPLES],
    chunk_samples: usize,
) {
    for chunk in output.chunks_mut(chunk_samples) {
        let len = chunk.len();
        fill_f32_output(&mut scratch[..len], ring, is_deafened);
        for (destination, &source) in chunk.iter_mut().zip(&scratch[..len]) {
            *destination = f32_to_u16(source);
        }
    }
}

/// Single-producer/single-consumer sample ring used to cross the cpal callback boundary.
/// Atomic f32 bits avoid unsafe shared storage and every operation is non-blocking.
#[derive(Debug)]
struct AudioSampleRing {
    slots: Vec<AtomicU32>,
    mask: usize,
    read: AtomicUsize,
    write: AtomicUsize,
}

impl AudioSampleRing {
    fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1).next_power_of_two();
        Self {
            slots: (0..capacity).map(|_| AtomicU32::new(0)).collect(),
            mask: capacity - 1,
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }

    fn capacity(&self) -> usize {
        self.slots.len()
    }

    fn len(&self) -> usize {
        // Snapshot the consumer cursor first. The producer can only advance
        // `write`, so this ordering can never observe read > write concurrently.
        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    fn available(&self) -> usize {
        self.capacity() - self.len()
    }

    fn push(&self, samples: &[f32]) -> usize {
        let read = self.read.load(Ordering::Acquire);
        let write = self.write.load(Ordering::Relaxed);
        let count = samples
            .len()
            .min(self.capacity() - write.wrapping_sub(read));
        for (offset, sample) in samples[..count].iter().enumerate() {
            self.slots[(write + offset) & self.mask].store(sample.to_bits(), Ordering::Relaxed);
        }
        self.write.store(write + count, Ordering::Release);
        count
    }

    fn pop(&self, output: &mut [f32]) -> usize {
        let write = self.write.load(Ordering::Acquire);
        let read = self.read.load(Ordering::Relaxed);
        let count = output.len().min(write.wrapping_sub(read));
        for (offset, slot) in output[..count].iter_mut().enumerate() {
            *slot = f32::from_bits(self.slots[(read + offset) & self.mask].load(Ordering::Relaxed));
        }
        self.read.store(read + count, Ordering::Release);
        count
    }

    /// Consumer-only reset, used to discard stale audio immediately when deafened.
    fn clear(&self) {
        let write = self.write.load(Ordering::Acquire);
        self.read.store(write, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::codec::OpusAudioEncoder;
    use std::sync::atomic::AtomicU64;

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

        fn advance(&self, duration: Duration) {
            self.offset_micros
                .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
        }
    }

    fn timed_mixer() -> (Arc<Mutex<AudioMixer>>, Arc<ManualClock>) {
        let clock = ManualClock::new();
        let mixer = AudioMixer::with_clock({
            let clock = Arc::clone(&clock);
            move || clock.base + Duration::from_micros(clock.offset_micros.load(Ordering::Relaxed))
        });
        (Arc::new(Mutex::new(mixer)), clock)
    }

    fn sine_packet(frequency: f32, amplitude: f32) -> Vec<u8> {
        let frame: Vec<f32> = (0..crate::media::codec::SAMPLES_PER_CHANNEL)
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

    #[test]
    fn sample_ring_wraps_without_overwriting_unread_samples() {
        let ring = AudioSampleRing::new(4);
        assert_eq!(ring.push(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0]), 4);
        assert_eq!(ring.len(), 4);
        assert_eq!(ring.available(), 0);

        let mut output = [0.0; 2];
        assert_eq!(ring.pop(&mut output), 2);
        assert_eq!(output, [0.0, 1.0]);
        assert_eq!(ring.push(&[4.0, 5.0]), 2);

        let mut output = [0.0; 4];
        assert_eq!(ring.pop(&mut output), 4);
        assert_eq!(output, [2.0, 3.0, 4.0, 5.0]);
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn sample_ring_len_stays_consistent_during_concurrent_transfer() {
        let ring = Arc::new(AudioSampleRing::new(1024));
        let done = Arc::new(AtomicBool::new(false));
        let consumed = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|scope| {
            let producer_ring = Arc::clone(&ring);
            let producer_done = Arc::clone(&done);
            scope.spawn(move || {
                for chunk in 0..500 {
                    let samples = [chunk as f32; 64];
                    producer_ring.push(&samples);
                    thread::yield_now();
                }
                producer_done.store(true, Ordering::Release);
            });

            let consumer_ring = Arc::clone(&ring);
            let consumer_done = Arc::clone(&done);
            let consumer_count = Arc::clone(&consumed);
            scope.spawn(move || {
                let mut output = [0.0f32; 64];
                while !consumer_done.load(Ordering::Acquire) || consumer_ring.len() != 0 {
                    let count = consumer_ring.pop(&mut output);
                    consumer_count.fetch_add(count, Ordering::Relaxed);
                    assert!(consumer_ring.len() <= consumer_ring.capacity());
                    thread::yield_now();
                }
            });
        });

        assert!(consumed.load(Ordering::Relaxed) > 0);
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn integer_callbacks_handle_chunks_larger_than_stack_scratch() {
        let sample_count = CALLBACK_SCRATCH_SAMPLES * 2 + 123;
        let mut scratch = [0.0f32; CALLBACK_SCRATCH_SAMPLES];
        let deafened = AtomicBool::new(false);

        let i16_ring = AudioSampleRing::new(16_384);
        push_i16_input(&i16_ring, &vec![i16::MAX; sample_count], &mut scratch, 4096);
        let mut i16_samples = vec![0.0; sample_count];
        assert_eq!(i16_ring.pop(&mut i16_samples), sample_count);
        assert!(
            i16_samples
                .iter()
                .all(|sample| (*sample - 32_767.0 / 32_768.0).abs() < 1e-6)
        );

        let u16_ring = AudioSampleRing::new(16_384);
        push_u16_input(&u16_ring, &vec![u16::MAX; sample_count], &mut scratch, 4096);
        let mut u16_samples = vec![0.0; sample_count];
        assert_eq!(u16_ring.pop(&mut u16_samples), sample_count);
        assert!(
            u16_samples
                .iter()
                .all(|sample| (*sample - 32_767.0 / 32_768.0).abs() < 1e-6)
        );

        let output_ring = AudioSampleRing::new(16_384);
        output_ring.push(&vec![0.5; sample_count]);
        let mut i16_output = vec![0i16; sample_count];
        fill_i16_output(&mut i16_output, &output_ring, &deafened, &mut scratch, 4096);
        assert!(i16_output.iter().all(|sample| *sample == 16_384));

        output_ring.push(&vec![0.5; sample_count]);
        let mut u16_output = vec![0u16; sample_count];
        fill_u16_output(&mut u16_output, &output_ring, &deafened, &mut scratch, 4096);
        assert!(u16_output.iter().all(|sample| *sample == 49_152));
        assert_eq!(aligned_scratch_samples(3), 4095);
    }

    #[test]
    fn output_callback_fills_underrun_with_silence() {
        let ring = AudioSampleRing::new(8);
        ring.push(&[0.25; 4]);
        let deafened = AtomicBool::new(false);
        let mut output = [0.5; 6];
        fill_f32_output(&mut output, &ring, &deafened);
        assert_eq!(output, [0.25, 0.25, 0.25, 0.25, 0.0, 0.0]);
    }

    #[test]
    fn deafened_output_callback_discards_already_buffered_audio() {
        let ring = AudioSampleRing::new(8);
        ring.push(&[0.75; 6]);
        let deafened = AtomicBool::new(true);
        let mut output = [0.5; 3];
        fill_f32_output(&mut output, &ring, &deafened);
        assert_eq!(output, [0.0; 3]);
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn playback_renderer_resamples_48k_to_44100_without_drift() {
        let (mixer, clock) = timed_mixer();
        let packet = sine_packet(440.0, 0.1);
        for sequence in 1..=100u16 {
            mixer
                .lock()
                .unwrap()
                .feed_opus_packet(1, sequence, u32::from(sequence) * 960, &packet)
                .unwrap();
        }

        let ring = Arc::new(AudioSampleRing::new(131_072));
        let mut renderer = PlaybackRenderer::new(
            Arc::clone(&mixer),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU32::new(1.0f32.to_bits())),
            44_100,
            2,
            Arc::clone(&ring),
        );
        for _ in 0..50 {
            clock.advance(Duration::from_millis(20));
            renderer.render_frame();
        }

        let expected = 44_100 * 2; // One second of interleaved stereo at 44.1 kHz.
        assert!(
            (ring.len() as i64 - expected as i64).abs() <= 4,
            "got {} samples",
            ring.len()
        );
    }

    #[test]
    fn playback_renderer_converts_to_mono_device_channels() {
        let (mixer, clock) = timed_mixer();
        let packet = sine_packet(440.0, 0.1);
        for sequence in 1..=10u16 {
            mixer
                .lock()
                .unwrap()
                .feed_opus_packet(2, sequence, u32::from(sequence) * 960, &packet)
                .unwrap();
        }

        let ring = Arc::new(AudioSampleRing::new(16_384));
        let mut renderer = PlaybackRenderer::new(
            mixer,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU32::new(1.0f32.to_bits())),
            48_000,
            1,
            Arc::clone(&ring),
        );
        clock.advance(Duration::from_millis(20));
        renderer.render_frame();
        assert_eq!(ring.len(), 960);
    }

    #[test]
    fn output_gain_scales_low_level_audio_without_clipping() {
        let packet = sine_packet(440.0, 0.05);
        let mut levels = Vec::new();
        for gain in [1.0f32, 2.0] {
            let (mixer, clock) = timed_mixer();
            mixer
                .lock()
                .unwrap()
                .feed_opus_packet(7, 1, 960, &packet)
                .unwrap();
            let ring = Arc::new(AudioSampleRing::new(4_096));
            let mut renderer = PlaybackRenderer::new(
                mixer,
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicU32::new(gain.to_bits())),
                48_000,
                2,
                Arc::clone(&ring),
            );
            clock.advance(Duration::from_millis(20));
            renderer.render_frame();
            let mut output = vec![0.0; ring.len()];
            ring.pop(&mut output);
            levels.push(
                (output.iter().map(|sample| sample * sample).sum::<f32>() / output.len() as f32)
                    .sqrt(),
            );
            assert!(output.iter().all(|sample| sample.abs() <= 1.0));
        }
        assert!((levels[0] * 2.0 - levels[1]).abs() < 0.02);
    }

    #[test]
    fn non_finite_output_gain_handle_falls_back_to_unity() {
        let packet = sine_packet(440.0, 0.05);
        let mut levels = Vec::new();
        for gain in [1.0f32, f32::NAN, f32::INFINITY] {
            let (mixer, clock) = timed_mixer();
            mixer
                .lock()
                .unwrap()
                .feed_opus_packet(8, 1, 960, &packet)
                .unwrap();
            let ring = Arc::new(AudioSampleRing::new(4_096));
            let mut renderer = PlaybackRenderer::new(
                mixer,
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicU32::new(gain.to_bits())),
                48_000,
                2,
                Arc::clone(&ring),
            );
            clock.advance(Duration::from_millis(20));
            renderer.render_frame();
            let mut output = vec![0.0; ring.len()];
            ring.pop(&mut output);
            levels.push(
                (output.iter().map(|sample| sample * sample).sum::<f32>() / output.len() as f32)
                    .sqrt(),
            );
        }
        assert!((levels[0] - levels[1]).abs() < 1e-6);
        assert!((levels[0] - levels[2]).abs() < 1e-6);
    }

    #[test]
    fn engine_forwards_rtp_metadata_to_the_mixer() {
        let engine = AudioEngine::new();
        let packet = sine_packet(440.0, 0.1);
        engine
            .receive_opus_packet(77, 65_535, 0xFFFF_FF00, &packet)
            .unwrap();
        let stats = engine
            .mixer()
            .lock()
            .unwrap()
            .participant_stats(77)
            .unwrap();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.played, 0, "packet must not be decoded before playout");
        assert_eq!(stats.queue_len, 1);
    }

    #[test]
    fn engine_gain_api_is_bounded_and_defaults_to_unity() {
        let engine = AudioEngine::new();
        assert_eq!(engine.input_gain(), 1.0);
        assert_eq!(engine.output_gain(), 1.0);
        engine.set_input_gain(-1.0);
        engine.set_output_gain(3.0);
        assert_eq!(engine.input_gain(), 0.0);
        assert_eq!(engine.output_gain(), 2.0);
        engine.set_input_gain(f32::NAN);
        engine.set_output_gain(f32::INFINITY);
        assert_eq!(engine.input_gain(), 1.0);
        assert_eq!(engine.output_gain(), 1.0);
    }

    #[test]
    fn invalid_stream_configs_are_rejected() {
        assert!(matches!(
            validate_stream_config(0, 2),
            Err(AudioError::InvalidSampleRate(0))
        ));
        assert!(matches!(
            validate_stream_config(48_000, 0),
            Err(AudioError::InvalidChannels(0))
        ));
        assert!(matches!(
            validate_stream_config(48_000, CALLBACK_SCRATCH_SAMPLES as u16 + 1),
            Err(AudioError::InvalidChannels(_))
        ));
        assert!(validate_stream_config(44_100, 1).is_ok());
    }
}
