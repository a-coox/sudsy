//! Rule diagnostics as LSP diagnostics.

use crate::position::Positions;
use crate::workspace::Workspace;
use core_incremental::{Failure, Stage, lint, project_diagnostics};
use core_rules::{Diagnostic, Severity};
use lsp_types::{DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Uri};
use std::path::Path;

pub const SOURCE: &str = "sudsy";

pub(crate) fn to_lsp(
    d: &Diagnostic,
    positions: &Positions<'_>,
    uri: &Uri,
) -> lsp_types::Diagnostic {
    let related: Vec<_> = d
        .related
        .iter()
        .map(|r| DiagnosticRelatedInformation {
            location: Location::new(
                uri.clone(),
                positions.range(r.range.start().into(), r.range.end().into()),
            ),
            message: r.message.clone(),
        })
        .collect();
    lsp_types::Diagnostic {
        range: positions.range(d.range.start().into(), d.range.end().into()),
        severity: Some(match d.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        code: Some(NumberOrString::String(d.rule.clone())),
        source: Some(SOURCE.to_string()),
        message: d.message.clone(),
        related_information: (!related.is_empty()).then_some(related),
        ..Default::default()
    }
}

impl Workspace {
    /// Why the semantic rules had nothing to look at in an open document, if they did not.
    pub fn semantic_warning(&self, path: &Path) -> Option<Failure> {
        let linted = lint(&self.db, self.file(path)?).as_ref().ok()?;
        Some(Failure {
            stage: Stage::Semantics,
            message: format!(
                "semantic rules are skipped: {}",
                linted.semantics_unavailable.as_ref()?
            ),
        })
    }

    /// The diagnostics of an open document; none for a document that is not open. A failure is
    /// why the rules could not run (config, worker).
    pub fn diagnostics(
        &self,
        path: &Path,
        uri: &Uri,
    ) -> Result<Vec<lsp_types::Diagnostic>, Failure> {
        let Some(file) = self.file(path) else {
            return Ok(Vec::new());
        };
        let linted = lint(&self.db, file).as_ref().map_err(Clone::clone)?;
        let text = file.text(&self.db);
        let positions = Positions::new(text);
        let mut items: Vec<_> = linted
            .diagnostics
            .iter()
            .map(|d| to_lsp(d, &positions, uri))
            .collect();
        // `analysis: deep`: what the project-scope rules found in this file.
        if self.is_deep() {
            let files = self.project_files();
            let project_wide = project_diagnostics(&self.db, &files)?;
            items.extend(
                project_wide
                    .iter()
                    .filter(|(p, _)| p == path)
                    .map(|(_, d)| to_lsp(d, &positions, uri)),
            );
        }
        Ok(items)
    }
}
