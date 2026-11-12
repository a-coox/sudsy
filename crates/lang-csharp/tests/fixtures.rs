//! Edge-case fixtures (`tests/fixtures/csharp/*.cs`): the tree of each is snapshotted, and each is
//! held to the conformance checks: lossless, trivia attachment, token texts, and (when it parses
//! without errors) the bodies of the grammar. `error_*.cs` fixtures must report parse errors.

use core_syntax::shape::ShapeChecker;
use core_syntax::{dump_tree, grammars, print, trivia};
use lang_csharp::provider;
use provider_api::{LanguageProvider, SourceFile};
use std::fmt::Write;
use std::path::PathBuf;

fn fixtures() -> Vec<(String, PathBuf)> {
    let dir = crate::common::repo_root().join("tests/fixtures/csharp");
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "cs"))
        .map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), p))
        .collect();
    out.sort();
    assert!(out.len() >= 15, "fixtures missing in {}", dir.display());
    out
}

fn check(provider: &impl LanguageProvider, name: &str, text: &str, expect_errors: bool) -> String {
    let result = provider
        .parse(&SourceFile::new(name, 1, text))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(print(&result.tree), text, "{name}: not lossless");
    trivia::check_attachment(&result.tree, |k| grammars::csharp().is_trivia(k))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    crate::common::check_token_texts(&result.tree).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        !result.errors.is_empty(),
        expect_errors,
        "{name}: errors {:?}",
        result.errors
    );
    if !expect_errors {
        ShapeChecker::new(grammars::csharp())
            .check(&result.tree)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let mut dump = dump_tree(&result.tree, grammars::csharp());
    for e in &result.errors {
        let _ = writeln!(
            dump,
            "error {} at {}..{}: {}",
            e.code,
            e.start,
            e.start + e.len,
            e.message
        );
    }
    dump
}

#[test]
fn fixture_trees_are_stable() {
    let provider = provider(&crate::common::worker_dll());
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path("../../../tests/fixtures/csharp/snapshots");
    settings.set_prepend_module_to_snapshot(false);
    for (name, path) in fixtures() {
        let text = std::fs::read_to_string(&path).unwrap();
        let dump = check(&provider, &name, &text, name.starts_with("error_"));
        settings.bind(|| insta::assert_snapshot!(name.as_str(), dump));
    }
}

#[test]
fn fixtures_hold_with_crlf_line_endings() {
    let provider = provider(&crate::common::worker_dll());
    for (name, path) in fixtures() {
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\r\n", "\n")
            .replace('\n', "\r\n");
        check(&provider, &name, &text, name.starts_with("error_"));
    }
}

#[test]
fn a_byte_order_mark_is_stripped_before_parsing() {
    let provider = provider(&crate::common::worker_dll());
    let source =
        SourceFile::from_bytes("bom.cs", 1, b"\xEF\xBB\xBFclass A { }\n".to_vec()).unwrap();
    assert!(source.had_bom);
    let result = provider.parse(&source).unwrap();
    assert_eq!(print(&result.tree), "class A { }\n");
    assert!(result.errors.is_empty());
}
