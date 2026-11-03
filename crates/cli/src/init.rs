//! `sudsy init [--preset <name>]`: writes a starter `sudsy.yaml` in the working directory.

use anyhow::{Result, bail};
use clap::Args;
use languages::{providers, schemas};
use std::path::Path;

#[derive(Args)]
pub struct InitArgs {
    /// A preset to extend, e.g. `csharp/google`; repeat for several languages. Without it the
    /// starter extends the default preset of each language.
    #[arg(long = "preset")]
    presets: Vec<String>,
}

/// The preset each language gets when none is named.
const DEFAULT_PRESETS: [&str; 3] = ["csharp/microsoft", "json/default", "yaml/default"];

pub fn run(args: InitArgs) -> Result<()> {
    let path = Path::new("sudsy.yaml");
    if path.exists() || Path::new("sudsy.yml").exists() {
        bail!("a sudsy config already exists here; not overwriting it");
    }
    let providers = providers();
    let schemas = schemas(&providers);
    let known: Vec<&str> = schemas
        .iter()
        .flat_map(|(_, s)| s.presets().iter().map(|p| p.name.as_str()))
        .collect();
    let presets: Vec<String> = if args.presets.is_empty() {
        DEFAULT_PRESETS.iter().map(|p| (*p).to_string()).collect()
    } else {
        args.presets
    };
    let mut text = String::from(
        "# https://github.com/a-coox/sudsy
extends:
",
    );
    for preset in &presets {
        if !known.contains(&preset.as_str()) {
            bail!(
                "unknown preset `{preset}`; known presets: {}",
                known.join(", ")
            );
        }
        text.push_str(&format!(
            "  - preset:{preset}
"
        ));
    }
    std::fs::write(path, text)?;
    eprintln!("wrote sudsy.yaml");
    Ok(())
}
