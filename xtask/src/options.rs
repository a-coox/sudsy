//! The option schema and presets of a language, from the `options` and `presets` keys of its
//! `lang.yaml`: `src/schema.generated.rs` holds one const per option name and `option_schema()`,
//! which the crate includes where its formatter lives (docs/architecture/adding-a-language.md).

use crate::lang_crate::Lang;
use anyhow::{Result, bail};
use serde_json::Value;

/// `shared::` constructors with no argument.
const SHARED_NO_ARG: [&str; 2] = ["indent_style", "end_of_line"];
/// Names `shared::NAMES` already has a const for.
const SHARED_NAMES: [&str; 8] = [
    "max_blank_lines",
    "line_width",
    "indent_size",
    "indent_style",
    "final_newline",
    "end_of_line",
    "bracket_spacing",
    "array_bracket_spacing",
];

fn lit(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s:?}"),
        other => other.to_string(),
    }
}

fn strs(v: &Value) -> String {
    let items = v.as_array().into_iter().flatten();
    format!("&[{}]", items.map(lit).collect::<Vec<_>>().join(", "))
}

/// One `OptionDef` expression and the option's name.
fn def(entry: &Value) -> Result<(String, String)> {
    if let Some(name) = entry.as_str() {
        if !SHARED_NO_ARG.contains(&name) {
            bail!("option `{name}` needs a value");
        }
        return Ok((name.into(), format!("shared::{name}()")));
    }
    let Some((name, body)) = entry.as_object().and_then(|m| m.iter().next()) else {
        bail!("an option is a name or a one-key map: {entry}");
    };
    let expr = match body {
        // A shared option with its default (and the description, for `max_blank_lines`).
        Value::Array(args) => format!(
            "shared::{name}({})",
            args.iter().map(lit).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(m) if !m.contains_key("doc") => bail!("option `{name}` has no `doc`"),
        Value::Object(m) => {
            let doc = lit(&m["doc"]);
            let default = m.get("default").map(lit).unwrap_or_default();
            if let Some(values) = m.get("values") {
                format!(
                    "OptionDef::choice({name:?}, {}, {default}, {doc})",
                    strs(values)
                )
            } else if let Some(r) = m.get("range").and_then(Value::as_array) {
                format!(
                    "OptionDef::int({name:?}, {default}, ({}, {}), {doc})",
                    lit(&r[0]),
                    lit(&r[1])
                )
            } else if m.get("kind").and_then(Value::as_str) == Some("words") {
                format!("OptionDef::words({name:?}, {doc})")
            } else if m.get("kind").and_then(Value::as_str) == Some("word-sets") {
                format!("OptionDef::word_sets({name:?}, {doc})")
            } else {
                format!("OptionDef::boolean({name:?}, {default}, {doc})")
            }
        }
        scalar => format!("shared::{name}({})", lit(scalar)),
    };
    let expr = match expr.starts_with("shared::") {
        true => expr,
        false => format!("core_config::{expr}"),
    };
    Ok((
        name.clone(),
        expr.replace("shared::", "core_config::shared::"),
    ))
}

/// `src/schema.generated.rs` of a language with an `options` key.
pub fn generate(name: &str, l: &Lang) -> Result<Option<String>> {
    let Some(options) = l.yaml.get("options").and_then(Value::as_array) else {
        return Ok(None);
    };
    let mut consts = String::new();
    let mut defs = String::new();
    for entry in options {
        let (opt, expr) = def(entry)?;
        if !SHARED_NAMES.contains(&opt.as_str()) {
            consts += &format!(
                "#[allow(dead_code)]\npub const {}: &str = {opt:?};\n",
                opt.to_uppercase()
            );
        }
        defs += &format!("        {expr},\n");
    }
    let presets = l.yaml.get("presets").and_then(Value::as_array);
    let presets = presets.into_iter().flatten().filter_map(Value::as_str);
    let presets: String = presets
        .map(|p| format!("        core_config::Preset::new(\"{name}/{p}\", include_str!(\"../presets/{p}.yaml\")),\n"))
        .collect();
    Ok(Some(format!(
        "// Generated from lang.yaml by `cargo xtask codegen`.\n{consts}\n#[rustfmt::skip]\npub fn option_schema() -> core_config::OptionSchema {{\n    core_config::OptionSchema::new(vec![\n{defs}    ])\n    .with_presets(vec![\n{presets}    ])\n}}\n"
    )))
}
