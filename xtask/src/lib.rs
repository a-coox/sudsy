//! Repository automation, stage 1: `cargo xtask codegen` writes the glue of the language crates
//! (registration, provider, node mapping, options) and the worker's grammar constants from
//! `lang.yaml` and the grammar files only, so it compiles when that glue is stale. Stage 2
//! (`xtask-full`) builds the languages and writes what derives from them (schema, docs, editor
//! lists). `cargo xtask codegen --check` (and the `codegen_is_fresh` test) fails when either is stale.

mod css_data;
mod lang_crate;
mod mapping;
mod new_rule;
mod options;
mod registry;

pub use css_data::generate as css_data;
pub use new_rule::new_rule;

use anyhow::{Context, Result, ensure};
use core_syntax::{Grammar, codegen};
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the repo root")
        .to_path_buf()
}

/// The stage 1 files as (path relative to the repo root, contents).
pub fn generate() -> Result<Vec<(PathBuf, String)>> {
    let root = repo_root();
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(root.join("grammars"))? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "grammar") {
            continue;
        }
        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let grammar =
            Grammar::parse(&src).with_context(|| format!("parsing {}", path.display()))?;
        if path.file_stem().is_some_and(|n| n == "csharp") {
            files.push((
                "workers/csharp/Sudsy.Worker/Generated/CsharpGrammar.g.cs".into(),
                codegen::csharp_constants(&grammar, "grammars/csharp.grammar")?,
            ));
        }
        ids.push(grammar.language_id);
    }
    let n = ids.len();
    ids.sort();
    ids.dedup();
    ensure!(ids.len() == n, "two grammars share a language id");
    let langs = lang_crate::load(&root)?;
    files.extend(lang_crate::generate(&langs)?);
    files.extend(registry::generate(&lang_crate::tree_sitter(&langs))?);
    Ok(files)
}

/// Paths of generated files whose checked-in content differs from fresh output.
pub fn stale(files: Vec<(PathBuf, String)>) -> Vec<PathBuf> {
    let root = repo_root();
    files
        .into_iter()
        .filter(|(rel, content)| {
            std::fs::read_to_string(root.join(rel)).ok().as_deref() != Some(content)
        })
        .map(|(rel, _)| rel)
        .collect()
}

/// Writes the files; returns those that changed on disk.
pub fn write(files: Vec<(PathBuf, String)>) -> Result<Vec<PathBuf>> {
    let root = repo_root();
    let mut changed = Vec::new();
    for (rel, content) in files {
        let path = root.join(&rel);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(&content) {
            continue;
        }
        std::fs::create_dir_all(path.parent().expect("file has a parent"))?;
        std::fs::write(&path, content)?;
        println!("wrote {}", rel.display());
        changed.push(rel);
    }
    Ok(changed)
}
