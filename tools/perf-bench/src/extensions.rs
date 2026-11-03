//! `perf-bench extensions [CS_DIR] [LIMIT]` and `perf-bench extensions-cli SUDSY_EXE`: what M6's
//! pattern rules and Wasm plugins cost (see docs/decisions.md, "M6: Extensibility, performance").
//!
//! * `extensions` (in process): loading the sample plugin (cold compile, the compiled component
//!   reused, and the cost of one call on an empty file); the plugin boundary per file (a cheap
//!   file-scope rule run built in and as a plugin on the same parsed C# files, JSON generated at
//!   two sizes with a node-scope rule); pattern rules (compile time per rule, matcher time per
//!   file for 0, 1, 10 and 100 rules, with kinds the engine can dispatch on and without).
//!   Needs the sample plugin built (`PERF_PLUGIN`, default `target/sample-plugin/...`), the C#
//!   worker and a C# tree.
//! * `extensions-cli` (processes): the wall time of `sudsy lint` on a small repository with no
//!   extension, a pattern file and the Wasm plugin, the number a CI run pays for loading them.
//!
//! `tests/conformance/speed-extensions.sh` builds everything and runs both.

use anyhow::{Result, anyhow, bail};
use core_patterns::SnippetParser;
use core_rules::engine::run_file;
use core_rules::{
    ActiveRule, Extensions, FileContext, LineIndex, Rule, RuleOptions, Severity, activate,
};
use core_syntax::wire::FlatTree;
use perf_bench::inputs;
use provider_api::{LanguageProvider, ProviderSnippets, SourceFile};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "cs") {
            out.push(path);
        }
    }
    Ok(())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn us(d: Duration, n: usize) -> f64 {
    d.as_secs_f64() * 1e6 / n.max(1) as f64
}

pub(crate) fn plugin_path() -> PathBuf {
    std::env::var_os("PERF_PLUGIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target"))
                .join("sample-plugin/wasm32-wasip2/release/sample_plugin.wasm")
        })
}

/// A parsed file and everything a `FileContext` borrows.
pub(crate) struct Parsed {
    pub(crate) text: String,
    pub(crate) tree: core_syntax::SyntaxNode,
    pub(crate) lines: LineIndex,
}

pub(crate) fn parse_all(provider: &dyn LanguageProvider, texts: Vec<String>) -> Vec<Parsed> {
    texts
        .into_iter()
        .filter_map(|text| {
            let parsed = provider
                .parse(&SourceFile::new("bench", 1, text.clone()))
                .ok()?;
            parsed.errors.is_empty().then(|| Parsed {
                lines: LineIndex::new(&text),
                tree: parsed.tree,
                text,
            })
        })
        .collect()
}

pub(crate) fn context<'a>(provider: &'a dyn LanguageProvider, p: &'a Parsed) -> FileContext<'a> {
    FileContext {
        path: Path::new("bench"),
        text: &p.text,
        root: &p.tree,
        grammar: provider.grammar(),
        lines: &p.lines,
        body_kinds: provider.body_kinds(),
        semantics: None,
    }
}

pub(crate) fn active(rule: &'static dyn Rule) -> ActiveRule<'static> {
    ActiveRule {
        rule,
        severity: Severity::Warning,
        options: RuleOptions::resolve(rule.meta().options, &BTreeMap::new()).expect("options"),
    }
}

/// Wall time of `run_file` with `rules` over all files, best of three passes, and the number of
/// diagnostics. Every pass must find the same: a plugin that was switched off (it reports that
/// once) would otherwise look fast.
fn time_rules(
    provider: &dyn LanguageProvider,
    files: &[Parsed],
    rules: &[ActiveRule<'_>],
) -> (Duration, usize) {
    let mut best = Duration::MAX;
    let mut counts = Vec::new();
    for _ in 0..3 {
        let started = Instant::now();
        let mut found = 0;
        for p in files {
            let report = run_file(&context(provider, p), rules, false);
            if let Some(d) = report
                .diagnostics
                .iter()
                .find(|d| d.message.contains("is disabled"))
            {
                panic!("{}", d.message);
            }
            found += report.diagnostics.len();
        }
        best = best.min(started.elapsed());
        counts.push(found);
    }
    assert!(
        counts.windows(2).all(|w| w[0] == w[1]),
        "passes differ: {counts:?}"
    );
    (best, counts[0])
}

pub(crate) fn wasm_rule(
    rules: &[(&'static str, &'static dyn Rule)],
    id: &str,
) -> &'static dyn Rule {
    rules
        .iter()
        .find(|(_, r)| r.meta().id == id)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("the sample plugin has no rule {id}"))
}

pub fn run(dir: Option<&Path>, limit: Option<usize>) -> Result<()> {
    let wasm = plugin_path();
    let bytes = std::fs::read(&wasm).map_err(|e| {
        anyhow!(
            "{}: {e} (build the sample plugin: tests/conformance/speed-extensions.sh)",
            wasm.display()
        )
    })?;
    println!(
        "plugin: {} ({:.2} MB)",
        wasm.display(),
        bytes.len() as f64 / 1e6
    );

    // 1. Loading.
    let started = Instant::now();
    let rules = core_plugins::load(&wasm, &bytes)?;
    let cold = started.elapsed();
    let started = Instant::now();
    let rules_again = core_plugins::load(&wasm, &bytes)?;
    let reused = started.elapsed();
    println!(
        "load (compile + link + ask for the rules): {:8.1} ms first   {:6.1} ms with the compiled component reused",
        ms(cold),
        ms(reused)
    );
    drop(rules_again);

    let json = lang_json::JsonProvider::new();

    // 2. The plugin boundary on C#.
    if let Some(dir) = dir {
        let provider =
            lang_csharp::provider(&lang_csharp::locate_worker().map_err(|e| anyhow!(e))?);
        let mut paths = Vec::new();
        collect(dir, &mut paths)?;
        paths.sort();
        paths.truncate(limit.unwrap_or(500));
        let texts = paths
            .iter()
            .filter_map(|p| String::from_utf8(std::fs::read(p).ok()?).ok())
            .map(|t| t.trim_start_matches('\u{feff}').to_string())
            .collect();
        let files = parse_all(&provider, texts);
        boundary(&provider, &files, &rules, "C# files", "todo-marker")?;
        patterns(&provider, &files)?;
    } else {
        println!("(no C# directory given: the C# measurements are skipped)");
    }

    // 3. The plugin boundary on generated JSON, with a node-scope rule.
    for objects in [100, 10_000] {
        let files = parse_all(&json, vec![inputs::json(objects)]);
        let label = format!("generated JSON, {objects} objects");
        boundary(&json, &files, &rules, &label, "no-debug-flag")?;
    }
    Ok(())
}

static NATIVE_TODO: sample_plugin::TodoMarker = sample_plugin::TodoMarker;
static NATIVE_DEBUG: sample_plugin::NoDebugFlag = sample_plugin::NoDebugFlag;

/// The same rule built in and as a plugin on the same parsed files.
fn boundary(
    provider: &dyn LanguageProvider,
    files: &[Parsed],
    plugin: &[(&'static str, &'static dyn Rule)],
    label: &str,
    id: &str,
) -> Result<()> {
    let native: &'static dyn Rule = match id {
        "todo-marker" => &NATIVE_TODO,
        "no-debug-flag" => &NATIVE_DEBUG,
        other => bail!("no native rule {other}"),
    };
    let wasm = wasm_rule(plugin, &format!("sample/{id}"));
    let (native_time, native_found) = time_rules(provider, files, &[active(native)]);
    let (wasm_time, wasm_found) = time_rules(provider, files, &[active(wasm)]);
    anyhow::ensure!(
        native_found == wasm_found,
        "{id}: built in finds {native_found}, the plugin {wasm_found}"
    );
    let bytes: usize = files.iter().map(|p| p.text.len()).sum();
    let mut encode = Duration::ZERO;
    let mut wire = 0;
    for p in files {
        let started = Instant::now();
        let tree = FlatTree::from_tree(&p.tree).to_wire();
        encode += started.elapsed();
        wire += tree.kinds.len() + tree.values.len();
    }
    println!(
        "{label}: {} file(s), {:.2} MB, rule {id} ({native_found} findings)",
        files.len(),
        bytes as f64 / 1e6
    );
    println!(
        "    built in {:9.1} us/file   plugin {:9.1} us/file   (+{:.1} us/file, x{:.1}; {:.1} us/KB of source)",
        us(native_time, files.len()),
        us(wasm_time, files.len()),
        us(wasm_time, files.len()) - us(native_time, files.len()),
        wasm_time.as_secs_f64() / native_time.as_secs_f64(),
        (wasm_time.as_secs_f64() - native_time.as_secs_f64()) * 1e6 / (bytes as f64 / 1e3),
    );
    println!(
        "    of which the host encodes the tree: {:.1} us/file ({:.1} bytes of tree per KB of source)",
        us(encode, files.len()),
        wire as f64 / (bytes as f64 / 1e3)
    );
    // The fixed cost of a call: an empty file.
    let tiny = parse_all(
        provider,
        vec![match provider.grammar().language.as_str() {
            "json" => "{}".to_string(),
            _ => "class A { }".to_string(),
        }],
    );
    let calls = 200;
    let many: Vec<Parsed> = (0..calls).map(|_| clone(&tiny[0])).collect();
    let (time, _) = time_rules(provider, &many, &[active(wasm)]);
    println!(
        "    one plugin call on an empty file: {:.1} us",
        us(time, calls)
    );
    Ok(())
}

fn clone(p: &Parsed) -> Parsed {
    Parsed {
        text: p.text.clone(),
        tree: p.tree.clone(),
        lines: LineIndex::new(&p.text),
    }
}

/// `n` pattern rules, all starting at the same node kind and none matching: `Foo<i>($$$A)`,
/// which the first token rules out at once, or `$X.Foo<i>($$$A)`, which starts with a metavariable
/// and is matched down to the callee's name.
pub(crate) fn call_rules(n: usize, with_receiver: bool) -> String {
    let callee = if with_receiver { "$X.Foo" } else { "Foo" };
    (0..n)
        .map(|i| {
            format!(
                "id: bench-call-{i}\nlanguage: csharp\nmessage: m\nrule:\n  pattern: {callee}{i}($$$A)\n---\n"
            )
        })
        .collect()
}

pub(crate) fn load_rules(provider: &dyn LanguageProvider, source: &str) -> Result<Extensions> {
    let snippets = ProviderSnippets(provider);
    let loaded = core_patterns::load(source.trim_end_matches("---\n"), &|_| {
        Some(&snippets as &dyn SnippetParser)
    })?;
    Ok(Extensions {
        rules: loaded
            .into_iter()
            .map(|(language, rule)| core_rules::ExtensionRule {
                language: Box::leak(language.into_boxed_str()),
                rule: Box::leak(Box::new(rule)),
            })
            .collect(),
        files: Vec::new(),
    })
}

fn patterns(provider: &dyn LanguageProvider, files: &[Parsed]) -> Result<()> {
    let bytes: usize = files.iter().map(|p| p.text.len()).sum();
    println!(
        "pattern rules on {} C# files ({:.2} MB), matcher only (files parsed before):",
        files.len(),
        bytes as f64 / 1e6
    );
    let builtin: Vec<ActiveRule<'_>> =
        activate(provider.rules().iter().copied(), &BTreeMap::new(), |_| {
            false
        })?;
    let (base, found) = time_rules(provider, files, &builtin);
    println!(
        "    reference: the {} built-in C# rules that are on by default: {:7.1} us/file ({found} findings)",
        builtin.len(),
        us(base, files.len())
    );
    let (none, _) = time_rules(provider, files, &[]);
    println!(
        "    no rule (the traversal and suppression scan): {:7.1} us/file",
        us(none, files.len())
    );
    for (label, with_receiver) in [
        ("starts with a token (Foo<i>(..))", false),
        ("starts with $X ($X.Foo<i>(..))", true),
    ] {
        for n in [1usize, 10, 100] {
            let source = call_rules(n, with_receiver);
            let started = Instant::now();
            let ext = load_rules(provider, &source)?;
            let compile = started.elapsed();
            let active: Vec<ActiveRule<'_>> = ext.rules.iter().map(|r| active(r.rule)).collect();
            let (time, found) = time_rules(provider, files, &active);
            println!(
                "    {n:3} rules, {label:34} compile {:7.1} ms ({:5.2} ms/rule)   match {:8.1} us/file (+{:6.1} over no rules, {:5.2} per rule)   {found} findings",
                ms(compile),
                ms(compile) / n as f64,
                us(time, files.len()),
                us(time, files.len()) - us(none, files.len()),
                (us(time, files.len()) - us(none, files.len())) / n as f64,
            );
        }
    }
    // A rule that matches and builds a fix.
    let ext = load_rules(
        provider,
        "id: bench-tostring\nlanguage: csharp\nmessage: 'x $X'\nrule:\n  pattern: $X.ToString()\nfix: $X.ToString(\"G\")\n",
    )?;
    let active: Vec<ActiveRule<'_>> = ext.rules.iter().map(|r| active(r.rule)).collect();
    let (time, found) = time_rules(provider, files, &active);
    println!(
        "    1 rule that matches (`$X.ToString()`) and builds a fix: {:8.1} us/file ({found} findings)",
        us(time, files.len())
    );
    Ok(())
}

/// Wall time of `sudsy lint .` in a small repository, per variant.
pub fn cli(exe: &Path) -> Result<()> {
    let wasm = plugin_path();
    let root = std::env::temp_dir().join(format!("sudsy-bench-{}", std::process::id()));
    std::fs::create_dir_all(root.join("rules"))?;
    for i in 0..20 {
        std::fs::write(root.join(format!("f{i}.json")), inputs::json(20))?;
    }
    std::fs::write(
        root.join("rules/p.yaml"),
        "id: bench-debug\nlanguage: json\nmessage: m\nrule:\n  pattern: '\"debug\": true'\n",
    )?;
    std::fs::copy(&wasm, root.join("rules/sample.wasm"))?;
    // Where the Wasm compile cache is: the default, off, or a directory that is empty each run.
    #[derive(Clone, Copy)]
    enum Cache {
        Default,
        Off,
        Empty,
    }
    let variants = [
        (
            "no extension",
            "lint:
  rules: {}
",
            Cache::Default,
        ),
        (
            "one pattern file",
            "plugins:
  patterns: [\"rules/p.yaml\"]
",
            Cache::Default,
        ),
        (
            "Wasm, cache off",
            "plugins:
  wasm: [\"rules/sample.wasm\"]
",
            Cache::Off,
        ),
        (
            "Wasm, cache empty",
            "plugins:
  wasm: [\"rules/sample.wasm\"]
",
            Cache::Empty,
        ),
        (
            "Wasm, cache warm",
            "plugins:
  wasm: [\"rules/sample.wasm\"]
",
            Cache::Default,
        ),
    ];
    let runs = 7;
    println!(
        "`sudsy lint .` on 20 small JSON files, median of {runs} runs ({}):",
        exe.display()
    );
    for (label, config, cache) in variants {
        std::fs::write(root.join("sudsy.yaml"), config)?;
        let mut times = Vec::new();
        for run in 0..runs {
            let mut command = std::process::Command::new(exe);
            command
                .args(["lint", "."])
                .current_dir(&root)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            match cache {
                Cache::Default => {}
                Cache::Off => {
                    command.env("SUDSY_WASM_CACHE", "off");
                }
                Cache::Empty => {
                    command.env("SUDSY_WASM_CACHE", root.join(format!("cache-{run}")));
                }
            }
            let started = Instant::now();
            let status = command.status()?;
            times.push(started.elapsed());
            anyhow::ensure!(
                status.code().is_some_and(|c| c <= 1),
                "sudsy lint failed: {status}"
            );
        }
        times.sort();
        println!(
            "    {label:18} {:8.1} ms   (best {:.1} ms)",
            ms(times[runs / 2]),
            ms(times[0])
        );
    }
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

/// `perf-bench plugin-load`: loads the sample plugin once and prints the time, for runs of the
/// process under different `SUDSY_WASM_CACHE` settings (see tests/conformance/speed-extensions.sh).
pub fn plugin_load() -> Result<()> {
    let wasm = plugin_path();
    let bytes = std::fs::read(&wasm)?;
    let started = Instant::now();
    core_plugins::load(&wasm, &bytes)?;
    println!("{:.1}", ms(started.elapsed()));
    Ok(())
}
