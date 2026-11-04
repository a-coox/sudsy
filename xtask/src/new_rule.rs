//! `cargo xtask new-rule <language> <id> [--rust]`: the files a new rule starts from. The starter
//! rule reports empty statements (C#) or empty mappings (the data languages), so the generated
//! rule and its fixture pass the gate as they are; edit both, then run `cargo fmt`. Generated docs
//! and the schema come from `cargo xtask codegen`.

use anyhow::{Context, Result, bail, ensure};
use std::fs;
use std::path::Path;

/// A language with scaffolding. Rust rules go under `crates/lang-<name>/src/rules/` and are listed
/// in `registry`; pattern rules (`crates/lang-<name>/rules/*.yaml`) exist only where the provider
/// loads built-in pattern files (`patterns`).
struct Lang {
    name: &'static str,
    /// Fixture extension.
    ext: &'static str,
    /// The file with `mod` lines and the `RULES` list, relative to the crate.
    registry: &'static str,
    patterns: bool,
    /// The starter fixture: an input line the starter rule reports, with the comment marker and
    /// the column of the report (for the carets).
    case: &'static str,
    marker: &'static str,
    column: usize,
    width: usize,
}

const LANGUAGES: &[Lang] = &[
    Lang {
        name: "csharp",
        ext: "cs",
        registry: "src/rules/mod.rs",
        patterns: true,
        case: "class A {\n    void M() {\n        ;\n",
        marker: "//",
        column: 8,
        width: 1,
    },
    Lang {
        name: "json",
        ext: "json",
        registry: "src/rules.rs",
        patterns: false,
        case: "{\n  \"a\": {}\n",
        marker: "//",
        column: 7,
        width: 2,
    },
    Lang {
        name: "yaml",
        ext: "yaml",
        registry: "src/rules.rs",
        patterns: false,
        case: "a: {}\n",
        marker: "#",
        column: 3,
        width: 2,
    },
    Lang {
        name: "toml",
        ext: "toml",
        registry: "src/rules.rs",
        patterns: false,
        case: "a = {}\n",
        marker: "#",
        column: 4,
        width: 2,
    },
];

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

fn rust_rule(lang: &Lang, id: &str) -> String {
    // C# rules subscribe to a node kind; the data languages' to the grammar tag `mapping`.
    let (imports, subscription, body) = if lang.name == "csharp" {
        (
            "use super::kind;\nuse core_rules::{RuleContext, rule};\nuse core_syntax::SyntaxNode;\n",
            "kinds: &[kind::EMPTY_STATEMENT],",
            "",
        )
    } else {
        (
            "use core_rules::{RuleContext, rule};\nuse core_syntax::SyntaxNode;\n",
            "tags: &[\"mapping\"],",
            "        if node.children().next().is_some() {\n            return;\n        }\n",
        )
    };
    format!(
        "{imports}\nrule! {{\n    pub {name} {{\n        id: \"{id}\",\n        category: Style,\n        \
         default: Off,\n        description: \"TODO what the rule reports, and what it leaves alone.\",\n        \
         example: \"TODO code that triggers the rule   // write: what it should be\",\n        \
         {subscription}\n    }}\n\n    \
         fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {{\n{body}        \
         let range = core_rules::tree::significant_range(node, ctx.file.grammar);\n        \
         ctx.report(range, \"TODO what the diagnostic says\");\n    }}\n}}\n",
        name = pascal(id)
    )
}

/// The starter input: the language's line, then the annotation under the line that is reported
/// (the last of `case`), then whatever closes the construct.
fn fixture(lang: &Lang, id: &str) -> String {
    let mut text = lang.case.to_string();
    let pad = " ".repeat(lang.column.saturating_sub(lang.marker.len()));
    text.push_str(&format!(
        "{}{pad}{} {id}\n",
        lang.marker,
        "^".repeat(lang.width)
    ));
    if lang.name == "csharp" {
        text.push_str("    }\n}\n");
    } else if lang.name == "json" {
        text.push_str("}\n");
    }
    text
}

fn write_new(path: &Path, text: &str) -> Result<()> {
    ensure!(!path.exists(), "{} exists already", path.display());
    fs::create_dir_all(path.parent().expect("a file path"))?;
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Inserts `mod <snake>;` in order among the module declarations of the registry (before the
/// `RULES` list when it has none) and `&<snake>::<Name>,` at the end of its `RULES` list.
fn register(registry: &Path, snake: &str, name: &str) -> Result<()> {
    let text = fs::read_to_string(registry)?;
    let crlf = text.contains("\r\n");
    let text = text.replace("\r\n", "\n");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let decl = format!("mod {snake};");
    let list = lines
        .iter()
        .position(|l| l.starts_with("pub static RULES"))
        .context("no `RULES` list in the rules registry")?;
    match lines
        .iter()
        .position(|l| l.starts_with("mod ") && *l > decl)
        .or_else(|| {
            lines
                .iter()
                .rposition(|l| l.starts_with("mod "))
                .map(|i| i + 1)
        }) {
        Some(at) => lines.insert(at, decl),
        None => {
            lines.insert(list, String::new());
            lines.insert(list, decl);
        }
    }
    let list = lines
        .iter()
        .position(|l| l.starts_with("pub static RULES"))
        .expect("found above");
    let entry = format!("    &{snake}::{name},");
    if let Some(head) = lines[list].strip_suffix("];") {
        // A one-line list: `= &[];` or `= &[&a::A];`.
        let (head, items) = head
            .split_once("= &[")
            .context("an unreadable `RULES` list")?;
        let mut expanded = vec![format!("{head}= &[")];
        expanded.extend(
            items
                .split(',')
                .map(str::trim)
                .filter(|i| !i.is_empty())
                .map(|i| format!("    {i},")),
        );
        expanded.push(entry);
        expanded.push("];".to_string());
        lines.splice(list..=list, expanded);
    } else {
        let end = list
            + lines[list..]
                .iter()
                .position(|l| l == "];")
                .context("unterminated `RULES` list")?;
        lines.insert(end, entry);
    }
    let mut out = lines.join("\n") + "\n";
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    fs::write(registry, out).map_err(Into::into)
}

/// Creates the rule's files under `root`; returns what to tell the user.
pub fn new_rule(root: &Path, language: &str, id: &str, rust: bool) -> Result<String> {
    let Some(lang) = LANGUAGES.iter().find(|l| l.name == language) else {
        let known: Vec<_> = LANGUAGES.iter().map(|l| l.name).collect();
        bail!(
            "no scaffolding for `{language}` (known: {}); add the rule by hand as docs/development/testing.md describes",
            known.join(", ")
        );
    };
    ensure!(
        rust || lang.patterns,
        "{language} has no built-in pattern rule files yet; use `--rust`"
    );
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
        write_new(&file, &rust_rule(lang, id))?;
        register(&krate.join(lang.registry), &snake, &pascal(id))?;
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
        .join(format!("starter.{}", lang.ext));
    write_new(&case, &fixture(lang, id))?;
    Ok(format!(
        "created {}\ncreated {}\n\
         next: edit both, `cargo fmt`, `SUDSY_BLESS=1 cargo test -p lang-{language} --test rule_fixtures`, then `cargo xtask codegen`.\n\
         If another tool has an equivalent, put this on its entry in docs/parity/<tool>.yaml:\n    sudsy: \"rule: {id}\"\n    coverage: \"covered\"",
        created.display(),
        case.display()
    ))
}
