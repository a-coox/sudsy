//! Which languages are installed. JSON, YAML and C# are in the binary and only a toggle in the
//! state file removes them; every other language is a tree-sitter grammar that is linked in
//! (`static-grammars`, where the toggle is all there is) or loaded from its language pack.

use provider_api::LanguageProvider;
use std::collections::HashMap;
use std::path::Path;

const BUILT_IN: &[&str] = &["json", "yaml", "csharp"];

fn from_pack(language: &str) -> bool {
    !cfg!(feature = "static-grammars") && !BUILT_IN.contains(&language)
}

/// Why `language` cannot be used, or `Ok` when its files can be processed. Loads (and so
/// verifies) the pack of a pack language.
pub fn check(language: &str) -> Result<(), String> {
    if core_packs::is_removed(language) {
        return Err("not installed".into());
    }
    if from_pack(language) {
        core_packs::load(language, "").map_err(|e| match e {
            core_packs::PackError::NotInstalled(_) | core_packs::PackError::NoPack(_) => {
                "not installed".to_string()
            }
            e => e.to_string(),
        })?;
    }
    Ok(())
}

/// Finds the files whose language cannot be used, checking each language once.
pub struct Checker<'a> {
    providers: &'a [Box<dyn LanguageProvider>],
    cache: HashMap<&'a str, Result<(), String>>,
}

impl<'a> Checker<'a> {
    pub fn new(providers: &'a [Box<dyn LanguageProvider>]) -> Self {
        Checker {
            providers,
            cache: HashMap::new(),
        }
    }

    /// `(language, why)` when `path` is a known language that cannot be used.
    pub fn missing(&mut self, path: &Path) -> Option<(&'a str, String)> {
        let (index, _) = provider_api::select_provider_index(self.providers, path)?;
        let language = self.providers[index].language();
        let result = self
            .cache
            .entry(language)
            .or_insert_with(|| check(language));
        result.clone().err().map(|why| (language, why))
    }
}

/// One row of `sudsy languages list`.
pub struct Row {
    pub language: String,
    pub installed: bool,
    /// Size of the pack, or `built in`.
    pub size: String,
}

pub fn list(providers: &[Box<dyn LanguageProvider>]) -> Vec<Row> {
    let mut rows: Vec<Row> = providers
        .iter()
        .map(|p| Row {
            language: p.language().to_string(),
            installed: check(p.language()).is_ok(),
            size: match (from_pack(p.language()), core_packs::entry(p.language())) {
                (false, _) => "built in".into(),
                (true, Ok(e)) => format!("{} KB", e.size.div_ceil(1000)),
                (true, Err(_)) => "no pack in this build".into(),
            },
        })
        .collect();
    rows.sort_by(|a, b| a.language.cmp(&b.language));
    rows
}

/// Installs `language`: from `dir` (the release's files) or the release assets. Does nothing
/// for a pack that loads.
pub fn add(language: &str, dir: Option<&Path>) -> Result<(), String> {
    core_packs::set_removed(language, false).map_err(|e| e.to_string())?;
    if !from_pack(language) || check(language).is_ok() {
        return Ok(());
    }
    match dir {
        Some(dir) => core_packs::install_from(language, dir),
        None => core_packs::download(language),
    }
    .map_err(|e| e.to_string())
}

pub fn remove(language: &str) -> Result<(), String> {
    core_packs::set_removed(language, true).map_err(|e| e.to_string())?;
    if from_pack(language) {
        core_packs::remove(language).map_err(|e| e.to_string())?;
    }
    Ok(())
}
