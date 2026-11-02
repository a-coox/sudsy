//! `sudsy format [--check] [paths...]`

use anyhow::{Context, Result};
use clap::Args;
use core_format::DirectiveTable;
use provider_api::{LanguageProvider, SourceFile, format_file, resolve_dialect, select_provider};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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

pub fn run(args: FormatArgs) -> Result<ExitCode> {
    let config = match core_config::discover(&std::env::current_dir()?)? {
        Some(path) => core_config::load_file(&path)?,
        None => core_config::Config::default(),
    };
    let providers = providers();
    let mut summary = Summary::default();
    for path in core_workspace::discover_files(&args.paths)? {
        let Some((provider, dialect)) = select_provider(&providers, &path) else {
            continue;
        };
        let options =
            config.format_options(&provider.grammar().language, provider.option_schema())?;
        match format_one(provider, dialect, options, &path, args.check) {
            Ok(changed) => {
                summary.formatted += 1;
                if changed {
                    summary.changed += 1;
                    if args.check {
                        println!("would reformat {}", path.display());
                    }
                }
            }
            Err(e) => {
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

/// Formats one file; returns whether its content changed (and was written unless `check`).
fn format_one(
    provider: &dyn LanguageProvider,
    matcher_dialect: Option<String>,
    mut options: core_config::ResolvedOptions,
    path: &Path,
    check: bool,
) -> Result<bool> {
    let bytes = std::fs::read(path).context("reading the file")?;
    let mut source = SourceFile::from_bytes(path.to_string_lossy(), 1, bytes)?;
    source.language_version = resolve_dialect(&mut options, matcher_dialect);
    let output = format_file(provider, &source, &options, &DirectiveTable::default())?;
    if output == source.text {
        return Ok(false);
    }
    if !check {
        let mut bytes = Vec::with_capacity(output.len() + 3);
        if source.had_bom {
            bytes.extend_from_slice("\u{feff}".as_bytes());
        }
        bytes.extend_from_slice(output.as_bytes());
        std::fs::write(path, bytes).context("writing the file")?;
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
