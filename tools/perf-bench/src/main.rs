//! `cargo run --release -p perf-bench -- [json|yaml|all] [--phases-only]`
//!
//! Generates the benchmark inputs and times every phase of `format_file` separately, then the
//! whole call at 1x, 4x and 16x sizes to show the scaling. The 4x row is the headline size
//! (40k JSON objects, 60k YAML items). `--features count` also reports allocations per phase.

#[cfg(feature = "mimalloc")]
#[global_allocator]
static A: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "count")]
mod counting;
#[cfg(feature = "count")]
#[global_allocator]
static A: counting::Counting = counting::Counting;

use anyhow::{Result, anyhow};
use core_format::{DirectiveTable, FormatCtx, safety};
use perf_bench::{file, inputs};
use provider_api::LanguageProvider;
use std::time::{Duration, Instant};

const JSON_OBJECTS: usize = 40_000;
const YAML_ITEMS: usize = 60_000;

/// Time and allocations of one phase.
#[derive(Clone, Copy)]
struct Cost {
    time: Duration,
    allocs: u64,
}

#[cfg(feature = "count")]
fn allocs() -> u64 {
    counting::count()
}

#[cfg(not(feature = "count"))]
fn allocs() -> u64 {
    0
}

/// With `--sites` (and `--features count`): which code allocates during `f`.
#[cfg(feature = "count")]
fn sites<T>(label: &str, f: impl FnOnce() -> T) -> T {
    if !std::env::args().any(|a| a == "--sites") {
        return f();
    }
    counting::sample(true);
    let v = f();
    counting::sample(false);
    println!("  allocation sites during {label}:");
    counting::report(14);
    v
}

#[cfg(not(feature = "count"))]
fn sites<T>(_label: &str, f: impl FnOnce() -> T) -> T {
    f()
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Cost) {
    let a = allocs();
    let t = Instant::now();
    let v = f();
    let time = t.elapsed();
    (
        v,
        Cost {
            time,
            allocs: allocs() - a,
        },
    )
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn phases(p: &dyn LanguageProvider, name: &str, text: &str) -> Result<Vec<(&'static str, Cost)>> {
    let (f, opts) = file(p, name, 1, text);
    let (parsed, parse) = timed(|| p.parse(&f));
    let parsed = parsed?;
    if let Some(e) = parsed.errors.first() {
        return Err(anyhow!("input does not parse: {}", e.message));
    }
    let directives = DirectiveTable::default();
    let cx = FormatCtx::new(p.grammar(), &opts, &directives, p.formatter());
    let (doc, layout) = timed(|| sites("layout", || cx.format_unguarded(&parsed.tree)));
    let mut doc = doc?;
    let ((out, _), print) = timed(|| {
        core_format::printer::print_protected(&mut doc, &core_format::print_options(&opts))
    });
    let (_, drop_doc) = timed(|| drop(doc));
    let (again, _) = file(p, name, 2, &out);
    let (re, reparse) = timed(|| p.parse(&again));
    let re = re?;
    let (cmp, compare) =
        timed(|| safety::equal_ignoring_trivia(p.grammar(), &parsed.tree, &re.tree));
    cmp.map_err(|e| anyhow!(e))?;
    let (extra, extra_t) = timed(|| {
        sites("extra check", || {
            p.formatter().extra_check(&parsed.tree, &re.tree)
        })
    });
    extra.map_err(|e| anyhow!(e))?;
    Ok(vec![
        ("parse", parse),
        ("layout document", layout),
        ("print", print),
        ("drop layout document", drop_doc),
        ("safety: re-parse", reparse),
        ("safety: tree compare", compare),
        ("safety: extra check (data model)", extra_t),
    ])
}

fn whole(p: &dyn LanguageProvider, name: &str, text: &str) -> Result<Duration> {
    let (f, opts) = file(p, name, 1, text);
    let (r, c) = timed(|| provider_api::format_file(p, &f, &opts, &DirectiveTable::default()));
    r?;
    Ok(c.time)
}

fn report(
    label: &str,
    p: &dyn LanguageProvider,
    name: &str,
    make: fn(usize) -> String,
    base: usize,
    scaling: bool,
) -> Result<()> {
    let text = make(base);
    let mb = text.len() as f64 / 1e6;
    let ph = phases(p, name, &text)?;
    let total: Duration = ph.iter().map(|x| x.1.time).sum();
    println!("== {label}: {mb:.2} MB ==");
    for (n, c) in &ph {
        let allocs = if cfg!(feature = "count") {
            format!("{:>10} allocs", c.allocs)
        } else {
            String::new()
        };
        println!(
            "  {n:<34} {:>9.1} ms {:>6.1}% {allocs}",
            ms(c.time),
            100.0 * c.time.as_secs_f64() / total.as_secs_f64()
        );
    }
    println!(
        "  {:<34} {:>9.1} ms  {:.2} MB/s",
        "total (phases summed)",
        ms(total),
        mb / total.as_secs_f64()
    );
    if !scaling {
        return Ok(());
    }
    println!("  scaling of format_file (best of 3):");
    for (factor, n) in [("1x", base / 4), ("4x", base), ("16x", base * 4)] {
        let t = make(n);
        let best = (0..3)
            .map(|_| whole(p, name, &t))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .min()
            .unwrap();
        let size = t.len() as f64 / 1e6;
        println!(
            "    {factor:>3} {size:>6.2} MB {:>9.1} ms {:>7.2} MB/s",
            ms(best),
            size / best.as_secs_f64()
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let which = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map_or("all", String::as_str);
    let scaling = !args.iter().any(|a| a == "--phases-only");
    if matches!(which, "json" | "all") {
        report(
            "JSON",
            &lang_json::JsonProvider::new(),
            "bench.json",
            inputs::json,
            JSON_OBJECTS,
            scaling,
        )?;
    }
    if matches!(which, "json-flat" | "all") {
        report(
            "JSON, one line",
            &lang_json::JsonProvider::new(),
            "flat.json",
            inputs::json_flat,
            50_000,
            scaling,
        )?;
    }
    if matches!(which, "yaml" | "all") {
        report(
            "YAML",
            &lang_yaml::YamlProvider::new(),
            "bench.yaml",
            inputs::yaml,
            YAML_ITEMS,
            scaling,
        )?;
    }
    Ok(())
}
