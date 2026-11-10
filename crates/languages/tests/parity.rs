//! The parity catalog (`docs/parity/*.yaml`) only names rules and options that exist.

use core_rules::{Extensions, SYNTAX_ERROR};
use languages::catalog::{self, Catalog, Entry};
use std::collections::BTreeMap;
use std::path::PathBuf;

const COVERAGE: [&str; 6] = [
    "covered", "partial", "missing", "sdk", "wontfix", "deferred",
];

fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/parity")
}

fn catalogs() -> Vec<Catalog> {
    let mut out = Vec::new();
    for file in std::fs::read_dir(parity_dir()).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_some_and(|e| e == "yaml") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.push(catalog::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
        }
    }
    out.sort_by(|a, b| a.tool.cmp(&b.tool));
    out
}

/// `status` names something that exists, or is one of the statuses that name nothing checkable.
fn check_status(
    catalog: &Catalog,
    entry: &Entry,
    rules: &[(String, &'static str)],
    options: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let status = entry.sudsy.as_str();
    let (kind, value) = status.split_once(": ").unwrap_or((status, ""));
    match kind {
        "planned" | "deferred" | "formatter" if value.is_empty() => Ok(()),
        "sdk" | "config" | "wontfix" if !value.is_empty() => Ok(()),
        "rule" => {
            if value.contains(' ') {
                return Err("a rule status names exactly one id".into());
            }
            let known = value == SYNTAX_ERROR
                || rules.iter().any(|(language, id)| {
                    *id == value && (language == "all" || catalog.languages.contains(language))
                });
            known.then_some(()).ok_or_else(|| {
                format!(
                    "no rule `{value}` for {}; add the details to `notes`",
                    catalog.languages.join("/")
                )
            })
        }
        "option" => {
            let parts: Vec<&str> = value.split('.').collect();
            let [language, "format", name] = parts[..] else {
                return Err(format!(
                    "`{value}` is not <language>.format.<name> (use `config:` for the rest)"
                ));
            };
            match options.get(language) {
                Some(names) if names.iter().any(|n| n == name) => Ok(()),
                Some(_) => Err(format!("no {language} option `{name}`")),
                None => Err(format!("no language `{language}`")),
            }
        }
        _ => Err(format!("unknown status `{status}`")),
    }
}

#[test]
fn every_rule_and_option_a_status_names_exists() {
    let providers = languages::providers();
    let rules: Vec<(String, &'static str)> = languages::rules(&providers, &Extensions::default())
        .into_iter()
        .map(|(language, rule)| (language, rule.meta().id))
        .collect();
    let options: BTreeMap<String, Vec<String>> = languages::schemas(&providers)
        .into_iter()
        .map(|(language, schema)| {
            let names = schema.defs().iter().map(|d| d.name.to_string()).collect();
            (language.to_string(), names)
        })
        .collect();
    let mut problems = Vec::new();
    for catalog in catalogs() {
        for entry in &catalog.entries {
            if !COVERAGE.contains(&entry.coverage.as_str()) {
                problems.push(format!(
                    "{} {}: unknown coverage `{}`",
                    catalog.tool, entry.id, entry.coverage
                ));
            }
            if let Err(e) = check_status(&catalog, entry, &rules, &options) {
                problems.push(format!("{} {}: {e}", catalog.tool, entry.id));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
