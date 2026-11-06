//! The reference tables of the docs, generated from the registered rules, the option schemas and
//! the parity catalog. A generated block sits between `<!-- generated:NAME -->` and
//! `<!-- /generated -->`; the prose around it is hand-written.

use anyhow::{Context, Result, bail};
use core_config::{OptionKind, OptionSchema, OptionValue};
use provider_api::LanguageProvider;
use std::collections::BTreeMap;
use std::path::PathBuf;

const END: &str = "<!-- /generated -->";

/// Every doc with generated blocks as (path relative to the repo root, contents).
pub fn generate(providers: &[Box<dyn LanguageProvider>]) -> Result<Vec<(PathBuf, String)>> {
    let rule_docs = crate::rules::collect(providers);
    let schemas = languages::schemas(providers);
    let mut out = vec![
        splice(
            "docs/rules/builtin.md",
            &[("rules", crate::rules::table(&rule_docs))],
        )?,
        (
            "docs/rules/rules.json".into(),
            crate::rules::json(&rule_docs),
        ),
    ];
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
    let mut titles: Vec<(&str, &str)> = providers
        .iter()
        .map(|p| (p.grammar().title.as_str(), p.language()))
        .collect();
    titles.sort();
    let (last, rest) = titles.split_last().expect("a language");
    let names = format!(
        "{} and {}",
        rest.iter().map(|t| t.0).collect::<Vec<_>>().join(", "),
        last.0
    );
    let crates = titles
        .iter()
        .map(|t| format!("`lang-{}`", t.1))
        .collect::<Vec<_>>()
        .join(", ");
    for rel in [
        "README.md",
        "editors/vscode/README.md",
        "docs/editor/vscode.md",
    ] {
        out.push(splice(rel, &[("languages", names.clone())])?);
    }
    out.push(splice(
        "docs/architecture/overview.md",
        &[("crates", crates)],
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
        // A marker followed by text on its line is an inline block: the body replaces the text.
        let newline = if text[from..].starts_with('\n') {
            "\n"
        } else {
            ""
        };
        text.replace_range(from..from + len, &format!("{newline}{body}"));
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
