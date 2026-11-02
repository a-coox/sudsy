//! Conformance: every file of the pinned dotnet/runtime corpus round-trips losslessly through
//! worker -> Rust tree and satisfies the trivia attachment rule.
//!
//! Run: `bash tests/conformance/fetch-corpus.sh && cargo test -p provider-api --test corpus_roundtrip -- --ignored --nocapture`

mod common;

use core_syntax::{grammars, print, trivia};
use provider_api::{LanguageProvider, SourceFile};
use std::path::Path;
use std::time::Instant;

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "cs") {
            out.push(path);
        }
    }
}

#[test]
#[ignore = "needs the corpus: bash tests/conformance/fetch-corpus.sh"]
fn corpus_round_trips_losslessly() {
    let root = common::repo_root()
        .join("tests/conformance/corpus/runtime/src/libraries/System.Private.CoreLib/src");
    assert!(
        root.exists(),
        "corpus missing; run tests/conformance/fetch-corpus.sh"
    );
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();

    let provider = provider_api::csharp::provider(&common::worker_dll());
    let (mut bytes, mut with_errors, mut elements) = (0usize, 0usize, 0usize);
    let started = Instant::now();
    for path in &files {
        let source =
            SourceFile::from_bytes(path.to_string_lossy(), 1, std::fs::read(path).unwrap())
                .unwrap_or_else(|e| panic!("{e}"));
        let result = provider
            .parse(&source)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            print(&result.tree) == source.text,
            "{}: not lossless",
            path.display()
        );
        trivia::check_attachment(&result.tree, |k| grammars::csharp().is_trivia(k))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        bytes += source.text.len();
        elements += result.tree.descendants_with_tokens().count();
        with_errors += usize::from(!result.errors.is_empty());
    }
    println!(
        "corpus: {} files, {:.1} MB, {} tree elements, {} files with parse errors, {:.1}s end to end",
        files.len(),
        bytes as f64 / 1e6,
        elements,
        with_errors,
        started.elapsed().as_secs_f64()
    );
    assert!(files.len() > 1000);
}
