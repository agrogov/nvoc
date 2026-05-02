//! Command-line interface parsing and configuration

use crate::constants::app;
use clap::{Arg, Command};

fn device_arg() -> Arg {
    Arg::new("device")
        .short('d')
        .long("device")
        .value_name("INDEX")
        .help("GPU index")
        .global(true)
        .value_parser(clap::value_parser!(u32))
}

fn match_arg() -> Arg {
    Arg::new("match")
        .long("match")
        .value_name("REGEX")
        .help("Match GPUs by device name (regex)")
        .global(true)
        .value_parser(clap::value_parser!(String))
}

fn all_arg() -> Arg {
    Arg::new("all")
        .long("all")
        .help("Target all GPUs")
        .global(true)
        .action(clap::ArgAction::SetTrue)
}

fn no_table_arg() -> Arg {
    Arg::new("no-table")
        .long("no-table")
        .help("Disable table output (use legacy text output)")
        .global(true)
        .action(clap::ArgAction::SetTrue)
}

fn dry_run_arg() -> Arg {
    Arg::new("dry-run")
        .long("dry-run")
        .help("Preview")
        .global(true)
        .action(clap::ArgAction::SetTrue)
}

#[derive(Debug)]
pub struct OverclockParams {
    pub clocks: Option<(u32, u32)>,
    pub graphics_offset: Option<i32>,
    pub memory_offset: Option<i32>,
    pub power_limit: Option<u32>,
    pub dry_run: bool,
}

#[derive(Debug)]
pub enum Operation {
    Info,
    Reset { dry_run: bool },
    Overclock(OverclockParams),
}

impl Operation {
    pub fn modifies_gpu(&self) -> bool {
        matches!(self, Operation::Reset { .. } | Operation::Overclock(_))
    }
}

#[derive(Debug)]
pub struct Config {
    pub selector: DeviceSelector,
    pub no_table: bool,
    pub operation: Operation,
}

#[derive(Debug, Clone)]
pub enum DeviceSelector {
    Single(u32),
    All,
    Match(String),
}

fn parse_clocks(s: &str) -> std::result::Result<(u32, u32), &'static str> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err("Clock format must be 'min,max'");
    }

    let min = parts[0]
        .parse::<u32>()
        .map_err(|_| "Invalid minimum clock value")?;
    let max = parts[1]
        .parse::<u32>()
        .map_err(|_| "Invalid maximum clock value")?;

    if min >= max {
        return Err("Minimum clock must be less than maximum clock");
    }

    Ok((min, max))
}

impl Config {
    pub fn from_args() -> Result<Self, clap::Error> {
        let matches = Command::new(app::NAME)
            .version(app::VERSION)
            .author(app::AUTHOR)
            .about(app::DESCRIPTION)
            .subcommand_required(false)
            .subcommand(
                Command::new("reset")
                    .about("Reset GPU to defaults")
                    .arg(dry_run_arg()),
            )
            .subcommand(
                Command::new("info")
                    .about("Show GPU information")
                    ,
            )
            .arg(
                Arg::new("clocks")
                    .short('c')
                    .long("clocks")
                    .value_name("MIN,MAX")
                    .help("GPU clocks MHz")
                    .value_parser(parse_clocks),
            )
            .arg(
                Arg::new("offset")
                    .short('o')
                    .long("offset")
                    .value_name("GRAPHICS_OFFSET")
                    .help("GPU offset MHz")
                    .allow_hyphen_values(true)
                    .value_parser(clap::value_parser!(i32)),
            )
            .arg(
                Arg::new("memory-offset")
                    .short('m')
                    .long("memory-offset")
                    .value_name("MEMORY_OFFSET")
                    .help("Mem offset MHz")
                    .allow_hyphen_values(true)
                    .value_parser(clap::value_parser!(i32)),
            )
            .arg(
                Arg::new("power")
                    .short('p')
                    .long("power")
                    .value_name("PERCENT")
                    .help("Power limit %")
                    .value_parser(clap::value_parser!(u32)),
            )
            .arg(device_arg())
            .arg(match_arg())
            .arg(all_arg())
            .arg(no_table_arg())
            .group(
                clap::ArgGroup::new("selector")
                    .args(["device", "match", "all"])
                    .multiple(false),
            )
            .arg(dry_run_arg())
            .get_matches();

        let operation = match matches.subcommand() {
            Some(("reset", sub_matches)) => Operation::Reset {
                dry_run: sub_matches.get_flag("dry-run"),
            },
            Some(("info", _sub_matches)) => Operation::Info,
            _ => {
                let clocks = matches.get_one::<(u32, u32)>("clocks").copied();
                let graphics_offset = matches.get_one::<i32>("offset").copied();
                let memory_offset = matches.get_one::<i32>("memory-offset").copied();
                let power_limit = matches.get_one::<u32>("power").copied();

                if clocks.is_none()
                    && graphics_offset.is_none()
                    && memory_offset.is_none()
                    && power_limit.is_none()
                {
                    return Err(Command::new(app::NAME)
                        .error(clap::error::ErrorKind::MissingRequiredArgument, "No operation specified. Use a subcommand (info, reset) or provide overclock options (-c, -o, -m, -p)."));
                }

                Operation::Overclock(OverclockParams {
                    clocks,
                    graphics_offset,
                    memory_offset,
                    power_limit,
                    dry_run: matches.get_flag("dry-run"),
                })
            }
        };

        let no_table = matches.get_flag("no-table");
        let device = matches.get_one::<u32>("device").copied();
        let match_regex = matches.get_one::<String>("match").cloned();
        let all = matches.get_flag("all");

        let selector = if let Some(device) = device {
            DeviceSelector::Single(device)
        } else if all {
            DeviceSelector::All
        } else if let Some(re) = match_regex {
            DeviceSelector::Match(re)
        } else {
            match operation {
                Operation::Info => DeviceSelector::All,
                Operation::Reset { .. } | Operation::Overclock(_) => DeviceSelector::Single(0),
            }
        };

        Ok(Config {
            selector,
            no_table,
            operation,
        })
    }
}
