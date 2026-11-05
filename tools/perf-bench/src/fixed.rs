//! `perf-bench fixed LANG PHASE [--inputs DIR]`: one fixed, deterministic workload for scripts/bench.sh,
//! which runs it under callgrind (instruction counts) and with `--features count` (allocations).
//! LANG is a folder of `bench/inputs` (named after its files' extension: json, yaml, toml, xml, cs);
//! PHASE is one of `phases` (`perf-bench list` prints them per language). Inputs are the committed
//! files under DIR/LANG plus generated JSON, YAML, TOML and XML. Only the
//! work inside `measured` is counted; callgrind is told to collect between its entry and exit.

use crate::extensions::{self, Parsed};
use anyhow::{Result, anyhow, bail};
use core_format::DirectiveTable;
use core_rules::engine::run_file;
use core_rules::{ActiveRule, activate};
use perf_bench::{file, inputs};
use provider_api::LanguageProvider;
use std::collections::BTreeMap;
use std::path::Path;

/// Not inlined, so `valgrind --toggle-collect='*fixed*measured*'` has a symbol to switch on.
#[inline(never)]
fn measured(f: &mut dyn FnMut() -> usize) -> (usize, u64) {
    let before = crate::allocs();
    let work = f();
    (std::hint::black_box(work), crate::allocs() - before)
}

pub(crate) fn texts(lang: &str, dir: &Path) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(dir.join(lang))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == lang))
        .collect();
    names.sort();
    for p in names {
        let t = String::from_utf8(std::fs::read(&p)?)?;
        out.push((
            p.file_name().unwrap().to_string_lossy().into(),
            t.trim_start_matches('\u{feff}').into(),
        ));
    }
    match lang {
        "json" => out.push(("gen.json".into(), inputs::json(4_000))),
        "yaml" => out.push(("gen.yaml".into(), inputs::yaml(6_000))),
        "toml" => out.push(("gen.toml".into(), inputs::toml(3_000))),
        "xml" => out.push(("gen.xml".into(), inputs::xml(2_000))),
        _ => {}
    }
    Ok(out)
}

/// The pattern rule of the `patterns` phase; a language without one has no such phase.
fn pattern_source(lang: &str) -> Option<String> {
    Some(match lang {
        "json" => {
            "id: bench-debug\nlanguage: json\nmessage: m\nrule:\n  pattern: '\"debug\": true'\n"
                .into()
        }
        "yaml" => {
            "id: bench-debug\nlanguage: yaml\nmessage: m\nrule:\n  pattern: 'debug: true'\n".into()
        }
        "cs" => format!(
            "{}id: bench-tostring\nlanguage: csharp\nmessage: m\nrule:\n  pattern: $X.ToString()\n",
            extensions::call_rules(10, true)
        ),
        _ => return None,
    })
}

/// The rule of the sample plugin that the `plugins` phase runs; a language without one has no such
/// phase.
fn plugin_rule(lang: &str) -> Option<&'static str> {
    match lang {
        "json" => Some("sample/no-debug-flag"),
        "cs" => Some("sample/todo-marker"),
        _ => None,
    }
}

/// The languages whose provider is a separate process (the .NET worker), which callgrind does not
/// trace.
const WORKER: &[&str] = &["cs"];

/// `perf-bench list [--inputs DIR]`: a line `<lang> <native|worker> <phase>...` for every folder of
/// DIR that a provider claims (by the extension of the folder's name), so that a language needs
/// only its input files.
pub fn list(dir: &Path) -> Result<()> {
    let mut langs: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    langs.sort();
    for lang in langs {
        let Ok(provider) = make_provider(&lang) else {
            continue;
        };
        let mut phases = vec!["parse", "format", "format-query", "lint", "lint-all"];
        let documented = |r: &&dyn core_rules::Rule| DOCUMENTATION_RULES.contains(&r.meta().id);
        if provider.rules().iter().any(documented) {
            phases.push("lint-docs");
        }
        if pattern_source(&lang).is_some() {
            phases.push("patterns");
        }
        if plugin_rule(&lang).is_some() {
            phases.push("plugins");
        }
        let kind = if WORKER.contains(&lang.as_str()) {
            "worker"
        } else {
            "native"
        };
        println!("{lang} {kind} {}", phases.join(" "));
    }
    Ok(())
}

/// The provider of a tree-sitter language, found by name so that the harness builds against any
/// revision.
pub(crate) fn named_provider(language: &str) -> Box<dyn LanguageProvider> {
    languages::providers()
        .into_iter()
        .find(|p| p.grammar().language == language)
        .unwrap_or_else(|| panic!("{language} is registered"))
}

/// The provider that claims files with the extension `lang`.
pub(crate) fn make_provider(lang: &str) -> Result<Box<dyn LanguageProvider>> {
    let sample = std::path::PathBuf::from(format!("bench.{lang}"));
    languages::providers()
        .into_iter()
        .find(|p| p.file_matchers().iter().any(|m| m.matches(&sample)))
        .ok_or_else(|| anyhow!("no language claims .{lang} files"))
}

pub fn run(lang: &str, phase: &str, dir: &Path) -> Result<()> {
    let provider = make_provider(lang)?;
    let p = provider.as_ref();
    let texts = texts(lang, dir)?;
    let bytes: usize = texts.iter().map(|t| t.1.len()).sum();
    let parsed = || extensions::parse_all(p, texts.iter().map(|t| t.1.clone()).collect());
    let (files, work, allocs) = match phase {
        "parse" => {
            let sources: Vec<_> = texts.iter().map(|(n, t)| file(p, n, 1, t).0).collect();
            let (w, a) = measured(&mut || {
                sources
                    .iter()
                    .filter_map(|f| p.parse(f).ok())
                    .map(|r| r.errors.len() + 1)
                    .sum()
            });
            (texts.len(), w, a)
        }
        "format" => {
            let sources: Vec<_> = texts.iter().map(|(n, t)| file(p, n, 1, t)).collect();
            let table = DirectiveTable::default();
            let (w, a) = measured(&mut || {
                sources
                    .iter()
                    .filter_map(|(f, o)| provider_api::format_file(p, f, o, &table).ok())
                    .count()
            });
            (texts.len(), w, a)
        }
        "format-query" => {
            // The production path: the `format` query of the incremental engine (parse, options,
            // the layout rules' directives, format), one file after the other.
            let db = core_incremental::Database::new(vec![make_provider(lang)?]);
            let files: Vec<_> = texts
                .iter()
                .map(|(n, t)| {
                    core_incremental::File::new(
                        &db,
                        std::path::PathBuf::from(n),
                        1,
                        t.clone(),
                        false,
                        core_config::EditorConfig::default(),
                    )
                })
                .collect();
            let (w, a) = measured(&mut || {
                files
                    .iter()
                    .filter(|f| core_incremental::format(&db, **f).is_ok())
                    .count()
            });
            (files.len(), w, a)
        }
        "lint" => {
            let files = parsed();
            let rules = activate(own_and_shared(p), &BTreeMap::new(), |_| false)?;
            let (w, a) = measured(&mut || diagnostics(p, &files, &rules));
            (files.len(), w, a)
        }
        "lint-all" => {
            // Every rule of the language on at its default options, opt-in rules included.
            let files = parsed();
            let rules: Vec<ActiveRule<'_>> = own_and_shared(p).map(extensions::active).collect();
            let (w, a) = measured(&mut || diagnostics(p, &files, &rules));
            (files.len(), w, a)
        }
        "lint-docs" => {
            // The C# documentation comment rules only, at their defaults (they are off by
            // default). A baseline without them measures no rules.
            let files = parsed();
            let rules: Vec<ActiveRule<'_>> = own_and_shared(p)
                .filter(|r| DOCUMENTATION_RULES.contains(&r.meta().id))
                .map(extensions::active)
                .collect();
            let started = std::time::Instant::now();
            let (w, a) = measured(&mut || diagnostics(p, &files, &rules));
            eprintln!(
                "TIME {lang} {phase} rules={} ms={:.1}",
                rules.len(),
                started.elapsed().as_secs_f64() * 1e3
            );
            (files.len(), w, a)
        }
        "patterns" => {
            let files = parsed();
            let (w, a) = measured(&mut || {
                let ext = extensions::load_rules(p, &pattern_source(lang).expect("a pattern rule"))
                    .expect("pattern rules load");
                let rules: Vec<ActiveRule<'_>> = ext
                    .rules
                    .iter()
                    .map(|r| extensions::active(r.rule))
                    .collect();
                diagnostics(p, &files, &rules)
            });
            (files.len(), w, a)
        }
        "plugins" => {
            let id =
                plugin_rule(lang).ok_or_else(|| anyhow!("no sample plugin rule for {lang}"))?;
            let files = parsed();
            let wasm = extensions::plugin_path();
            let bytes = std::fs::read(&wasm).map_err(|e| anyhow!("{}: {e}", wasm.display()))?;
            let (w, a) = measured(&mut || {
                let rules = core_plugins::load(&wasm, &bytes).expect("plugin loads");
                let rule = extensions::wasm_rule(&rules, id);
                diagnostics(p, &files, &[extensions::active(rule)])
            });
            (files.len(), w, a)
        }
        _ => bail!(
            "PHASE is parse, format, format-query, lint, lint-all, lint-docs, patterns or plugins"
        ),
    };
    let allocs = if cfg!(feature = "count") {
        format!(" allocs={allocs}")
    } else {
        String::new()
    };
    println!("RESULT {lang} {phase} files={files} bytes={bytes} work={work}{allocs}");
    Ok(())
}

/// The C# documentation comment rules (StyleCop SA16xx), for the `lint-docs` phase.
const DOCUMENTATION_RULES: &[&str] = &[
    "constructor-summary-standard-text",
    "destructor-summary-standard-text",
    "doc-param-has-name",
    "doc-param-has-text",
    "doc-partial-required",
    "doc-partial-summary-required",
    "doc-partial-summary-text",
    "doc-partial-typeparams",
    "doc-returns-has-text",
    "doc-returns-required",
    "doc-summary-required",
    "doc-summary-text",
    "doc-text-ends-with-period",
    "doc-text-not-empty",
    "doc-typeparam-has-name",
    "doc-typeparam-has-text",
    "doc-value-text",
    "no-default-summary",
    "no-doc-placeholder",
    "no-doc-slashes-in-plain-comment",
    "no-duplicate-doc-text",
    "no-returns-on-void",
    "property-summary-matches-accessors",
    "property-summary-omits-restricted-accessor",
];

/// The provider's rules and the ones every language shares.
fn own_and_shared(p: &dyn LanguageProvider) -> impl Iterator<Item = &'static dyn core_rules::Rule> {
    p.rules().iter().chain(core_rules::shared::RULES).copied()
}

fn diagnostics(p: &dyn LanguageProvider, files: &[Parsed], rules: &[ActiveRule<'_>]) -> usize {
    files
        .iter()
        .map(|f| {
            run_file(&extensions::context(p, f), rules, false)
                .diagnostics
                .len()
                + 1
        })
        .sum()
}
