//! The reference tables of the docs, generated from the registered rules, the option schemas and
//! the parity catalog. A generated block sits between `<!-- generated:NAME -->` and
//! `<!-- /generated -->`; the prose around it is hand-written.

use anyhow::{Context, Result, bail};
use core_config::{OptionKind, OptionSchema, OptionValue};
use core_rules::{Check, Extensions, Rule};
use provider_api::LanguageProvider;
use std::collections::BTreeMap;
use std::path::PathBuf;

const END: &str = "<!-- /generated -->";

/// Every doc with generated blocks as (path relative to the repo root, contents).
pub fn generate(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<(PathBuf, String)>> {
    let rules = languages::rules(providers, &Extensions::default());
    let schemas = languages::schemas(providers);
    let mut out = vec![splice(
        "docs/rules/builtin.md",
        &[("rules", rules_table(&rules))],
    )?];
    for (language, schema) in &schemas {
        out.push(splice(
            &format!("docs/formatting/{language}.md"),
            &[("options", options_table(schema))],
        )?);
    }
    out.push(splice(
        "docs/formatting/engine.md",
        &[("shared-options", shared_options_table(&schemas))],
    )?);
    Ok(out)
}

/// The file with the body of each named block replaced.
fn splice(rel: &str, blocks: &[(&str, String)]) -> Result<(PathBuf, String)> {
    let path = crate::repo_root().join(rel);
    let mut text = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
    for (name, body) in blocks {
        let start = format!("<!-- generated:{name} -->");
        let from = text
            .find(&start)
            .with_context(|| format!("{rel}: no `{start}` marker"))?
            + start.len();
        let Some(len) = text[from..].find(END) else {
            bail!("{rel}: `{start}` has no `{END}`");
        };
        text.replace_range(from..from + len, &format!("\n{body}"));
    }
    Ok((rel.into(), text))
}

/// A table cell: one line, no pipes.
fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn language_rank(language: &str) -> usize {
    ["all", "json", "yaml", "toml", "csharp"]
        .iter()
        .position(|l| *l == language)
        .unwrap_or(5)
}

fn rules_table(rules: &[(String, &'static dyn Rule)]) -> String {
    let mut rows: Vec<_> = rules.iter().collect();
    rows.sort_by_key(|(language, rule)| (language_rank(language), rule.meta().id));
    let mut out = String::from(
        "| Rule | Language | Category | Default | Fix | Description |\n|---|---|---|---|---|---|\n",
    );
    for (language, rule) in rows {
        let m = rule.meta();
        let mut description = cell(m.description);
        if !m.options.is_empty() {
            let options: Vec<String> = m
                .options
                .iter()
                .map(|o| {
                    let choices = match o.check {
                        Check::OneOf(values) => format!(
                            "; one of {}",
                            values
                                .iter()
                                .map(|v| format!("`{v}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        _ => String::new(),
                    };
                    format!(
                        "`{}` (default `{}`{choices}): {}",
                        o.name,
                        o.default.display(),
                        cell(o.description).trim_end_matches('.')
                    )
                })
                .collect();
            description += &format!(" Options: {}", options.join("; "));
            if !description.ends_with('.') {
                description.push('.');
            }
        }
        let languages: Vec<&str> = std::iter::once(language.as_str()).collect();
        let equivalent = languages::catalog::equivalents(m.id, &languages);
        if !equivalent.is_empty() {
            description += &format!(" Equivalent: {}.", equivalent.join(", "));
        }
        out += &format!(
            "| `{}` | {language} | {} | {} | {} | {description} |\n",
            m.id,
            m.category.as_str(),
            m.default_level.as_str(),
            m.fix.map_or("none", |k| k.as_str()),
        );
    }
    out
}

fn show(value: &OptionValue) -> String {
    match value {
        OptionValue::Bool(b) => b.to_string(),
        OptionValue::Int(n) => n.to_string(),
        OptionValue::Float(f) => f.to_string(),
        OptionValue::String(s) => s.clone(),
        OptionValue::List(items) => {
            format!(
                "[{}]",
                items.iter().map(show).collect::<Vec<_>>().join(", ")
            )
        }
        OptionValue::Map(_) => "{}".into(),
    }
}

fn values(kind: &OptionKind) -> String {
    match kind {
        OptionKind::Bool => "bool".into(),
        OptionKind::Int { min, max } => format!("int {min}-{max}"),
        OptionKind::Enum(values) => values
            .iter()
            .map(|v| format!("`{v}`"))
            .collect::<Vec<_>>()
            .join(", "),
        OptionKind::Words => "list of words".into(),
        OptionKind::WordSets => "list of lists of words".into(),
    }
}

fn options_table(schema: &OptionSchema) -> String {
    let mut out = String::from("| Option | Values | Default | Description |\n|---|---|---|---|\n");
    for d in schema.defs() {
        out += &format!(
            "| `{}` | {} | `{}` | {} |\n",
            d.name,
            values(&d.kind),
            show(&d.default),
            cell(d.description)
        );
    }
    out
}

/// The options several languages declare, with each language's default.
fn shared_options_table(schemas: &[(&str, &OptionSchema)]) -> String {
    let mut by_name: BTreeMap<&str, Vec<(&str, &core_config::OptionDef)>> = BTreeMap::new();
    for (language, schema) in schemas {
        for d in schema.defs() {
            by_name.entry(d.name).or_default().push((language, d));
        }
    }
    let mut out = String::from("| Option | Values | Defaults |\n|---|---|---|\n");
    for (name, declared) in by_name.iter().filter(|(_, d)| d.len() > 1) {
        let defaults: Vec<String> = declared
            .iter()
            .map(|(language, d)| format!("{language} `{}`", show(&d.default)))
            .collect();
        let kinds: Vec<String> = declared.iter().map(|(_, d)| values(&d.kind)).collect();
        let kind = if kinds.iter().all(|k| *k == kinds[0]) {
            kinds[0].clone()
        } else {
            declared
                .iter()
                .zip(&kinds)
                .map(|((language, _), k)| format!("{language}: {k}"))
                .collect::<Vec<_>>()
                .join("; ")
        };
        out += &format!("| `{name}` | {kind} | {} |\n", defaults.join(", "));
    }
    out
}
