//! `sudsy format [--check] [paths...]`

use anyhow::{Context, Result, anyhow};
use clap::Args;
use core_incremental::{Database, File, Stage, config, format};
use provider_api::{LanguageProvider, SourceFile};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Args)]
pub struct FormatArgs {
    /// Report files that would change instead of writing them.
    #[arg(long)]
    check: bool,
    /// Files or directories; the current directory when omitted.
    paths: Vec<PathBuf>,
}

/// Providers that can format. Languages without a formatter yet are not listed, so their files
/// are skipped.
fn providers() -> Vec<Box<dyn LanguageProvider>> {
    vec![
        Box::new(lang_json::JsonProvider::new()),
        Box::new(lang_yaml::YamlProvider::new()),
    ]
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

/// One database per run: the config is an input, and so is every file, created by the thread
/// that reads it.
pub fn run(args: FormatArgs) -> Result<ExitCode> {
    let mut db = Database::new(providers());
    load_config(&mut db)?;
    let paths: Vec<PathBuf> = core_workspace::discover_files(&args.paths)?
        .into_iter()
        .filter(|path| db.supports(path))
        .collect();
    let loaded = load_and_format(&db, &paths);

    // Reported in path order, so the output does not depend on thread timing.
    let mut summary = Summary::default();
    for (path, file) in paths.iter().zip(loaded) {
        match finish(&db, file, args.check) {
            Ok(changed) => {
                summary.formatted += 1;
                if changed {
                    summary.changed += 1;
                    if args.check {
                        println!("would reformat {}", path.display());
                    }
                }
            }
            Err(Failed::Run(e)) => return Err(e),
            Err(Failed::File(e)) => {
                summary.failed += 1;
                eprintln!("error: {}: {e:#}", path.display());
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

/// Hands the nearest `sudsy.yaml` to the engine and fails the run if it is invalid.
fn load_config(db: &mut Database) -> Result<()> {
    if let Some(path) = core_config::discover(&std::env::current_dir()?)? {
        let text = std::fs::read_to_string(&path).map_err(|e| core_config::ConfigError {
            path: Some(path.clone()),
            line: None,
            column: None,
            message: e.to_string(),
        })?;
        db.set_config(Some(path), Some(text));
    }
    match config(db) {
        Ok(_) => Ok(()),
        Err(failure) => Err(anyhow!("{}", failure.message)),
    }
}

/// Reads and formats every file in parallel. Inputs are only ever created, never set, while the
/// workers run: each works through its own handle on the database (a salsa handle shares the
/// memos) and claims the next path from a shared counter. Results come back in `paths` order.
fn load_and_format(db: &Database, paths: &[PathBuf]) -> Vec<Result<File>> {
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(paths.len());
    let next = AtomicUsize::new(0);
    let mut slots: Vec<Option<Result<File>>> = paths.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                let db = db.clone();
                let next = &next;
                scope.spawn(move || {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = paths.get(index) else {
                            break done;
                        };
                        done.push((index, load(&db, path)));
                    }
                })
            })
            .collect();
        for worker in workers {
            match worker.join() {
                Ok(done) => {
                    for (index, result) in done {
                        slots[index] = Some(result);
                    }
                }
                Err(panic) => std::panic::resume_unwind(panic),
            }
        }
    });
    slots
        .into_iter()
        .map(|slot| slot.expect("every path is claimed by one worker"))
        .collect()
}

/// Reads one file into the database and runs the format query on it; the result stays memoized
/// for [`finish`].
fn load(db: &Database, path: &Path) -> Result<File> {
    let bytes = std::fs::read(path).context("reading the file")?;
    let source = SourceFile::from_bytes(path.to_string_lossy(), 1, bytes)?;
    let file = File::new(db, path.to_path_buf(), 1, source.text, source.had_bom);
    format(db, file);
    Ok(file)
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
