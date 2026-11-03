//! `perf-bench semantic DIR [LIMIT]`: what the semantic layers cost at each analysis depth on a
//! C# tree, and whether the flow rules agree with the compiler. The files under DIR are one
//! workspace; every file is linted in turn at `fast`, `standard` and `deep`, and at `standard`
//! the findings of `unreachable-code` and `unused-variable` are compared with the compiler's
//! CS0162, CS0168 and CS0219 for the same file.

use anyhow::{Result, anyhow};
use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, Db, File, lint, project_diagnostics};
use provider_api::SourceFile;
use std::collections::BTreeSet;
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

const RULES: &str = "    unreachable-code:
      level: warn
      options:
        non_returning_calls: false
    unused-variable: warn
    unused-assignment: warn
    remove-unused-usings: warn
    possible-null-dereference: warn
    unused-private-member: warn
";

fn database(worker: &Path, depth: &str, dir: &Path) -> Result<Database> {
    let mut db = Database::new(vec![Box::new(lang_csharp::provider(worker))]);
    db.set_config(Ok(ConfigChain::parse(&format!(
        "analysis: {depth}\nlint:\n  rules:\n{RULES}"
    ))?));
    db.open_workspace(&[dir.to_path_buf()]);
    Ok(db)
}

/// Findings as (compiler diagnostic id, file offset).
type Findings = BTreeSet<(&'static str, u32)>;

pub fn run(dir: &Path, limit: Option<usize>) -> Result<()> {
    let worker = lang_csharp::locate_worker().map_err(|e| anyhow!(e))?;
    let mut paths = Vec::new();
    collect(dir, &mut paths)?;
    paths.sort();
    paths.truncate(limit.unwrap_or(usize::MAX));
    let dir = std::fs::canonicalize(dir)?;
    let mut texts = Vec::new();
    for p in &paths {
        let p = std::fs::canonicalize(p)?;
        if let Ok(t) = String::from_utf8(std::fs::read(&p)?) {
            texts.push((p, t.trim_start_matches('\u{feff}').to_string()));
        }
    }
    println!(
        "{} files, {:.1} MB",
        texts.len(),
        texts.iter().map(|t| t.1.len()).sum::<usize>() as f64 / 1e6
    );
    let (mut agree, mut ours_only, mut compiler_only) = (0, Vec::new(), Vec::new());
    let only = std::env::var("PERF_DEPTHS").unwrap_or_default();
    for depth in ["fast", "standard", "deep"] {
        if !only.is_empty() && !only.split(',').any(|d| d == depth) {
            continue;
        }
        let db = database(&worker, depth, &dir)?;
        let started = Instant::now();
        let mut files = Vec::new();
        let (mut first, mut times, mut found) = (Duration::ZERO, Vec::new(), 0usize);
        for (i, (path, text)) in texts.iter().enumerate() {
            let t = Instant::now();
            let file = File::new(
                &db,
                path.clone(),
                1,
                text.clone(),
                false,
                EditorConfig::default(),
            );
            if let Ok(linted) = lint(&db, file) {
                found += linted.diagnostics.len();
                if depth == "standard" {
                    let mut ours = Findings::new();
                    for d in &linted.diagnostics {
                        let kind = match d.rule.as_str() {
                            "unreachable-code" => "CS0162",
                            "unused-variable" if d.message.contains("declared but") => "CS0168",
                            "unused-variable" => "CS0219",
                            _ => continue,
                        };
                        ours.insert((kind, d.range.start().into()));
                    }
                    let source = SourceFile::new(path.to_string_lossy(), 1, text.clone());
                    let ids = ["CS0162", "CS0168", "CS0219"];
                    let theirs: Findings = db.providers()[0]
                        .compiler_diagnostics(&source, &ids)?
                        .iter()
                        .filter_map(|d| ids.iter().find(|i| **i == d.id).map(|i| (*i, d.start)))
                        .collect();
                    agree += ours.intersection(&theirs).count();
                    for f in ours.difference(&theirs) {
                        ours_only.push((path.clone(), *f));
                    }
                    for f in theirs.difference(&ours) {
                        compiler_only.push((path.clone(), *f));
                    }
                }
            }
            files.push(file);
            let elapsed = t.elapsed();
            if i == 0 {
                first = elapsed;
            }
            times.push(elapsed);
        }
        let project = if depth == "deep" {
            project_diagnostics(&db, &files)
                .map(|d| d.len())
                .unwrap_or(0)
        } else {
            0
        };
        times.sort();
        println!(
            "{depth:9} total {:7.1} s   first file {:6.2} s   median {:5.1} ms   p95 {:6.1} ms   diagnostics {found}   project-scope {project}",
            started.elapsed().as_secs_f64(),
            first.as_secs_f64(),
            times[times.len() / 2].as_secs_f64() * 1e3,
            times[times.len() * 95 / 100].as_secs_f64() * 1e3,
        );
    }
    println!(
        "compiler agreement (CS0162/CS0168/CS0219): {agree} agree, {} ours only, {} compiler only",
        ours_only.len(),
        compiler_only.len()
    );
    for (label, list) in [("ours only", &ours_only), ("compiler only", &compiler_only)] {
        for (path, (kind, at)) in list.iter().take(15) {
            let text = &texts.iter().find(|t| &t.0 == path).unwrap().1;
            let line = text[..*at as usize].matches('\n').count() + 1;
            println!("  {label}: {kind} {}:{line}", path.display());
        }
    }
    Ok(())
}
