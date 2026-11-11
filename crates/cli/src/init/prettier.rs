//! Prettier (`.prettierrc*`, `package.json` "prettier"): the options that apply to JSON, YAML, JavaScript/TypeScript and CSS/SCSS/Less.

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
/// Languages with `line_width`, `indent_size` and `end_of_line`.
const ALL: &[&str] = &["json", "yaml", "js", "css"];
const JS: &[&str] = &["js"];
const QUOTE_PROPS: [&str; 3] = ["as-needed", "consistent", "preserve"];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object (a shared config name cannot be followed)");
    };
    out.extend("json/prettier-compat");
    out.extend("yaml/prettier-compat");
    for (key, value) in root {
        match (key.as_str(), value) {
            ("printWidth", v) if v.is_u64() => out.opt("printWidth", ALL, "line_width", v.clone()),
            ("tabWidth", v) if v.is_u64() => out.opt("tabWidth", ALL, "indent_size", v.clone()),
            ("useTabs", Value::Bool(true)) => {
                out.opt("useTabs", &["json", "js", "css"], "indent_style", "tab");
            }
            ("useTabs", Value::Bool(false)) => {}
            ("endOfLine", Value::String(s)) if matches!(s.as_str(), "lf" | "crlf" | "auto") => {
                out.opt("endOfLine", ALL, "end_of_line", s.clone());
            }
            ("endOfLine", _) => {
                out.skip_why("endOfLine", "`cr` is not supported; use lf, crlf or auto")
            }
            ("bracketSpacing", Value::Bool(b)) => {
                out.opt(
                    "bracketSpacing",
                    &["json", "yaml", "js"],
                    "bracket_spacing",
                    *b,
                );
            }
            ("singleQuote", Value::Bool(b)) => {
                let style = if *b { "single" } else { "prefer_double" };
                out.opt("singleQuote (yaml)", &["yaml"], "quote_style", style);
                out.opt_free(JS, "quote_style", if *b { "single" } else { "double" });
                out.opt_free(&["css"], "single_quote", *b);
            }
            ("semi", Value::Bool(b)) => out.opt_free(JS, "semi", *b),
            ("quoteProps", Value::String(s)) if QUOTE_PROPS.contains(&s.as_str()) => {
                out.opt_free(JS, "quote_props", s.replace('-', "_"));
            }
            ("trailingComma", Value::String(s)) if matches!(s.as_str(), "all" | "es5" | "none") => {
                out.opt_free(JS, "trailing_commas", s.clone());
            }
            ("arrowParens", Value::String(s)) if matches!(s.as_str(), "always" | "avoid") => {
                out.opt_free(JS, "arrow_parens", s.clone());
            }
            ("bracketSameLine", Value::Bool(b)) => out.opt_free(JS, "bracket_same_line", *b),
            ("quoteProps" | "trailingComma" | "semi" | "arrowParens" | "bracketSameLine", _) => {
                out.skip_why(key, "not a value Sudsy maps");
            }
            ("objectWrap", Value::String(s)) if s == "preserve" => {}
            ("objectWrap", Value::String(s)) if s == "collapse" => {
                out.opt("objectWrap", &["json"], "collapse", "always");
            }
            ("proseWrap", Value::String(s)) if s == "preserve" => {}
            ("proseWrap", _) => out.skip("proseWrap"),
            ("overrides", _) => out.skip("overrides / .prettierignore / .editorconfig"),
            ("embeddedLanguageFormatting", _) => out.skip("embeddedLanguageFormatting"),
            ("insertPragma" | "requirePragma" | "checkIgnorePragma", _) => {
                out.skip("insertPragma / requirePragma / checkIgnorePragma");
            }
            ("rangeStart" | "rangeEnd", _) => out.skip("rangeStart / rangeEnd"),
            (
                "$schema"
                | "jsxSingleQuote"
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
