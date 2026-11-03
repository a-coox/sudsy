//! `sudsy check [paths...]`: `format --check` and `lint` in one run, for CI. A file that
//! `format` would change is reported like any other diagnostic (rule `format`, an error), so every
//! output format carries it.

use crate::lint::{self, ReportArgs, collect, exit_code};
use anyhow::{Result, anyhow};
use core_incremental::{Stage, format, lint as lint_query};
use core_rules::{Category, Diagnostic, FORMAT, Severity};
use core_syntax::{TextRange, TextSize};
use std::process::ExitCode;

pub fn run(args: ReportArgs) -> Result<ExitCode> {
    let session = args.open()?;
    let loaded = session.load_all(|db, file| {
        lint_query(db, file);
        format(db, file);
    });
    let mut collected = collect(&session, &loaded)?;
    let db = &session.db;
    for (entry, file) in collected.files.iter_mut().zip(&loaded) {
        let Ok(file) = file else { continue };
        // A file that does not parse is reported by `lint`; it has nothing to format.
        if lint_query(db, *file).as_ref().is_ok_and(|l| !l.parses) {
            continue;
        }
        match format(db, *file) {
            Ok(output) if *output == *file.text(db) => {}
            Ok(_) => {
                entry.diagnostics.insert(
                    0,
                    Diagnostic {
                        rule: FORMAT.into(),
                        category: Category::Style,
                        severity: Severity::Error,
                        message: "the file is not formatted; run `sudsy format`".into(),
                        range: TextRange::empty(TextSize::from(0)),
                        related: Vec::new(),
                        fixes: Vec::new(),
                    },
                );
            }
            Err(failure) if matches!(failure.stage, Stage::Options | Stage::Config) => {
                return Err(anyhow!("{}", failure.message));
            }
            Err(failure) => {
                collected.failed += 1;
                crate::report::file_error(&entry.path, &failure.message);
            }
        }
    }
    lint::print(&session, args.format, &collected.files);
    Ok(exit_code(&collected))
}
