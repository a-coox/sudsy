//! Prettier (`.prettierrc*`, `package.json` "prettier"): the options that apply to JSON and YAML.

use super::out::Out;
use anyhow::{Result, bail};
use serde_json::Value;

pub const FILES: [&str; 4] = [
    ".prettierrc",
    ".prettierrc.json",
    ".prettierrc.yaml",
    ".prettierrc.yml",
];
/// Config files in formats `init` does not read.
pub const UNSUPPORTED: [&str; 11] = [
    ".prettierrc.json5",
    ".prettierrc.toml",
    ".prettierrc.js",
    ".prettierrc.cjs",
    ".prettierrc.mjs",
    ".prettierrc.ts",
    "prettier.config.js",
    "prettier.config.cjs",
    "prettier.config.mjs",
    "prettier.config.ts",
    ".prettierrc.mts",
];
const BOTH: &[&str] = &["json", "yaml"];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object (a shared config name cannot be followed)");
    };
    out.extend("json/prettier-compat");
    out.extend("yaml/prettier-compat");
    for (key, value) in root {
        match (key.as_str(), value) {
            ("printWidth", v) if v.is_u64() => out.opt("printWidth", BOTH, "line_width", v.clone()),
            ("tabWidth", v) if v.is_u64() => out.opt("tabWidth", BOTH, "indent_size", v.clone()),
            ("useTabs", Value::Bool(true)) => out.opt("useTabs", &["json"], "indent_style", "tab"),
            ("useTabs", Value::Bool(false)) => {}
            ("endOfLine", Value::String(s)) if matches!(s.as_str(), "lf" | "crlf" | "auto") => {
                out.opt("endOfLine", BOTH, "end_of_line", s.clone());
            }
            ("endOfLine", _) => {
                out.skip_why("endOfLine", "`cr` is not supported; use lf, crlf or auto")
            }
            ("bracketSpacing", Value::Bool(b)) => {
                out.opt("bracketSpacing", BOTH, "bracket_spacing", *b)
            }
            ("singleQuote", Value::Bool(b)) => {
                let style = if *b { "single" } else { "prefer_double" };
                out.opt("singleQuote (yaml)", &["yaml"], "quote_style", style);
            }
            ("trailingComma", Value::String(s)) if s == "none" => {}
            ("trailingComma", _) => out.skip("trailingComma"),
            ("objectWrap", Value::String(s)) if s == "preserve" => {}
            ("objectWrap", Value::String(s)) if s == "collapse" => {
                out.opt("objectWrap", &["json"], "collapse", "always");
            }
            ("proseWrap", Value::String(s)) if s == "preserve" => {}
            ("proseWrap", _) => out.skip("proseWrap"),
            ("overrides", _) => out.skip("overrides / .prettierignore / .editorconfig"),
            ("quoteProps", _) => out.skip("quoteProps"),
            ("embeddedLanguageFormatting", _) => out.skip("embeddedLanguageFormatting"),
            ("insertPragma" | "requirePragma" | "checkIgnorePragma", _) => {
                out.skip("insertPragma / requirePragma / checkIgnorePragma");
            }
            ("rangeStart" | "rangeEnd", _) => out.skip("rangeStart / rangeEnd"),
            (
                "$schema"
                | "semi"
                | "jsxSingleQuote"
                | "arrowParens"
                | "bracketSameLine"
                | "singleAttributePerLine"
                | "htmlWhitespaceSensitivity"
                | "vueIndentScriptAndStyle"
                | "experimentalTernaries"
                | "experimentalOperatorPosition",
                _,
            ) => {}
            (other, _) => out.skip_why(other, "not a Prettier option Sudsy maps"),
        }
    }
    Ok(())
}
