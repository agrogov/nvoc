//! GPU overclocking operations

use crate::cli::OverclockParams;
use crate::gpu::power::apply_power_limit;
use crate::nvml::{
    device_set_clock_offset, device_set_gpu_locked_clocks, device_set_memory_vf_offset,
    NvmlClockType, NvmlDevice, NvmlPerfState,
};
use crate::AppError;

fn apply_clocks(device: NvmlDevice, clocks: (u32, u32), dry_run: bool) -> Result<String, AppError> {
    let (min, max) = clocks;
    if dry_run {
        return Ok(format!("{min}-{max}"));
    }
    device_set_gpu_locked_clocks(device, min, max)
        .map_err(|e| AppError::new("clocks", e))?;
    Ok(format!("{min}-{max}"))
}

fn apply_graphics_offset(device: NvmlDevice, offset: i32, dry_run: bool) -> Result<String, AppError> {
    if dry_run {
        return Ok(format!("{:+}", offset));
    }
    device_set_clock_offset(device, NvmlClockType::Graphics, NvmlPerfState::P0, offset)
        .map_err(|e| AppError::new("gpu offset", e))?;
    Ok(format!("{:+}", offset))
}

fn apply_memory_offset(device: NvmlDevice, offset: i32, dry_run: bool) -> Result<String, AppError> {
    if dry_run {
        return Ok(format!("{:+}", offset));
    }
    device_set_memory_vf_offset(device, offset)
        .map_err(|e| AppError::new("mem offset", e))?;
    Ok(format!("{:+}", offset))
}

#[derive(Debug, Clone)]
pub struct OverclockSummary {
    pub clocks: Option<String>,
    pub graphics_offset: Option<String>,
    pub memory_offset: Option<String>,
    pub power: Option<String>,
}

pub fn apply(device: NvmlDevice, params: &OverclockParams) -> Result<OverclockSummary, AppError> {
    let mut summary = OverclockSummary {
        clocks: None,
        graphics_offset: None,
        memory_offset: None,
        power: None,
    };

    if let Some(clocks) = params.clocks {
        summary.clocks = Some(apply_clocks(device, clocks, params.dry_run)?);
    }
    if let Some(offset) = params.graphics_offset {
        summary.graphics_offset = Some(apply_graphics_offset(device, offset, params.dry_run)?);
    }
    if let Some(offset) = params.memory_offset {
        summary.memory_offset = Some(apply_memory_offset(device, offset, params.dry_run)?);
    }
    if let Some(percentage) = params.power_limit {
        summary.power = Some(apply_power_limit(device, percentage, params.dry_run)?);
    }

    Ok(summary)
}
