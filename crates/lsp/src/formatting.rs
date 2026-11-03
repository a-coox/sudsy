//! `textDocument/formatting`.

use crate::position::minimal_edit;
use crate::workspace::Workspace;
use core_incremental::{Failure, format};
use lsp_types::TextEdit;
use std::path::Path;

impl Workspace {
    /// The edit that formats an open document (none when it is formatted already or not open).
    /// A failure is why the file is left alone: syntax errors, an unavailable worker, an invalid
    /// config.
    pub fn format(&self, path: &Path) -> Result<Vec<TextEdit>, Failure> {
        let Some(file) = self.file(path) else {
            return Ok(Vec::new());
        };
        let formatted = format(&self.db, file).as_ref().map_err(Clone::clone)?;
        Ok(minimal_edit(file.text(&self.db), formatted)
            .into_iter()
            .collect())
    }
}
