//! JSONTestSuite (nst/JSONTestSuite, pinned in tests/conformance/fetch-json-corpus.sh).
//!
//! * `y_` files must parse without errors, `n_` files must produce errors, `i_` files
//!   (implementation-defined) may do either; they are counted and listed.
//! * Every file's tree must print back to the exact bytes, satisfy the trivia attachment rule,
//!   and every file we accept must format with all the shared checks.
//!
//! Run: `bash tests/conformance/fetch-json-corpus.sh &&
//! cargo test --profile corpus -p lang-json --test conformance -- --ignored --nocapture`

mod common;

use common::subject;
use core_format::testing::{Subject, check};
use core_syntax::{print, trivia};
use std::path::PathBuf;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/conformance/corpus/JSONTestSuite/test_parsing")
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-json-corpus.sh"]
fn json_test_suite() {
    let dir = corpus();
    assert!(
        dir.is_dir(),
        "corpus missing, run tests/conformance/fetch-json-corpus.sh"
    );
    let json = subject("x.json", &[]);
    let jsonc = subject("x.jsonc", &[]);
    let g = json.grammar();

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    let (mut y, mut n, mut i_ok, mut i_err) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    let mut implementation_defined = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".json") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(text) = String::from_utf8(bytes) else {
            // Not valid UTF-8: rejected before parsing, which is correct for n_ and allowed for i_.
            if name.starts_with("y_") {
                failures.push(format!("{name}: not UTF-8"));
            }
            match &name[..2] {
                "n_" => n += 1,
                "i_" => {
                    i_err += 1;
                    implementation_defined.push(format!("rejected {name} (invalid UTF-8)"));
                }
                _ => {}
            }
            continue;
        };
        let parsed = json.parse(&text);
        let tree = match &parsed {
            Ok(t) => t.clone(),
            // Parse again to get the tree of a file with errors.
            Err(_) => lang_json_tree(&text),
        };
        if print(&tree) != text {
            failures.push(format!("{name}: does not round-trip"));
        }
        if let Err(e) = trivia::check_attachment(&tree, |k| g.is_trivia(k)) {
            failures.push(format!("{name}: trivia attachment: {e}"));
        }
        match (&name[..2], &parsed) {
            ("y_", Ok(_)) => y += 1,
            ("y_", Err(e)) => failures.push(format!("{name}: valid JSON rejected: {e}")),
            ("n_", Err(_)) => n += 1,
            ("n_", Ok(_)) => failures.push(format!("{name}: invalid JSON accepted")),
            ("i_", Ok(_)) => {
                i_ok += 1;
                implementation_defined.push(format!("accepted {name}"));
            }
            ("i_", Err(e)) => {
                i_err += 1;
                implementation_defined.push(format!("rejected {name}: {e}"));
            }
            _ => {}
        }
        // Everything we accept must format, idempotently and without changing meaning. The
        // JSONC dialect accepts a superset, so n_ files that are only invalid because of
        // comments or trailing commas are still not formatted.
        if parsed.is_ok() {
            if let Err(e) = check(&json, &text) {
                failures.push(format!("{name}: format check: {e}"));
            }
            if let Err(e) = check(&jsonc, &text) {
                failures.push(format!("{name}: jsonc format check: {e}"));
            }
        }
    }
    println!(
        "JSONTestSuite: y_ accepted {y}/95, n_ rejected {n}/188, i_ accepted {i_ok}, i_ rejected {i_err}"
    );
    for line in &implementation_defined {
        println!("  i_: {line}");
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!((y, n, i_ok + i_err), (95, 188, 35));
}

/// The tree of a text, errors or not.
fn lang_json_tree(text: &str) -> core_syntax::SyntaxNode {
    use provider_api::{LanguageProvider, SourceFile};
    lang_json::JsonProvider::new()
        .parse(&SourceFile::new("x.json", 1, text))
        .unwrap()
        .tree
}
