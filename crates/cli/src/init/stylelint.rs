//! stylelint (`.stylelintrc*`, `package.json` "stylelint", `.stylelintignore`), with the rules of
//! the `scss/` and `less/` plugins. JavaScript configs are not read.

use super::out::Out;
use anyhow::{Result, bail};
use serde_json::{Map, Value};

pub const FILES: [&str; 4] = [
    ".stylelintrc",
    ".stylelintrc.json",
    ".stylelintrc.yaml",
    ".stylelintrc.yml",
];
/// JavaScript configs; they cannot be read without running them.
pub const JS_CONFIGS: [&str; 9] = [
    ".stylelintrc.js",
    ".stylelintrc.cjs",
    ".stylelintrc.mjs",
    "stylelint.config.js",
    "stylelint.config.cjs",
    "stylelint.config.mjs",
    "stylelint.config.ts",
    "stylelint.config.mts",
    "stylelint.config.cts",
];
pub const IGNORE_FILE: &str = ".stylelintignore";
/// Secondary options that only change the report.
const REPORT_ONLY: [&str; 4] = ["severity", "message", "url", "reportDisables"];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object");
    };
    let warn = root.get("defaultSeverity").and_then(Value::as_str) == Some("warning");
    for name in strings(root.get("extends")) {
        extend(name, out);
    }
    for pattern in strings(root.get("ignoreFiles")) {
        exclude(pattern, out);
    }
    if let Some(rules) = root.get("rules").and_then(Value::as_object) {
        for (name, value) in rules {
            rule(name, value, warn, out);
        }
    }
    if root.contains_key("overrides") {
        out.skip_why(
            "overrides",
            "per-file overrides are not supported; set the rules by hand",
        );
    }
    Ok(())
}

/// A string or an array of strings.
fn strings(value: Option<&Value>) -> Vec<&str> {
    match value {
        Some(Value::String(s)) => vec![s],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}

/// Lines of `.stylelintignore` (gitignore syntax).
pub fn ignore_file(text: &str, out: &mut Out) {
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        exclude(line, out);
    }
}

fn exclude(pattern: &str, out: &mut Out) {
    if pattern.starts_with('!') {
        out.skip_why(pattern, "negated ignore patterns are not supported");
    } else {
        out.exclude(pattern);
    }
}

/// `stylelint-config-recommended` and `-standard` (also the `-scss` and `-less` variants, whose
/// plugin rules come only from the `rules` of the config).
fn extend(name: &str, out: &mut Out) {
    let kind = name
        .strip_prefix("stylelint-config-")
        .and_then(|n| n.split('-').next())
        .filter(|k| matches!(*k, "recommended" | "standard"));
    let Some(kind) = kind else {
        out.skip_why(
            name,
            "a shared config is not followed; its rules are not migrated",
        );
        return;
    };
    out.extend(&format!("css/stylelint-{kind}"));
    if name.ends_with("-scss") || name.ends_with("-less") {
        out.note(format!(
            "{name}: only the core rules of the preset are included"
        ));
    }
}

fn rule(name: &str, value: &Value, warn: bool, out: &mut Out) {
    let (primary, secondary) = match value {
        Value::Array(a) => (a.first(), a.get(1).and_then(Value::as_object)),
        v => (Some(v), None),
    };
    let severity = secondary
        .and_then(|s| s.get("severity"))
        .and_then(Value::as_str);
    let level = if matches!(primary, None | Some(Value::Null)) {
        "off"
    } else if severity == Some("warning") || severity.is_none() && warn {
        "warn"
    } else {
        "error"
    };
    let Some(entry) = out.entry(name, "not a stylelint rule Sudsy knows") else {
        return;
    };
    let lang = name.split_once('/').map_or("css", |(plugin, _)| plugin);
    if let Some(sudsy) = entry.sudsy.strip_prefix("rule: ") {
        out.rule(name, lang, sudsy, level);
        if level != "off" {
            options(name, sudsy, lang, primary, secondary, out);
        }
    } else if level != "off" {
        out.skip(name);
    }
}

/// The one string primary that a Sudsy rule without options implements.
fn fixed(sudsy: &str) -> Option<&'static str> {
    Some(match sudsy {
        "function-name-case" | "selector-type-case" => "lower",
        "function-url-quotes" | "selector-attribute-quotes" => "always",
        "import-notation" => "string",
        "selector-pseudo-element-colon-notation" => "double",
        "keyframe-selector-notation" => "percentage",
        "font-family-name-quotes" => "always-where-required",
        _ => return None,
    })
}

/// The options of a rule that Sudsy's rule of the same name has: a list, a pattern, a maximum,
/// the hex style, the one string a rule without options implements. Anything else the config sets is listed.
fn options(
    name: &str,
    sudsy: &str,
    lang: &str,
    primary: Option<&Value>,
    secondary: Option<&Map<String, Value>>,
    out: &mut Out,
) {
    let mut set = |opt: &str, v: Value| out.rule_opt(name, lang, sudsy, opt, v);
    let mut unmapped = false;
    match primary {
        Some(v @ Value::Array(_)) if sudsy.ends_with("-list") => set("list", v.clone()),
        Some(v @ Value::Number(_)) if sudsy.contains("max") => set("max", v.clone()),
        Some(Value::String(s)) if sudsy.ends_with("-pattern") => {
            let regex = s
                .strip_prefix('/')
                .and_then(|r| r.strip_suffix('/'))
                .unwrap_or(s);
            set("pattern", regex.into());
        }
        Some(Value::String(s)) if sudsy == "color-hex-length" => set("style", s.as_str().into()),
        Some(Value::String(s)) if fixed(sudsy) == Some(s.as_str()) => {}
        Some(Value::Bool(_)) | None => {}
        Some(_) => unmapped = true,
    }
    for (key, v) in secondary.into_iter().flatten() {
        let ignore = v.as_array().into_iter().flatten().next();
        if key == "ignore"
            && sudsy == "declaration-block-no-duplicate-properties"
            && ignore.and_then(Value::as_str)
                == Some("consecutive-duplicates-with-different-values")
        {
            set("ignore_consecutive_different_values", true.into());
        } else if !REPORT_ONLY.contains(&key.as_str()) {
            unmapped = true;
        }
    }
    if unmapped {
        out.skip_why(
            &format!("{name} options"),
            "Sudsy's rule has no option for them; set them by hand",
        );
    }
}
