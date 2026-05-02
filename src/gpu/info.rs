//! GPU information display

use crate::gpu::domain::{get_power_info, get_power_usage_watts};
use crate::nvml::{
    device_get_clock_info, device_get_clock_offsets, device_get_temperature, NvmlClockType,
    NvmlDevice,
};

#[derive(Debug, Clone)]
pub struct InfoRow {
    pub index: u32,
    pub name: String,
    pub gpu_clock_mhz: Option<u32>,
    pub gpu_offset_mhz: Option<i32>,
    pub mem_clock_mhz: Option<u32>,
    pub mem_offset_mhz: Option<i32>,
    pub temp_c: Option<u32>,
    pub power_w: Option<u32>,
    pub power_limit_w: Option<u32>,
    pub power_limit_pct: Option<u32>,
    pub power_range: Option<String>,
}

pub fn get_gpu_info(device: NvmlDevice, device_index: u32, name: String) -> InfoRow {
    let gpu_clock_mhz = device_get_clock_info(device, NvmlClockType::Graphics).ok();
    let gpu_offset_mhz = device_get_clock_offsets(device, NvmlClockType::Graphics)
        .map(|o| o.clockOffsetMHz)
        .ok();
    let mem_clock_mhz = device_get_clock_info(device, NvmlClockType::Memory).ok();
    let mem_offset_mhz = device_get_clock_offsets(device, NvmlClockType::Memory)
        .map(|o| o.clockOffsetMHz)
        .ok();
    let temp_c = device_get_temperature(device).ok();
    let power_w = get_power_usage_watts(device).ok();

    let (power_limit_w, power_limit_pct, power_range) = match get_power_info(device) {
        Ok(info) => (
            Some(info.limit_watts),
            Some(info.current_percentage()),
            Some(format!(
                "{}-{}W (hard {}W)",
                info.min_watts, info.default_watts, info.max_watts
            )),
        ),
        Err(_) => (None, None, None),
    };

    InfoRow {
        index: device_index,
        name,
        gpu_clock_mhz,
        gpu_offset_mhz,
        mem_clock_mhz,
        mem_offset_mhz,
        temp_c,
        power_w,
        power_limit_w,
        power_limit_pct,
        power_range,
    }
}

fn print_field<T: std::fmt::Display>(label: &str, unit: &str, val: Option<T>) {
    match val {
        Some(val) => println!("{label}: {val}{unit}"),
        None => println!("{label}: n/a"),
    }
}

/// Legacy single-GPU text output.
pub fn print_gpu_info_text(row: &InfoRow) {
    println!("gpu {}: {}", row.index, row.name);
    print_field("gpu clock", "MHz", row.gpu_clock_mhz);
    print_field("gpu offset", "MHz", row.gpu_offset_mhz);
    print_field("mem clock", "MHz", row.mem_clock_mhz);
    print_field("mem offset", "MHz", row.mem_offset_mhz);
    print_field("temp", "°C", row.temp_c);
    print_field("power", "W", row.power_w);

    match (row.power_limit_w, row.power_limit_pct, row.power_range.as_deref()) {
        (Some(w), Some(pct), Some(range)) => {
            println!("power limit: {w}W ({pct}%)");
            println!("power range: {range}");
        }
        _ => println!("power limit: n/a"),
    }
}
