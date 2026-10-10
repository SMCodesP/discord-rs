use std::f32::consts::PI;
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

    // Opus introduces algorithmic lookahead delay (~120-312 samples at 48 kHz).
    // To verify waveform preservation, compute max cross-correlation across possible lags.
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

    // Also verify RMS energy conservation
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

    // Invalid frame sizes
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

    // Valid frame size (1920 = 48000 Hz * 0.02s * 2 channels)
    let valid_frame = vec![0.0f32; STEREO_FRAME_SAMPLES];
    assert!(encoder.encode_frame(&valid_frame).is_ok());
}

#[test]
fn test_resampling_and_channel_conversion() {
    // 1. Resample 44.1 kHz to 48 kHz
    let input_44k = vec![0.5f32; 441 * 2]; // ~10 ms of 44.1 kHz stereo
    let resampled_48k = resample_linear(&input_44k, 44100, 48000, 2);
    // Expected output frames = 441 * 48000 / 44100 = 480 frames = 960 samples
    assert_eq!(resampled_48k.len(), 960);

    // 2. Resample 16 kHz to 48 kHz
    let input_16k = vec![0.3f32; 160 * 2]; // 10 ms of 16 kHz stereo
    let resampled_48k_from_16k = resample_linear(&input_16k, 16000, 48000, 2);
    assert_eq!(resampled_48k_from_16k.len(), 480 * 2);

    // 3. Mono to stereo conversion
    let mono = vec![0.1f32, 0.2, 0.3];
    let stereo = convert_to_stereo(&mono, 1);
    assert_eq!(stereo.len(), 6);
    for (s, expected) in stereo.iter().zip([0.1, 0.1, 0.2, 0.2, 0.3, 0.3]) {
        assert!((s - expected).abs() < 1e-5);
    }

    // 4. Stereo to mono conversion
    let stereo_in = vec![0.2f32, 0.4, 0.6, 0.8];
    let mono_out = convert_stereo_to_channels(&stereo_in, 1);
    assert_eq!(mono_out.len(), 2);
    assert!((mono_out[0] - 0.3).abs() < 1e-4);
    assert!((mono_out[1] - 0.7).abs() < 1e-4);

    // 5. AudioFrameAccumulator chunking into 20ms frames
    let mut accumulator = AudioFrameAccumulator::new(STEREO_FRAME_SAMPLES);
    // Push chunks of 500 samples
    for _ in 0..3 {
        accumulator.push_samples(&vec![0.1f32; 500]);
        assert!(accumulator.pop_frame().is_none());
    }
    // Now total is 1500, push another 500 (total 2000)
    accumulator.push_samples(&vec![0.1f32; 500]);
    let frame = accumulator.pop_frame();
    assert!(frame.is_some());
    assert_eq!(frame.unwrap().len(), STEREO_FRAME_SAMPLES);
    assert_eq!(accumulator.len(), 2000 - STEREO_FRAME_SAMPLES); // 80 remaining
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

    // Feed packets from two different participants
    mixer.feed_opus_packet(ssrc_a, &packet_a).expect("Feed SSRC A");
    mixer.feed_opus_packet(ssrc_b, &packet_b).expect("Feed SSRC B");

    assert_eq!(mixer.participant_count(), 2);
    assert!(mixer.is_participant_speaking(ssrc_a));
    assert!(mixer.is_participant_speaking(ssrc_b));
    assert!(mixer.participant_audio_level(ssrc_a) > 0.05);

    // Pull mixed frame (not deafened)
    let mixed = mixer.pull_mixed_frame(false);
    assert_eq!(mixed.len(), STEREO_FRAME_SAMPLES);

    // The mixed frame should have non-zero energy
    let energy: f32 = mixed.iter().map(|&x| x * x).sum();
    assert!(energy > 1.0, "Mixed audio energy should be non-zero");

    // Pull frame while deafened: must be complete silence (zeroes)
    let silent = mixer.pull_mixed_frame(true);
    assert_eq!(silent.len(), STEREO_FRAME_SAMPLES);
    assert!(
        silent.iter().all(|&x| x == 0.0),
        "Deafened frame must be 100% silence"
    );
}

#[tokio::test]
async fn test_audio_engine_transport_contract_and_controls() {
    let mut engine = AudioEngine::new();
    let mut outgoing_rx = engine.take_outgoing_receiver().expect("Receiver must exist");

    // 1. Initial state
    assert!(!engine.is_muted());
    assert!(!engine.is_deafened());
    assert!(!engine.is_speaking());
    assert_eq!(engine.audio_level(), 0.0);

    // 2. Process synthetic speech frame
    let speech_frame = generate_sine_wave(500.0, 0.6);
    let packet_opt = engine
        .process_synthetic_input_frame(&speech_frame)
        .expect("Synthetic frame process");
    assert!(packet_opt.is_some(), "Active speech must produce Opus packet");
    assert!(engine.is_speaking(), "VAD should detect speaking");
    assert!(engine.audio_level() > 0.1, "Audio level should be positive");

    // Verify packet arrived on outgoing channel
    let received_packet = outgoing_rx.try_recv().expect("Packet should be in channel");
    assert_eq!(received_packet, packet_opt.unwrap());

    // 3. Test mute control
    engine.set_muted(true);
    assert!(engine.is_muted());

    let muted_packet = engine
        .process_synthetic_input_frame(&speech_frame)
        .expect("Synthetic frame process while muted");
    assert!(
        muted_packet.is_none(),
        "Muted engine must not produce output packets"
    );
    assert!(!engine.is_speaking(), "Speaking indicator must turn off when muted");
    assert_eq!(engine.audio_level(), 0.0, "Audio level must be zero when muted");

    // Unmute
    engine.set_muted(false);
    assert!(!engine.is_muted());

    // 4. Test receive_opus_packet transport contract (from remote SSRC)
    let remote_ssrc = 8888;
    let mut encoder = OpusAudioEncoder::new().unwrap();
    let remote_packet = encoder.encode_frame(&speech_frame).unwrap();

    engine
        .receive_opus_packet(remote_ssrc, &remote_packet)
        .expect("Receive Opus packet from remote SSRC");

    let mixed = engine.pull_mixed_frame_synthetic();
    assert_eq!(mixed.len(), STEREO_FRAME_SAMPLES);
    assert!(
        mixed.iter().any(|&s| s.abs() > 0.01),
        "Remote participant audio must be in mixed frame"
    );

    // 5. Test deafen control
    engine.set_deafened(true);
    assert!(engine.is_deafened());
    let deafened_frame = engine.pull_mixed_frame_synthetic();
    assert!(
        deafened_frame.iter().all(|&s| s == 0.0),
        "Deafened engine must output silence"
    );
}

#[test]
fn test_media_device_manager_zero_mocks_and_switching() {
    let mgr = MediaDeviceManager::new();

    // Listing devices must either return real devices or an empty list (ZERO MOCKS)
    let inputs = mgr.list_input_devices().expect("List inputs");
    let outputs = mgr.list_output_devices().expect("List outputs");

    // Ensure no fake/mock devices like "Ferris", "Virtual Mock Mic", etc.
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

    // Attempting to select a non-existent device must return DeviceNotFound
    let mut mgr_mut = MediaDeviceManager::new();
    let fake_result = mgr_mut.select_input_device(Some("non_existent_microphone_xyz_999"));
    assert!(matches!(fake_result, Err(AudioError::DeviceNotFound(_))));

    // Resetting to default (None) must succeed
    assert!(mgr_mut.select_input_device(None).is_ok());
    assert_eq!(mgr_mut.selected_input_id(), None);

    // AudioEngine device switching logic
    let mut engine = AudioEngine::new();
    assert!(engine.switch_input_device(None).is_ok());
    assert!(engine.switch_output_device(None).is_ok());

    // Selecting non-existent device on engine must also return error
    assert!(engine
        .switch_input_device(Some("invalid_input_device_id"))
        .is_err());
}
