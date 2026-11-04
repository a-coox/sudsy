//! The parity catalog (`docs/parity/*.yaml`), embedded: it is the source of the reasons printed
//! for settings that cannot be migrated and of the checks that each mapping agrees with it.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
pub struct Entry {
    pub id: String,
    pub sudsy: String,
    #[serde(default)]
    pub coverage: String,
    #[serde(default)]
    pub gap: String,
}

#[derive(Deserialize)]
struct File {
    entries: Vec<Entry>,
}

macro_rules! catalog {
    ($name:literal) => {{
        static CELL: OnceLock<Vec<Entry>> = OnceLock::new();
        CELL.get_or_init(|| {
            let text = include_str!(concat!("../../../../docs/parity/", $name, ".yaml"));
            serde_saphyr::from_str::<File>(text)
                .expect("parity catalog parses")
                .entries
        })
    }};
}

/// The entries of the catalog named `tool` (the file name without `.yaml`).
pub fn entries(tool: &str) -> &'static [Entry] {
    match tool {
        "yamllint" => catalog!("yamllint"),
        "prettier" => catalog!("prettier"),
        "biome" => catalog!("biome"),
        "eslint-plugin-jsonc" => catalog!("eslint-plugin-jsonc"),
        "stylecop" => catalog!("stylecop"),
        "csharpier" => catalog!("csharpier"),
        _ => &[],
    }
}

/// The entry for an upstream setting. Some entries name several settings (`a / b`, `a, b`).
pub fn find(tool: &str, id: &str) -> Option<&'static Entry> {
    entries(tool)
        .iter()
        .find(|e| e.id == id || e.id.split([',', '/']).any(|part| part.trim() == id))
}

/// Why a setting cannot be migrated, in the catalog's words.
pub fn reason(tool: &str, id: &str) -> String {
    match find(tool, id) {
        Some(e) if !e.gap.is_empty() => e.gap.clone(),
        Some(e) if e.sudsy == "formatter" => "fixed formatter behavior, not configurable".into(),
        Some(e) if e.sudsy.starts_with("rule: ") || e.sudsy.starts_with("option: ") => {
            format!("this setting has no Sudsy option; {}", e.sudsy)
        }
        Some(e) => e.sudsy.clone(),
        None => "not in the parity catalog".into(),
    }
}
