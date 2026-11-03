//! Rule diagnostics as LSP diagnostics.

use crate::position::Positions;
use crate::workspace::Workspace;
use core_incremental::{Failure, lint};
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
        Ok(linted
            .diagnostics
            .iter()
            .map(|d| to_lsp(d, &positions, uri))
            .collect())
    }
}
