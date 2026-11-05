//! `sudsy rules [<term>] [--language <lang>]`: list or search the rules, or explain one.

use anyhow::{Result, bail};
use clap::Args;
use core_config::suggest;
use core_rules::{Rule, RuleMeta};
use languages::catalog;
use std::fmt::Write;

type Rules = Vec<(String, &'static dyn Rule)>;

#[derive(Args)]
pub struct RulesArgs {
    /// A rule id (explains it), an id of another tool such as `SA1005` or `truthy` (explains the
    /// rule that covers it), or text to search for in ids, descriptions and the ids of other
    /// tools. Lists every rule when omitted.
    term: Option<String>,
    /// Only the rules that apply to this language (shared rules included).
    #[arg(long)]
    language: Option<String>,
}

fn fix_text(meta: &RuleMeta) -> &'static str {
    meta.fix.map_or("none", |k| k.as_str())
}

pub fn run(args: RulesArgs) -> Result<()> {
    let providers = languages::providers();
    let project = languages::Project::load(&std::env::current_dir()?, &providers)?;
    let mut rules = languages::rules(&providers, &project.extensions);
    if let Some(language) = &args.language {
        rules.retain(|(l, _)| l == language || l == "all");
        if rules.is_empty() {
            bail!("no rules for language `{language}`");
        }
    }
    let presets = languages::rule_presets(&providers);
    let mut out = String::new();
    match args.term {
        None => list(&mut out, &rules),
        Some(term) => {
            let ids = resolve(&rules, &term);
            if !ids.is_empty() {
                for id in ids {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    explain(
                        &mut out,
                        &rules,
                        id,
                        presets.get(id).map_or(&[], Vec::as_slice),
                    );
                }
            } else {
                let found: Rules = rules
                    .iter()
                    .filter(|r| matches(r, &term))
                    .cloned()
                    .collect();
                if found.is_empty() {
                    let hint = suggest(&term, rules.iter().map(|(_, r)| r.meta().id))
                        .map(|s| format!("; did you mean `{s}`?"))
                        .unwrap_or_default();
                    bail!("no rule matches `{term}`{hint}");
                }
                list(&mut out, &found);
            }
        }
    }
    print!("{out}");
    Ok(())
}

/// The rule with this id, else the rules that cover an upstream setting with this id.
fn resolve(rules: &Rules, term: &str) -> Vec<&'static str> {
    let exact = rules.iter().find(|(_, r)| r.meta().id == term);
    if let Some((_, r)) = exact {
        return vec![r.meta().id];
    }
    let mut ids = catalog::rules_for(term);
    ids.retain(|id| rules.iter().any(|(_, r)| r.meta().id == *id));
    ids
}

/// The term is in the id, the description or an upstream id, ignoring case.
fn matches((language, rule): &(String, &'static dyn Rule), term: &str) -> bool {
    let term = term.to_lowercase();
    let m = rule.meta();
    m.id.contains(&term)
        || m.description.to_lowercase().contains(&term)
        || catalog::upstream(m.id, &[language])
            .iter()
            .any(|u| u.id.to_lowercase().contains(&term))
}

fn list(out: &mut String, rules: &Rules) {
    let width = rules
        .iter()
        .map(|(_, r)| r.meta().id.len())
        .max()
        .unwrap_or(0);
    for (language, rule) in rules {
        let m = rule.meta();
        let _ = writeln!(
            out,
            "{:width$}  {:8}  {:11}  {:5}  fix: {:10}  {}",
            m.id,
            language,
            m.category.as_str(),
            m.default_level.as_str(),
            fix_text(m),
            m.description.split(". ").next().unwrap_or(m.description),
        );
    }
}

fn explain(out: &mut String, rules: &Rules, id: &str, presets: &[String]) {
    let matching: Vec<_> = rules.iter().filter(|(_, r)| r.meta().id == id).collect();
    let m = matching[0].1.meta();
    let languages: Vec<&str> = matching.iter().map(|(l, _)| l.as_str()).collect();
    let _ = writeln!(out, "{}", m.id);
    let _ = writeln!(out, "  languages:     {}", languages.join(", "));
    let _ = writeln!(out, "  category:      {}", m.category.as_str());
    let _ = writeln!(out, "  default level: {}", m.default_level.as_str());
    let _ = writeln!(out, "  fix:           {}", fix_text(m));
    if !presets.is_empty() {
        let _ = writeln!(out, "  presets:       {}", presets.join(", "));
    }
    let equivalent: Vec<String> = catalog::upstream(m.id, &languages)
        .iter()
        .map(|u| u.to_string())
        .collect();
    if !equivalent.is_empty() {
        let _ = writeln!(out, "  equivalent:    {}", equivalent.join(", "));
    }
    let _ = writeln!(out, "\n{}", m.description);
    if !m.options.is_empty() {
        let _ = writeln!(out, "\nOptions:");
        for o in m.options {
            let _ = writeln!(
                out,
                "  {} ({}, default {}): {}",
                o.name,
                o.default.type_name(),
                o.default.display(),
                o.description
            );
        }
    }
    let _ = writeln!(out, "\nExample:");
    for line in m.example.lines() {
        let _ = writeln!(out, "  {line}");
    }
}
