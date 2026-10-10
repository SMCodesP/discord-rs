use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

use crate::media::AudioError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDeviceInfo {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub is_default: bool,
    pub sample_rates: Vec<u32>,
    pub channels: Vec<u16>,
}

pub struct MediaDeviceManager {
    host: cpal::Host,
    selected_input_id: Option<String>,
    selected_output_id: Option<String>,
}

impl Default for MediaDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaDeviceManager {
    pub fn new() -> Self {
        Self {
            host: cpal::default_host(),
            selected_input_id: None,
            selected_output_id: None,
        }
    }

    /// Lists real system audio input devices (microphones).
    /// If no devices exist on the host, returns an empty list without inventing mock data.
    pub fn list_input_devices(&self) -> Result<Vec<MediaDeviceInfo>, AudioError> {
        let default_name = self.host.default_input_device().and_then(|d| d.name().ok());
        let devices = match self.host.input_devices() {
            Ok(iter) => iter,
            Err(e) => return Err(AudioError::DeviceUnavailable(format!("Failed to enumerate input devices: {e:?}"))),
        };

        let mut result = Vec::new();
        for dev in devices {
            if let Ok(name) = dev.name() {
                let is_default = default_name.as_deref() == Some(&name);
                let (rates, channels) = extract_device_capabilities(&dev, DeviceType::Input);

                result.push(MediaDeviceInfo {
                    id: name.clone(),
                    name,
                    device_type: DeviceType::Input,
                    is_default,
                    sample_rates: rates,
                    channels,
                });
            }
        }

        Ok(result)
    }

    /// Lists real system audio output devices (speakers/headphones).
    /// If no devices exist on the host, returns an empty list without inventing mock data.
    pub fn list_output_devices(&self) -> Result<Vec<MediaDeviceInfo>, AudioError> {
        let default_name = self.host.default_output_device().and_then(|d| d.name().ok());
        let devices = match self.host.output_devices() {
            Ok(iter) => iter,
            Err(e) => return Err(AudioError::DeviceUnavailable(format!("Failed to enumerate output devices: {e:?}"))),
        };

        let mut result = Vec::new();
        for dev in devices {
            if let Ok(name) = dev.name() {
                let is_default = default_name.as_deref() == Some(&name);
                let (rates, channels) = extract_device_capabilities(&dev, DeviceType::Output);

                result.push(MediaDeviceInfo {
                    id: name.clone(),
                    name,
                    device_type: DeviceType::Output,
                    is_default,
                    sample_rates: rates,
                    channels,
                });
            }
        }

        Ok(result)
    }

    /// Retrieves the system default input device, or returns `AudioError::NoDefaultDevice`
    /// if none is configured.
    pub fn default_input_device(&self) -> Result<MediaDeviceInfo, AudioError> {
        let dev = self
            .host
            .default_input_device()
            .ok_or_else(|| AudioError::NoDefaultDevice("No default input device available".to_string()))?;

        let name = dev
            .name()
            .map_err(|e| AudioError::DeviceUnavailable(format!("Cannot read input device name: {e:?}")))?;

        let (rates, channels) = extract_device_capabilities(&dev, DeviceType::Input);

        Ok(MediaDeviceInfo {
            id: name.clone(),
            name,
            device_type: DeviceType::Input,
            is_default: true,
            sample_rates: rates,
            channels,
        })
    }

    /// Retrieves the system default output device, or returns `AudioError::NoDefaultDevice`
    /// if none is configured.
    pub fn default_output_device(&self) -> Result<MediaDeviceInfo, AudioError> {
        let dev = self
            .host
            .default_output_device()
            .ok_or_else(|| AudioError::NoDefaultDevice("No default output device available".to_string()))?;

        let name = dev
            .name()
            .map_err(|e| AudioError::DeviceUnavailable(format!("Cannot read output device name: {e:?}")))?;

        let (rates, channels) = extract_device_capabilities(&dev, DeviceType::Output);

        Ok(MediaDeviceInfo {
            id: name.clone(),
            name,
            device_type: DeviceType::Output,
            is_default: true,
            sample_rates: rates,
            channels,
        })
    }

    pub fn selected_input_id(&self) -> Option<&str> {
        self.selected_input_id.as_deref()
    }

    pub fn selected_output_id(&self) -> Option<&str> {
        self.selected_output_id.as_deref()
    }

    /// Sets the selected input device by ID. If `None` or empty, resets to default.
    pub fn select_input_device(&mut self, id: Option<&str>) -> Result<(), AudioError> {
        if let Some(target_id) = id {
            let available = self.list_input_devices()?;
            if !available.iter().any(|d| d.id == target_id) {
                return Err(AudioError::DeviceNotFound(target_id.to_string()));
            }
            self.selected_input_id = Some(target_id.to_string());
        } else {
            self.selected_input_id = None;
        }
        Ok(())
    }

    /// Sets the selected output device by ID. If `None` or empty, resets to default.
    pub fn select_output_device(&mut self, id: Option<&str>) -> Result<(), AudioError> {
        if let Some(target_id) = id {
            let available = self.list_output_devices()?;
            if !available.iter().any(|d| d.id == target_id) {
                return Err(AudioError::DeviceNotFound(target_id.to_string()));
            }
            self.selected_output_id = Some(target_id.to_string());
        } else {
            self.selected_output_id = None;
        }
        Ok(())
    }

    /// Finds the corresponding `cpal::Device` for an input device ID (or system default if None).
    pub fn get_cpal_input_device(&self, id: Option<&str>) -> Result<cpal::Device, AudioError> {
        if let Some(target_id) = id {
            let devices = self
                .host
                .input_devices()
                .map_err(|e| AudioError::DeviceUnavailable(format!("Cannot enumerate input devices: {e:?}")))?;

            for dev in devices {
                if let Ok(name) = dev.name() {
                    if name == target_id {
                        return Ok(dev);
                    }
                }
            }
            Err(AudioError::DeviceNotFound(target_id.to_string()))
        } else {
            self.host
                .default_input_device()
                .ok_or_else(|| AudioError::NoDefaultDevice("No default input device found".to_string()))
        }
    }

    /// Finds the corresponding `cpal::Device` for an output device ID (or system default if None).
    pub fn get_cpal_output_device(&self, id: Option<&str>) -> Result<cpal::Device, AudioError> {
        if let Some(target_id) = id {
            let devices = self
                .host
                .output_devices()
                .map_err(|e| AudioError::DeviceUnavailable(format!("Cannot enumerate output devices: {e:?}")))?;

            for dev in devices {
                if let Ok(name) = dev.name() {
                    if name == target_id {
                        return Ok(dev);
                    }
                }
            }
            Err(AudioError::DeviceNotFound(target_id.to_string()))
        } else {
            self.host
                .default_output_device()
                .ok_or_else(|| AudioError::NoDefaultDevice("No default output device found".to_string()))
        }
    }
}

fn extract_device_capabilities(dev: &cpal::Device, dev_type: DeviceType) -> (Vec<u32>, Vec<u16>) {
    let mut rates = Vec::new();
    let mut channels = Vec::new();

    let configs_result = match dev_type {
        DeviceType::Input => dev.supported_input_configs().map(|iter| iter.collect::<Vec<_>>()),
        DeviceType::Output => dev.supported_output_configs().map(|iter| iter.collect::<Vec<_>>()),
    };

    if let Ok(configs) = configs_result {
        for cfg in configs {
            let ch = cfg.channels();
            if !channels.contains(&ch) {
                channels.push(ch);
            }
            let min_r = cfg.min_sample_rate().0;
            let max_r = cfg.max_sample_rate().0;
            for candidate in [16000, 24000, 44100, 48000, 96000] {
                if candidate >= min_r && candidate <= max_r && !rates.contains(&candidate) {
                    rates.push(candidate);
                }
            }
        }
    }

    rates.sort_unstable();
    channels.sort_unstable();
    (rates, channels)
}
