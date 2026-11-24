use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use discord_rs::media::{
    amplitude_to_db, db_to_amplitude, perceptual_gain_to_amplitude, sanitize_user_gain,
    UserVolumesStore, USER_VOLUME_DEFAULT, USER_VOLUME_MAX, USER_VOLUME_MIN,
};

static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(0);

fn temp_file(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "discord_rs_user_volumes_{name}_{}_{}_{}.json",
        std::process::id(),
        nanos,
        seq
    ))
}

#[test]
fn sanitize_gain_clamps_and_handles_non_finite() {
    assert_eq!(sanitize_user_gain(-1.0), USER_VOLUME_MIN);
    assert_eq!(sanitize_user_gain(0.0), 0.0);
    assert_eq!(sanitize_user_gain(1.0), 1.0);
    assert_eq!(sanitize_user_gain(2.0), USER_VOLUME_MAX);
    assert_eq!(sanitize_user_gain(2.5), USER_VOLUME_MAX);
    assert_eq!(sanitize_user_gain(f32::NAN), USER_VOLUME_DEFAULT);
    assert_eq!(sanitize_user_gain(f32::INFINITY), USER_VOLUME_DEFAULT);
    assert_eq!(sanitize_user_gain(f32::NEG_INFINITY), USER_VOLUME_DEFAULT);
}

#[test]
fn perceptual_curve_has_expected_anchor_points() {
    let at_zero = perceptual_gain_to_amplitude(0.0);
    let at_unity = perceptual_gain_to_amplitude(1.0);
    let at_max = perceptual_gain_to_amplitude(2.0);

    assert_eq!(at_zero, 0.0, "gain 0.0 must be silence");
    assert!((at_unity - 1.0).abs() < 1e-6, "gain 1.0 must be unity: {at_unity}");
    assert!((at_max - 2.0).abs() < 1e-4, "gain 2.0 must map to +6dB (2x): {at_max}");

    let below_unity = perceptual_gain_to_amplitude(0.5);
    assert!(below_unity > 0.0 && below_unity < 1.0, "0.5 gain should be between 0 and 1: {below_unity}");

    // Perceptual dB curve: 0.5 slider is -30dB relative to the 0..1 segment, not linear 0.5 amplitude.
    let expected_db = -60.0 + (0.0 - -60.0) * 0.5;
    let expected_amp = db_to_amplitude(expected_db);
    assert!(
        (below_unity - expected_amp).abs() < 1e-5,
        "0.5 should follow dB curve: got {below_unity}, expected {expected_amp}"
    );
}

#[test]
fn db_amplitude_roundtrip_basic() {
    let amp = db_to_amplitude(0.0);
    assert!((amp - 1.0).abs() < 1e-6);
    let db = amplitude_to_db(1.0);
    assert!(db.abs() < 1e-6);
    let amp6 = db_to_amplitude(6.020599913279624);
    assert!((amp6 - 2.0).abs() < 1e-4);
}

#[test]
fn store_defaults_and_mutation() {
    let path = temp_file("defaults");
    let _ = fs::remove_file(&path);
    let mut store = UserVolumesStore::with_path(&path);

    let settings = store.get("user-1");
    assert_eq!(f32::from(settings.gain), USER_VOLUME_DEFAULT);
    assert!(!settings.muted);
    assert_eq!(store.effective_amplitude("user-1"), 1.0);

    let gain = store.set_gain("user-1", 1.5);
    assert!((gain - 1.5).abs() < 1e-6);
    assert!(!store.is_muted("user-1"));
    let amp = store.effective_amplitude("user-1");
    let expected = perceptual_gain_to_amplitude(1.5);
    assert!((amp - expected).abs() < 1e-6, "amp {amp} expected {expected}");

    store.set_muted("user-1", true);
    assert!(store.is_muted("user-1"));
    assert_eq!(store.effective_amplitude("user-1"), 0.0);

    let _ = fs::remove_file(&path);
}

#[test]
fn store_persists_and_reloads_from_isolated_config_dir() {
    let dir = std::env::temp_dir().join(format!(
        "discord_rs_user_volumes_dir_{}_{}",
        std::process::id(),
        NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("user_volumes.json");

    {
        let mut store = UserVolumesStore::with_path(&path);
        store.set_gain("alice", 0.75).to_string();
        store.set_muted("alice", false);
        store.set_gain("bob", 1.25);
        store.set_muted("bob", true);
        store.save().unwrap();
    }

    let loaded = UserVolumesStore::load(&path).unwrap();
    assert_eq!(loaded.path(), path.as_path());
    assert!((loaded.gain("alice") - 0.75).abs() < 1e-6);
    assert!(!loaded.is_muted("alice"));
    assert!((loaded.gain("bob") - 1.25).abs() < 1e-6);
    assert!(loaded.is_muted("bob"));
    assert_eq!(loaded.effective_amplitude("bob"), 0.0);

    // Missing user returns defaults without mutating persisted data.
    assert_eq!(loaded.gain("missing"), USER_VOLUME_DEFAULT);
    assert!(!loaded.is_muted("missing"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn set_and_persist_helpers_write_file() {
    let path = temp_file("persist_helpers");
    let _ = fs::remove_file(&path);
    let mut store = UserVolumesStore::with_path(&path);

    let gain = store.set_gain_and_persist("u1", 2.0).unwrap();
    assert!((gain - 2.0).abs() < 1e-6);
    let muted = store.set_muted_and_persist("u1", true).unwrap();
    assert!(muted);

    let raw = fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\"u1\""));
    assert!(raw.contains("\"muted\": true"));

    let reloaded = UserVolumesStore::load(&path).unwrap();
    assert!((reloaded.gain("u1") - 2.0).abs() < 1e-6);
    assert!(reloaded.is_muted("u1"));

    let _ = fs::remove_file(&path);
}

#[test]
fn reload_missing_file_clears_in_memory_state() {
    let path = temp_file("missing_reload");
    let _ = fs::remove_file(&path);
    let mut store = UserVolumesStore::with_path(&path);
    store.set_gain("u1", 0.5);
    store.set_muted("u1", true);
    store.save().unwrap();
    assert!(path.exists());

    fs::remove_file(&path).unwrap();
    store.reload().unwrap();
    assert_eq!(store.gain("u1"), USER_VOLUME_DEFAULT);
    assert!(!store.is_muted("u1"));
}
