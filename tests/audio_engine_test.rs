use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

use discord_rs::media::*;

/// Helper to generate a 48 kHz stereo sine wave frame (20 ms = 1920 interleaved samples).
fn generate_sine_wave(freq_hz: f32, amplitude: f32) -> Vec<f32> {
    let mut pcm = Vec::with_capacity(STEREO_FRAME_SAMPLES);
    for i in 0..SAMPLES_PER_CHANNEL {
        let t = i as f32 / OPUS_SAMPLE_RATE as f32;
        let sample = (2.0 * PI * freq_hz * t).sin() * amplitude;
        pcm.push(sample); // Left
        pcm.push(sample); // Right
    }
    pcm
}

/// Helper to generate arbitrary sample rate & channel sine wave PCM.
fn generate_arbitrary_sine(sample_rate: u32, channels: usize, freq_hz: f32, duration_secs: f32) -> Vec<f32> {
    let frames = (sample_rate as f32 * duration_secs) as usize;
    let mut pcm = Vec::with_capacity(frames * channels);
    for i in 0..frames {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * PI * freq_hz * t).sin() * 0.5;
        for _ in 0..channels {
            pcm.push(sample);
        }
    }
    pcm
}

#[test]
fn test_opus_roundtrip_synthetic_signal() {
    let mut encoder = OpusAudioEncoder::new().expect("Failed to create Opus encoder");
    let mut decoder = OpusAudioDecoder::new().expect("Failed to create Opus decoder");

    // Generate 440 Hz test tone at 0.5 amplitude
    let original = generate_sine_wave(440.0, 0.5);
    assert_eq!(original.len(), STEREO_FRAME_SAMPLES);

    // Encode
    let opus_packet = encoder
        .encode_frame(&original)
        .expect("Failed to encode 20ms frame");
    assert!(!opus_packet.is_empty(), "Opus packet should not be empty");
    assert!(
        opus_packet.len() <= MAX_OPUS_PACKET_SIZE,
        "Opus packet exceeds maximum size"
    );

    // Decode
    let decoded = decoder
        .decode_frame(&opus_packet)
        .expect("Failed to decode Opus packet");
    assert_eq!(
        decoded.len(),
        STEREO_FRAME_SAMPLES,
        "Decoded frame must be exactly 1920 samples (20 ms at 48 kHz stereo)"
    );

    // Opus lookahead lag compensation
    let mut max_corr = 0.0f32;
    for lag in 0..400 {
        let mut num = 0.0f32;
        let mut den_orig = 0.0f32;
        let mut den_dec = 0.0f32;

        for i in 0..(original.len() - lag) {
            let o = original[i];
            let d = decoded[i + lag];
            num += o * d;
            den_orig += o * o;
            den_dec += d * d;
        }

        if den_orig > 0.0 && den_dec > 0.0 {
            let corr = num / (den_orig.sqrt() * den_dec.sqrt());
            if corr > max_corr {
                max_corr = corr;
            }
        }
    }

    assert!(
        max_corr > 0.90,
        "Opus round-trip peak cross-correlation {max_corr:.3} is too low (expected > 0.90)"
    );

    let orig_rms = (original.iter().map(|&x| x * x).sum::<f32>() / original.len() as f32).sqrt();
    let dec_rms = (decoded.iter().map(|&x| x * x).sum::<f32>() / decoded.len() as f32).sqrt();
    assert!(
        (orig_rms - dec_rms).abs() < 0.1,
        "RMS energy mismatch: orig {orig_rms:.3}, dec {dec_rms:.3}"
    );
}

#[test]
fn test_frame_size_and_timing_validation() {
    let mut encoder = OpusAudioEncoder::new().expect("Failed to create encoder");

    let too_short = vec![0.0f32; 100];
    let too_long = vec![0.0f32; 2000];

    assert!(matches!(
        encoder.encode_frame(&too_short),
        Err(AudioError::InvalidFrameSize {
            expected: STEREO_FRAME_SAMPLES,
            actual: 100
        })
    ));

    assert!(matches!(
        encoder.encode_frame(&too_long),
        Err(AudioError::InvalidFrameSize {
            expected: STEREO_FRAME_SAMPLES,
            actual: 2000
        })
    ));

    let valid_frame = vec![0.0f32; STEREO_FRAME_SAMPLES];
    assert!(encoder.encode_frame(&valid_frame).is_ok());
}

#[test]
fn test_stateful_resampler_chunked_vs_batch_fidelity() {
    // Audit Requirement 3: Reamostrar um seno 44.1 kHz -> 48 kHz em blocos pequenos e irregulares
    // deve dar o mesmo resultado que reamostrar o sinal inteiro, sem descontinuidades de fase.
    let full_signal_44k = generate_arbitrary_sine(44100, 2, 440.0, 0.5); // 0.5s of audio

    // 1. One-shot batch resampling
    let batch_resampled = resample_linear(&full_signal_44k, 44100, 48000, 2);

    // 2. Stateful chunked resampling with irregular chunk sizes
    let mut resampler = LinearResampler::new(44100, 48000, 2);
    let mut chunked_resampled = Vec::new();
    let chunk_sizes = [128 * 2, 256 * 2, 317 * 2, 441 * 2, 199 * 2, 512 * 2];
    let mut offset = 0;
    let mut chunk_idx = 0;

    while offset < full_signal_44k.len() {
        let size = chunk_sizes[chunk_idx % chunk_sizes.len()];
        let end = (offset + size).min(full_signal_44k.len());
        let chunk = &full_signal_44k[offset..end];
        let out_chunk = resampler.resample(chunk);
        chunked_resampled.extend(out_chunk);
        offset = end;
        chunk_idx += 1;
    }
    chunked_resampled.extend(resampler.flush());

    // Verify sample count matches exactly
    assert_eq!(
        chunked_resampled.len(),
        batch_resampled.len(),
        "Chunked resampling sample count must match batch count exactly"
    );

    // Verify waveform continuity: max point-wise difference between chunked and batch must be near zero
    let mut max_diff = 0.0f32;
    for (c, b) in chunked_resampled.iter().zip(batch_resampled.iter()) {
        let diff = (c - b).abs();
        if diff > max_diff {
            max_diff = diff;
        }
    }
    assert!(
        max_diff < 1e-5,
        "Max difference between chunked and batch resampling is {max_diff} (expected < 1e-5)"
    );
}

#[tokio::test]
async fn test_capture_processor_vad_hysteresis_and_silence_suppression() {
    // Audit Requirement 4 & 5:
    // Test real CaptureProcessor: silence suppression, speaking transitions, and 200 ms hysteresis.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel(100);
    let (spk_tx, mut spk_rx) = tokio::sync::mpsc::channel(20);

    let is_muted = Arc::new(AtomicBool::new(false));
    let audio_level_bits = Arc::new(AtomicU32::new(0f32.to_bits()));
    let is_speaking = Arc::new(AtomicBool::new(false));
    let vad_threshold_bits = Arc::new(AtomicU32::new(0.01f32.to_bits()));

    let mut processor = CaptureProcessor::new(
        48000,
        2,
        out_tx,
        Some(spk_tx),
        Arc::clone(&is_muted),
        Arc::clone(&audio_level_bits),
        Arc::clone(&is_speaking),
        Arc::clone(&vad_threshold_bits),
    )
    .expect("Create CaptureProcessor");

    // 1. SILENCE: feed 3 frames of total silence (0.0 amplitude)
    let silence_frame = vec![0.0f32; STEREO_FRAME_SAMPLES];
    for _ in 0..3 {
        let pkts = processor.process_input_samples(&silence_frame).unwrap();
        assert!(
            pkts.is_empty(),
            "Silence must NOT emit Opus packets (silence suppression)"
        );
    }
    assert!(!processor.is_speaking_active());
    assert!(!is_speaking.load(std::sync::atomic::Ordering::Relaxed));
    assert!(out_rx.try_recv().is_err(), "Outgoing channel must be empty during silence");
    assert!(spk_rx.try_recv().is_err(), "Speaking channel must not emit events during silence");

    // 2. SPEECH: feed 1 frame of active speech (> threshold)
    let speech_frame = generate_sine_wave(500.0, 0.6);
    let speech_pkts = processor.process_input_samples(&speech_frame).unwrap();
    assert_eq!(speech_pkts.len(), 1, "Active speech must produce 1 Opus packet");
    assert!(processor.is_speaking_active());
    assert!(is_speaking.load(std::sync::atomic::Ordering::Relaxed));

    // Verify SpeakingStarted transition received
    let event = spk_rx.try_recv().expect("SpeakingStarted event expected");
    assert_eq!(event, SpeakingEvent::SpeakingStarted);

    // Verify Opus packet in outgoing channel
    let pkt = out_rx.try_recv().expect("Opus packet expected in outgoing channel");
    assert_eq!(pkt, speech_pkts[0]);

    // 3. SPEECH ENDS: feed silence.
    // 200 ms hysteresis = 10 hangover frames.
    // For frames 1 to 10 of silence, packets should CONTINUE to be transmitted.
    for frame_idx in 1..=10 {
        let hangover_pkts = processor.process_input_samples(&silence_frame).unwrap();
        assert_eq!(
            hangover_pkts.len(),
            1,
            "Hangover frame {frame_idx}/10 must still transmit Opus packet"
        );
        assert!(processor.is_speaking_active());
        let _ = out_rx.try_recv().expect("Packet in hangover queue");
    }

    // On the 11th frame of silence: hangover is exhausted, speaking stops!
    let post_hangover_pkts = processor.process_input_samples(&silence_frame).unwrap();
    assert!(
        post_hangover_pkts.is_empty(),
        "Frame after hangover must NOT emit packets"
    );
    assert!(!processor.is_speaking_active());
    assert!(!is_speaking.load(std::sync::atomic::Ordering::Relaxed));

    // Verify SpeakingStopped transition received
    let stop_event = spk_rx.try_recv().expect("SpeakingStopped event expected");
    assert_eq!(stop_event, SpeakingEvent::SpeakingStopped);

    // 4. MUTE: speech while muted must never transmit
    is_muted.store(true, std::sync::atomic::Ordering::Relaxed);
    let muted_pkts = processor.process_input_samples(&speech_frame).unwrap();
    assert!(muted_pkts.is_empty(), "Muted processor must not transmit speech");
    assert!(!processor.is_speaking_active());
    assert_eq!(out_rx.try_recv().err(), Some(tokio::sync::mpsc::error::TryRecvError::Empty));
}

#[test]
fn test_audio_mixer_multi_ssrc_and_deafen() {
    let mut mixer = AudioMixer::new();
    let mut encoder = OpusAudioEncoder::new().expect("Failed to create encoder");

    let signal_a = generate_sine_wave(300.0, 0.4);
    let signal_b = generate_sine_wave(700.0, 0.4);

    let packet_a = encoder.encode_frame(&signal_a).unwrap();
    let packet_b = encoder.encode_frame(&signal_b).unwrap();

    let ssrc_a = 1001;
    let ssrc_b = 1002;

    mixer.feed_opus_packet(ssrc_a, &packet_a).expect("Feed SSRC A");
    mixer.feed_opus_packet(ssrc_b, &packet_b).expect("Feed SSRC B");

    assert_eq!(mixer.participant_count(), 2);
    assert!(mixer.is_participant_speaking(ssrc_a));
    assert!(mixer.is_participant_speaking(ssrc_b));
    assert!(mixer.participant_audio_level(ssrc_a) > 0.05);

    let mixed = mixer.pull_mixed_frame(false);
    assert_eq!(mixed.len(), STEREO_FRAME_SAMPLES);

    let energy: f32 = mixed.iter().map(|&x| x * x).sum();
    assert!(energy > 1.0, "Mixed audio energy should be non-zero");

    let silent = mixer.pull_mixed_frame(true);
    assert_eq!(silent.len(), STEREO_FRAME_SAMPLES);
    assert!(
        silent.iter().all(|&x| x == 0.0),
        "Deafened frame must be 100% silence"
    );
}

#[test]
fn test_media_device_manager_zero_mocks_and_switching_error_preservation() {
    let mgr = MediaDeviceManager::new();

    let inputs = mgr.list_input_devices().expect("List inputs");
    let outputs = mgr.list_output_devices().expect("List outputs");

    // Zero mocks validation
    for dev in &inputs {
        assert_eq!(dev.device_type, DeviceType::Input);
        assert!(!dev.name.is_empty());
        assert!(!dev.name.contains("Ferris"));
        assert!(!dev.name.contains("Mock"));
    }
    for dev in &outputs {
        assert_eq!(dev.device_type, DeviceType::Output);
        assert!(!dev.name.is_empty());
        assert!(!dev.name.contains("Ferris"));
        assert!(!dev.name.contains("Mock"));
    }

    let mut mgr_mut = MediaDeviceManager::new();
    let fake_result = mgr_mut.select_input_device(Some("non_existent_microphone_xyz_999"));
    assert!(matches!(fake_result, Err(AudioError::DeviceNotFound(_))));

    assert!(mgr_mut.select_input_device(None).is_ok());
    assert_eq!(mgr_mut.selected_input_id(), None);

    // Audit Requirement 1: switching error preserves existing stream & selection
    let mut engine = AudioEngine::new();
    assert!(engine.switch_input_device(None).is_ok());
    assert_eq!(engine.device_manager().read().unwrap().selected_input_id(), None);

    // Switching to invalid device fails and does NOT corrupt selection
    let fail_res = engine.switch_input_device(Some("invalid_input_device_id"));
    assert!(fail_res.is_err());
    assert_eq!(
        engine.device_manager().read().unwrap().selected_input_id(),
        None,
        "Failed device switch must preserve original device selection"
    );
}
