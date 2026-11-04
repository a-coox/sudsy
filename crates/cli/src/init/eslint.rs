//! eslint-plugin-jsonc, from a legacy JSON or YAML ESLint config (`.eslintrc.*`, `package.json`
//! "eslintConfig"). Flat configs (`eslint.config.*`) are JavaScript and are not read.

use super::out::Out;
use anyhow::{Result, bail};
use serde_json::Value;

pub const FILES: [&str; 4] = [
    ".eslintrc.json",
    ".eslintrc.yaml",
    ".eslintrc.yml",
    ".eslintrc",
];
/// JavaScript configs; they cannot be read without running them.
pub const JS_CONFIGS: [&str; 10] = [
    "eslint.config.js",
    "eslint.config.mjs",
    "eslint.config.cjs",
    "eslint.config.ts",
    "eslint.config.mts",
    "eslint.config.cts",
    ".eslintrc.js",
    ".eslintrc.cjs",
    ".eslintrc.config.js",
    ".eslintrc.mjs",
];
const J: &[&str] = &["json"];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object");
    };
    if let Some(rules) = root.get("rules").and_then(Value::as_object) {
        rules_of(rules, out);
    }
    // Overrides for JSON files carry the jsonc rules in most setups.
    for entry in root
        .get("overrides")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let files = match entry.get("files") {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if files.iter().any(|f| f.contains(".json"))
            && let Some(rules) = entry.get("rules").and_then(Value::as_object)
        {
            rules_of(rules, out);
        }
    }
    Ok(())
}

/// `(level, options)` of an ESLint rule setting: `"error"`, `2`, `["warn", {...}]`.
fn setting(value: &Value) -> Option<(&'static str, &[Value])> {
    let (sev, opts) = match value {
        Value::Array(a) => (a.first()?, a.get(1..).unwrap_or_default()),
        v => (v, [].as_slice()),
    };
    let level = match sev {
        Value::String(s) => match s.as_str() {
            "off" => "off",
            "warn" => "warn",
            "error" => "error",
            _ => return None,
        },
        Value::Number(n) => match n.as_u64()? {
            0 => "off",
            1 => "warn",
            2 => "error",
            _ => return None,
        },
        _ => return None,
    };
    Some((level, opts))
}

fn rules_of(rules: &serde_json::Map<String, Value>, out: &mut Out) {
    for (name, value) in rules {
        let Some(rule) = name.strip_prefix("jsonc/") else {
            continue;
        };
        let Some((level, opts)) = setting(value) else {
            continue;
        };
        let Some(entry) = super::catalog::find("eslint-plugin-jsonc", name) else {
            out.skip_why(name, "not an eslint-plugin-jsonc rule Sudsy knows");
            continue;
        };
        let status = entry.sudsy.as_str();
        if let Some(sudsy) = status.strip_prefix("rule: ") {
            if sudsy == "syntax-error" {
                // Always reported by the parser; only turning it off cannot be followed.
                if level == "off" {
                    out.skip_why(name, "syntax errors are always reported");
                }
                continue;
            }
            out.rule(name, "json", sudsy, level);
            if level != "off" {
                rule_options(name, sudsy, opts, out);
            }
        } else if level != "off" {
            if status.starts_with("option: ") {
                format_rule(rule, name, opts, out);
            } else if status != "formatter" {
                out.skip(name);
            }
        }
    }
}

fn rule_options(name: &str, sudsy: &str, opts: &[Value], out: &mut Out) {
    let Some(first) = opts.first() else { return };
    // `sorted-keys` and `sorted-array-values` take an array of option objects in ESLint.
    let object = first.as_object();
    match (sudsy, object) {
        ("key-name-casing", Some(o)) => {
            let styles = [
                "camelCase",
                "PascalCase",
                "snake_case",
                "kebab-case",
                "SCREAMING_SNAKE_CASE",
            ];
            let allowed: Vec<Value> = styles
                .iter()
                .filter(|s| {
                    o.get(**s)
                        .and_then(Value::as_bool)
                        .unwrap_or(**s == "camelCase")
                })
                .map(|s| Value::from(*s))
                .collect();
            out.rule_opt(name, "json", sudsy, "allowed", allowed.into());
            if let Some(ignores) = o.get("ignores") {
                out.rule_opt(name, "json", sudsy, "ignored_keys", ignores.clone());
            }
        }
        ("sorted-keys" | "sorted-array-values", _) => {
            // Either `"asc", { minKeys }` or `{ pathPattern, order, minKeys }, ...`.
            let (order, object) = match first {
                Value::String(_) => (Some(first.clone()), opts.get(1).and_then(Value::as_object)),
                _ => (object.and_then(|o| o.get("order").cloned()), object),
            };
            let scoped = first.is_object() && opts.len() > 1
                || object.is_some_and(|o| {
                    o.contains_key("pathPattern") || o.contains_key("hasProperties")
                });
            if scoped {
                out.skip_why(
                    &format!("{name} options"),
                    "per-path option sets are not supported; set the options by hand",
                );
                return;
            }
            sort_options(name, sudsy, object, order.as_ref(), out);
        }
        _ => {}
    }
}

fn sort_options(
    name: &str,
    sudsy: &str,
    object: Option<&serde_json::Map<String, Value>>,
    order: Option<&Value>,
    out: &mut Out,
) {
    let ty = match order {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Object(o)) => o.get("type").and_then(Value::as_str),
        _ => None,
    };
    if let Some(dir @ ("asc" | "desc")) = ty {
        out.rule_opt(name, "json", sudsy, "direction", dir.into());
    }
    if let Some(Value::Object(o)) = order {
        let natural = o.get("natural").and_then(Value::as_bool) == Some(true);
        let insensitive = o.get("caseSensitive").and_then(Value::as_bool) == Some(false);
        let compare = if natural {
            "natural"
        } else if insensitive {
            "case_insensitive"
        } else {
            ""
        };
        if !compare.is_empty() {
            out.rule_opt(name, "json", sudsy, "compare", compare.into());
        }
    }
    if let Some(o) = object {
        let min = if sudsy == "sorted-keys" {
            "minKeys"
        } else {
            "minValues"
        };
        if let Some(n) = o.get(min) {
            let opt = if sudsy == "sorted-keys" {
                "min_keys"
            } else {
                "min_values"
            };
            out.rule_opt(name, "json", sudsy, opt, n.clone());
        }
        if let Some(b) = o.get("allowLineSeparatedGroups") {
            out.rule_opt(name, "json", sudsy, "blank_line_groups", b.clone());
        }
    }
}

fn format_rule(rule: &str, id: &str, opts: &[Value], out: &mut Out) {
    let first = opts.first().and_then(Value::as_str);
    match rule {
        "array-bracket-spacing" => out.opt(id, J, "array_bracket_spacing", first == Some("always")),
        "object-curly-spacing" => out.opt(id, J, "bracket_spacing", first == Some("always")),
        "comma-dangle" => {
            let never = first == Some("never")
                || opts
                    .first()
                    .and_then(Value::as_object)
                    .is_some_and(|o| o.values().all(|v| v == "never"));
            out.opt(
                id,
                J,
                "trailing_commas",
                if never { "never" } else { "multiline" },
            );
        }
        "indent" => match opts.first() {
            Some(Value::String(s)) if s == "tab" => out.opt_free(J, "indent_style", "tab"),
            Some(Value::Number(n)) if n.is_u64() => out.opt(id, J, "indent_size", n.clone()),
            _ => {}
        },
        _ => out.skip(id),
    }
}
