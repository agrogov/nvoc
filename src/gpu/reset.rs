//! GPU reset operations
//!
//! Unlike other operations that bail on first error, reset attempts all
//! operations and reports individual failures. Errors are printed at the
//! call site rather than bubbled up because the caller needs to see each
//! failure as it continues through remaining operations.

use crate::constants::clocks;
use crate::gpu::domain::reset_power_limit;
use crate::nvml::{
    device_reset_gpu_locked_clocks, device_reset_memory_locked_clocks, device_set_clock_offset,
    device_set_gpu_locked_clocks, device_set_memory_vf_offset, NvmlClockType, NvmlDevice,
    NvmlPerfState, Result,
};
use crate::AppError;

fn try_reset(domain: &str, f: impl FnOnce() -> Result<()>, errors: &mut Vec<String>) {
    if let Err(e) = f() {
        errors.push(format!("{domain}: {}", e.user_message()));
    }
}

pub fn reset_gpu_settings(device: NvmlDevice, dry_run: bool) -> std::result::Result<(), AppError> {
    if dry_run {
        return Ok(());
    }

    let mut errors: Vec<String> = Vec::new();

    // Blackwell requires setting idle clocks before reset will succeed
    let idle_ok = device_set_gpu_locked_clocks(device, clocks::BLACKWELL_IDLE_MIN, clocks::BLACKWELL_IDLE_MAX).is_ok();
    if idle_ok {
        try_reset("gpu clocks", || device_reset_gpu_locked_clocks(device), &mut errors);
    } else {
        errors.push("gpu clocks: failed to set idle clocks for reset".to_string());
    }

    try_reset("mem clocks", || device_reset_memory_locked_clocks(device), &mut errors);

    try_reset("gpu offset", || {
        device_set_clock_offset(device, NvmlClockType::Graphics, NvmlPerfState::P0, clocks::DEFAULT_GRAPHICS_OFFSET)
    }, &mut errors);

    try_reset("mem offset", || {
        device_set_memory_vf_offset(device, clocks::DEFAULT_MEMORY_OFFSET)
    }, &mut errors);

    try_reset("power limit", || reset_power_limit(device), &mut errors);

    if !errors.is_empty() {
        return Err(AppError::msg("reset", errors.join("; ")));
    }

    Ok(())
}
