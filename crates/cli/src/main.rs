mod table;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zinnia", version, about = "Disk hub for Arch Linux")]
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
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Volumes { json } => run_volumes(json),
    };
    std::process::exit(code);
}

fn run_volumes(json: bool) -> i32 {
    match zbus::block_on(zinnia_core::volumes::list_volumes()) {
        Ok(report) => {
            if json {
                println!("{}", table::render_json(&report.drives));
            } else {
                if let Some(reason) = &report.fallback_reason {
                    eprintln!("note: udisks2 unavailable ({reason}); drive grouping is off");
                }
                print!("{}", table::render_table(&report.drives));
            }
            0
        }
        Err(e) => {
            if json {
                println!("[]");
            }
            eprintln!("zinnia: {e}");
            1
        }
    }
}
