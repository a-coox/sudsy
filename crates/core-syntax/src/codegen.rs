//! Code generation from grammars: the typed wrappers of a language crate (its build script writes
//! them to `OUT_DIR`) and the C# constants that `cargo xtask codegen` checks in.

use crate::grammar::{Cardinality, Grammar, Item, KindDef, Target};
use crate::kind::SyntaxKind;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;

#[derive(Debug, thiserror::Error)]
#[error("codegen: {0}")]
pub struct CodegenError(String);

fn fail<T>(msg: String) -> Result<T, CodegenError> {
    Err(CodegenError(msg))
}

/// `UTF8StringLiteralToken` -> `UTF8_STRING_LITERAL_TOKEN`.
pub fn screaming(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_ascii_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

fn pascal(tag: &str) -> String {
    tag.split('-')
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn check_ident(what: &str, name: &str) -> Result<(), CodegenError> {
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        fail(format!("{what} `{name}` is not a valid identifier"))
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

fn method_name(label: &str) -> String {
    if RUST_KEYWORDS.contains(&label) {
        format!("{label}_")
    } else {
        label.to_string()
    }
}

struct Names<'g> {
    g: &'g Grammar,
    consts: HashMap<SyntaxKind, String>,
}

impl<'g> Names<'g> {
    fn new(g: &'g Grammar) -> Result<Self, CodegenError> {
        let mut consts = HashMap::new();
        let mut seen: HashMap<String, String> = HashMap::new();
        for d in g.tokens().iter().chain(g.nodes()) {
            check_ident("kind", &d.name)?;
            let c = screaming(&d.name);
            if let Some(other) = seen.insert(c.clone(), d.name.clone()) {
                return fail(format!(
                    "kinds `{other}` and `{}` both map to constant {c}",
                    d.name
                ));
            }
            consts.insert(d.kind, c);
        }
        Ok(Names { g, consts })
    }

    fn konst(&self, k: SyntaxKind) -> &str {
        &self.consts[&k]
    }

    fn target_doc(&self, t: &Target) -> String {
        match t {
            Target::Kind(k) => self.g.name(*k).to_string(),
            Target::Tag(tag) => format!("#{tag}"),
        }
    }

    fn item_doc(&self, i: &Item) -> String {
        let mut s = String::new();
        if let Some(l) = &i.label {
            let _ = write!(s, "{l}:");
        }
        s.push_str(&self.target_doc(&i.target));
        s.push_str(match i.cardinality {
            Cardinality::One => "",
            Cardinality::Optional => "?",
            Cardinality::Many => "*",
        });
        s
    }

    /// Is this target a token class (as opposed to nodes)?
    fn is_token_target(&self, t: &Target) -> bool {
        match t {
            Target::Kind(k) => k.is_token(),
            Target::Tag(tag) => self
                .g
                .kinds_with_tag(tag)
                .next()
                .is_some_and(|d| d.kind.is_token()),
        }
    }

    /// Rust expression for the sorted slice of kinds a target accepts: a single kind, or the
    /// shared list of its tag.
    fn kinds_expr(&self, t: &Target) -> String {
        match t {
            Target::Kind(k) => format!("&[kind::{}]", self.konst(*k)),
            Target::Tag(tag) => tag_const(tag),
        }
    }

    fn node_type(&self, t: &Target) -> String {
        match t {
            Target::Kind(k) => self.g.name(*k).to_string(),
            Target::Tag(tag) => format!("Any{}", pascal(tag)),
        }
    }
}

fn tag_const(tag: &str) -> String {
    format!("TAG_{}", tag.replace('-', "_").to_ascii_uppercase())
}

/// The kinds of a tag as a sorted Rust slice body (`kind::A, kind::B`).
fn kind_list(n: &Names, kinds: impl Iterator<Item = SyntaxKind>) -> String {
    let mut kinds: Vec<SyntaxKind> = kinds.collect();
    kinds.sort();
    kinds
        .iter()
        .map(|k| format!("kind::{}", n.konst(*k)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The pattern constant and one accessor per labeled item. Items are matched against the
/// children in order (see `core_syntax::ast::assign`), so labels with the same target stay apart.
fn accessors(n: &Names, def: &KindDef) -> Result<String, CodegenError> {
    let mut out = String::new();
    let slots: Vec<String> = def
        .body
        .iter()
        .map(|i| {
            format!(
                "Slot {{ kinds: {}, many: {} }}",
                n.kinds_expr(&i.target),
                i.cardinality == Cardinality::Many
            )
        })
        .collect();
    let _ = writeln!(
        out,
        "    const PATTERN: Pattern<'static> = Pattern {{ slots: &[{}], skip: SKIP }};",
        slots.join(", ")
    );
    let mut methods = BTreeSet::new();
    for (index, item) in def.body.iter().enumerate() {
        let Some(label) = &item.label else { continue };
        check_ident("label", label)?;
        let m = method_name(label);
        if !methods.insert(m.clone()) || m == "syntax" || m == "cast" {
            return fail(format!(
                "{}: label `{label}` collides with another accessor",
                def.name
            ));
        }
        let token = n.is_token_target(&item.target);
        let many = item.cardinality == Cardinality::Many;
        let (ret, body) = match (token, many) {
            (true, false) => (
                "::core::option::Option<SyntaxToken>".to_string(),
                format!("support::token(&self.0, &Self::PATTERN, {index})"),
            ),
            (true, true) => (
                "impl ::core::iter::Iterator<Item = SyntaxToken>".to_string(),
                format!("support::tokens(&self.0, &Self::PATTERN, {index})"),
            ),
            (false, false) => (
                format!("::core::option::Option<{}>", n.node_type(&item.target)),
                format!("support::node(&self.0, &Self::PATTERN, {index})"),
            ),
            (false, true) => (
                format!(
                    "impl ::core::iter::Iterator<Item = {}>",
                    n.node_type(&item.target)
                ),
                format!("support::nodes(&self.0, &Self::PATTERN, {index})"),
            ),
        };
        let _ = writeln!(out, "    pub fn {m}(&self) -> {ret} {{ {body} }}");
    }
    Ok(out)
}

/// The generated Rust module for a grammar, for a language crate (`include_generated!`).
/// `grammar_path` is shown in the header.
pub fn rust_module(g: &Grammar, grammar_path: &str) -> Result<String, CodegenError> {
    let crate_path = "core_syntax";
    let n = Names::new(g)?;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// @generated by the build script from {grammar_path}. Do not edit."
    );
    let _ = writeln!(
        out,
        "use {crate_path}::ast::{{support, AstNode, Pattern, Slot}};"
    );
    let _ = writeln!(
        out,
        "use {crate_path}::{{ast_any, ast_node, LanguageId, SyntaxKind, SyntaxToken}};\n"
    );
    let _ = writeln!(
        out,
        "pub const LANGUAGE: LanguageId = LanguageId::new({});",
        g.language_id.get()
    );
    let _ = writeln!(
        out,
        "/// The grammar's version; the worker must report the same value."
    );
    let _ = writeln!(out, "pub const GRAMMAR_HASH: &str = \"{}\";\n", g.hash());

    out.push_str("pub mod kind {\n    use super::*;\n");
    for d in g.tokens().iter().chain(g.nodes()) {
        let _ = writeln!(
            out,
            "    pub const {}: SyntaxKind = SyntaxKind::new(LANGUAGE, {}, {});",
            n.konst(d.kind),
            d.kind.is_token(),
            d.kind.index()
        );
    }
    out.push_str("}\n\n");

    // Kinds that never take part in matching a body, and the kinds behind every tag a body uses.
    let skip = kind_list(
        &n,
        g.tokens()
            .iter()
            .chain(g.nodes())
            .filter(|d| d.has_tag("trivia"))
            .map(|d| d.kind),
    );
    let _ = writeln!(
        out,
        "/// Trivia and `ERROR` nodes, which bodies ignore. Sorted.\nconst SKIP: &[SyntaxKind] = &[SyntaxKind::ERROR{}{skip}];\n",
        if skip.is_empty() { "" } else { ", " }
    );
    let mut used_tags = BTreeSet::new();
    for d in g.nodes() {
        for i in &d.body {
            if let Target::Tag(t) = &i.target {
                used_tags.insert(t.clone());
            }
        }
    }
    for tag in &used_tags {
        let list = kind_list(&n, g.kinds_with_tag(tag).map(|d| d.kind));
        let _ = writeln!(out, "const {}: &[SyntaxKind] = &[{list}];", tag_const(tag));
    }
    out.push('\n');

    // Tag-typed node fields need an `Any<Tag>` wrapper.
    let mut tags = BTreeSet::new();
    for d in g.nodes() {
        for i in &d.body {
            if let Target::Tag(t) = &i.target
                && i.label.is_some()
                && !n.is_token_target(&i.target)
            {
                tags.insert(t.clone());
            }
        }
    }
    for tag in &tags {
        let ty = format!("Any{}", pascal(tag));
        if g.nodes().iter().any(|d| d.name == ty) {
            return fail(format!(
                "node `{ty}` collides with the wrapper for tag #{tag}"
            ));
        }
        let alts: Vec<String> = g
            .kinds_with_tag(tag)
            .map(|d| format!("kind::{}", n.konst(d.kind)))
            .collect();
        let _ = writeln!(
            out,
            "ast_any!(
    /// Any node tagged `#{tag}`.
    {ty}, {});
",
            alts.join(" | ")
        );
    }

    for d in g.nodes() {
        let rule: Vec<String> = d.body.iter().map(|i| n.item_doc(i)).collect();
        let _ = writeln!(
            out,
            "ast_node!(
    /// `{} = {}`",
            d.name,
            rule.join(" ")
        );
        let _ = writeln!(out, "    {}, kind::{});", d.name, n.konst(d.kind));
        let acc = accessors(&n, d)?;
        if !acc.is_empty() {
            let _ = writeln!(out, "impl {} {{\n{acc}}}", d.name);
        }
        out.push('\n');
    }
    Ok(out.trim_end().to_string() + "\n")
}

/// C# constants for the worker: grammar hash and kind ids by name.
pub fn csharp_constants(g: &Grammar, grammar_path: &str) -> Result<String, CodegenError> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// <auto-generated>\n// Generated by `cargo xtask codegen` from {grammar_path}. Do not edit.\n// </auto-generated>"
    );
    out.push_str("namespace Sudsy.Worker.Generated;\n\n");
    let _ = writeln!(
        out,
        "public static class {}Grammar\n{{",
        pascal(&g.language)
    );
    let _ = writeln!(out, "    public const string Hash = \"{}\";", g.hash());
    let _ = writeln!(
        out,
        "    public const byte LanguageId = {};\n",
        g.language_id.get()
    );
    out.push_str("    /// <summary>Kind id by grammar name; tokens carry bit 11.</summary>\n");
    out.push_str("    public static readonly (string Name, uint Id)[] Kinds =\n    [\n");
    for d in g.tokens().iter().chain(g.nodes()) {
        check_ident("kind", &d.name)?;
        let _ = writeln!(out, "        (\"{}\", 0x{:08X}),", d.name, d.kind.0);
    }
    out.push_str("    ];\n}\n");
    Ok(out)
}
