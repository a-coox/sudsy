//! StyleCop.Analyzers: `stylecop.json` settings and `dotnet_diagnostic.SA*.severity` in
//! `.editorconfig`.

use super::{catalog, editorconfig, out::Out};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const FILES: [&str; 2] = ["stylecop.json", ".stylecop.json"];
const CS: &[&str] = &["csharp"];

pub fn migrate(settings: Option<&Value>, out: &mut Out) -> Result<()> {
    out.extend("csharp/stylecop");
    if let Some(settings) = settings {
        for (group, body) in settings
            .get("settings")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            match body.as_object() {
                Some(body) => body
                    .iter()
                    .for_each(|(key, value)| setting(group, key, value, out)),
                None => out.skip_why(&format!("settings.{group}"), "not an object"),
            }
        }
    }
    severities(out)?;
    out.editorconfig = editorconfig::suppressions(true);
    Ok(())
}

fn setting(group: &str, key: &str, value: &Value, out: &mut Out) {
    match (group, key, value) {
        ("indentation", "useTabs", Value::Bool(true)) => out.opt_free(CS, "indent_style", "tab"),
        ("indentation", "useTabs", Value::Bool(false)) => out.opt_free(CS, "indent_style", "space"),
        ("indentation", "indentationSize", v) if v.is_u64() => {
            out.opt_free(CS, "indent_size", v.clone())
        }
        // The tab width is the indent size.
        ("indentation", "tabSize", _) => {}
        ("layoutRules", "newlineAtEndOfFile", Value::String(s)) => match s.as_str() {
            "require" => out.opt_free(CS, "final_newline", true),
            "omit" => out.opt_free(CS, "final_newline", false),
            _ => {}
        },
        ("layoutRules", "allowDoWhileOnClosingBrace", Value::Bool(true)) => {
            out.opt_free(CS, "do_while_line", "same_line");
        }
        ("layoutRules", "allowDoWhileOnClosingBrace", _) => {}
        ("orderingRules", "systemUsingDirectivesFirst", Value::Bool(false)) => {
            out.rule_opt_free("csharp", "using-order", "system_first", json!(false));
        }
        ("orderingRules", "systemUsingDirectivesFirst", _) => {}
        ("orderingRules", "elementOrder", Value::Array(_)) => {
            out.rule_opt_free("csharp", "member-order", "element_order", value.clone());
        }
        ("documentationRules", _, _) => out.skip_why(
            &format!("settings.documentationRules.{key}"),
            "the documentation rules (SA16xx) are deferred",
        ),
        ("$schema", ..) => {}
        _ => out.skip_why(
            &format!("settings.{group}.{key}"),
            "no Sudsy option for this setting",
        ),
    }
}

/// `dotnet_diagnostic.SA*.severity` entries: a rule Sudsy has takes the level, others are listed.
fn severities(out: &mut Out) -> Result<()> {
    let mut levels: BTreeMap<String, (u8, String)> = BTreeMap::new();
    for (id, severity) in editorconfig::sa_severities()? {
        let level = match severity.as_str() {
            "none" | "silent" => ("off", 0),
            "suggestion" | "warning" => ("warn", 1),
            "error" => ("error", 2),
            _ => continue,
        };
        let key = format!("dotnet_diagnostic.{id}.severity");
        let Some(entry) = catalog::find("stylecop", &id) else {
            out.skip_why(&key, "not a StyleCop.Analyzers rule");
            continue;
        };
        if let Some(rule) = entry.sudsy.strip_prefix("rule: ") {
            let best = levels.entry(rule.to_string()).or_insert((0, id.clone()));
            if level.1 >= best.0 {
                *best = (level.1, id.clone());
            }
        } else if level.1 > 0 && entry.coverage != "covered" {
            out.skip_why(&key, &catalog::reason("stylecop", &id));
        }
    }
    for (rule, (rank, id)) in levels {
        let level = ["off", "warn", "error"][usize::from(rank)];
        out.rule(&id, "csharp", &rule, level);
    }
    Ok(())
}
