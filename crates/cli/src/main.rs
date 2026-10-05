mod table;

use clap::{Parser, Subcommand};
use gazania_core::client::Client;
use gazania_core::io::IoSampler;
use gazania_core::stream::{self, Event, Intervals, Kinds};
use std::io::Write;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "gazania", version, about = "Disk hub for Arch Linux")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List drives and volumes with size, usage and mount points
    Volumes {
        /// Emit the drive tree as JSON with raw byte counts
        #[arg(long)]
        json: bool,
    },
    /// Drive temperature, power-on hours and SMART status
    Health {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Read and write throughput per drive, sampled over one second
    Io {
        /// Emit JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Stream volumes, throughput and health as JSON lines until stdout closes
    Watch {
        /// Comma-separated subset of volumes,io,health
        #[arg(long, default_value = "volumes,io,health", value_parser = Kinds::parse)]
        only: Kinds,
        /// Seconds between throughput samples
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..=86400))]
        io_interval: u64,
        /// Seconds between health readings
        #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u64).range(1..=86400))]
        health_interval: u64,
        /// Seconds between volume usage refreshes
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=86400))]
        usage_interval: u64,
    },
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Volumes { json } => run_volumes(json),
        Command::Health { json } => run_health(json),
        Command::Io { json } => run_io(json),
        Command::Watch {
            only,
            io_interval,
            health_interval,
            usage_interval,
        } => run_watch(
            only,
            Intervals {
                io: Duration::from_secs(io_interval),
                health: Duration::from_secs(health_interval),
                usage: Duration::from_secs(usage_interval),
            },
        ),
    };
    std::process::exit(code);
}

fn run_volumes(json: bool) -> i32 {
    match zbus::block_on(gazania_core::volumes::list_volumes()) {
        Ok(report) => {
            if let Some(reason) = &report.fallback_reason {
                eprintln!("note: udisks2 unavailable ({reason}); drive grouping is off");
            }
            if json {
                println!("{}", table::render_json(&report.drives));
            } else {
                print!("{}", table::render_table(&report.drives));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("gazania: {e}");
            1
        }
    }
}

fn run_health(json: bool) -> i32 {
    let result = zbus::block_on(async { Client::connect().await?.health().await });
    match result {
        Ok(health) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&health).unwrap_or_else(|_| "[]".into())
                );
            } else {
                print!("{}", table::render_health_table(&health));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("gazania: {e}");
            1
        }
    }
}

fn run_io(json: bool) -> i32 {
    let result = zbus::block_on(async {
        let drives = Client::connect().await?.drives().await?;
        let mut sampler = IoSampler::new();
        sampler.sample(&drives)?;
        std::thread::sleep(Duration::from_secs(1));
        sampler.sample(&drives)
    });
    match result {
        Ok(rates) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rates).unwrap_or_else(|_| "[]".into())
                );
            } else {
                print!("{}", table::render_io_table(&rates));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("gazania: {e}");
            1
        }
    }
}

/// One JSON object per line; a failed write (the reader went away) ends the
/// stream with exit code 0.
fn run_watch(kinds: Kinds, intervals: Intervals) -> i32 {
    let stdout = std::io::stdout();
    let mut emit = |event: &Event| -> std::io::Result<()> {
        let mut out = stdout.lock();
        serde_json::to_writer(&mut out, event).map_err(std::io::Error::other)?;
        out.write_all(b"\n")?;
        out.flush()
    };
    zbus::block_on(stream::run(kinds, intervals, &mut emit));
    0
}
