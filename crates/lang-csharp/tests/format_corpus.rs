//! Formatting over the pinned corpora (see `corpus.rs`): every file must format, pass the safety
//! check (re-parse, tree comparison) and be idempotent. Failures are listed by category.
//!
//! Run: `bash tests/conformance/fetch-corpus.sh && cargo test --release -p lang-csharp --test format_corpus -- --ignored --nocapture`
//! `SUDSY_CORPUS_FAILURES=<file>` receives every failure; `SUDSY_CORPUS_FILTER=<substring>` limits
//! the run to matching paths.

mod common;

use core_format::{DirectiveTable, FormatCtx, safety};
use provider_api::{LanguageProvider, SourceFile, format_file};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::{Duration, Instant};

fn source(path: &Path, text: String) -> SourceFile {
    let mut file = SourceFile::new(path.to_string_lossy().into_owned(), 1, text);
    file.preprocessor_symbols = common::symbols_for(path);
    file
}

/// The category of a failure: the first words of its message.
fn category(message: &str) -> String {
    let first = message.lines().next().unwrap_or("");
    let first = first.split(": ").next().unwrap_or(first);
    first.chars().take(60).collect()
}

fn check(path: &Path, options: &core_config::ResolvedOptions) -> Result<usize, String> {
    let provider = common::shared_provider();
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let size = text.len();
    let directives = DirectiveTable::default();
    let once = catch_unwind(AssertUnwindSafe(|| {
        format_file(provider, &source(path, text), options, &directives)
    }))
    .map_err(|_| "panic: while formatting".to_string())?
    .map_err(|e| format!("{}: {e}", failure_kind(&e.to_string())))?;
    let twice = catch_unwind(AssertUnwindSafe(|| {
        format_file(provider, &source(path, once.clone()), options, &directives)
    }))
    .map_err(|_| "panic: while formatting the output".to_string())?
    .map_err(|e| format!("second format: {e}"))?;
    if twice != once {
        let at = once
            .lines()
            .zip(twice.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        return Err(format!(
            "not idempotent: first difference at line {}:\n  {:?}\n  {:?}",
            at + 1,
            once.lines().nth(at),
            twice.lines().nth(at)
        ));
    }
    Ok(size)
}

fn failure_kind(message: &str) -> &'static str {
    if message.contains("does not parse") {
        "output does not parse"
    } else if message.contains("trees differ") || message.contains("meaning") {
        "tree differs"
    } else {
        "format"
    }
}

/// The options of the preset `csharp/csharpier-compat@1`.
fn compat_options() -> core_config::ResolvedOptions {
    core_config::ConfigChain::parse(
        "extends: [\"preset:csharp/csharpier-compat@1\"]
",
    )
    .unwrap()
    .format_options(
        &core_config::FileInput::language("csharp"),
        &lang_csharp::option_schema(),
    )
    .unwrap()
    .options
}

#[test]
#[ignore = "needs the corpora: bash tests/conformance/fetch-corpus.sh"]
fn corpora_format_pass_the_safety_check_and_are_idempotent() {
    println!("default options (csharp/microsoft):");
    corpora_pass(&lang_csharp::option_schema().defaults());
    println!("csharp/csharpier-compat:");
    corpora_pass(&compat_options());
}

fn corpora_pass(options: &core_config::ResolvedOptions) {
    let filter = std::env::var("SUDSY_CORPUS_FILTER").unwrap_or_default();
    let started = Instant::now();
    let (mut files, mut bytes) = (0usize, 0usize);
    let mut failures: Vec<(String, String)> = Vec::new();
    for corpus in common::CORPORA {
        let base = common::corpus_root().join(corpus.name);
        assert!(base.exists(), "corpus {} missing", corpus.name);
        let mut paths = Vec::new();
        for dir in corpus.dirs {
            common::collect_cs(&base.join(dir), &mut paths);
        }
        paths.sort();
        let (mut ok, mut bad) = (0, 0);
        for path in paths
            .iter()
            .filter(|p| p.to_string_lossy().contains(&filter))
        {
            files += 1;
            match check(path, options) {
                Ok(size) => {
                    ok += 1;
                    bytes += size;
                }
                Err(message) => {
                    bad += 1;
                    failures.push((path.display().to_string(), message));
                }
            }
        }
        println!("{}: {ok} files ok, {bad} failed", corpus.name);
    }
    let mut by_category: BTreeMap<String, Vec<&(String, String)>> = BTreeMap::new();
    for f in &failures {
        by_category.entry(category(&f.1)).or_default().push(f);
    }
    println!(
        "total: {files} files, {:.1} MB, {} failed, {:.0}s",
        bytes as f64 / 1e6,
        failures.len(),
        started.elapsed().as_secs_f64()
    );
    for (cat, list) in &by_category {
        println!("  {cat}: {}", list.len());
        for (path, message) in list.iter().take(5) {
            println!("    {path}\n      {}", message.replace('\n', "\n      "));
        }
    }
    if let Some(out) = std::env::var_os("SUDSY_CORPUS_FAILURES") {
        let text: Vec<String> = failures
            .iter()
            .map(|(p, m)| format!("{p}\n{m}\n"))
            .collect();
        std::fs::write(out, text.join("\n")).unwrap();
    }
    assert!(failures.is_empty(), "{} files failed", failures.len());
}

/// Where the time of `sudsy format` on CoreLib goes: worker parse (including the mapping and the
/// transfer), layout, printing, the safety re-parse and the tree comparison.
#[test]
#[ignore = "needs the corpora: bash tests/conformance/fetch-corpus.sh"]
fn corelib_format_throughput() {
    let provider = common::shared_provider();
    let options = lang_csharp::option_schema().defaults();
    let directives = DirectiveTable::default();
    let base = common::corpus_root().join("runtime");
    let mut paths = Vec::new();
    common::collect_cs(
        &base.join("src/libraries/System.Private.CoreLib/src"),
        &mut paths,
    );
    paths.sort();
    let sources: Vec<SourceFile> = paths
        .iter()
        .map(|p| source(p, std::fs::read_to_string(p).unwrap()))
        .collect();
    let bytes: usize = sources.iter().map(|s| s.text.len()).sum();
    let zero = Duration::ZERO;
    let (mut parse, mut layout, mut print, mut reparse, mut compare) =
        (zero, zero, zero, zero, zero);
    let (mut out_bytes, mut skipped) = (0usize, 0usize);
    let g = provider.grammar();
    let total = Instant::now();
    for file in &sources {
        let t = Instant::now();
        let parsed = provider.parse(file).unwrap();
        parse += t.elapsed();
        if !parsed.errors.is_empty() {
            skipped += 1;
            continue;
        }
        let cx = FormatCtx::new(g, &options, &directives, provider.formatter());
        let t = Instant::now();
        let mut doc = cx.format_unguarded(&parsed.tree).unwrap();
        layout += t.elapsed();
        let t = Instant::now();
        let (printed, _) =
            core_format::printer::print_protected(&mut doc, &core_format::print_options(&options));
        print += t.elapsed();
        out_bytes += printed.len();
        let mut again = file.clone();
        again.version += 1;
        again.text = printed;
        let t = Instant::now();
        let second = provider.parse(&again).unwrap();
        reparse += t.elapsed();
        let t = Instant::now();
        let _ = safety::equal_ignoring_trivia(g, &parsed.tree, &second.tree);
        compare += t.elapsed();
    }
    let all = total.elapsed();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "CoreLib: {} files, {:.1} MB in, {:.1} MB out, {skipped} skipped",
        sources.len(),
        bytes as f64 / 1e6,
        out_bytes as f64 / 1e6
    );
    println!(
        "  worker parse (parse, mapping, transfer): {:.0} ms",
        ms(parse)
    );
    println!("  layout document: {:.0} ms", ms(layout));
    println!("  print: {:.0} ms", ms(print));
    println!("  safety re-parse: {:.0} ms", ms(reparse));
    println!("  safety tree comparison: {:.0} ms", ms(compare));
    println!(
        "  total: {:.0} ms = {:.2} MB/s",
        ms(all),
        bytes as f64 / 1e6 / all.as_secs_f64()
    );
}
