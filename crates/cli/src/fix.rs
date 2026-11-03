//! `sudsy fix [--unsafe] [paths...]`: applies fixes until none is left, then
//! formats, and reports what remains.

use crate::lint::{self, Collected, ReportArgs, exit_code};
use crate::report::FileDiagnostics;
use crate::session::{Session, load, par_map};
use anyhow::{Context, Result, anyhow};
use clap::Args;
use core_incremental::{Database, Stage, fix_text, format, lint as lint_query, transient};
use core_rules::Diagnostic;
use languages::Project;
use std::path::Path;
use std::process::ExitCode;

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

fn fix_file(
    db: &Database,
    project: &Project,
    path: &Path,
    allow_unsafe: bool,
) -> Result<Fixed, Failed> {
    let file = load(db, project, path).map_err(Failed::File)?;
    let (before, had_bom) = (file.text(db).clone(), file.had_bom(db));
    let outcome = fix_text(db, file, |fix| fix.kind.applies(allow_unsafe))?;
    let mut text = outcome.text;
    // Then format; a file that does not parse is left as it is.
    let fixed = transient(db, file, &text);
    let linted = lint_query(db, fixed)
        .as_ref()
        .map_err(|f| Failed::from(f.clone()))?;
    if linted.parses {
        text = format(db, fixed)
            .as_ref()
            .map_err(|f| Failed::from(f.clone()))?
            .clone();
    }
    let changed = text != before;
    if changed {
        let mut bytes = Vec::with_capacity(text.len() + 3);
        if had_bom {
            bytes.extend_from_slice("\u{feff}".as_bytes());
        }
        bytes.extend_from_slice(text.as_bytes());
        crate::atomic::write(path, &bytes)
            .context("writing the file")
            .map_err(Failed::File)?;
    }
    let after = transient(db, file, &text);
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
                crate::report::file_error(&project.show(path), &format!("{e:#}"));
            }
        }
    }
    eprintln!("{changed} file(s) changed");
    lint::print(&session, args.report.format, &collected.files);
    Ok(exit_code(&collected))
}
