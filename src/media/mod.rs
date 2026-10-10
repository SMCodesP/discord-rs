pub mod audio;
pub mod codec;
pub mod devices;
pub mod mixer;
pub mod resample;

pub use audio::*;
pub use codec::*;
pub use devices::*;
pub use mixer::*;
pub use resample::*;

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum AudioError {
    OpusEncodeError(String),
    OpusDecodeError(String),
    DeviceNotFound(String),
    NoDefaultDevice(String),
    DeviceUnavailable(String),
    StreamError(String),
    UnsupportedFormat(String),
    InvalidFrameSize { expected: usize, actual: usize },
    InvalidSampleRate(u32),
    InvalidChannels(u16),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpusEncodeError(msg) => write!(f, "Opus encode error: {msg}"),
            Self::OpusDecodeError(msg) => write!(f, "Opus decode error: {msg}"),
            Self::DeviceNotFound(name) => write!(f, "Audio device not found: {name}"),
            Self::NoDefaultDevice(msg) => write!(f, "No default audio device: {msg}"),
            Self::DeviceUnavailable(msg) => write!(f, "Audio device unavailable: {msg}"),
            Self::StreamError(msg) => write!(f, "Audio stream error: {msg}"),
            Self::UnsupportedFormat(msg) => write!(f, "Unsupported audio format: {msg}"),
            Self::InvalidFrameSize { expected, actual } => {
                write!(f, "Invalid frame size: expected {expected}, got {actual}")
            }
            Self::InvalidSampleRate(rate) => write!(f, "Invalid sample rate: {rate} Hz"),
            Self::InvalidChannels(ch) => write!(f, "Invalid channel count: {ch}"),
        }
    }
}

impl std::error::Error for AudioError {}
