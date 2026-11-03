//! `sudsy fix [--unsafe] [paths...]`: applies fixes until none is left (plan.md fix loop), then
//! formats, and reports what remains.

use crate::lint::{self, Collected, ReportArgs, exit_code};
use crate::project::Project;
use crate::report::FileDiagnostics;
use crate::session::{Session, par_map};
use anyhow::{Context, Result, anyhow};
use clap::Args;
use core_config::EditorConfig;
use core_incremental::{Database, File, Stage, format, lint as lint_query};
use core_rules::{Analysis, Diagnostic, fix_loop};
use provider_api::SourceFile;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Args)]
pub struct FixArgs {
    #[command(flatten)]
    report: ReportArgs,
    /// Also apply fixes that can change behavior or need a newer language version.
    #[arg(long = "unsafe")]
    allow_unsafe: bool,
}

/// What happened to one file.
struct Fixed {
    path: String,
    text: String,
    changed: bool,
    diagnostics: Vec<Diagnostic>,
    /// Rules whose fixes made the file unparseable, so that round was not applied.
    broke: Option<Vec<String>>,
}

/// Why a file was not processed.
enum Failed {
    File(anyhow::Error),
    /// The config or the format options are invalid: the run stops.
    Run(anyhow::Error),
}

impl From<core_incremental::Failure> for Failed {
    fn from(f: core_incremental::Failure) -> Failed {
        let e = anyhow!("{}", f.message);
        if matches!(f.stage, Stage::Options | Stage::Config) {
            Failed::Run(e)
        } else {
            Failed::File(e)
        }
    }
}

/// A new database input for `text`: the same file at the next version.
fn input(db: &Database, path: &Path, version: &AtomicU64, text: &str, had_bom: bool) -> File {
    File::new(
        db,
        path.to_path_buf(),
        version.fetch_add(1, Ordering::Relaxed),
        text.to_string(),
        had_bom,
        EditorConfig::for_file(path),
    )
}

fn fix_file(
    db: &Database,
    project: &Project,
    path: &Path,
    allow_unsafe: bool,
) -> Result<Fixed, Failed> {
    let bytes = std::fs::read(path)
        .context("reading the file")
        .map_err(Failed::File)?;
    let source =
        SourceFile::from_bytes(project.show(path), 1, bytes).map_err(|e| Failed::File(e.into()))?;
    let version = AtomicU64::new(1);
    let analyze = |text: &str| -> Result<Analysis, Failed> {
        let file = input(db, path, &version, text, source.had_bom);
        let linted = lint_query(db, file)
            .as_ref()
            .map_err(|f| Failed::from(f.clone()))?;
        Ok(Analysis {
            diagnostics: linted.diagnostics.clone(),
            parses: linted.parses,
        })
    };
    let outcome = fix_loop(
        source.text.clone(),
        |fix| fix.kind.applies(allow_unsafe),
        analyze,
    )?;
    let mut text = outcome.text;
    // Then format; a file that does not parse is left as it is.
    let file = input(db, path, &version, &text, source.had_bom);
    let linted = lint_query(db, file)
        .as_ref()
        .map_err(|f| Failed::from(f.clone()))?;
    if linted.parses {
        text = format(db, file)
            .as_ref()
            .map_err(|f| Failed::from(f.clone()))?
            .clone();
    }
    let changed = text != source.text;
    if changed {
        let mut bytes = Vec::with_capacity(text.len() + 3);
        if source.had_bom {
            bytes.extend_from_slice("\u{feff}".as_bytes());
        }
        bytes.extend_from_slice(text.as_bytes());
        std::fs::write(path, bytes)
            .context("writing the file")
            .map_err(Failed::File)?;
    }
    let after = input(db, path, &version, &text, source.had_bom);
    let diagnostics = lint_query(db, after)
        .as_ref()
        .map_err(|f| Failed::from(f.clone()))?
        .diagnostics
        .clone();
    Ok(Fixed {
        path: project.show(path),
        text,
        changed,
        diagnostics,
        broke: outcome.broke,
    })
}

pub fn run(args: FixArgs) -> Result<ExitCode> {
    let session: Session = args.report.open()?;
    let (db, project) = (&session.db, &session.project);
    let results = par_map(db, &session.paths, |db, path| {
        fix_file(db, project, path, args.allow_unsafe)
    });
    let mut collected = Collected {
        files: Vec::new(),
        failed: 0,
    };
    let mut changed = 0;
    for (path, result) in session.paths.iter().zip(results) {
        match result {
            Ok(fixed) => {
                changed += usize::from(fixed.changed);
                if let Some(rules) = &fixed.broke {
                    eprintln!(
                        "warning: {}: fixes of {} would leave the file unparseable and were not applied",
                        fixed.path,
                        rules.join(", ")
                    );
                }
                collected.files.push(FileDiagnostics {
                    path: fixed.path,
                    text: fixed.text,
                    diagnostics: fixed.diagnostics,
                });
            }
            Err(Failed::Run(e)) => return Err(e),
            Err(Failed::File(e)) => {
                collected.failed += 1;
                eprintln!("error: {}: {e:#}", project.show(path));
            }
        }
    }
    eprintln!("{changed} file(s) changed");
    lint::print(&session, args.report.format, &collected.files);
    Ok(exit_code(&collected))
}
