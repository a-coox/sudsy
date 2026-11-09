//! The languages Sudsy ships and what is derived from the whole set: the providers, their option
//! schemas and rule lists. The CLI, the LSP server and `xtask` all need exactly this list.

pub mod catalog;
mod extensions;
pub mod installed;
mod project;

pub use extensions::load_extensions;

pub use project::{Project, ProjectError, describe_error};

use core_config::{OptionSchema, RuleSchema};
use core_rules::{Extensions, Rule};
use provider_api::LanguageProvider;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The providers of the languages Sudsy handles. The C# worker is started on the first C# file
/// only; when it cannot be found, the C# files fail with the reason and the rest are processed.
pub fn providers() -> Vec<Box<dyn LanguageProvider>> {
    let worker = lang_csharp::locate_worker()
        .unwrap_or_else(|_| PathBuf::from("csharp-worker/Sudsy.Worker.dll"));
    vec![
        Box::new(lang_json::JsonProvider::new()),
        Box::new(lang_yaml::YamlProvider::new()),
        Box::new(lang_toml::provider()),
        Box::new(lang_xml::provider()),
        Box::new(lang_markdown::provider()),
        Box::new(lang_editorconfig::provider()),
        Box::new(lang_ini::provider()),
        Box::new(lang_dockerfile::provider()),
        Box::new(lang_shell::provider()),
        Box::new(lang_powershell::provider()),
        Box::new(lang_csharp::provider(&worker)),
    ]
}

/// `(language, option schema)` of every provider.
pub fn schemas(providers: &[Box<dyn LanguageProvider>]) -> Vec<(&str, &OptionSchema)> {
    providers
        .iter()
        .map(|p| (p.language(), p.option_schema()))
        .collect()
}

/// `(language, rule)` of every rule, the project's pattern and plugin rules included; the
/// language is `all` for rules that apply to every language. Sorted by rule id.
pub fn rules(
    providers: &[Box<dyn LanguageProvider>],
    extensions: &Extensions,
) -> Vec<(String, &'static dyn Rule)> {
    const ALL: &str = "all";
    let mut out: Vec<(String, &'static dyn Rule)> = core_rules::shared::RULES
        .iter()
        .map(|r| (ALL.to_string(), *r))
        .collect();
    for provider in providers {
        let language = provider.language().to_string();
        out.extend(provider.rules().iter().map(|r| (language.clone(), *r)));
    }
    out.extend(extensions.rules.iter().map(|r| match r.language {
        core_rules::ALL_LANGUAGES => (ALL.to_string(), r.rule),
        language => (language.to_string(), r.rule),
    }));
    out.sort_by_key(|(language, r)| (r.meta().id, language.clone()));
    out
}

/// The ids of every rule of every language, shared ones included.
pub fn rule_ids(providers: &[Box<dyn LanguageProvider>]) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = rules(providers, &Extensions::default())
        .iter()
        .map(|(_, r)| r.meta().id)
        .collect();
    ids.dedup();
    ids
}

/// For each rule id, the presets (`csharp/stylecop`) whose `lint.rules` turn it on, read from the
/// preset files at any depth (a language section, an override).
pub fn rule_presets(providers: &[Box<dyn LanguageProvider>]) -> BTreeMap<String, Vec<String>> {
    fn walk(value: &serde_json::Value, in_lint: bool, enabled: &mut Vec<String>) {
        let Some(map) = value.as_object() else { return };
        for (key, v) in map {
            if in_lint && key == "rules" {
                for (id, level) in v.as_object().into_iter().flatten() {
                    let level = level.get("level").unwrap_or(level);
                    if level.as_str() != Some("off") {
                        enabled.push(id.clone());
                    }
                }
            } else {
                walk(v, key == "lint", enabled);
            }
        }
    }
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for preset in providers.iter().flat_map(|p| p.option_schema().presets()) {
        let value = serde_saphyr::from_str(preset.text).expect("preset parses");
        let mut enabled = Vec::new();
        walk(&value, false, &mut enabled);
        for id in enabled {
            out.entry(id).or_default().push(preset.name.clone());
        }
    }
    out
}

/// The rules as the config's JSON Schema describes them, one per id.
pub fn rule_schemas(providers: &[Box<dyn LanguageProvider>]) -> Vec<RuleSchema> {
    let mut out: Vec<RuleSchema> = rules(providers, &Extensions::default())
        .iter()
        .map(|(_, r)| r.meta().schema())
        .collect();
    out.dedup_by(|a, b| a.id == b.id);
    out
}
