//! Repository automation. `cargo xtask codegen` regenerates checked-in files from
//! the grammars, the JSON Schema of the config and the reference tables of the docs; `cargo xtask codegen --check` (and the `codegen_is_fresh` test)
//! fails when they are stale.

mod docs;

use anyhow::{Context, Result};
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
    let load = |name: &str| -> Result<Grammar> {
        let path = root.join("grammars").join(format!("{name}.grammar"));
        let src = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Grammar::parse(&src).with_context(|| format!("parsing {}", path.display()))
    };
    let json = load("json")?;
    let csharp = load("csharp")?;
    let yaml = load("yaml")?;
    let toml = load("toml")?;
    let providers = languages::providers();
    let mut files = vec![
        (
            "schemas/sudsy.schema.json".into(),
            config_schema(&providers)?,
        ),
        (
            "crates/lang-json/src/generated.rs".into(),
            codegen::rust_module(&json, "grammars/json.grammar", "core_syntax")?,
        ),
        (
            "crates/lang-yaml/src/generated.rs".into(),
            codegen::rust_module(&yaml, "grammars/yaml.grammar", "core_syntax")?,
        ),
        (
            "crates/lang-toml/src/generated.rs".into(),
            codegen::rust_module(&toml, "grammars/toml.grammar", "core_syntax")?,
        ),
        (
            "crates/lang-csharp/src/generated.rs".into(),
            codegen::rust_module(&csharp, "grammars/csharp.grammar", "core_syntax")?,
        ),
        (
            "workers/csharp/Sudsy.Worker/Generated/CsharpGrammar.g.cs".into(),
            codegen::csharp_constants(&csharp, "grammars/csharp.grammar")?,
        ),
    ];
    files.extend(docs::generate(&providers)?);
    Ok(files)
}

/// The JSON Schema of `sudsy.yaml`, from the option declarations of the providers; editors register it for completion and validation of the config.
fn config_schema(providers: &[Box<dyn LanguageProvider>]) -> Result<String> {
    let schema = core_config::json_schema(
        &languages::schemas(providers),
        &languages::rule_schemas(providers),
    );
    Ok(serde_json::to_string_pretty(&schema)?
        + "
")
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
