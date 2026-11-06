//! Rule fixtures: `tests/rules/<language>/<folder>/<name>.<ext>`, one input file per case, run
//! by [`rule_fixtures`] (docs/development/testing.md describes the layout). A folder is named for
//! a rule or for a family of rules that share inputs; inputs are small and cover the rules' own
//! cases.
//!
//! - `<name>.<ext>` is the input with its expected diagnostics written under the lines they point
//!   at (`// ^^^ rule-id`, see [`crate::testing::expect`]); no annotation means no diagnostic.
//! - `<name>.fixed.<ext>` is the text after the safe fixes, `<name>.unsafe.<ext>` after the unsafe
//!   ones too; a missing file means the fixes change nothing beyond the previous step.
//! - `rules.yaml` in the folder holds the rule entries (the mapping under `lint.rules`); without
//!   it the rule runs as `<rule-id>: warn`. `<name>.rules.yaml` next to an input replaces it for
//!   that input alone (options, other rules). A folder of several rules lists them in `rules.yaml`.
//!
//! Guards, reported with the failures: every rule of the provider is named by a folder that has
//! an input or by an annotation (a rule cannot be silently untested); no two inputs are the same
//! text with the same rules; no input is longer than [`MAX_LINES`].
//!
//! `SUDSY_BLESS=1` rewrites the annotations and the `.fixed`/`.unsafe` files from what the rules
//! report now; review the diff before committing.

use crate::provider::LanguageProvider;
use crate::testing;
use core_rules::Extensions;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

const CONFIG: &str = "rules.yaml";

/// Longest fixture input, annotations left out.
const MAX_LINES: usize = 60;

/// Runs every fixture of the provider's language and panics with the failures, each named by its
/// file.
pub fn rule_fixtures(provider: &dyn LanguageProvider) {
    let root = fixture_root(provider.language());
    let bless = std::env::var_os("SUDSY_BLESS").is_some();
    let (mut count, mut failures) = (0, Vec::new());
    let marker = testing::marker(provider);
    let (mut named, mut seen) = (HashSet::new(), HashMap::new());
    for dir in sorted(&root).into_iter().filter(|p| p.is_dir()) {
        let id = dir.file_name().expect("a folder").to_string_lossy();
        let folder_rules =
            fs::read_to_string(dir.join(CONFIG)).unwrap_or_else(|_| format!("{id}: warn"));
        for file in sorted(&dir).into_iter().filter(|p| is_input(p)) {
            count += 1;
            named.insert(id.to_string());
            let rules = fs::read_to_string(sibling(&file, "rules"))
                .unwrap_or_else(|_| folder_rules.clone());
            let annotated = testing::split_annotations(
                &marker,
                &fs::read_to_string(&file).expect("readable fixture"),
            );
            let lines = annotated.text.lines().count();
            if lines > MAX_LINES {
                let at = file.display();
                failures.push(format!("{at}\nhas {lines} lines; the limit is {MAX_LINES}"));
            }
            if let Some(other) = seen.insert((annotated.text, rules.clone()), file.clone()) {
                failures.push(format!("{}\nrepeats {}", file.display(), other.display()));
            }
            let run = || check(provider, &file, &rules, bless);
            if let Err(panic) = catch_unwind(AssertUnwindSafe(run)) {
                let message = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("panicked");
                failures.push(format!("{}\n{message}", file.display()));
            }
            // Read after the run, which rewrites the annotations when blessing.
            let after = fs::read_to_string(&file).expect("readable fixture");
            let annotated = testing::split_annotations(&marker, &after);
            named.extend(annotated.expected.into_iter().map(|e| e.3));
        }
    }
    assert!(count > 0, "no rule fixtures under {}", root.display());
    for rule in provider.rules() {
        let id = rule.meta().id;
        if !named.contains(id) {
            failures.push(format!(
                "rule `{id}` has no fixture: add a folder named {id}, or annotate it"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {count} rule fixtures failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// A fixture input that reports something, for the rule documentation.
#[derive(Clone)]
pub struct Example {
    /// The fixture folder.
    pub folder: String,
    /// The input without annotations.
    pub text: String,
    /// `(line, column, length, rule)` of the reported spans, lines and columns from 1.
    pub spans: Vec<(usize, usize, usize, String)>,
    /// The text after the safe fixes, else after the unsafe ones too, if they change anything.
    pub fixed: Option<String>,
    /// The input's own `lint.rules` (options), when it has one.
    pub config: Option<String>,
}

/// Every input with annotations of the provider's language, in fixture order.
pub fn examples(provider: &dyn LanguageProvider) -> Vec<Example> {
    let marker = testing::marker(provider);
    let mut out = Vec::new();
    for dir in sorted(&fixture_root(provider.language())) {
        for file in sorted(&dir).into_iter().filter(|p| is_input(p)) {
            let annotated = testing::split_annotations(
                &marker,
                &fs::read_to_string(&file).expect("readable fixture"),
            );
            if annotated.expected.is_empty() {
                continue;
            }
            out.push(Example {
                folder: dir.file_name().expect("a folder").to_string_lossy().into(),
                text: annotated.text,
                spans: annotated
                    .expected
                    .into_iter()
                    .map(|(line, column, carets, rule)| (line, column, carets.max(1), rule))
                    .collect(),
                fixed: fs::read_to_string(sibling(&file, "fixed"))
                    .or_else(|_| fs::read_to_string(sibling(&file, "unsafe")))
                    .ok(),
                config: fs::read_to_string(sibling(&file, "rules")).ok(),
            });
        }
    }
    out
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

/// Not a result file or config of another input and not the folder's config.
fn is_input(path: &Path) -> bool {
    let name = path.file_name().unwrap().to_string_lossy();
    path.is_file()
        && name != CONFIG
        && !name.ends_with(".rules.yaml")
        && !name.contains(".fixed.")
        && !name.contains(".unsafe.")
}

/// `a.json` -> `a.fixed.json`; the per-input config is `a.rules.yaml` whatever the extension.
fn sibling(file: &Path, kind: &str) -> PathBuf {
    let name = file.file_name().unwrap().to_string_lossy();
    let (stem, ext) = name.rsplit_once('.').expect("a fixture has an extension");
    let ext = if kind == "rules" { "yaml" } else { ext };
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
