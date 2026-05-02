//! NVOC - NVIDIA GPU overclocking utility for Linux
//!
//! Command-line utility for GPU overclocking using NVML.
//! Designed for RTX 5000 series GPUs with nvidia-open drivers.

use std::process;

mod cli;
mod constants;
mod gpu;
mod nvml;
mod output;

use cli::{DeviceSelector, Operation};
use nvml::NvmlError;

pub struct AppError {
    domain: &'static str,
    source: Option<NvmlError>,
    message: Option<String>,
    printed: bool,
}

impl AppError {
    pub fn new(domain: &'static str, source: NvmlError) -> Self {
        Self { domain, source: Some(source), message: None, printed: false }
    }

    pub fn msg(domain: &'static str, message: String) -> Self {
        Self { domain, source: None, message: Some(message), printed: false }
    }

    pub fn printed(domain: &'static str) -> Self {
        Self { domain, source: None, message: None, printed: true }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match (&self.source, &self.message) {
            (Some(source), _) => write!(f, "error[{}]: {}", self.domain, source.user_message()),
            (_, Some(msg)) => write!(f, "error[{}]: {}", self.domain, msg),
            _ => write!(f, "error[{}]", self.domain),
        }
    }
}

fn run() -> Result<(), AppError> {
    let config = cli::Config::from_args().unwrap_or_else(|e| e.exit());

    if config.operation.modifies_gpu() {
        gpu::validation::check_system_for_modification()
            .map_err(|e| AppError::new("nvoc", e))?;
    }

    let _cleanup = gpu::init_with_cleanup()?;
    let devices = gpu::select_devices(&config.selector).map_err(|e| AppError::new("device", e))?;
    if devices.is_empty() {
        return Err(AppError::msg("device", "no GPUs matched selector".to_string()));
    }

    match config.operation {
        Operation::Info => {
            let version = gpu::driver_version().map_err(|e| AppError::new("driver", e))?;
            println!("driver: {version}");
            let table = !config.no_table && !matches!(config.selector, DeviceSelector::Single(_));
            if table {
                print_info_table(&devices);
            } else {
                for d in devices {
                    let row = gpu::info::get_gpu_info(d.device, d.index, d.name);
                    gpu::info::print_gpu_info_text(&row);
                }
            }
        }
        Operation::Reset { dry_run } => {
            let table = !config.no_table;
            if table {
                let mut any_err = false;
                let mut t = output::table::Table::new(vec![
                    output::table::Column { header: "idx", align: output::table::Align::Right, max_width: None },
                    output::table::Column { header: "name", align: output::table::Align::Left, max_width: Some(40) },
                    output::table::Column { header: "dry_run", align: output::table::Align::Left, max_width: None },
                    output::table::Column { header: "status", align: output::table::Align::Left, max_width: None },
                    output::table::Column { header: "error", align: output::table::Align::Left, max_width: Some(80) },
                ]);

                for d in devices {
                    let mut status = "OK".to_string();
                    let mut err = "-".to_string();

                    if let Err(e) = gpu::validation::validate_blackwell_architecture(d.device) {
                        status = "ERR".to_string();
                        err = e.user_message().to_string();
                        any_err = true;
                    } else if let Err(e) = gpu::reset::reset_gpu_settings(d.device, dry_run) {
                        status = "ERR".to_string();
                        err = e.to_string();
                        any_err = true;
                    }

                    t.push_row([
                        d.index.to_string(),
                        d.name,
                        dry_run.to_string(),
                        status,
                        err,
                    ]);
                }
                t.print();
                if any_err {
                    return Err(AppError::printed("reset"));
                }
            } else {
                let multi = devices.len() > 1;
                for d in devices {
                    if multi {
                        println!("gpu {}: {}", d.index, d.name);
                    }
                    gpu::validation::validate_blackwell_architecture(d.device)
                        .map_err(|e| AppError::new("gpu", e))?;
                    gpu::reset::reset_gpu_settings(d.device, dry_run)?;
                    if dry_run {
                        println!("reset: all (dry run)");
                    } else {
                        println!("reset: all");
                    }
                }
            }
        }
        Operation::Overclock(ref params) => {
            let table = !config.no_table;
            if table {
                let mut any_err = false;
                let mut t = output::table::Table::new(vec![
                    output::table::Column { header: "idx", align: output::table::Align::Right, max_width: None },
                    output::table::Column { header: "name", align: output::table::Align::Left, max_width: Some(40) },
                    output::table::Column { header: "clocks", align: output::table::Align::Right, max_width: None },
                    output::table::Column { header: "gpu_off", align: output::table::Align::Right, max_width: None },
                    output::table::Column { header: "mem_off", align: output::table::Align::Right, max_width: None },
                    output::table::Column { header: "power", align: output::table::Align::Right, max_width: None },
                    output::table::Column { header: "dry_run", align: output::table::Align::Left, max_width: None },
                    output::table::Column { header: "status", align: output::table::Align::Left, max_width: None },
                    output::table::Column { header: "error", align: output::table::Align::Left, max_width: Some(80) },
                ]);

                for d in devices {
                    let mut status = "OK".to_string();
                    let mut err = "-".to_string();
                    let mut clocks = "-".to_string();
                    let mut gpu_off = "-".to_string();
                    let mut mem_off = "-".to_string();
                    let mut power = "-".to_string();

                    if let Err(e) = gpu::validation::validate_blackwell_architecture(d.device) {
                        status = "ERR".to_string();
                        err = e.user_message().to_string();
                        any_err = true;
                    } else {
                        match gpu::overclock::apply(d.device, params) {
                            Ok(summary) => {
                                if let Some(v) = summary.clocks { clocks = v; }
                                if let Some(v) = summary.graphics_offset { gpu_off = v; }
                                if let Some(v) = summary.memory_offset { mem_off = v; }
                                if let Some(v) = summary.power { power = v; }
                            }
                            Err(e) => {
                                status = "ERR".to_string();
                                err = e.to_string();
                                any_err = true;
                            }
                        }
                    }

                    t.push_row([
                        d.index.to_string(),
                        d.name,
                        clocks,
                        gpu_off,
                        mem_off,
                        power,
                        params.dry_run.to_string(),
                        status,
                        err,
                    ]);
                }
                t.print();
                if any_err {
                    return Err(AppError::printed("nvoc"));
                }
            } else {
                let multi = devices.len() > 1;
                for d in devices {
                    if multi {
                        println!("gpu {}: {}", d.index, d.name);
                    }
                    gpu::validation::validate_blackwell_architecture(d.device)
                        .map_err(|e| AppError::new("gpu", e))?;
                    let summary = gpu::overclock::apply(d.device, params)?;
                    print_overclock_summary_text(&summary, params.dry_run);
                }
            }
        }
    };

    Ok(())
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| "n/a".to_string())
}

fn print_info_table(devices: &[gpu::DeviceRef]) {
    let mut t = output::table::Table::new(vec![
        output::table::Column { header: "idx", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "name", align: output::table::Align::Left, max_width: Some(40) },
        output::table::Column { header: "gpu_clk", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "gpu_off", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "mem_clk", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "mem_off", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "temp", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "power", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "pwr_w", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "pwr_%", align: output::table::Align::Right, max_width: None },
        output::table::Column { header: "pwr_range", align: output::table::Align::Left, max_width: Some(28) },
    ]);

    for d in devices {
        let row = gpu::info::get_gpu_info(d.device, d.index, d.name.clone());
        t.push_row([
            row.index.to_string(),
            row.name,
            opt(row.gpu_clock_mhz),
            opt(row.gpu_offset_mhz),
            opt(row.mem_clock_mhz),
            opt(row.mem_offset_mhz),
            opt(row.temp_c),
            opt(row.power_w),
            opt(row.power_limit_w),
            opt(row.power_limit_pct),
            row.power_range.unwrap_or_else(|| "n/a".to_string()),
        ]);
    }
    t.print();
}

fn print_overclock_summary_text(summary: &gpu::overclock::OverclockSummary, dry_run: bool) {
    let suffix = if dry_run { " (dry run)" } else { "" };
    if let Some(v) = &summary.clocks {
        println!("clocks: {v}{suffix}");
    }
    if let Some(v) = &summary.graphics_offset {
        println!("gpu offset: {v}{suffix}");
    }
    if let Some(v) = &summary.memory_offset {
        println!("mem offset: {v}{suffix}");
    }
    if let Some(v) = &summary.power {
        println!("power limit: {v}{suffix}");
    }
}

fn main() {
    if let Err(e) = run() {
        if !e.printed {
            eprintln!("{e}");
        }
        process::exit(1);
    }
}
