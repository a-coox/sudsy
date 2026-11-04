//! `cargo xtask new-rule <language> <id> [--rust]`: the files a new rule starts from. The starter
//! rule reports empty statements, so the generated rule and its fixture pass the gate as they are;
//! edit both. Generated docs and the schema come from `cargo xtask codegen`.

use anyhow::{Context, Result, bail, ensure};
use std::fs;
use std::path::Path;

/// Languages with built-in rules in Rust under `crates/lang-<language>/src/rules/` and in YAML
/// under `crates/lang-<language>/rules/`, with their fixture extension.
const LANGUAGES: &[(&str, &str)] = &[("csharp", "cs")];

fn pascal(id: &str) -> String {
    id.split('-')
        .map(|w| w[..1].to_uppercase() + &w[1..])
        .collect()
}

fn yaml_rule(language: &str, id: &str) -> String {
    format!(
        "id: {id}\nlanguage: {language}\nmessage: TODO what the diagnostic says\n\
         description: TODO what the rule reports, and what it leaves alone.\n\
         example: 'TODO code that triggers the rule   // write: what it should be'\n\
         severity: off\ncategory: style\nrule:\n  kind: EmptyStatement\n"
    )
}

fn rust_rule(id: &str) -> String {
    format!(
        "use super::kind;\nuse core_rules::{{RuleContext, rule}};\nuse core_syntax::SyntaxNode;\n\n\
         rule! {{\n    pub {name} {{\n        id: \"{id}\",\n        category: Style,\n        \
         default: Off,\n        description: \"TODO what the rule reports, and what it leaves alone.\",\n        \
         example: \"TODO code that triggers the rule   // write: what it should be\",\n        \
         kinds: &[kind::EMPTY_STATEMENT],\n    }}\n\n    \
         fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {{\n        \
         let range = core_rules::tree::significant_range(node, ctx.file.grammar);\n        \
         ctx.report(range, \"TODO what the diagnostic says\");\n    }}\n}}\n",
        name = pascal(id)
    )
}

fn fixture(id: &str) -> String {
    format!("class A {{\n    void M() {{\n        ;\n//      ^ {id}\n    }}\n}}\n")
}

fn write_new(path: &Path, text: &str) -> Result<()> {
    ensure!(!path.exists(), "{} exists already", path.display());
    fs::create_dir_all(path.parent().expect("a file path"))?;
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Inserts `mod <snake>;` in order among the module declarations of a rules `mod.rs` and
/// `&<snake>::<Name>,` at the end of its `RULES` list.
fn register(mod_rs: &Path, snake: &str, name: &str) -> Result<()> {
    let text = fs::read_to_string(mod_rs)?;
    let crlf = text.contains("\r\n");
    let text = text.replace("\r\n", "\n");
    let mut lines: Vec<&str> = text.lines().collect();
    let decl = format!("mod {snake};");
    let at = lines
        .iter()
        .position(|l| l.starts_with("mod ") && **l > *decl)
        .or_else(|| {
            lines
                .iter()
                .rposition(|l| l.starts_with("mod "))
                .map(|i| i + 1)
        })
        .context("no `mod` declarations in rules/mod.rs")?;
    lines.insert(at, &decl);
    let start = lines
        .iter()
        .position(|l| l.starts_with("pub static RULES"))
        .context("no `RULES` list in rules/mod.rs")?;
    let end = start
        + lines[start..]
            .iter()
            .position(|l| *l == "];")
            .context("unterminated `RULES` list")?;
    let entry = format!("    &{snake}::{name},");
    lines.insert(end, &entry);
    let mut out = lines.join("\n") + "\n";
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    fs::write(mod_rs, out).map_err(Into::into)
}

/// Creates the rule's files under `root`; returns what to tell the user.
pub fn new_rule(root: &Path, language: &str, id: &str, rust: bool) -> Result<String> {
    let Some((_, ext)) = LANGUAGES.iter().find(|(l, _)| *l == language) else {
        let known: Vec<_> = LANGUAGES.iter().map(|(l, _)| *l).collect();
        bail!(
            "no scaffolding for `{language}` (known: {}); add the rule by hand as docs/development/testing.md describes",
            known.join(", ")
        );
    };
    ensure!(
        !id.is_empty()
            && id.starts_with(|c: char| c.is_ascii_lowercase())
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !id.ends_with('-')
            && !id.contains("--"),
        "the rule id must be lowercase words joined by `-`: `{id}`"
    );
    let krate = root.join("crates").join(format!("lang-{language}"));
    let snake = id.replace('-', "_");
    let created = if rust {
        let file = krate.join("src/rules").join(format!("{snake}.rs"));
        write_new(&file, &rust_rule(id))?;
        register(&krate.join("src/rules/mod.rs"), &snake, &pascal(id))?;
        file
    } else {
        let file = krate.join("rules").join(format!("{id}.yaml"));
        write_new(&file, &yaml_rule(language, id))?;
        file
    };
    let case = root
        .join("tests/rules")
        .join(language)
        .join(id)
        .join(format!("starter.{ext}"));
    write_new(&case, &fixture(id))?;
    Ok(format!(
        "created {}\ncreated {}\n\
         next: edit both, `SUDSY_BLESS=1 cargo test -p lang-{language} --test rule_fixtures`, then `cargo xtask codegen`.\n\
         If another tool has an equivalent, put this on its entry in docs/parity/<tool>.yaml:\n    sudsy: \"rule: {id}\"\n    coverage: \"covered\"",
        created.display(),
        case.display()
    ))
}
