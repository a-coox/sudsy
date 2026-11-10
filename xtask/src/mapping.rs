//! The tree-sitter node table and composite settings of a language, from the `mapping` and
//! `composite` maps of its `lang.yaml` (docs/architecture/adding-a-language.md): consts appended
//! to `src/provider.generated.rs`.

use crate::lang_crate::Lang;
use anyhow::{Context, Result};
use core_syntax::Grammar;
use serde_json::Value;

/// A value of the maps: a list, or a string of words split on blanks.
fn words(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a
            .iter()
            .filter_map(|s| s.as_str().map(String::from))
            .collect(),
        Value::String(s) => s.split_whitespace().map(String::from).collect(),
        _ => Vec::new(),
    }
}

fn pascal(snake: &str) -> String {
    let up = |w: &str| w[..1].to_uppercase() + &w[1..];
    snake.split('_').filter(|w| !w.is_empty()).map(up).collect()
}

fn pairs(rows: &[(String, String)]) -> String {
    rows.iter()
        .map(|(a, b)| format!("        ({a:?}, {b:?}),\n"))
        .collect()
}

/// `MAPPING` (and `COMPOSITE`) for a `mapping` map: its entries as written, then `same` names
/// (a rule and its kind named alike in snake and Pascal case), then the grammar's fixed-text
/// tokens by their text. The first entry of a name wins.
pub fn consts(name: &str, l: &Lang) -> Result<String> {
    let Some(Value::Object(map)) = l.yaml.get("mapping") else {
        return Ok(String::new());
    };
    let (mut kinds, mut wrap) = (Vec::new(), Vec::new());
    let mut same = Vec::new();
    for (key, v) in map {
        match key.as_str() {
            "same" => same = words(v),
            "wrap" => {
                for (from, to) in v.as_object().context("`wrap` is a map")? {
                    wrap.push((from.clone(), to.as_str().unwrap_or_default().to_string()));
                }
            }
            kind => kinds.extend(words(v).into_iter().map(|from| (from, kind.to_string()))),
        }
    }
    kinds.extend(same.into_iter().map(|n| (n.clone(), pascal(&n))));
    let path = crate::repo_root().join(format!("grammars/{name}.grammar"));
    let src = std::fs::read_to_string(&path).with_context(|| format!("reading {path:?}"))?;
    let grammar = Grammar::parse(&src).map_err(|e| anyhow::anyhow!("{path:?}: {e}"))?;
    for t in grammar.tokens() {
        if let Some(text) = &t.text {
            kinds.push((text.clone(), t.name.clone()));
        }
    }
    let mut seen = std::collections::HashSet::new();
    kinds.retain(|(from, _)| seen.insert(from.clone()));
    let mut out = format!(
        "\npub const MAPPING: Mapping = Mapping {{\n    whitespace: \"Whitespace\",\n    newline: \"Newline\",\n    eof: \"Eof\",\n    unknown: \"Unknown\",\n    wrap: &[\n{}    ],\n    kinds: &[\n{}    ],\n}};\n",
        pairs(&wrap),
        pairs(&kinds)
    );
    if let Some(Value::Object(c)) = l.yaml.get("composite") {
        let text = c.get("text").and_then(Value::as_str).unwrap_or_default();
        let flag = c
            .get("continuation")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let tolerant = format!("{:?}", c.get("tolerant").map_or(vec![], words));
        out += &format!(
            "\npub const COMPOSITE: Composite = Composite {{\n    text: {text:?},\n    injected: &[],\n    refine: &[],\n    continuation: {flag},\n    tolerant: &{tolerant},\n}};\n"
        );
    }
    Ok(out)
}
