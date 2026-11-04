//! Biome (`biome.json`, `biome.jsonc`): the JSON formatter and linter.

use super::out::Out;
use anyhow::{Result, bail};
use serde_json::{Map, Value};

pub const FILES: [&str; 2] = ["biome.json", "biome.jsonc"];
const J: &[&str] = &["json"];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object");
    };
    out.extend("json/biome");
    let section = |name: &str| {
        root.get(name)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    let json = section("json");
    let global = section("formatter");
    let local = json
        .get("formatter")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    // json.formatter overrides formatter.
    let mut fmt = global;
    fmt.extend(local);
    formatter(&fmt, out);
    if let Some(parser) = json.get("parser").and_then(Value::as_object) {
        let on = |k: &str| parser.get(k) == Some(&Value::Bool(true));
        if on("allowComments") || on("allowTrailingCommas") {
            let id = if on("allowComments") {
                "json.parser.allowComments"
            } else {
                "json.parser.allowTrailingCommas"
            };
            out.opt(id, J, "dialect", "jsonc");
        }
    }
    if json.get("linter").and_then(|l| l.get("enabled")) == Some(&Value::Bool(false)) {
        out.skip("json.linter.enabled");
    }
    rules(config, out);
    if root.contains_key("files") {
        out.skip("files.includes");
    }
    for key in ["overrides", "extends", "vcs"] {
        if root.contains_key(key) {
            out.skip_why(
                key,
                "use `include`/`exclude` globs and `overrides` in sudsy.yaml",
            );
        }
    }
    Ok(())
}

fn formatter(fmt: &Map<String, Value>, out: &mut Out) {
    for (key, value) in fmt {
        let id = format!("json.formatter.{key}");
        match (key.as_str(), value) {
            ("indentStyle", Value::String(s)) if s == "tab" || s == "space" => {
                out.opt(&id, J, "indent_style", s.clone())
            }
            ("indentWidth", v) if v.is_u64() => out.opt(&id, J, "indent_size", v.clone()),
            ("lineWidth", v) if v.is_u64() => out.opt(&id, J, "line_width", v.clone()),
            ("lineEnding", Value::String(s)) if s == "lf" || s == "crlf" => {
                out.opt(&id, J, "end_of_line", s.clone())
            }
            ("lineEnding", _) => {
                out.skip_why("lineEnding", "`cr` is not supported; use lf or crlf")
            }
            ("trailingCommas", Value::String(s)) => match s.as_str() {
                "none" => out.opt(&id, J, "trailing_commas", "never"),
                _ => out.opt(&id, J, "trailing_commas", "multiline"),
            },
            ("bracketSpacing", Value::Bool(b)) => out.opt(&id, J, "bracket_spacing", *b),
            ("delimiterSpacing", Value::Bool(b)) => out.opt(&id, J, "array_bracket_spacing", *b),
            ("expand", Value::String(s)) => match s.as_str() {
                "auto" => out.opt(&id, J, "collapse", "opening_break"),
                "always" => out.opt(&id, J, "collapse", "never"),
                "never" => out.opt(&id, J, "collapse", "always"),
                _ => out.skip(&id),
            },
            ("trailingNewline", Value::Bool(b)) => out.opt(&id, J, "final_newline", *b),
            ("enabled", Value::Bool(true))
            | ("useEditorconfig", _)
            | ("formatWithErrors", Value::Bool(false)) => {}
            ("includes", _) => out.skip("formatter.includes"),
            ("enabled", _) => out.skip(&id),
            ("formatWithErrors", Value::Bool(true)) => out.skip("formatter.formatWithErrors"),
            ("attributePosition", _) => out.skip("formatter.attributePosition"),
            (other, _) => out.skip_why(
                &format!("formatter.{other}"),
                "not a Biome JSON formatter option Sudsy maps",
            ),
        }
    }
}

/// Biome rule settings: a severity, or `{ level, options }`.
fn severity(value: &Value) -> Option<&'static str> {
    let level = value.get("level").unwrap_or(value).as_str()?;
    match level {
        "off" => Some("off"),
        "error" => Some("error"),
        "warn" | "info" | "on" => Some("warn"),
        _ => None,
    }
}

fn rules(root: &Value, out: &mut Out) {
    let groups = root.pointer("/linter/rules").and_then(Value::as_object);
    for (group, rules) in groups
        .into_iter()
        .flatten()
        .filter_map(|(g, r)| Some((g, r.as_object()?)))
    {
        for (name, value) in rules {
            let id = format!("{group}/{name}");
            let Some(entry) = super::catalog::find("biome", &id) else {
                continue;
            };
            let Some(level) = severity(value) else {
                continue;
            };
            if let Some(rule) = entry.sudsy.strip_prefix("rule: ") {
                out.rule(&id, "json", rule, level);
            } else if level != "off" {
                out.skip(&id);
            }
        }
    }
    let sorted = root.pointer("/assist/actions/source/useSortedKeys");
    if let Some(level) = sorted.and_then(severity) {
        let on = if level == "off" { "off" } else { "warn" };
        out.rule("assist/source/useSortedKeys", "json", "sorted-keys", on);
    }
}
