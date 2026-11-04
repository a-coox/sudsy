//! `perf-bench fixed LANG PHASE [--inputs DIR]`: one fixed, deterministic workload for scripts/bench.sh,
//! which runs it under callgrind (instruction counts) and with `--features count` (allocations).
//! LANG is json, yaml or cs; PHASE is parse, format, format-query, lint, lint-all, patterns or plugins. Inputs are the
//! committed files under DIR (default bench/inputs/<lang>) plus generated JSON and YAML. Only the
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
    let ext = if lang == "cs" { "cs" } else { lang };
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(dir.join(lang))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == ext))
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
        _ => {}
    }
    Ok(out)
}

fn pattern_source(lang: &str) -> String {
    match lang {
        "json" => {
            "id: bench-debug\nlanguage: json\nmessage: m\nrule:\n  pattern: '\"debug\": true'\n"
                .into()
        }
        "yaml" => {
            "id: bench-debug\nlanguage: yaml\nmessage: m\nrule:\n  pattern: 'debug: true'\n".into()
        }
        _ => format!(
            "{}id: bench-tostring\nlanguage: csharp\nmessage: m\nrule:\n  pattern: $X.ToString()\n",
            extensions::call_rules(10, true)
        ),
    }
}

pub(crate) fn make_provider(lang: &str) -> Result<Box<dyn LanguageProvider>> {
    Ok(match lang {
        "json" => Box::new(lang_json::JsonProvider::new()),
        "yaml" => Box::new(lang_yaml::YamlProvider::new()),
        "cs" => Box::new(lang_csharp::provider(
            &lang_csharp::locate_worker().map_err(|e| anyhow!(e))?,
        )),
        _ => bail!("LANG is json, yaml or cs"),
    })
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
            // the layout rules' directives, format, safety check), one file after the other.
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
            let rules = activate(p.rules().iter().copied(), &BTreeMap::new(), |_| false)?;
            let (w, a) = measured(&mut || diagnostics(p, &files, &rules));
            (files.len(), w, a)
        }
        "lint-all" => {
            // Every rule of the provider on at its default options, opt-in rules included.
            let files = parsed();
            let rules: Vec<ActiveRule<'_>> =
                p.rules().iter().map(|r| extensions::active(*r)).collect();
            let (w, a) = measured(&mut || diagnostics(p, &files, &rules));
            (files.len(), w, a)
        }
        "patterns" => {
            let files = parsed();
            let (w, a) = measured(&mut || {
                let ext =
                    extensions::load_rules(p, &pattern_source(lang)).expect("pattern rules load");
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
            let id = match lang {
                "json" => "sample/no-debug-flag",
                "cs" => "sample/todo-marker",
                _ => bail!("no sample plugin rule for {lang}"),
            };
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
        _ => bail!("PHASE is parse, format, format-query, lint, lint-all, patterns or plugins"),
    };
    let allocs = if cfg!(feature = "count") {
        format!(" allocs={allocs}")
    } else {
        String::new()
    };
    println!("RESULT {lang} {phase} files={files} bytes={bytes} work={work}{allocs}");
    Ok(())
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
