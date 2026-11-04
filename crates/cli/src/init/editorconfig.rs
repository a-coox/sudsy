//! `.editorconfig` side of a C# migration: reading `dotnet_diagnostic.SA*` severities, and the
//! entries that switch off the SDK/StyleCop checks Sudsy now does.

use super::sources;
use anyhow::Result;
use languages::catalog;
use std::path::Path;

const FILE: &str = ".editorconfig";
const HEADER: &str = "# Added by `sudsy init`: Sudsy formats and lints these.";

/// `(SA id, severity)` of every `dotnet_diagnostic.SA####.severity` line, in file order.
pub fn sa_severities() -> Result<Vec<(String, String)>> {
    let path = Path::new(FILE);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for line in sources::read(path)?.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_uppercase();
        let id = key
            .strip_prefix("DOTNET_DIAGNOSTIC.")
            .and_then(|k| k.strip_suffix(".SEVERITY"));
        if let Some(id) = id.filter(|i| i.starts_with("SA")) {
            let severity = value.split(['#', ';']).next().unwrap_or("").trim();
            found.push((id.to_string(), severity.to_ascii_lowercase()));
        }
    }
    Ok(found)
}

/// The StyleCop diagnostics Sudsy covers: a rule, a formatter option or fixed formatter behavior.
fn covered_stylecop() -> Vec<&'static str> {
    let mut ids: Vec<&str> = catalog::entries("stylecop")
        .iter()
        .filter(|e| e.coverage == "covered" && e.id.starts_with("SA"))
        .filter(|e| {
            ["rule: ", "option: "]
                .iter()
                .any(|p| e.sudsy.starts_with(p))
                || e.sudsy == "formatter"
        })
        .map(|e| e.id.as_str())
        .collect();
    ids.sort_unstable();
    ids
}

/// The lines for `[*.cs]` that turn the SDK's formatting check off, and StyleCop's covered rules
/// when `stylecop`.
pub fn suppressions(stylecop: bool) -> Vec<String> {
    let mut ids = vec!["IDE0055"];
    if stylecop {
        ids.extend(covered_stylecop());
    }
    ids.into_iter()
        .map(|id| format!("dotnet_diagnostic.{id}.severity = none"))
        .collect()
}

pub fn block(lines: &[String]) -> String {
    format!("{HEADER}\n[*.cs]\n{}\n", lines.join("\n"))
}

/// Appends the block to `.editorconfig` (created when missing). `false` when it is already there.
pub fn append(lines: &[String]) -> Result<bool> {
    let path = Path::new(FILE);
    let mut text = if path.is_file() {
        sources::read(path)?
    } else {
        String::new()
    };
    if text.contains(HEADER) {
        return Ok(false);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&block(lines));
    std::fs::write(path, text)?;
    Ok(true)
}
