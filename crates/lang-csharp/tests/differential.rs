//! Differential test (plan.md "Testing strategy"): `csharp/csharpier-compat` against CSharpier on
//! the corpus. Compares Sudsy's output with the pinned CSharpier's (dotnet-tools.json) and reports
//! how many files are identical and which kinds of difference are left.
//!
//! Run: `bash tests/conformance/fetch-corpus.sh && bash tests/conformance/csharpier-baseline.sh &&
//!       cargo test --release -p lang-csharp --test differential -- --ignored --nocapture`
//! `SUDSY_DIFF_DIR` is the directory of the baseline script (default `<temp>/sudsy-differential`);
//! `SUDSY_DIFF_FILTER=<substring>` limits the run to matching paths. Sudsy's output is written to
//! `<dir>/sudsy/` and the differing hunks to `<dir>/hunks.txt`, grouped by category.

mod common;

use core_config::ResolvedOptions;
use core_format::DirectiveTable;
use provider_api::{SourceFile, format_file};
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// CSharpier keeps a byte order mark; Sudsy formats the text without one.
const BOM: char = '\u{feff}';

fn diff_dir() -> PathBuf {
    std::env::var_os("SUDSY_DIFF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("sudsy-differential"))
}

fn compat_options() -> ResolvedOptions {
    core_config::ConfigChain::parse("extends:\n  - preset:csharp/csharpier-compat\n")
        .expect("valid config")
        .format_options(
            &core_config::FileInput::language("csharp"),
            &lang_csharp::option_schema(),
        )
        .expect("the preset resolves")
        .options
}

/// One run of changed lines: CSharpier's lines and Sudsy's lines, and where they start.
struct Hunk {
    line: usize,
    csharpier: Vec<String>,
    sudsy: Vec<String>,
}

fn hunks(csharpier: &str, sudsy: &str) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(csharpier, sudsy);
    let mut out: Vec<Hunk> = Vec::new();
    let mut open = false;
    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches(['\n', '\r']).to_string();
        match change.tag() {
            ChangeTag::Equal => open = false,
            tag => {
                if !open {
                    out.push(Hunk {
                        line: change.old_index().or(change.new_index()).unwrap_or(0) + 1,
                        csharpier: Vec::new(),
                        sudsy: Vec::new(),
                    });
                    open = true;
                }
                let h = out.last_mut().unwrap();
                if tag == ChangeTag::Delete {
                    h.csharpier.push(text);
                } else {
                    h.sudsy.push(text);
                }
            }
        }
    }
    out
}

fn squash(lines: &[String]) -> String {
    lines
        .iter()
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// A short name for what a hunk is about. Coarse on purpose: the point is to rank what to fix
/// next, then read examples in `hunks.txt`.
fn category(h: &Hunk) -> String {
    let non_blank = |v: &[String]| -> Vec<String> {
        v.iter().filter(|l| !l.trim().is_empty()).cloned().collect()
    };
    let (a, b) = (non_blank(&h.csharpier), non_blank(&h.sudsy));
    if a == b {
        return if h.csharpier.len() > h.sudsy.len() {
            "blank line: csharpier has one, sudsy not"
        } else {
            "blank line: sudsy has one, csharpier not"
        }
        .into();
    }
    if a.iter().map(|l| l.trim()).eq(b.iter().map(|l| l.trim())) {
        return "indentation".into();
    }
    let (sa, sb) = (squash(&a), squash(&b));
    if sa != sb {
        if sa.replace(',', "") == sb.replace(',', "") {
            return if sa.len() > sb.len() {
                "trailing comma: csharpier has one"
            } else {
                "trailing comma: sudsy has one"
            }
            .into();
        }
        return "text differs (directives, not layout)".into();
    }
    let shape = match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Less => "csharpier joins",
        std::cmp::Ordering::Greater => "csharpier breaks",
        std::cmp::Ordering::Equal => "moves",
    };
    let end = |v: &[String]| {
        v.first()
            .and_then(|l| l.trim_end().chars().last())
            .unwrap_or(' ')
    };
    let start = |v: &[String]| {
        v.get(1)
            .or(v.first())
            .and_then(|l| l.trim_start().chars().next())
            .unwrap_or(' ')
    };
    format!(
        "{shape}: csharpier line 1 ends `{}` next starts `{}` / sudsy ends `{}` next starts `{}`",
        end(&a),
        start(&a),
        end(&b),
        start(&b)
    )
}

#[test]
#[ignore = "needs the corpora and the CSharpier baseline: bash tests/conformance/csharpier-baseline.sh"]
fn csharpier_compat_matches_csharpier_on_the_corpus() {
    let dir = diff_dir();
    let (input, expected) = (dir.join("in"), dir.join("out"));
    assert!(
        expected.exists(),
        "run bash tests/conformance/csharpier-baseline.sh first"
    );
    let filter = std::env::var("SUDSY_DIFF_FILTER").unwrap_or_default();
    let options = compat_options();
    let provider = common::shared_provider();
    let directives = DirectiveTable::default();
    let mut paths = Vec::new();
    common::collect_cs(&input, &mut paths);
    paths.sort();

    let (mut files, mut identical, mut failed, mut unformatted) = (0, 0, 0, 0);
    let mut lines_total = 0usize;
    let mut by_corpus: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    // (relative path, CSharpier's output, Sudsy's output) of the files that differ.
    let mut differing: Vec<(String, String, String)> = Vec::new();
    for path in paths
        .iter()
        .filter(|p| p.to_string_lossy().contains(&filter))
    {
        let rel = path.strip_prefix(&input).unwrap();
        let corpus = rel.iter().next().unwrap().to_string_lossy().into_owned();
        let source_text = std::fs::read_to_string(path).unwrap();
        let theirs = std::fs::read_to_string(expected.join(rel)).unwrap();
        let theirs = theirs.strip_prefix(BOM).unwrap_or(&theirs).to_string();
        files += 1;
        let entry = by_corpus.entry(corpus).or_default();
        entry.0 += 1;
        let mut file = SourceFile::new(path.to_string_lossy().into_owned(), 1, source_text.clone());
        file.preprocessor_symbols = common::symbols_for(path);
        let ours = match format_file(provider, &file, &options, &directives) {
            Ok(text) => text,
            Err(e) => {
                failed += 1;
                if failed <= 5 {
                    println!("sudsy failed on {}: {e}", rel.display());
                }
                continue;
            }
        };
        let out_path = dir.join("sudsy").join(rel);
        std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
        std::fs::write(&out_path, &ours).unwrap();
        lines_total += theirs.lines().count();
        if ours == theirs {
            identical += 1;
            entry.1 += 1;
            continue;
        }
        if theirs == source_text.strip_prefix(BOM).unwrap_or(&source_text) {
            unformatted += 1; // generated code, a syntax error, too deep: not compared
            continue;
        }
        differing.push((rel.display().to_string(), theirs, ours));
    }

    // The diffs are the slow part: spread them over the cores.
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(4, |n| n.get()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((rel, theirs, ours)) = differing.get(i) else {
                        break;
                    };
                    let found: Vec<(String, Hunk)> = hunks(theirs, ours)
                        .into_iter()
                        .map(|h| (format!("{rel}:{}", h.line), h))
                        .collect();
                    results.lock().unwrap().push((rel.clone(), found));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lines_differ = 0usize;
    let mut by_category: BTreeMap<String, Vec<(String, Hunk)>> = BTreeMap::new();
    for (_, found) in results {
        for (at, h) in found {
            lines_differ += h.csharpier.len().max(h.sudsy.len());
            by_category.entry(category(&h)).or_default().push((at, h));
        }
    }

    let hunk_count: usize = by_category.values().map(Vec::len).sum();
    let mut summary = String::new();
    macro_rules! say {
        ($($arg:tt)*) => {{
            println!($($arg)*);
            writeln!(summary, $($arg)*).unwrap();
        }};
    }
    say!(
        "files: {files}, csharpier left {unformatted} alone (not compared), sudsy failed: {failed}; identical: {identical} of {} ({:.1}%)",
        files - unformatted,
        100.0 * identical as f64 / (files - unformatted) as f64
    );
    for (corpus, (n, same)) in &by_corpus {
        say!("  {corpus}: {same}/{n} identical");
    }
    say!(
        "lines: {lines_total}, in differing hunks: {lines_differ} ({:.2}%), hunks: {hunk_count}",
        100.0 * lines_differ as f64 / lines_total.max(1) as f64
    );
    let mut ranked: Vec<_> = by_category.iter().collect();
    ranked.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    let mut report = String::new();
    for (name, list) in &ranked {
        let mut per_file: BTreeMap<&str, usize> = BTreeMap::new();
        for (at, _) in (*list).iter() {
            *per_file
                .entry(at.rsplit_once(':').map_or(at, |p| p.0))
                .or_default() += 1;
        }
        say!("  {:>6}  {name}  ({} files)", list.len(), per_file.len());
        writeln!(report, "=== {} hunks: {name}", list.len()).unwrap();
        for (at, h) in list.iter().take(8) {
            writeln!(report, "--- {at}").unwrap();
            for l in &h.csharpier {
                writeln!(report, "csharpier |{l}").unwrap();
            }
            for l in &h.sudsy {
                writeln!(report, "sudsy     |{l}").unwrap();
            }
        }
    }
    std::fs::write(dir.join("hunks.txt"), report).unwrap();
    std::fs::write(dir.join("summary.txt"), summary).unwrap();
}
