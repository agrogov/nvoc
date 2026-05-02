//! GPU operations and device management

use crate::cli::DeviceSelector;
use crate::constants::hardware;
use crate::nvml::{
    device_get_count, device_get_handle_by_index, init, shutdown, system_get_driver_version,
    NvmlDevice, Result,
};

pub mod domain;
pub mod info;
pub mod overclock;
pub mod power;
pub mod reset;
pub mod validation;

#[derive(Debug, Clone)]
pub struct DeviceRef {
    pub index: u32,
    pub device: NvmlDevice,
    pub name: String,
}

/// Cleanup guard to ensure NVML is properly shut down
pub struct CleanupGuard;

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = shutdown();
    }
}

pub fn init_nvml() -> std::result::Result<(), crate::AppError> {
    init().map_err(|e| crate::AppError::new("driver", e))?;
    let driver_version = system_get_driver_version()
        .map_err(|e| crate::AppError::new("driver", e))?;
    let major: u32 = driver_version
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| crate::AppError::msg("driver", format!("unparseable version: {driver_version}")))?;
    if major < hardware::MIN_DRIVER_VERSION {
        return Err(crate::AppError::msg("driver", format!("version {driver_version} too old, need {}+", hardware::MIN_DRIVER_VERSION)));
    }
    Ok(())
}

pub fn init_with_cleanup() -> std::result::Result<CleanupGuard, crate::AppError> {
    init_nvml()?;
    Ok(CleanupGuard)
}

pub fn driver_version() -> Result<String> {
    system_get_driver_version()
}

pub fn enumerate_devices() -> Result<Vec<DeviceRef>> {
    let device_count = device_get_count()?;
    let mut devices = Vec::with_capacity(device_count as usize);

    for index in 0..device_count {
        let device = device_get_handle_by_index(index)?;
        let name = crate::nvml::device_get_name(device)?;
        devices.push(DeviceRef { index, device, name });
    }

    Ok(devices)
}

pub fn select_devices(selector: &DeviceSelector) -> Result<Vec<DeviceRef>> {
    let devices = enumerate_devices()?;
    match selector {
        DeviceSelector::All => Ok(devices),
        DeviceSelector::Single(index) => {
            let Some(device) = devices.into_iter().find(|d| &d.index == index) else {
                return Err(crate::nvml::NvmlError::InvalidArgument);
            };
            Ok(vec![device])
        }
        DeviceSelector::Match(re) => {
            let re = regex::Regex::new(re)
                .map_err(|_| crate::nvml::NvmlError::InvalidArgument)?;
            Ok(devices
                .into_iter()
                .filter(|d| re.is_match(&d.name))
                .collect())
        }
    }
}
