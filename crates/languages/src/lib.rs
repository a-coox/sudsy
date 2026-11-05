//! The languages Sudsy ships and what is derived from the whole set: the providers, their option
//! schemas and rule lists. The CLI, the LSP server and `xtask` all need exactly this list.

pub mod catalog;
mod extensions;
mod project;

pub use extensions::load_extensions;

pub use project::{Project, ProjectError, describe_error};

use core_config::{OptionSchema, RuleSchema};
use core_rules::{Extensions, Rule};
use provider_api::LanguageProvider;
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
        Box::new(lang_csharp::provider(&worker)),
    ]
}

/// `(language, option schema)` of every provider.
pub fn schemas(providers: &[Box<dyn LanguageProvider>]) -> Vec<(&str, &OptionSchema)> {
    providers
        .iter()
        .map(|p| (p.grammar().language.as_str(), p.option_schema()))
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
        let language = provider.grammar().language.clone();
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

/// The rules as the config's JSON Schema describes them, one per id.
pub fn rule_schemas(providers: &[Box<dyn LanguageProvider>]) -> Vec<RuleSchema> {
    let mut out: Vec<RuleSchema> = rules(providers, &Extensions::default())
        .iter()
        .map(|(_, r)| r.meta().schema())
        .collect();
    out.dedup_by(|a, b| a.id == b.id);
    out
}
