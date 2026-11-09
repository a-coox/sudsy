//! `cargo xtask new-rule <language> <id> [--rust]`: the files a new rule starts from. The starter
//! rule reports the node that the language's `starter.<Kind>.<ext>` marks, so the generated
//! rule and its fixture pass the gate as they are; edit both, then run `cargo fmt`. Generated docs
//! and the schema come from `cargo xtask codegen`.

use anyhow::{Context, Result, ensure};
use core_syntax::codegen::screaming;
use std::fs;
use std::path::Path;

/// The starter of a language: `crates/lang-<language>/starter.<Kind>.<ext>`, a rule fixture whose
/// last annotation (`^^^ ID`) marks the node of kind `<Kind>` that the starter rule reports.
struct Starter {
    kind: String,
    ext: String,
    /// The fixture, with `ID` for the rule id.
    text: String,
    /// The annotated text.
    reported: String,
}

fn starter(krate: &Path) -> Result<Starter> {
    let file = fs::read_dir(krate)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .find_map(|p| {
            let name = p.file_name()?.to_str()?.to_string();
            let mut parts = name.strip_prefix("starter.")?.splitn(2, '.');
            Some((
                p.clone(),
                parts.next()?.to_string(),
                parts.next()?.to_string(),
            ))
        })
        .context("the language has no `starter.<Kind>.<ext>` file")?;
    let text = fs::read_to_string(&file.0)?.replace("\r\n", "\n");
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .rposition(|l| l.contains("^ ID"))
        .context("the starter has no `^ ID` annotation")?;
    let carets = lines[at].find('^').context("a caret")?;
    let width = lines[at].chars().filter(|&c| c == '^').count();
    let reported = lines[at - 1][carets..carets + width].to_string();
    Ok(Starter {
        kind: file.1,
        ext: file.2,
        text,
        reported,
    })
}

fn pascal(id: &str) -> String {
    id.split('-')
        .map(|w| w[..1].to_uppercase() + &w[1..])
        .collect()
}

/// The text as an anchored regex.
fn regex(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|c| {
            let meta = "\\.+*?()|[]{}^$#&-~".contains(c);
            meta.then_some('\\').into_iter().chain([c])
        })
        .collect();
    format!("^{escaped}$")
}

fn yaml_rule(id: &str, starter: &Starter) -> String {
    let kind = &starter.kind;
    let regex = regex(&starter.reported).replace('\'', "''");
    format!(
        "id: {id}\nmessage: TODO what the diagnostic says\n\
         description: TODO what the rule reports, and what it leaves alone.\n\
         example: 'TODO code that triggers the rule   // write: what it should be'\n\
         severity: off\ncategory: style\nrule:\n  kind: {kind}\n  regex: '{regex}'\n"
    )
}

fn rust_rule(id: &str, starter: &Starter) -> String {
    let constant = screaming(&starter.kind);
    let reported = format!("{:?}", starter.reported);
    format!(
        "use crate::generated::kind;\nuse core_rules::{{RuleContext, rule}};\nuse core_syntax::SyntaxNode;\n\n\
         rule! {{\n    pub {name} {{\n        id: \"{id}\",\n        category: Style,\n        \
         default: Off,\n        description: \"TODO what the rule reports, and what it leaves alone.\",\n        \
         example: \"TODO code that triggers the rule   // write: what it should be\",\n        \
         kinds: &[kind::{constant}],\n    }}\n\n    \
         fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {{\n        \
         let range = core_rules::tree::significant_range(node, ctx.file.grammar);\n        \
         if ctx.file.text[std::ops::Range::<usize>::from(range)] != *{reported} {{\n            return;\n        }}\n        \
         ctx.report(range, \"TODO what the diagnostic says\");\n    }}\n}}\n",
        name = pascal(id)
    )
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
    let starter = starter(&krate).with_context(|| {
        format!("no scaffolding for `{language}`; add the rule by hand as docs/development/testing.md describes")
    })?;
    let snake = id.replace('-', "_");
    let created = if rust {
        let file = krate.join("src/rules").join(format!("{snake}.rs"));
        write_new(&file, &rust_rule(id, &starter))?;
        let nested = krate.join("src/rules/mod.rs");
        let registry = if nested.exists() {
            nested
        } else {
            krate.join("src/rules.rs")
        };
        register(&registry, &snake, &pascal(id))?;
        file
    } else {
        let file = krate.join("rules").join(format!("{id}.yaml"));
        write_new(&file, &yaml_rule(id, &starter))?;
        file
    };
    let case = root
        .join("tests/rules")
        .join(language)
        .join(id)
        .join(format!("starter.{}", starter.ext));
    write_new(&case, &starter.text.replace("^ ID", &format!("^ {id}")))?;
    Ok(format!(
        "created {}\ncreated {}\n\
         next: edit both, `cargo fmt`, `SUDSY_BLESS=1 cargo test -p lang-{language} --test rule_fixtures`, then `cargo xtask codegen`.\n\
         If another tool has an equivalent, put this on its entry in docs/parity/<tool>.yaml:\n    sudsy: \"rule: {id}\"\n    coverage: \"covered\"",
        created.display(),
        case.display()
    ))
}
