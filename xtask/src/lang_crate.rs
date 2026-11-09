//! The provider glue and `Cargo.toml` of the tree-sitter language crates, from
//! `crates/lang-<name>/lang.yaml` (keys: docs/architecture/adding-a-language.md).

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn list(v: &Value, key: &str) -> Vec<String> {
    let items = v.get(key).and_then(Value::as_array);
    let items = items.into_iter().flatten();
    items
        .map(|s| s.as_str().unwrap_or_default().to_string())
        .collect()
}

fn text<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}

fn workspace_deps(names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|d| match d.starts_with("lang-") {
            // A language crate used by another (a test's companion) links its grammar.
            true => format!("{d} = {{ workspace = true, features = [\"static-grammars\"] }}"),
            false => format!("{d}.workspace = true"),
        })
        .collect()
}

pub fn generate(root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root.join("crates"))? {
        let dir = entry?.path();
        let manifest = dir.join("lang.yaml");
        if !manifest.exists() {
            continue;
        }
        let krate = dir.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let src = std::fs::read_to_string(&manifest)?;
        let v: Value = serde_saphyr::from_str(&src)
            .with_context(|| format!("parsing {}", manifest.display()))?;
        let rel = PathBuf::from("crates").join(krate);
        files.push((rel.join("Cargo.toml"), cargo_toml(krate, &v)));
        let name = krate.trim_start_matches("lang-");
        files.push((rel.join("src/provider.generated.rs"), provider(name, &v)));
    }
    Ok(files)
}

fn cargo_toml(krate: &str, v: &Value) -> String {
    let grammar = text(v, "grammar-crate", "");
    let mut names: Vec<String> = [
        "core-config",
        "core-format",
        "core-packs",
        "core-rules",
        "core-syntax",
        "core-treesitter",
        "provider-api",
    ]
    .map(String::from)
    .into();
    names.extend(list(v, "deps"));
    let mut deps = workspace_deps(&names);
    deps.push(format!(
        "{grammar} = {{ workspace = true, optional = true }}"
    ));
    deps.sort();
    let mut out = format!(
        "# Generated from lang.yaml by `cargo xtask codegen`.\n[package]\nname = \"{krate}\"\nedition.workspace = true\nlicense.workspace = true\nversion.workspace = true\npublish.workspace = true\nbuild = \"../../build/lang-crate.rs\"\n\n[dependencies]\n{}\n",
        deps.join("\n")
    );
    let dev = list(v, "dev-deps");
    if !dev.is_empty() {
        out += &format!(
            "\n[dev-dependencies]\n{}\n",
            workspace_deps(&dev).join("\n")
        );
    }
    out + &format!(
        "\n[build-dependencies]\ncore-syntax.workspace = true\n\n[features]\n# The grammar is linked in; without it the language loads from its language pack.\ndefault = [\"static-grammars\"]\nstatic-grammars = [\"dep:{grammar}\"]\n\n[lints]\nworkspace = true\n"
    )
}

fn provider(name: &str, v: &Value) -> String {
    let grammar = text(v, "grammar-crate", "").replace('-', "_");
    let language = text(v, "language-const", "LANGUAGE");
    let upper = name.to_uppercase();
    let mapping = text(v, "mapping", "mapping::MAPPING");
    let hooks = text(v, "hooks", "core_rules::DEFAULT_HOOKS");
    let validate = v
        .get("validate")
        .and_then(Value::as_str)
        .map_or("None".into(), |f| format!("Some({f})"));
    let strings = |key| {
        format!("{:?}", list(v, key))
            .replace('[', "")
            .replace(']', "")
    };
    let spec = format!(
        "Spec {{ crate_name: env!(\"CARGO_PKG_NAME\"), language: || core_packs::language!({name:?}, {grammar}::{language}), grammar: grammars::{upper}, mapping: &{mapping}, extensions: &[{}], file_names: &[{}], schema: option_schema(), formatter: &{}, hooks: &{hooks}, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: {}, validate: {validate} }}",
        strings("extensions"),
        strings("file-names"),
        text(v, "formatter", ""),
        text(v, "families", "&[]"),
    );
    let body = match v.get("composite").and_then(Value::as_str) {
        Some(c) => format!("TreeSitterProvider::with_composite({spec}, Some({c}))"),
        None => format!("TreeSitterProvider::new({spec})"),
    };
    let body = match v.get("embedded").and_then(Value::as_str) {
        Some(f) => format!("{body}.with_embedded({f})"),
        None => body,
    };
    format!(
        "// Generated from lang.yaml by `cargo xtask codegen`.\nuse core_syntax::grammars;\nuse core_treesitter::{{Spec, TreeSitterProvider}};\n\n#[rustfmt::skip]\npub fn provider() -> TreeSitterProvider {{\n    {body}\n}}\n"
    )
}
