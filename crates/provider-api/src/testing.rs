//! A test harness for rules: run a provider's rules on a text with a given `lint:` config, render
//! the diagnostics for snapshots, and run the fix loop with the checks every fix must pass
//! (the file stays parseable, and a second run changes nothing).

use crate::provider::{LanguageProvider, SourceFile};
use core_config::{ConfigChain, FileInput};
use core_rules::{
    Analysis, Diagnostic, FileContext, LineIndex, SYNTAX_ERROR, activate, fix_loop, run_file,
};
use std::path::Path;

/// The diagnostics of `text`, as `path` would get them with `rules` as the `lint:` section of a
/// config (for example `"sorted-keys: warn"`, one rule per line).
pub fn lint(
    provider: &dyn LanguageProvider,
    path: &str,
    text: &str,
    rules: &str,
) -> Vec<Diagnostic> {
    analyze(provider, path, text, rules).diagnostics
}

fn analyze(provider: &dyn LanguageProvider, path: &str, text: &str, rules: &str) -> Analysis {
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
    let active = activate(
        provider
            .rules()
            .iter()
            .chain(core_rules::shared::RULES)
            .copied(),
        &resolved,
        |_| false,
    )
    .expect("the test rules are configured correctly");
    let lines = LineIndex::new(text);
    let context = FileContext {
        path: Path::new(path),
        text,
        root: &parsed.tree,
        grammar: provider.grammar(),
        lines: &lines,
    };
    let report = run_file(&context, &active, provider.body_kinds(), false);
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
    let run = |text: &str| {
        fix_loop(
            text.to_string(),
            |f| f.kind.applies(allow_unsafe),
            |t| Ok::<_, std::convert::Infallible>(analyze(provider, path, t, rules)),
        )
        .expect("infallible")
    };
    let first = run(text);
    assert_eq!(first.broke, None, "a fix left the file unparseable");
    let second = run(&first.text);
    assert_eq!(second.text, first.text, "fixing is not idempotent");
    first.text
}
