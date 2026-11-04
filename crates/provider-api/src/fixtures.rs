//! Rule fixtures: `tests/rules/<language>/<rule-id>/<name>.<ext>`, one input file per case, run
//! by [`rule_fixtures`] (docs/development/testing.md describes the layout).
//!
//! - `<name>.<ext>` is the input with its expected diagnostics written under the lines they point
//!   at (`// ^^^ rule-id`, see [`crate::testing::expect`]); no annotation means no diagnostic.
//! - `<name>.fixed.<ext>` is the text after the safe fixes, `<name>.unsafe.<ext>` after the unsafe
//!   ones too; a missing file means the fixes change nothing beyond the previous step.
//! - `rules.yaml` in the folder holds the rule entries (the mapping under `lint.rules`); without
//!   it the rule runs as `<rule-id>: warn`.
//!
//! `SUDSY_BLESS=1` rewrites the annotations and the `.fixed`/`.unsafe` files from what the rules
//! report now; review the diff before committing.

use crate::provider::LanguageProvider;
use crate::testing;
use core_rules::Extensions;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

const CONFIG: &str = "rules.yaml";

/// Runs every fixture of the provider's language and panics with the failures, each named by its
/// file.
pub fn rule_fixtures(provider: &dyn LanguageProvider) {
    let root = fixture_root(&provider.grammar().language);
    let bless = std::env::var_os("SUDSY_BLESS").is_some();
    let (mut count, mut failures) = (0, Vec::new());
    for dir in sorted(&root).into_iter().filter(|p| p.is_dir()) {
        let id = dir.file_name().expect("a folder").to_string_lossy();
        let rules = fs::read_to_string(dir.join(CONFIG)).unwrap_or_else(|_| format!("{id}: warn"));
        for file in sorted(&dir).into_iter().filter(|p| is_input(p)) {
            count += 1;
            let run = || check(provider, &file, &rules, bless);
            if let Err(panic) = catch_unwind(AssertUnwindSafe(run)) {
                let message = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("panicked");
                failures.push(format!("{}\n{message}", file.display()));
            }
        }
    }
    assert!(count > 0, "no rule fixtures under {}", root.display());
    assert!(
        failures.is_empty(),
        "{} of {count} rule fixtures failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// The rule folders of `language` that exist.
pub fn fixture_rules(language: &str) -> Vec<String> {
    let root = fixture_root(language);
    sorted(&root)
        .into_iter()
        .filter(|p| p.is_dir())
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

fn fixture_root(language: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/rules")
        .join(language)
}

fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| entries.map(|e| e.expect("readable").path()).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}

/// Not a result file of another input and not the folder's config.
fn is_input(path: &Path) -> bool {
    let name = path.file_name().unwrap().to_string_lossy();
    path.is_file() && name != CONFIG && !name.contains(".fixed.") && !name.contains(".unsafe.")
}

/// `a.json` -> `a.fixed.json`.
fn sibling(file: &Path, kind: &str) -> PathBuf {
    let name = file.file_name().unwrap().to_string_lossy();
    let (stem, ext) = name.rsplit_once('.').expect("a fixture has an extension");
    file.with_file_name(format!("{stem}.{kind}.{ext}"))
}

fn check(provider: &dyn LanguageProvider, file: &Path, rules: &str, bless: bool) {
    let path = file.file_name().unwrap().to_string_lossy();
    let mut annotated = fs::read_to_string(file).expect("readable fixture");
    let none = Extensions::default();
    if bless {
        let marker = testing::marker(provider);
        let clean = testing::split_annotations(&marker, &annotated).text;
        annotated = annotate(provider, &path, &clean, &marker, rules);
        fs::write(file, &annotated).expect("writable fixture");
        let clean = testing::split_annotations(&marker, &annotated).text;
        let safe = testing::fix(provider, &path, &clean, rules, false);
        let all = testing::fix(provider, &path, &clean, rules, true);
        write_or_remove(
            &sibling(file, "fixed"),
            (safe != clean).then_some(safe.as_str()),
        );
        write_or_remove(
            &sibling(file, "unsafe"),
            (all != safe).then_some(all.as_str()),
        );
    }
    let text = testing::expect(provider, &none, &path, &annotated, rules);
    let expected = |kind: &str, otherwise: &str| {
        fs::read_to_string(sibling(file, kind)).unwrap_or_else(|_| otherwise.to_string())
    };
    let safe = expected("fixed", &text);
    assert_eq!(
        testing::fix(provider, &path, &text, rules, false),
        safe,
        "safe fixes differ from {}",
        sibling(file, "fixed").display()
    );
    assert_eq!(
        testing::fix(provider, &path, &text, rules, true),
        expected("unsafe", &safe),
        "fixes with unsafe differ from {}",
        sibling(file, "unsafe").display()
    );
}

fn write_or_remove(path: &Path, text: Option<&str>) {
    match text {
        Some(text) => fs::write(path, text).expect("writable fixture"),
        None => {
            let _ = fs::remove_file(path);
        }
    }
}

/// `clean` with an annotation under each line a diagnostic starts on.
fn annotate(
    provider: &dyn LanguageProvider,
    path: &str,
    clean: &str,
    marker: &str,
    rules: &str,
) -> String {
    let mut found = testing::found(provider, &Extensions::default(), path, clean, rules);
    found.sort();
    let mut out = String::new();
    for (number, line) in clean.split_inclusive('\n').enumerate() {
        out.push_str(line);
        if !line.ends_with('\n') && found.iter().any(|f| f.0 == number + 1) {
            out.push('\n');
        }
        for (_, column, carets, rule) in found.iter().filter(|f| f.0 == number + 1) {
            let before = column - 1;
            if before >= marker.len() {
                out.push_str(marker);
                out.push_str(&" ".repeat(before - marker.len()));
            } else {
                out.push_str(&" ".repeat(before));
            }
            out.push_str(&"^".repeat((*carets).max(1)));
            out.push_str(&format!(" {rule}\n"));
        }
    }
    out
}
