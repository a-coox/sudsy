//! `cargo run --release -p perf-bench -- [json|yaml|all] [--phases-only]`
//!
//! Generates the benchmark inputs and times every phase of `format_file` separately, then the
//! whole call at 1x, 4x and 16x sizes to show the scaling. The 4x row is the headline size
//! (40k JSON objects, 60k YAML items).

#[cfg(feature = "mimalloc")]
#[global_allocator]
static A: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod inputs;
mod micro;

use anyhow::{Result, anyhow};
use core_format::{DirectiveTable, FormatCtx, safety};
use provider_api::{LanguageProvider, SourceFile};
use std::time::{Duration, Instant};

const JSON_OBJECTS: usize = 40_000;
const YAML_ITEMS: usize = 60_000;

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let t = Instant::now();
    let v = f();
    (v, t.elapsed())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn file(p: &dyn LanguageProvider, name: &str, version: u64, text: &str) -> (SourceFile, core_config::ResolvedOptions) {
    let config = core_config::Config::default();
    let mut opts = config
        .format_options(&p.grammar().language, p.option_schema())
        .expect("default options");
    let mut f = SourceFile::new(name, version, text);
    f.language_version = provider_api::resolve_dialect(&mut opts, None);
    (f, opts)
}

fn phases(p: &dyn LanguageProvider, name: &str, text: &str) -> Result<Vec<(&'static str, Duration)>> {
    let (f, opts) = file(p, name, 1, text);
    let (parsed, parse) = timed(|| p.parse(&f));
    let parsed = parsed?;
    if let Some(e) = parsed.errors.first() {
        return Err(anyhow!("input does not parse: {}", e.message));
    }
    let directives = DirectiveTable::default();
    let cx = FormatCtx::new(p.grammar(), &opts, &directives, p.formatter());
    let (doc, layout) = timed(|| cx.format_unguarded(&parsed.tree));
    let mut doc = doc?;
    let ((out, _), print) = timed(|| {
        core_format::printer::print_protected(&mut doc, &core_format::print_options(&opts))
    });
    drop(doc);
    let (again, _) = file(p, name, 2, &out);
    let (re, reparse) = timed(|| p.parse(&again));
    let re = re?;
    let (cmp, compare) =
        timed(|| safety::equal_ignoring_trivia(p.grammar(), &parsed.tree, &re.tree));
    cmp.map_err(|e| anyhow!(e))?;
    let (extra, extra_t) = timed(|| p.formatter().extra_check(text, &out));
    extra.map_err(|e| anyhow!(e))?;
    Ok(vec![
        ("parse", parse),
        ("layout document", layout),
        ("print", print),
        ("safety: re-parse", reparse),
        ("safety: tree compare", compare),
        ("safety: extra check (data model)", extra_t),
    ])
}

fn whole(p: &dyn LanguageProvider, name: &str, text: &str) -> Result<Duration> {
    let (f, opts) = file(p, name, 1, text);
    let (r, d) = timed(|| provider_api::format_file(p, &f, &opts, &DirectiveTable::default()));
    r?;
    Ok(d)
}

fn report(label: &str, p: &dyn LanguageProvider, name: &str, make: fn(usize) -> String, base: usize, scaling: bool) -> Result<()> {
    let text = make(base);
    let mb = text.len() as f64 / 1e6;
    let ph = phases(p, name, &text)?;
    let total: Duration = ph.iter().map(|x| x.1).sum();
    println!("== {label}: {mb:.2} MB ==");
    for (n, d) in &ph {
        println!("  {n:<34} {:>9.1} ms {:>6.1}%", ms(*d), 100.0 * d.as_secs_f64() / total.as_secs_f64());
    }
    println!("  {:<34} {:>9.1} ms  {:.2} MB/s", "total (phases summed)", ms(total), mb / total.as_secs_f64());
    if !scaling {
        return Ok(());
    }
    println!("  scaling of format_file (best of 3):");
    for (factor, n) in [("1x", base / 4), ("4x", base), ("16x", base * 4)] {
        let t = make(n);
        let best = (0..3).map(|_| whole(p, name, &t)).collect::<Result<Vec<_>>>()?.into_iter().min().unwrap();
        let size = t.len() as f64 / 1e6;
        println!("    {factor:>3} {size:>6.2} MB {:>9.1} ms {:>7.2} MB/s", ms(best), size / best.as_secs_f64());
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let which = args.iter().find(|a| !a.starts_with("--")).map_or("all", String::as_str);
    if args.iter().any(|a| a == "--micro") {
        micro::run(&lang_json::JsonProvider::new(), "a.json", &inputs::json(JSON_OBJECTS));
        return Ok(());
    }
    let scaling = !args.iter().any(|a| a == "--phases-only");
    if matches!(which, "json" | "all") {
        report("JSON", &lang_json::JsonProvider::new(), "bench.json", inputs::json, JSON_OBJECTS, scaling)?;
    }
    if matches!(which, "yaml" | "all") {
        report("YAML", &lang_yaml::YamlProvider::new(), "bench.yaml", inputs::yaml, YAML_ITEMS, scaling)?;
    }
    Ok(())
}
