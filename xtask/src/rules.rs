//! One entry per rule id, gathered once: `docs/rules/rules.json` (what the documentation site and
//! the VS Code extension read) and the rule table of `docs/rules/builtin.md` are both made from it.

use core_rules::{Check, Extensions, Opt, RuleMeta};
use languages::catalog::{self, Upstream};
use provider_api::LanguageProvider;
use provider_api::fixtures::{self, Example};
use serde_json::{Value, json};

pub struct RuleDoc {
    pub meta: &'static RuleMeta,
    /// `all` for the shared rules.
    pub languages: Vec<String>,
    /// Presets that turn the rule on.
    pub presets: Vec<String>,
    pub upstream: Vec<Upstream>,
    /// The first case of the rule's fixtures that reports it (the one in the rule's own folder
    /// when there is one), with its language.
    pub example: Option<(String, Example)>,
}

/// Every rule, shared rules first, then by language in provider order, then by id.
pub fn collect(providers: &[Box<dyn LanguageProvider>]) -> Vec<RuleDoc> {
    let rules = languages::rules(providers, &Extensions::default());
    let mut presets = languages::rule_presets(providers);
    let mut examples: Vec<(String, Example)> = Vec::new();
    for p in providers {
        let language = p.language().to_string();
        examples.extend(
            fixtures::examples(p.as_ref())
                .into_iter()
                .map(|e| (language.clone(), e)),
        );
    }
    let mut docs: Vec<RuleDoc> = Vec::new();
    for (language, rule) in rules {
        let meta = rule.meta();
        if let Some(doc) = docs.last_mut().filter(|d| d.meta.id == meta.id) {
            doc.languages.push(language);
            continue;
        }
        let hits = || {
            examples
                .iter()
                .filter(|(_, e)| e.spans.iter().any(|s| s.3 == meta.id))
        };
        let example = hits()
            .find(|(_, e)| e.folder == meta.id)
            .or_else(|| hits().next())
            .cloned();
        docs.push(RuleDoc {
            meta,
            languages: vec![language],
            presets: presets.remove(meta.id).unwrap_or_default(),
            upstream: Vec::new(),
            example,
        });
    }
    for doc in &mut docs {
        let languages: Vec<&str> = doc.languages.iter().map(String::as_str).collect();
        doc.upstream = catalog::upstream(doc.meta.id, &languages);
    }
    let order: Vec<&str> = std::iter::once("all")
        .chain(providers.iter().map(|p| p.language()))
        .collect();
    docs.sort_by_key(|d| {
        let rank = order.iter().position(|l| *l == d.languages[0]);
        (rank.unwrap_or(usize::MAX), d.meta.id)
    });
    docs
}

fn default_json(default: Opt) -> Value {
    match default {
        Opt::Bool(b) => json!(b),
        Opt::Int(i) => json!(i),
        Opt::Str(s) => json!(s),
        Opt::List(l) => json!(l),
    }
}

/// The text of `docs/rules/rules.json`.
pub fn json(docs: &[RuleDoc]) -> String {
    let rules: Vec<Value> = docs.iter().map(entry).collect();
    serde_json::to_string_pretty(&json!({ "rules": rules })).expect("serializes") + "\n"
}

fn entry(d: &RuleDoc) -> Value {
    let m = d.meta;
    let options: Vec<Value> = m
        .options
        .iter()
        .map(|o| {
            let kind = match o.default {
                Opt::Bool(_) => "bool",
                Opt::Int(_) => "int",
                Opt::Str(_) => "string",
                Opt::List(_) => "list",
            };
            let mut v = json!({
                "name": o.name, "type": kind, "default": default_json(o.default),
                "description": o.description,
            });
            if let Check::OneOf(values) = o.check {
                v["choices"] = json!(values);
            }
            v
        })
        .collect();
    let mut v = json!({
        "id": m.id,
        "languages": d.languages,
        "category": m.category.as_str(),
        "default_level": m.default_level.as_str(),
        "fix": m.fix.map(|k| k.as_str()),
        "description": m.description,
        "options": options,
        "presets": d.presets,
        "upstream": d.upstream.iter().map(|u| json!({
            "tool": u.tool, "id": u.id, "partial": u.partial,
        })).collect::<Vec<_>>(),
    });
    if let Some((language, e)) = &d.example {
        let spans: Vec<Value> = e
            .spans
            .iter()
            .filter(|s| s.3 == m.id)
            .map(|s| json!({ "line": s.0, "column": s.1, "length": s.2 }))
            .collect();
        v["example"] = json!({
            "language": language, "input": e.text, "spans": spans,
            "fixed": e.fixed, "config": e.config,
        });
    }
    v
}

/// A table cell: one line, no pipes.
fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// The rule table of `docs/rules/builtin.md`.
pub fn table(docs: &[RuleDoc]) -> String {
    let mut out = String::from(
        "| Rule | Language | Category | Default | Fix | Description |\n|---|---|---|---|---|---|\n",
    );
    for d in docs {
        let m = d.meta;
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
        if !d.upstream.is_empty() {
            let upstream: Vec<String> = d.upstream.iter().map(|u| u.to_string()).collect();
            description += &format!(" Equivalent: {}.", upstream.join(", "));
        }
        out += &format!(
            "| `{}` | {} | {} | {} | {} | {description} |\n",
            m.id,
            d.languages.join(", "),
            m.category.as_str(),
            m.default_level.as_str(),
            m.fix.map_or("none", |k| k.as_str()),
        );
    }
    out
}
