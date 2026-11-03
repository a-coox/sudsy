//! Fixing a text through the database: the fix loop of `core-rules` with the `lint` query as its
//! analysis. Shared by `sudsy fix` and the editor's "fix all" actions.

use crate::db::Db;
use crate::failure::Failure;
use crate::inputs::File;
use crate::rules::lint;
use core_rules::{Analysis, Fix, FixOutcome, fix_loop};
use std::sync::atomic::{AtomicU64, Ordering};

/// Versions of intermediate texts start here, far above any version an editor counts up to.
const TRANSIENT_VERSIONS: u64 = 1 << 62;
static NEXT_TRANSIENT: AtomicU64 = AtomicU64::new(TRANSIENT_VERSIONS);

/// A new input holding `text` for the file `like` stands for: same path, byte-order mark and
/// `.editorconfig` values, a version of its own. Intermediate texts (after a round of fixes, or
/// the formatted output) are inputs of their own so the queries can run on them.
pub fn transient(db: &dyn Db, like: File, text: &str) -> File {
    File::new(
        db,
        like.path(db).clone(),
        NEXT_TRANSIENT.fetch_add(1, Ordering::Relaxed),
        text.to_string(),
        like.had_bom(db),
        like.editorconfig(db).clone(),
    )
}

/// Applies the fixes `applies` accepts to the text of `file` until none is left. Fails when the
/// rules cannot run (config, worker); a file with syntax errors is returned unchanged.
pub fn fix_text(
    db: &dyn Db,
    file: File,
    applies: impl Fn(&Fix) -> bool,
) -> Result<FixOutcome, Failure> {
    let analyze = |text: &str| -> Result<Analysis, Failure> {
        let step = transient(db, file, text);
        let linted = lint(db, step).as_ref().map_err(Clone::clone)?;
        Ok(Analysis {
            diagnostics: linted.diagnostics.clone(),
            parses: linted.parses,
        })
    };
    fix_loop(file.text(db).clone(), applies, analyze)
}
