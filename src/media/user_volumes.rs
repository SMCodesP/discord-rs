use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const USER_VOLUME_MIN: f32 = 0.0;
pub const USER_VOLUME_MAX: f32 = 2.0;
pub const USER_VOLUME_DEFAULT: f32 = 1.0;
/// Minimum perceptual gain in dB at slider 0.0 (treated as silence).
const MIN_PERCEPTUAL_DB: f32 = -60.0;
/// Maximum gain in dB at slider 2.0 (+6.0206 dB).
const MAX_PERCEPTUAL_DB: f32 = 6.020599913279624;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserVolumeSettings {
    /// User gain in the inclusive range 0.0..=2.0.
    /// 1.0 is unity; the value is mapped through a perceptual dB curve when
    /// converted to linear amplitude for playback.
    #[serde(default = "default_gain")]
    pub gain: F32Bits,
    #[serde(default)]
    pub muted: bool,
}

impl Default for UserVolumeSettings {
    fn default() -> Self {
        Self {
            gain: default_gain(),
            muted: false,
        }
    }
}

fn default_gain() -> F32Bits {
    F32Bits(USER_VOLUME_DEFAULT.to_bits())
}

/// Serde wrapper that keeps JSON readable while preserving exact f32 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct F32Bits(pub u32);

impl Serialize for F32Bits {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = f32::from_bits(self.0);
        if value.is_finite() {
            serializer.serialize_f32(value)
        } else {
            serializer.serialize_f32(USER_VOLUME_DEFAULT)
        }
    }
}

impl<'de> Deserialize<'de> for F32Bits {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = f32::deserialize(deserializer)?;
        Ok(F32Bits(
            sanitize_user_gain(value).to_bits(),
        ))
    }
}

impl From<f32> for F32Bits {
    fn from(value: f32) -> Self {
        Self(sanitize_user_gain(value).to_bits())
    }
}

impl From<F32Bits> for f32 {
    fn from(value: F32Bits) -> Self {
        sanitize_user_gain(f32::from_bits(value.0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UserVolumesFile {
    #[serde(default)]
    users: HashMap<String, StoredUserVolume>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredUserVolume {
    #[serde(default = "default_gain")]
    gain: F32Bits,
    #[serde(default)]
    muted: bool,
}

#[derive(Debug, Clone)]
pub struct UserVolumesStore {
    file_path: PathBuf,
    users: HashMap<String, UserVolumeSettings>,
}

impl UserVolumesStore {
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(dirs::config_dir)
            .unwrap_or_else(|| PathBuf::from(".config"));
        base.join("discord-rs").join("user_volumes.json")
    }

    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: path.into(),
            users: HashMap::new(),
        }
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut store = Self::with_path(path);
        store.reload()?;
        Ok(store)
    }

    pub fn load_default() -> Result<Self> {
        Self::load(Self::default_path())
    }

    pub fn path(&self) -> &Path {
        &self.file_path
    }

    pub fn reload(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            self.users.clear();
            return Ok(());
        }

        let bytes = fs::read(&self.file_path)
            .with_context(|| format!("failed to read user volumes: {}", self.file_path.display()))?;
        let parsed: UserVolumesFile = serde_json::from_slice(&bytes).with_context(|| {
            format!("failed to parse user volumes: {}", self.file_path.display())
        })?;

        self.users = parsed
            .users
            .into_iter()
            .map(|(user_id, stored)| {
                (
                    user_id,
                    UserVolumeSettings {
                        gain: F32Bits(sanitize_user_gain(f32::from(stored.gain)).to_bits()),
                        muted: stored.muted,
                    },
                )
            })
            .collect();
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create user volumes dir: {}", parent.display())
            })?;
        }

        let file = UserVolumesFile {
            users: self
                .users
                .iter()
                .map(|(user_id, settings)| {
                    (
                        user_id.clone(),
                        StoredUserVolume {
                            gain: F32Bits(sanitize_user_gain(f32::from(settings.gain)).to_bits()),
                            muted: settings.muted,
                        },
                    )
                })
                .collect(),
        };

        let json = serde_json::to_vec_pretty(&file).context("failed to serialize user volumes")?;
        let tmp_path = self.file_path.with_extension("json.tmp");
        let mut tmp = fs::File::create(&tmp_path).with_context(|| {
            format!("failed to create temp user volumes file: {}", tmp_path.display())
        })?;
        tmp.write_all(&json).with_context(|| {
            format!("failed to write temp user volumes file: {}", tmp_path.display())
        })?;
        tmp.write_all(b"\n").ok();
        tmp.sync_all().ok();
        drop(tmp);
        fs::rename(&tmp_path, &self.file_path).with_context(|| {
            format!(
                "failed to persist user volumes: {}",
                self.file_path.display()
            )
        })?;
        Ok(())
    }

    pub fn get(&self, user_id: &str) -> UserVolumeSettings {
        self.users.get(user_id).copied().unwrap_or_default()
    }

    pub fn gain(&self, user_id: &str) -> f32 {
        f32::from(self.get(user_id).gain)
    }

    pub fn is_muted(&self, user_id: &str) -> bool {
        self.get(user_id).muted
    }

    pub fn set_gain(&mut self, user_id: impl Into<String>, gain: f32) -> f32 {
        let gain = sanitize_user_gain(gain);
        let entry = self.users.entry(user_id.into()).or_default();
        entry.gain = F32Bits(gain.to_bits());
        gain
    }

    pub fn set_muted(&mut self, user_id: impl Into<String>, muted: bool) -> bool {
        let entry = self.users.entry(user_id.into()).or_default();
        entry.muted = muted;
        muted
    }

    pub fn set_gain_and_persist(&mut self, user_id: impl Into<String>, gain: f32) -> Result<f32> {
        let gain = self.set_gain(user_id, gain);
        self.save()?;
        Ok(gain)
    }

    pub fn set_muted_and_persist(&mut self, user_id: impl Into<String>, muted: bool) -> Result<bool> {
        let muted = self.set_muted(user_id, muted);
        self.save()?;
        Ok(muted)
    }

    /// Linear amplitude multiplier for playback, derived from the stored gain
    /// using a perceptual dB curve. Muted users always return 0.0.
    pub fn effective_amplitude(&self, user_id: &str) -> f32 {
        let settings = self.get(user_id);
        if settings.muted {
            return 0.0;
        }
        perceptual_gain_to_amplitude(f32::from(settings.gain))
    }

    pub fn users(&self) -> &HashMap<String, UserVolumeSettings> {
        &self.users
    }
}

pub fn sanitize_user_gain(gain: f32) -> f32 {
    if gain.is_finite() {
        gain.clamp(USER_VOLUME_MIN, USER_VOLUME_MAX)
    } else {
        USER_VOLUME_DEFAULT
    }
}

/// Maps a user gain slider value (0.0..=2.0) to a linear amplitude multiplier
/// using a perceptual dB curve.
///
/// - 0.0 -> silence (0.0 amplitude)
/// - 1.0 -> unity (0.0 dB, amplitude 1.0)
/// - 2.0 -> +6.0206 dB (amplitude 2.0)
///
/// Values between 0.0 and 1.0 map logarithmically from MIN_PERCEPTUAL_DB to 0 dB,
/// which matches human loudness perception better than linear gain.
pub fn perceptual_gain_to_amplitude(gain: f32) -> f32 {
    let gain = sanitize_user_gain(gain);
    if gain <= 0.0 {
        return 0.0;
    }
    if (gain - USER_VOLUME_DEFAULT).abs() < f32::EPSILON {
        return 1.0;
    }

    let db = if gain <= 1.0 {
        // Map 0..1 to MIN_PERCEPTUAL_DB..0 dB perceptually.
        let t = gain / 1.0;
        MIN_PERCEPTUAL_DB + (0.0 - MIN_PERCEPTUAL_DB) * t
    } else {
        // Map 1..2 to 0..MAX_PERCEPTUAL_DB dB.
        let t = (gain - 1.0) / 1.0;
        t * MAX_PERCEPTUAL_DB
    };

    db_to_amplitude(db).clamp(0.0, USER_VOLUME_MAX)
}

/// Converts a dB value to linear amplitude.
pub fn db_to_amplitude(db: f32) -> f32 {
    if !db.is_finite() {
        return 1.0;
    }
    10.0_f32.powf(db / 20.0)
}

/// Converts linear amplitude to dB. Zero/negative returns negative infinity-like floor.
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    if !amplitude.is_finite() || amplitude <= 0.0 {
        return f32::NEG_INFINITY;
    }
    20.0 * amplitude.log10()
}
