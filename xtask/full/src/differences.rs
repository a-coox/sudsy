//! The intentional differences from the compared tools: the `choice` names of
//! `tests/parity/<tool>/differences.yaml` (format: docs/development/parity.md) become the block
//! `<!-- generated:differences:<tool> -->` of the one `docs/formatting/*.md` that holds the marker.

use anyhow::{Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Differences {
    /// name -> `<choice|gap>  <text>`
    #[serde(default)]
    differences: BTreeMap<String, String>,
}

pub fn generate() -> Result<Vec<(PathBuf, String)>> {
    let root = xtask::repo_root();
    let docs: Vec<_> = std::fs::read_dir(root.join("docs/formatting"))?
        .map(|f| f.map(|f| f.path()))
        .collect::<std::io::Result<_>>()?;
    let mut blocks: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for folder in std::fs::read_dir(root.join("tests/parity"))? {
        let path = folder?.path().join("differences.yaml");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let tool = path
            .parent()
            .and_then(|p| p.file_name())
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let d: Differences = serde_saphyr::from_str(&text)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let body: String = d
            .differences
            .iter()
            .filter_map(|(name, value)| Some((name, value.strip_prefix("choice")?.trim())))
            .map(|(name, text)| format!("- `{name}`: {text}\n"))
            .collect();
        // A blank line after the marker keeps the list a list for Prettier and Sudsy.
        let body = if body.is_empty() {
            body
        } else {
            format!("\n{body}")
        };
        let marker = format!("<!-- generated:differences:{tool} -->");
        let doc = docs
            .iter()
            .find(|d| std::fs::read_to_string(d).is_ok_and(|t| t.contains(&marker)));
        match (doc, body.is_empty()) {
            (Some(doc), _) => {
                let rel = format!(
                    "docs/formatting/{}",
                    doc.file_name().unwrap().to_string_lossy()
                );
                blocks
                    .entry(rel)
                    .or_default()
                    .push((format!("differences:{tool}"), body));
            }
            (None, false) => bail!(
                "tests/parity/{tool}/differences.yaml has choice names: add `{marker}` ... `<!-- /generated -->` to its docs/formatting file"
            ),
            (None, true) => {}
        }
    }
    blocks
        .into_iter()
        .map(|(rel, blocks)| {
            let blocks: Vec<(&str, String)> = blocks
                .iter()
                .map(|(n, b)| (n.as_str(), b.clone()))
                .collect();
            crate::docs::splice(&rel, &blocks)
        })
        .collect()
}
