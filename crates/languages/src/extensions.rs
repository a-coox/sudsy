//! The pattern rules and plugins a config names (`plugins:`), loaded into the rules that join
//! the providers' own (`core_rules::Extensions`).

use core_config::{ConfigChain, ConfigError, PluginEntry, PluginKind};
use core_rules::{ExtensionRule, Extensions, Rule};
use provider_api::{LanguageProvider, ProviderSnippets};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Largest plugin `.wasm` read and compiled (bytes).
const MAX_WASM_BYTES: u64 = 32 << 20;

type Loaded = Vec<(&'static str, &'static dyn Rule)>;

/// Rules read from a pattern file, by the file's text. A rule is leaked once for as long as the
/// process lives (rules are `'static`), so reloading an unchanged config, which an editor does
/// often, costs nothing and an unchanged rule stays the same object.
fn cached(
    cache: &Mutex<HashMap<String, Loaded>>,
    source: &str,
    load: impl FnOnce() -> Result<Loaded, String>,
) -> Result<Loaded, String> {
    if let Some(found) = cache.lock().expect("cache").get(source) {
        return Ok(found.clone());
    }
    let loaded = load()?;
    cache
        .lock()
        .expect("cache")
        .insert(source.to_string(), loaded.clone());
    Ok(loaded)
}

fn load_patterns(source: &str, providers: &[Box<dyn LanguageProvider>]) -> Result<Loaded, String> {
    static PATTERNS: std::sync::LazyLock<Mutex<HashMap<String, Loaded>>> =
        std::sync::LazyLock::new(Mutex::default);
    cached(&PATTERNS, source, || {
        let snippets: Vec<(String, ProviderSnippets)> = providers
            .iter()
            .map(|p| (p.grammar().language.clone(), ProviderSnippets(p.as_ref())))
            .collect();
        let rules = core_patterns::load(source, &|language| {
            snippets
                .iter()
                .find(|(l, _)| l == language)
                .map(|(_, s)| s as &dyn core_patterns::SnippetParser)
        })
        .map_err(|e| e.0)?;
        Ok(rules
            .into_iter()
            .map(|(language, rule)| {
                let language: &'static str = Box::leak(language.into_boxed_str());
                (language, &*Box::leak(Box::new(rule)) as &'static dyn Rule)
            })
            .collect())
    })
}

/// Plugins by their bytes, for the same reason as patterns: an unchanged plugin stays the same
/// object (and keeps its state) when the config is read again.
fn load_wasm(path: &Path, bytes: Vec<u8>) -> Result<Loaded, String> {
    static PLUGINS: std::sync::LazyLock<Mutex<HashMap<Vec<u8>, Loaded>>> =
        std::sync::LazyLock::new(Mutex::default);
    if let Some(found) = PLUGINS.lock().expect("cache").get(&bytes) {
        return Ok(found.clone());
    }
    let loaded = core_plugins::load(path, &bytes).map_err(|e| e.0)?;
    PLUGINS.lock().expect("cache").insert(bytes, loaded.clone());
    Ok(loaded)
}

fn files_of(entry: &PluginEntry, root: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let dir = entry.dir.as_deref().unwrap_or(root);
    core_workspace::expand(dir, &entry.entry)
        .map_err(|e| ConfigError::new(format!("plugin `{}`: {e}", entry.entry)))
}

/// Loads every pattern file the config names. `root` is the base of entries that no config file
/// directory applies to. Fails, naming the file, when one cannot be read or compiled, when two
/// rules of a language share an id, or when a rule reuses the id of a built-in one.
pub fn load_extensions(
    chain: &ConfigChain,
    providers: &[Box<dyn LanguageProvider>],
    root: &Path,
) -> Result<Extensions, ConfigError> {
    // Only a project's own rules are checked against these, so they are collected when it has some.
    let mut builtin = None;
    let mut extensions = Extensions::default();
    let mut origin: HashMap<(&'static str, &'static str), PathBuf> = HashMap::new();
    for entry in chain.plugin_entries() {
        for path in files_of(&entry, root)? {
            let at = |message: String| {
                let mut e = ConfigError::new(message);
                e.path = Some(path.clone());
                e
            };
            // Errors of the loaders quote the file; not for one outside the project.
            let inside = std::fs::canonicalize(&path)
                .is_ok_and(|p| p.starts_with(std::fs::canonicalize(root).unwrap_or(root.into())));
            let quiet = |message: String| {
                at(if inside {
                    message
                } else {
                    "invalid (details withheld: the file is outside the project)".to_string()
                })
            };
            let rules = match entry.kind {
                PluginKind::Pattern => {
                    let bytes = core_config::read_capped(&path, core_config::MAX_CONFIG_BYTES)
                        .map_err(|e| at(format!("cannot read the pattern file: {e}")))?;
                    let source = String::from_utf8(bytes)
                        .map_err(|_| at("cannot read the pattern file: not valid UTF-8".into()))?;
                    load_patterns(&source, providers).map_err(quiet)?
                }
                PluginKind::Wasm => {
                    let bytes = core_config::read_capped(&path, MAX_WASM_BYTES)
                        .map_err(|e| at(format!("cannot read the plugin: {e}")))?;
                    load_wasm(&path, bytes).map_err(quiet)?
                }
            };
            extensions.files.push(path.clone());
            for (language, rule) in rules {
                let id = rule.meta().id;
                if language != core_rules::ALL_LANGUAGES
                    && !providers.iter().any(|p| p.grammar().language == language)
                {
                    let known = providers.iter().map(|p| p.grammar().language.as_str());
                    return Err(at(format!(
                        "rule `{id}`: {}",
                        core_config::unknown_name("language", language, known)
                    )));
                }
                if builtin
                    .get_or_insert_with(|| crate::rule_ids(providers))
                    .contains(&id)
                {
                    return Err(at(format!("rule `{id}` is the id of a built-in rule")));
                }
                if let Some(first) = origin.insert((language, id), path.clone()) {
                    return Err(at(format!(
                        "rule `{id}` for {language} is also defined in {}",
                        core_workspace::display_path(&first, root)
                    )));
                }
                extensions.rules.push(ExtensionRule { language, rule });
            }
        }
    }
    Ok(extensions)
}
