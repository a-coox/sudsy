//! `perf-bench csharp DIR [PASSES]`: `sudsy format --check` on the C# files under DIR, several
//! passes in one process with one worker pool, so that every pass after the first runs against a
//! warm worker (the number a long-lived `sudsy lsp` gives, and the one the per-run CLI cannot).
//! Each pass makes the same work as one run of the CLI (parse, format, safety check) with the
//! `csharp/csharpier-compat` preset, on all cores; then one pass on a single thread reports the
//! time per file, which is what an editor asking for one file at a time sees.

use anyhow::{Result, anyhow};
use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, format};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// Formats every file on `threads` threads; returns the wall time, the number of files that could
/// not be formatted and the time of each file.
fn pass(
    db: &Database,
    texts: &[(PathBuf, String)],
    threads: usize,
) -> (Duration, usize, Vec<Duration>) {
    let next = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let started = Instant::now();
    let mut times = Vec::new();
    std::thread::scope(|scope| {
        // A salsa handle is not shared between threads: each thread gets its own.
        let handles: Vec<Database> = (0..threads).map(|_| db.clone()).collect();
        let workers: Vec<_> = handles
            .into_iter()
            .map(|db| {
                let (next, failed) = (&next, &failed);
                scope.spawn(move || {
                    let mut mine = Vec::new();
                    while let Some((path, text)) = texts.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let t = Instant::now();
                        let file = File::new(
                            &db,
                            path.clone(),
                            1,
                            text.clone(),
                            false,
                            EditorConfig::default(),
                        );
                        if format(&db, file).is_err() {
                            failed.fetch_add(1, Ordering::Relaxed);
                        }
                        mine.push(t.elapsed());
                    }
                    mine
                })
            })
            .collect();
        for w in workers {
            times.extend(w.join().expect("worker thread"));
        }
    });
    (started.elapsed(), failed.into_inner(), times)
}

pub fn run(dir: &Path, passes: usize) -> Result<()> {
    let worker = lang_csharp::locate_worker().map_err(|e| anyhow!(e))?;
    let mut paths = Vec::new();
    collect(dir, &mut paths)?;
    paths.sort();
    let mut texts = Vec::new();
    let mut bytes = 0;
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
        bytes += text.len();
        texts.push((path, text));
    }
    let mut db = Database::new(vec![Box::new(lang_csharp::provider(&worker))]);
    db.set_config(Ok(ConfigChain::parse(
        "extends:\n  - preset:csharp/csharpier-compat\n",
    )?));
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!("{} files, {:.1} MB", texts.len(), bytes as f64 / 1e6);
    for n in 1..=passes {
        let (wall, failed, _) = pass(&db, &texts, threads);
        println!(
            "pass {n} ({threads} threads): {:.2} s, {failed} files not formatted",
            wall.as_secs_f64()
        );
    }
    let (wall, _, mut times) = pass(&db, &texts, 1);
    times.sort();
    let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize].as_secs_f64() * 1e3;
    println!(
        "one thread: {:.2} s, per file median {:.1} ms, p95 {:.1} ms, p99 {:.1} ms",
        wall.as_secs_f64(),
        at(0.5),
        at(0.95),
        at(0.99)
    );
    Ok(())
}
