//! CSharpier (`.csharpierrc`, `.csharpierrc.json`, `.csharpierrc.yaml`, `.csharpierrc.yml`).

use super::{editorconfig, out::Out};
use anyhow::{Result, bail};
use serde_json::Value;

pub const FILES: [&str; 4] = [
    ".csharpierrc",
    ".csharpierrc.json",
    ".csharpierrc.yaml",
    ".csharpierrc.yml",
];
const CS: &[&str] = &["csharp"];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not an object");
    };
    out.extend("csharp/csharpier-compat");
    for (key, value) in root {
        match (key.as_str(), value) {
            ("printWidth", v) if v.is_u64() => out.opt("printWidth", CS, "line_width", v.clone()),
            ("useTabs", Value::Bool(b)) => out.opt(
                "useTabs",
                CS,
                "indent_style",
                if *b { "tab" } else { "space" },
            ),
            ("indentSize" | "tabWidth", v) if v.is_u64() => {
                out.opt("indentSize", CS, "indent_size", v.clone())
            }
            ("endOfLine", Value::String(s)) if matches!(s.as_str(), "lf" | "crlf" | "auto") => {
                out.opt("endOfLine", CS, "end_of_line", s.clone());
            }
            ("endOfLine", _) => out.skip_why("endOfLine", "use lf, crlf or auto"),
            ("overrides", _) => out.skip("overrides"),
            ("preprocessorSymbolSets", _) => out.skip("preprocessorSymbolSets"),
            ("xmlWhitespaceSensitivity", _) => out.skip("xmlWhitespaceSensitivity"),
            ("$schema", _) => {}
            (other, _) => out.skip_why(other, "not a CSharpier option Sudsy maps"),
        }
    }
    out.editorconfig = editorconfig::suppressions(false);
    Ok(())
}
