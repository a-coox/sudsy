//! Conformance over the pinned corpora (dotnet/runtime's CoreLib, dotnet/roslyn, dotnet/aspnetcore;
//! about 90 MB): every file parses without errors, round-trips byte for byte, satisfies the trivia
//! attachment rule, has every token in the tree, and fits the bodies of the grammar.
//!
//! Run: `bash tests/conformance/fetch-corpus.sh && cargo test --profile corpus -p lang-csharp --test integration -- --ignored --nocapture corpus::`

use crate::common::CORPORA;

use core_syntax::shape::ShapeChecker;
use core_syntax::{grammars, print, trivia};
use provider_api::{LanguageProvider, SourceFile};
use std::path::Path;
use std::time::Instant;

#[derive(Default)]
struct Totals {
    files: usize,
    bytes: usize,
    elements: usize,
    with_errors: usize,
    failures: Vec<String>,
}

fn check_file(
    provider: &impl LanguageProvider,
    shapes: &ShapeChecker,
    path: &Path,
    totals: &mut Totals,
) {
    let mut fail = |what: String| totals.failures.push(format!("{}: {what}", path.display()));
    let mut source =
        SourceFile::from_bytes(path.to_string_lossy(), 1, std::fs::read(path).unwrap())
            .unwrap_or_else(|e| panic!("{e}"));
    source.preprocessor_symbols = crate::common::symbols_for(path);
    let result = match provider.parse(&source) {
        Ok(r) => r,
        Err(e) => return fail(format!("parse failed: {e}")),
    };
    totals.files += 1;
    totals.bytes += source.text.len();
    totals.elements += result.tree.descendants_with_tokens().count();
    if print(&result.tree) != source.text {
        return fail("not lossless".into());
    }
    if let Err(e) = trivia::check_attachment(&result.tree, |k| grammars::csharp().is_trivia(k)) {
        fail(format!("trivia attachment: {e}"));
    }
    if let Err(e) = crate::common::check_token_texts(&result.tree) {
        fail(format!("token text: {e}"));
    }
    if result.errors.is_empty() {
        if let Err(e) = shapes.check(&result.tree) {
            fail(format!("shape: {e}"));
        }
    } else {
        totals.with_errors += 1;
        let first = &result.errors[..result.errors.len().min(3)];
        fail(format!("parse errors: {first:?}"));
    }
}

#[test]
#[ignore = "needs the corpora: bash tests/conformance/fetch-corpus.sh"]
fn corpora_parse_round_trip_and_fit_the_grammar() {
    let provider = lang_csharp::provider(&crate::common::worker_dll());
    let shapes = ShapeChecker::new(grammars::csharp());
    let started = Instant::now();
    let mut all = Totals::default();
    for corpus in CORPORA {
        let base = crate::common::corpus_root().join(corpus.name);
        assert!(
            base.exists(),
            "corpus {} missing; run tests/conformance/fetch-corpus.sh",
            corpus.name
        );
        let mut files = Vec::new();
        for dir in corpus.dirs {
            crate::common::collect_cs(&base.join(dir), &mut files);
        }
        files.sort();
        let mut totals = Totals::default();
        let t = Instant::now();
        for path in &files {
            check_file(&provider, &shapes, path, &mut totals);
        }
        println!(
            "{}: {} files, {:.1} MB, {} tree elements, {} files with parse errors, {} failures, {:.1}s",
            corpus.name,
            totals.files,
            totals.bytes as f64 / 1e6,
            totals.elements,
            totals.with_errors,
            totals.failures.len(),
            t.elapsed().as_secs_f64()
        );
        all.files += totals.files;
        all.bytes += totals.bytes;
        all.elements += totals.elements;
        all.with_errors += totals.with_errors;
        all.failures.extend(totals.failures);
    }
    println!(
        "total: {} files, {:.1} MB, {} tree elements, {:.1}s end to end",
        all.files,
        all.bytes as f64 / 1e6,
        all.elements,
        started.elapsed().as_secs_f64()
    );
    if let Some(path) = std::env::var_os("SUDSY_CORPUS_FAILURES") {
        std::fs::write(
            path,
            all.failures.join(
                "
",
            ),
        )
        .unwrap();
    }
    assert!(all.files > 7000);
    assert!(
        all.failures.is_empty(),
        "{} failures, first {}:\n{}",
        all.failures.len(),
        all.failures.len().min(25),
        all.failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
