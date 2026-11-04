//! A test harness for rules: run a provider's rules on a text with a given `lint:` config, render
//! the diagnostics for snapshots, and run the fix loop with the checks every fix must pass
//! (the file stays parseable, and a second run changes nothing).

use crate::provider::{LanguageProvider, SourceFile};
use core_config::{ConfigChain, FileInput};
use core_rules::{
    Analysis, Diagnostic, Extensions, FileContext, LineIndex, SYNTAX_ERROR, activate, fix_loop,
    layers_needed, run_file,
};
use core_semantics::SemanticView;
use std::path::Path;

/// Why a pattern rule file is refused, if it is.
pub fn pattern_error(provider: &dyn LanguageProvider, source: &str) -> Option<String> {
    let snippets = crate::ProviderSnippets(provider);
    core_patterns::load(source, &|_| {
        Some(&snippets as &dyn core_patterns::SnippetParser)
    })
    .err()
    .map(|e| e.0)
}

/// The rules of a pattern rule file for `provider`'s language, as a project would load them.
/// Panics when the file is invalid.
pub fn pattern_rules(provider: &dyn LanguageProvider, source: &str) -> Extensions {
    let snippets = crate::ProviderSnippets(provider);
    let loaded = core_patterns::load(source, &|_| {
        Some(&snippets as &dyn core_patterns::SnippetParser)
    })
    .unwrap_or_else(|e| panic!("invalid pattern file: {e}"));
    Extensions {
        rules: loaded
            .into_iter()
            .map(|(language, rule)| core_rules::ExtensionRule {
                language: Box::leak(language.into_boxed_str()),
                rule: Box::leak(Box::new(rule)),
            })
            .collect(),
        files: Vec::new(),
    }
}

/// The diagnostics of `text`, as `path` would get them with `rules` as the `lint:` section of a
/// config (for example `"sorted-keys: warn"`, one rule per line).
pub fn lint(
    provider: &dyn LanguageProvider,
    path: &str,
    text: &str,
    rules: &str,
) -> Vec<Diagnostic> {
    lint_with(provider, &Extensions::default(), path, text, rules)
}

/// [`lint`] with the project's pattern and plugin rules added to the provider's.
pub fn lint_with(
    provider: &dyn LanguageProvider,
    extensions: &Extensions,
    path: &str,
    text: &str,
    rules: &str,
) -> Vec<Diagnostic> {
    analyze(provider, extensions, path, text, rules).diagnostics
}

fn analyze(
    provider: &dyn LanguageProvider,
    extensions: &Extensions,
    path: &str,
    text: &str,
    rules: &str,
) -> Analysis {
    let yaml = format!(
        "lint:\n  rules:\n{}",
        rules
            .lines()
            .map(|l| format!("    {l}\n"))
            .collect::<String>()
    );
    let chain = ConfigChain::parse(&yaml).expect("the test config is valid");
    let resolved = chain
        .rules(
            &FileInput::language(&provider.grammar().language),
            provider.option_schema(),
        )
        .expect("the test rules resolve");
    let parsed = provider
        .parse(&SourceFile::new(path, 1, text))
        .expect("the provider parses");
    if !parsed.errors.is_empty() {
        let diagnostics = parsed
            .errors
            .iter()
            .map(|e| Diagnostic {
                rule: SYNTAX_ERROR.into(),
                category: core_rules::Category::Correctness,
                severity: core_rules::Severity::Error,
                message: e.message.clone(),
                range: core_syntax::TextRange::at(e.start.into(), e.len.into()),
                related: Vec::new(),
                fixes: Vec::new(),
            })
            .collect();
        return Analysis {
            diagnostics,
            parses: false,
        };
    }
    let capabilities = provider.capabilities();
    let active = activate(
        extensions.with_own(provider.rules(), &provider.grammar().language),
        &resolved,
        |l| capabilities.contains(l),
    )
    .expect("the test rules are configured correctly");
    let lines = LineIndex::new(text);
    let needed = layers_needed(&active);
    let data = if needed.is_empty() {
        None
    } else {
        provider
            .semantics(&SourceFile::new(path, 1, text), needed)
            .expect("the provider answers semantic queries")
    };
    let summaries = provider.summaries();
    let context = FileContext {
        path: Path::new(path),
        text,
        root: &parsed.tree,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: provider.body_kinds(),
        semantics: data.as_ref().map(|data| SemanticView {
            data,
            summaries: &summaries,
        }),
    };
    let report = run_file(&context, &active, false);
    Analysis {
        diagnostics: report.diagnostics,
        parses: true,
    }
}

/// One line per diagnostic: `line:col severity[rule] message`, then its related spans and fixes.
pub fn render(text: &str, diagnostics: &[Diagnostic]) -> String {
    let lines = LineIndex::new(text);
    let at = |offset| {
        let (l, c) = lines.line_col(text, offset);
        format!("{l}:{c}")
    };
    let mut out = String::new();
    for d in diagnostics {
        out.push_str(&format!(
            "{} {}[{}] {}\n",
            at(d.range.start()),
            d.severity.as_str(),
            d.rule,
            d.message
        ));
        for r in &d.related {
            out.push_str(&format!(
                "  related {}: {}\n",
                at(r.range.start()),
                r.message
            ));
        }
        for f in &d.fixes {
            out.push_str(&format!("  fix ({}): {}\n", f.kind.as_str(), f.title));
        }
    }
    out
}

/// The text after the fix loop with `rules` on. Panics when a fix would leave the file
/// unparseable or when running the loop on its own output changes it again.
pub fn fix(
    provider: &dyn LanguageProvider,
    path: &str,
    text: &str,
    rules: &str,
    allow_unsafe: bool,
) -> String {
    fix_with(
        provider,
        &Extensions::default(),
        path,
        text,
        rules,
        allow_unsafe,
    )
}

/// [`fix`] with the project's pattern and plugin rules added to the provider's.
pub fn fix_with(
    provider: &dyn LanguageProvider,
    extensions: &Extensions,
    path: &str,
    text: &str,
    rules: &str,
    allow_unsafe: bool,
) -> String {
    let run = |text: &str| {
        fix_loop(
            text.to_string(),
            |f| f.kind.applies(allow_unsafe),
            |t| Ok::<_, std::convert::Infallible>(analyze(provider, extensions, path, t, rules)),
        )
        .expect("infallible")
    };
    let first = run(text);
    assert_eq!(first.broke, None, "a fix left the file unparseable");
    let second = run(&first.text);
    assert_eq!(second.text, first.text, "fixing is not idempotent");
    first.text
}

/// An annotated fixture split into the source text and the diagnostics it expects.
pub(crate) struct Annotated {
    pub(crate) text: String,
    /// `(line, column, carets, rule)`, lines and columns from 1.
    pub(crate) expected: Vec<(usize, usize, usize, String)>,
}

/// Annotation lines hold the language's line-comment marker, carets under the reported span and
/// the rule id: `// ^^^ no-empty-catch` (`#` in YAML) points at the line above. The marker may be
/// left out (`^^^ no-empty-catch`), which is how a span in the first columns is written. They are
/// removed before the rules run, so they never shift a position.
pub(crate) fn split_annotations(marker: &str, annotated: &str) -> Annotated {
    let mut text = String::new();
    let mut expected = Vec::new();
    let mut lines = 0;
    for line in annotated.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        // The marker is optional: carets in the first columns have no room for it.
        let trimmed = body.trim_start();
        let rest = trimmed
            .strip_prefix(marker)
            .map_or(trimmed, str::trim_start);
        if rest.starts_with('^') {
            let column = body.chars().count() - rest.chars().count() + 1;
            let carets = rest.chars().take_while(|&c| c == '^').count();
            let rule = rest[carets..].trim().to_string();
            assert!(lines > 0, "annotation `{body}` has no line above it");
            expected.push((lines, column, carets, rule));
        } else {
            text.push_str(line);
            lines += 1;
        }
    }
    Annotated { text, expected }
}

pub(crate) fn marker(provider: &dyn LanguageProvider) -> String {
    provider
        .grammar()
        .comments
        .line
        .clone()
        .unwrap_or_else(|| "//".into())
}

/// `(line, column, carets, rule)` of every diagnostic; a multi-line span has 0 carets.
pub(crate) fn found(
    provider: &dyn LanguageProvider,
    extensions: &Extensions,
    path: &str,
    text: &str,
    rules: &str,
) -> Vec<(usize, usize, usize, String)> {
    let index = LineIndex::new(text);
    lint_with(provider, extensions, path, text, rules)
        .iter()
        .map(|d| {
            let (line, column) = index.line_col(text, d.range.start());
            let (end_line, end_column) = index.line_col(text, d.range.end());
            let carets = if end_line == line {
                end_column - column
            } else {
                0
            };
            (line, column, carets, d.rule.clone())
        })
        .collect()
}

/// Runs `rules` on a fixture whose expected diagnostics are written under the lines they point
/// at (see [`split_annotations`]) and panics when the diagnostics differ in position, length
/// (single-line spans) or rule. Returns the text without the annotations.
pub fn expect(
    provider: &dyn LanguageProvider,
    extensions: &Extensions,
    path: &str,
    annotated: &str,
    rules: &str,
) -> String {
    let Annotated { text, expected } = split_annotations(&marker(provider), annotated);
    let mut actual = found(provider, extensions, path, &text, rules);
    let mut expected = expected;
    // A multi-line span has no caret count to compare.
    for e in &mut expected {
        if actual
            .iter()
            .any(|a| (a.0, a.1, &a.3) == (e.0, e.1, &e.3) && a.2 == 0)
        {
            e.2 = 0;
        }
    }
    actual.sort();
    expected.sort();
    assert_eq!(
        actual, expected,
        "diagnostics (line, column, length, rule) differ from the annotations in:\n{text}"
    );
    text
}

/// Builds the sample plugin (`tests/plugins/sample`) to a Wasm component once per test process
/// and returns the file. Needs the `wasm32-wasip2` target (`rustup target add wasm32-wasip2`).
pub fn sample_plugin() -> std::path::PathBuf {
    static WASM: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    WASM.get_or_init(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| root.join("target"))
            .join("sample-plugin");
        let status = std::process::Command::new("cargo")
            .args(["build", "--release", "--target", "wasm32-wasip2", "--quiet"])
            .arg("--manifest-path")
            .arg(root.join("tests/plugins/sample/Cargo.toml"))
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .expect("cargo runs");
        assert!(
            status.success(),
            "building the sample plugin failed (is the wasm32-wasip2 target installed?              `rustup target add wasm32-wasip2`)"
        );
        target.join("wasm32-wasip2/release/sample_plugin.wasm")
    })
    .clone()
}
