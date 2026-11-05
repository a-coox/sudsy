//! Repository automation. `cargo xtask codegen` regenerates checked-in files from
//! the C# grammar (the worker's constants), the language providers, the JSON Schema of the config and the reference tables
//! of the docs; `cargo xtask codegen --check` (and the `codegen_is_fresh` test)
//! fails when they are stale.

mod docs;
mod new_rule;
mod rules;
mod vscode;

pub use new_rule::new_rule;

use anyhow::{Context, Result, ensure};
use core_syntax::{Grammar, codegen};
use provider_api::LanguageProvider;
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
    let providers = languages::providers();
    let mut files: Vec<(PathBuf, String)> = vec![(
        "schemas/sudsy.schema.json".into(),
        config_schema(&providers)?,
    )];
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
    files.extend(vscode::generate(&providers)?);
    files.extend(docs::generate(&providers)?);
    Ok(files)
}

/// The JSON Schema of `sudsy.yaml`, from the option declarations of the providers; editors register it for completion and validation of the config.
fn config_schema(providers: &[Box<dyn LanguageProvider>]) -> Result<String> {
    let schema = core_config::json_schema(
        &languages::schemas(providers),
        &languages::rule_schemas(providers),
    );
    Ok(serde_json::to_string_pretty(&schema)? + "\n")
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
