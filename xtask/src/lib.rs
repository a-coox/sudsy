//! Repository automation. `cargo xtask codegen` regenerates checked-in files from
//! the grammars; `cargo xtask codegen --check` (and the `codegen_is_fresh` test)
//! fails when they are stale.

use anyhow::{Context, Result};
use core_syntax::{Grammar, codegen};
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in the repo root")
        .to_path_buf()
}

/// Every generated file as (path relative to the repo root, contents).
pub fn generate() -> Result<Vec<(PathBuf, String)>> {
    let root = repo_root();
    let load = |name: &str| -> Result<Grammar> {
        let path = root.join("grammars").join(format!("{name}.grammar"));
        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Grammar::parse(&src).with_context(|| format!("parsing {}", path.display()))
    };
    let json = load("json")?;
    let csharp = load("csharp")?;
    Ok(vec![
        (
            "crates/core-syntax/src/generated/json.rs".into(),
            codegen::rust_module(&json, "grammars/json.grammar")?,
        ),
        (
            "crates/core-syntax/src/generated/csharp.rs".into(),
            codegen::rust_module(&csharp, "grammars/csharp.grammar")?,
        ),
        (
            "workers/csharp/Sudsy.Worker/Generated/CSharpGrammar.g.cs".into(),
            codegen::csharp_constants(&csharp, "grammars/csharp.grammar")?,
        ),
    ])
}

/// Paths of generated files whose checked-in content differs from fresh output.
pub fn stale_files() -> Result<Vec<PathBuf>> {
    let root = repo_root();
    Ok(generate()?
        .into_iter()
        .filter(|(rel, content)| {
            std::fs::read_to_string(root.join(rel)).ok().as_deref() != Some(content)
        })
        .map(|(rel, _)| rel)
        .collect())
}

pub fn write_generated() -> Result<()> {
    let root = repo_root();
    for (rel, content) in generate()? {
        let path = root.join(&rel);
        std::fs::create_dir_all(path.parent().expect("file has a parent"))?;
        std::fs::write(&path, content)?;
        println!("wrote {}", rel.display());
    }
    Ok(())
}
