//! Repository automation, stage 2: what needs the compiled languages. `codegen` writes the JSON
//! Schema of the config, the VS Code language list and the reference tables of the docs from
//! the language providers; `affected` reads the providers to select what a change needs checked.
//! Built after `xtask` has written the glue those languages are compiled from.

pub mod affected;
mod differences;
mod docs;
pub mod manifests;
mod rules;
mod vscode;

use anyhow::Result;
use provider_api::LanguageProvider;
use std::path::PathBuf;

/// Every stage 2 file as (path relative to the repo root, contents).
pub fn generate() -> Result<Vec<(PathBuf, String)>> {
    let providers = languages::providers();
    let mut files = vec![(
        "schemas/sudsy.schema.json".into(),
        config_schema(&providers)?,
    )];
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

/// Paths of stage 1 and stage 2 files whose checked-in content differs from fresh output.
pub fn stale_files() -> Result<Vec<PathBuf>> {
    let mut stale = xtask::stale(xtask::generate()?);
    stale.extend(xtask::stale(generate()?));
    Ok(stale)
}
