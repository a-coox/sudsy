//! `sudsy rules [<id>]`: list the rules, or explain one.

use anyhow::{Result, bail};
use clap::Args;
use core_config::suggest;
use core_rules::{Rule, RuleMeta};
use std::fmt::Write;

#[derive(Args)]
pub struct RulesArgs {
    /// Explain this rule; list all rules when omitted.
    id: Option<String>,
}

fn fix_text(meta: &RuleMeta) -> &'static str {
    meta.fix.map_or("none", |k| k.as_str())
}

pub fn run(args: RulesArgs) -> Result<()> {
    let providers = languages::providers();
    let project = languages::Project::load(&std::env::current_dir()?, &providers)?;
    let rules = languages::rules(&providers, &project.extensions);
    let mut out = String::new();
    match args.id {
        None => list(&mut out, &rules),
        Some(id) => explain(&mut out, &rules, &id)?,
    }
    print!("{out}");
    Ok(())
}

fn list(out: &mut String, rules: &[(String, &'static dyn Rule)]) {
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

fn explain(out: &mut String, rules: &[(String, &'static dyn Rule)], id: &str) -> Result<()> {
    let matching: Vec<_> = rules.iter().filter(|(_, r)| r.meta().id == id).collect();
    let Some((_, first)) = matching.first() else {
        let hint = suggest(id, rules.iter().map(|(_, r)| r.meta().id))
            .map(|s| format!("; did you mean `{s}`?"))
            .unwrap_or_default();
        bail!("unknown rule `{id}`{hint}");
    };
    let m = first.meta();
    let languages: Vec<&str> = matching.iter().map(|(l, _)| l.as_str()).collect();
    let _ = writeln!(out, "{}", m.id);
    let _ = writeln!(out, "  languages:     {}", languages.join(", "));
    let _ = writeln!(out, "  category:      {}", m.category.as_str());
    let _ = writeln!(out, "  default level: {}", m.default_level.as_str());
    let _ = writeln!(out, "  fix:           {}", fix_text(m));
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
    Ok(())
}
