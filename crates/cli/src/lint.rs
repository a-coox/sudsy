//! `sudsy lint [paths...]`, and what `check` and `fix` share with it: gathering the diagnostics of
//! the files and printing them.

use crate::report::{self, Catalog, FileDiagnostics, OutputFormat};
use crate::session::Session;
use anyhow::{Result, anyhow};
use clap::{Args, ValueEnum};
use core_config::Depth;
use core_incremental::{File, Stage, lint, project_diagnostics};
use core_rules::{SYNTAX_ERROR, Severity};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Analysis {
    Fast,
    Standard,
    Deep,
}

impl From<Analysis> for Depth {
    fn from(a: Analysis) -> Depth {
        match a {
            Analysis::Fast => Depth::Fast,
            Analysis::Standard => Depth::Standard,
            Analysis::Deep => Depth::Deep,
        }
    }
}

/// The options of every command that reports diagnostics.
#[derive(Args)]
pub struct ReportArgs {
    /// Files or directories; the current directory when omitted.
    pub paths: Vec<PathBuf>,
    /// How diagnostics are printed.
    #[arg(long, value_enum, default_value_t)]
    pub format: OutputFormat,
    /// Analysis depth; overrides `analysis` in sudsy.yaml.
    #[arg(long, value_enum)]
    pub analysis: Option<Analysis>,
}

impl ReportArgs {
    pub fn open(&self) -> Result<Session> {
        Session::open(&self.paths, self.analysis.map(Depth::from))
    }
}

/// The diagnostics of all files, and how many files could not be linted.
pub struct Collected {
    pub files: Vec<FileDiagnostics>,
    pub failed: usize,
}

/// Reads the memoized `lint` results of `loaded` (in `session.paths` order) and the project phase.
/// A file that could not be read or linted is reported on stderr and counted.
pub fn collect(session: &Session, loaded: &[anyhow::Result<File>]) -> Result<Collected> {
    let (db, project) = (&session.db, &session.project);
    let mut out = Collected {
        files: Vec::new(),
        failed: 0,
    };
    let mut ok_files = Vec::new();
    let mut warned = std::collections::HashSet::new();
    for (path, file) in session.paths.iter().zip(loaded) {
        let file = match file {
            Ok(file) => *file,
            Err(e) => {
                out.failed += 1;
                eprintln!("error: {}: {e:#}", project.show(path));
                continue;
            }
        };
        match lint(db, file) {
            Ok(linted) => {
                if let Some(why) = &linted.semantics_unavailable
                    && warned.insert(why.clone())
                {
                    eprintln!("warning: semantic rules are skipped: {why}");
                }
                ok_files.push(file);
                out.files.push(FileDiagnostics {
                    path: project.show(path),
                    text: file.text(db).clone(),
                    diagnostics: linted.diagnostics.clone(),
                });
            }
            Err(failure) if matches!(failure.stage, Stage::Options | Stage::Config) => {
                return Err(anyhow!("{}", failure.message));
            }
            Err(failure) => {
                out.failed += 1;
                report::file_error(&project.show(path), &failure.message);
            }
        }
    }
    let project_wide =
        project_diagnostics(db, &ok_files).map_err(|failure| anyhow!("{}", failure.message))?;
    for (path, diagnostic) in project_wide {
        let shown = project.show(&path);
        if let Some(file) = out.files.iter_mut().find(|f| f.path == shown) {
            file.diagnostics.push(diagnostic);
        }
    }
    Ok(out)
}

/// The rule table SARIF carries: every rule, plus the two ids that are not rules.
pub fn catalog(session: &Session) -> Catalog {
    use core_incremental::Db;
    let mut rules: Vec<(String, String)> =
        languages::rules(session.db.providers(), &session.project.extensions)
            .iter()
            .map(|(_, r)| (r.meta().id.to_string(), r.meta().description.to_string()))
            .collect();
    rules.dedup();
    rules.push((SYNTAX_ERROR.into(), "The file does not parse.".into()));
    rules.push((
        core_rules::FORMAT.into(),
        "The file is not formatted; run `sudsy format`.".into(),
    ));
    rules
}

/// Prints the report on stdout and a one-line summary on stderr.
pub fn print(session: &Session, format: OutputFormat, files: &[FileDiagnostics]) {
    print!("{}", report::render(format, files, &catalog(session)));
    let (errors, warnings) = report::count(files);
    eprintln!(
        "{} file(s) checked, {errors} error(s), {warnings} warning(s)",
        files.len()
    );
}

pub fn has_errors(files: &[FileDiagnostics]) -> bool {
    files
        .iter()
        .flat_map(|f| &f.diagnostics)
        .any(|d| d.severity == Severity::Error)
}

/// Exit code of the commands that report: 2 when a file could not be processed, else 1 on errors.
pub fn exit_code(collected: &Collected) -> ExitCode {
    if collected.failed > 0 {
        ExitCode::from(2)
    } else if has_errors(&collected.files) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

pub fn run(args: ReportArgs) -> Result<ExitCode> {
    let session = args.open()?;
    let loaded = session.load_all(|db, file| {
        lint(db, file);
    });
    let collected = collect(&session, &loaded)?;
    print(&session, args.format, &collected.files);
    Ok(exit_code(&collected))
}
