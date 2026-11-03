//! `sudsy format [--check] [paths...]`

use crate::session::Session;
use anyhow::{Context, Result, anyhow};
use clap::Args;
use core_incremental::{Database, File, Stage, format};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Args)]
pub struct FormatArgs {
    /// Report files that would change instead of writing them.
    #[arg(long)]
    check: bool,
    /// Files or directories; the current directory when omitted.
    paths: Vec<PathBuf>,
}

#[derive(Default)]
struct Summary {
    formatted: usize,
    changed: usize,
    failed: usize,
}

/// Why a file did not format.
enum Failed {
    /// Only this file; the run goes on.
    File(anyhow::Error),
    /// Invalid format options: the run stops, as it always has.
    Run(anyhow::Error),
}

pub fn run(args: FormatArgs) -> Result<ExitCode> {
    let session = Session::open(&args.paths, None)?;
    let (db, project) = (&session.db, &session.project);
    let loaded = session.load_all(|db, file| {
        format(db, file);
    });

    // Reported in path order, so the output does not depend on thread timing.
    let mut summary = Summary::default();
    for (path, file) in session.paths.iter().zip(loaded) {
        match finish(db, file, args.check) {
            Ok(changed) => {
                summary.formatted += 1;
                if changed {
                    summary.changed += 1;
                    if args.check {
                        println!("would reformat {}", project.show(path));
                    }
                }
            }
            Err(Failed::Run(e)) => return Err(e),
            Err(Failed::File(e)) => {
                summary.failed += 1;
                eprintln!("error: {}: {e:#}", project.show(path));
            }
        }
    }
    report(&summary, args.check);
    Ok(if summary.failed > 0 {
        ExitCode::from(2)
    } else if args.check && summary.changed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
/// Writes the formatted file unless `check`; returns whether its content changed.
fn finish(db: &Database, file: Result<File>, check: bool) -> Result<bool, Failed> {
    let file = file.map_err(Failed::File)?;
    let output = match format(db, file) {
        Ok(output) => output,
        Err(failure) if matches!(failure.stage, Stage::Options | Stage::Config) => {
            return Err(Failed::Run(anyhow!("{}", failure.message)));
        }
        Err(failure) => return Err(Failed::File(anyhow!("{}", failure.message))),
    };
    if *output == *file.text(db) {
        return Ok(false);
    }
    if !check {
        let mut bytes = Vec::with_capacity(output.len() + 3);
        if file.had_bom(db) {
            bytes.extend_from_slice("\u{feff}".as_bytes());
        }
        bytes.extend_from_slice(output.as_bytes());
        std::fs::write(file.path(db), bytes)
            .context("writing the file")
            .map_err(Failed::File)?;
    }
    Ok(true)
}

fn report(s: &Summary, check: bool) {
    let verb = if check {
        "would reformat"
    } else {
        "reformatted"
    };
    eprintln!(
        "{} file(s) checked, {} {verb}, {} unchanged{}",
        s.formatted + s.failed,
        s.changed,
        s.formatted - s.changed,
        if s.failed > 0 {
            format!(", {} failed", s.failed)
        } else {
            String::new()
        }
    );
}
