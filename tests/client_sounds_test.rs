use std::time::{Duration, Instant};

use discord_rs::media::{
    AdsrEnvelope, ClientSoundService, SOUND_SAMPLE_RATE, SoundEffect, SoundPlaybackPolicy,
    SoundSynthesizer,
};

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum = samples.iter().map(|s| s * s).sum::<f32>();
    (sum / samples.len() as f32).sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0f32, |acc, s| acc.max(s.abs()))
}

#[test]
fn sound_effect_variants_are_distinct_and_synthesized() {
    let synth = SoundSynthesizer::new();
    let effects = [
        SoundEffect::Message,
        SoundEffect::Mention,
        SoundEffect::CallCalling,
        SoundEffect::CallRinging,
        SoundEffect::UserJoin,
        SoundEffect::UserLeave,
        SoundEffect::UserMoved,
        SoundEffect::Mute,
        SoundEffect::Unmute,
        SoundEffect::Deafen,
        SoundEffect::Undeafen,
    ];

    let mut rendered: Vec<(SoundEffect, Vec<f32>)> = Vec::new();
    for effect in effects {
        let pcm = synth.synthesize(effect, 1.0);
        assert!(!pcm.is_empty(), "{effect:?} produced empty PCM");
        assert!(pcm.iter().all(|s| s.is_finite()), "{effect:?} has non-finite sample");
        assert!(peak(&pcm) <= 1.0, "{effect:?} peaks above 1.0");
        assert!(rms(&pcm) > 0.001, "{effect:?} is unexpectedly silent");
        rendered.push((effect, pcm));
    }

    // Ensure not all effects collapse to identical buffers.
    for i in 0..rendered.len() {
        for j in (i + 1)..rendered.len() {
            let (a, a_pcm) = &rendered[i];
            let (b, b_pcm) = &rendered[j];
            let same = a_pcm.len() == b_pcm.len() && a_pcm == b_pcm;
            assert!(!same, "effects {a:?} and {b:?} should not be identical");
        }
    }
}

#[test]
fn synthesizer_outputs_48khz_pcm_with_adsr_shape() {
    let synth = SoundSynthesizer::new();
    assert_eq!(synth.sample_rate(), SOUND_SAMPLE_RATE);

    let envelope = AdsrEnvelope::new(
        Duration::from_millis(20),
        Duration::from_millis(30),
        0.5,
        Duration::from_millis(40),
    );
    let pcm = synth.synthesize_with_envelope(SoundEffect::Message, 0.8, envelope);
    assert!(!pcm.is_empty());
    assert!(pcm.iter().all(|s| s.is_finite() && s.abs() <= 1.0));

    let attack_window = &pcm[..(SOUND_SAMPLE_RATE as usize * 20 / 1000).min(pcm.len())];
    let sustain_start = SOUND_SAMPLE_RATE as usize * 60 / 1000;
    let sustain_end = (sustain_start + SOUND_SAMPLE_RATE as usize * 20 / 1000).min(pcm.len());
    let sustain_window = &pcm[sustain_start.min(pcm.len())..sustain_end];

    let attack_peak = peak(attack_window);
    let sustain_rms = rms(sustain_window);
    let tail_start = pcm.len().saturating_sub(SOUND_SAMPLE_RATE as usize * 20 / 1000);
    let tail_rms = rms(&pcm[tail_start..]);

    assert!(attack_peak > 0.01, "attack should not be silent");
    assert!(sustain_rms > 0.001, "sustain should have energy");
    // Release tail should be quieter than sustain/body.
    assert!(
        tail_rms <= sustain_rms.max(attack_peak) + 0.02,
        "tail should not exceed body energy unexpectedly"
    );
}

#[test]
fn volume_controls_amplitude_and_zero_mutes_output() {
    let synth = SoundSynthesizer::new();
    let loud = synth.synthesize(SoundEffect::UserJoin, 1.0);
    let quiet = synth.synthesize(SoundEffect::UserJoin, 0.2);
    let muted = synth.synthesize(SoundEffect::UserJoin, 0.0);

    let loud_rms = rms(&loud);
    let quiet_rms = rms(&quiet);
    let muted_peak = peak(&muted);

    assert!(loud_rms > quiet_rms, "lower volume should reduce RMS");
    assert!(quiet_rms > 0.0);
    assert_eq!(muted_peak, 0.0, "volume 0 should render silence");
}

#[test]
fn dnd_suppresses_playback() {
    let mut svc = ClientSoundService::with_policy(
        SoundPlaybackPolicy::new()
            .with_dnd_suppressed(true)
            .with_cooldown(Duration::from_millis(0)),
    );
    let now = Instant::now();

    assert!(svc.play_at(SoundEffect::Message, now).is_none());
    assert!(svc.play_at(SoundEffect::CallRinging, now).is_none());

    svc.policy_mut().set_dnd_suppressed(false);
    assert!(svc.play_at(SoundEffect::Message, now).is_some());
}

#[test]
fn cooldown_prevents_spam_but_allows_after_window() {
    let mut policy = SoundPlaybackPolicy::new().with_cooldown(Duration::from_millis(300));
    let t0 = Instant::now();

    assert!(policy.try_play_at(SoundEffect::Mute, t0));
    assert!(!policy.try_play_at(SoundEffect::Mute, t0 + Duration::from_millis(100)));
    assert!(!policy.try_play_at(SoundEffect::Mute, t0 + Duration::from_millis(299)));
    assert!(policy.try_play_at(SoundEffect::Mute, t0 + Duration::from_millis(300)));

    // Different effect should not be blocked by Mute cooldown.
    assert!(policy.try_play_at(SoundEffect::UserJoin, t0 + Duration::from_millis(50)));
}

#[test]
fn client_sound_service_respects_policy_and_renders() {
    let mut svc = ClientSoundService::new();
    svc.policy_mut().set_volume(0.6);
    svc.policy_mut().set_cooldown(Duration::from_millis(200));
    let t0 = Instant::now();

    let first = svc.play_at(SoundEffect::Deafen, t0);
    let second = svc.play_at(SoundEffect::Deafen, t0 + Duration::from_millis(50));
    let third = svc.play_at(SoundEffect::Deafen, t0 + Duration::from_millis(250));

    assert!(first.is_some());
    assert!(second.is_none());
    assert!(third.is_some());

    let rendered = svc.render(SoundEffect::Undeafen);
    assert!(!rendered.is_empty());
    assert!(peak(&rendered) <= 0.7);
}
