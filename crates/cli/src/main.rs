mod debug;
mod format;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "sudsy",
    version,
    about = "Formatter and linter for C#, JSON and YAML"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Developer tools for inspecting the foundations (M0).
    #[command(subcommand)]
    Debug(debug::DebugCommand),
    /// Format files (JSON, JSONC and YAML so far); languages without a formatter are skipped.
    Format(format::FormatArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Debug(c) => debug::run(c).map(|()| ExitCode::SUCCESS),
        Command::Format(args) => format::run(args),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
