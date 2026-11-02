//! `sudsy init [--preset <name>]`: writes a starter `sudsy.yaml` in the working directory.

use crate::project::{providers, schemas};
use anyhow::{Result, bail};
use clap::Args;
use std::path::Path;

#[derive(Args)]
pub struct InitArgs {
    /// A preset to extend, e.g. `csharp/google@1`; repeat for several languages. Without it the
    /// starter extends the default preset of each language.
    #[arg(long = "preset")]
    presets: Vec<String>,
}

/// The preset each language gets when none is named.
const DEFAULT_PRESETS: [&str; 3] = ["csharp/microsoft@1", "json/default@1", "yaml/default@1"];

pub fn run(args: InitArgs) -> Result<()> {
    let path = Path::new("sudsy.yaml");
    if path.exists() || Path::new("sudsy.yml").exists() {
        bail!("a sudsy config already exists here; not overwriting it");
    }
    let providers = providers();
    let schemas = schemas(&providers);
    let known: Vec<&str> = schemas
        .iter()
        .flat_map(|(_, s)| s.presets().iter().map(|p| p.name))
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
        // A name without a version means the newest one.
        let versioned = if preset.contains('@') {
            Some(preset.as_str())
        } else {
            known
                .iter()
                .filter(|k| k.split('@').next() == Some(preset.as_str()))
                .max()
                .copied()
        };
        match versioned.filter(|v| known.contains(v)) {
            Some(v) => text.push_str(&format!(
                "  - preset:{v}
"
            )),
            None => bail!(
                "unknown preset `{preset}`; known presets: {}",
                known.join(", ")
            ),
        }
    }
    std::fs::write(path, text)?;
    eprintln!("wrote sudsy.yaml");
    Ok(())
}
