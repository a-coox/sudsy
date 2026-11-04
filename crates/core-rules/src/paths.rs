//! Key paths (`a.b[0]["odd key"].c`, the form eslint-plugin-jsonc's `pathPattern` matches) of
//! mappings and sequences, for the rules that can be limited to part of a document.

use crate::options::{Check, Opt, OptionDecl};
use crate::rule::RuleContext;
use crate::sorting::cached_regex;
use core_syntax::SyntaxNode;

pub const PATH_PATTERNS: OptionDecl = OptionDecl {
    name: "path_patterns",
    default: Opt::List(&[]),
    check: Check::Regex,
    description: "Regular expressions; only mappings (sequences) whose key path matches one are \
                  checked. The path is `a.b[0].c` (`[\"odd key\"]` for keys that are not \
                  identifiers), empty for the root; none listed means everywhere. A TOML table \
                  counts as its header key and an array table is not indexed.",
};

/// Is `node` inside the part of the document the rule's `path_patterns` select?
pub fn in_scope(node: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    let patterns = ctx.options.list("path_patterns");
    if patterns.is_empty() {
        return true;
    }
    let path = key_path(node, ctx);
    patterns
        .into_iter()
        .filter_map(cached_regex)
        .any(|r| r.is_match(&path))
}

fn key_path(node: &SyntaxNode, ctx: &RuleContext<'_>) -> String {
    let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
    let mut segments: Vec<String> = Vec::new();
    let key_of = |n: &SyntaxNode, out: &mut Vec<String>| {
        if let Some(key) = grammar.slot(n, "key") {
            let parts = hooks
                .key_parts(&key, text)
                .unwrap_or_else(|| vec![hooks.key_name(&key, text)]);
            out.extend(parts.iter().rev().map(|p| segment(p)));
        }
    };
    let mut current = node.clone();
    // A node with a key of its own (a TOML table header) is at that key.
    if grammar.has_tag(current.kind(), "mapping") {
        key_of(&current, &mut segments);
    }
    while let Some(parent) = current.parent() {
        if grammar.has_tag(parent.kind(), "key-value") {
            key_of(&parent, &mut segments);
        } else if grammar.has_tag(parent.kind(), "sequence") {
            let index = parent.children().position(|c| c == current).unwrap_or(0);
            segments.push(format!("[{index}]"));
        } else if grammar.has_tag(parent.kind(), "mapping") {
            key_of(&parent, &mut segments);
        }
        current = parent;
    }
    let path: String = segments.iter().rev().map(String::as_str).collect();
    path.strip_prefix('.').unwrap_or(&path).to_string()
}

/// One key as a path segment: `.name`, or `["name"]` when it is not an identifier.
fn segment(name: &str) -> String {
    let mut chars = name.chars();
    let identifier = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '$' || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '$' || c == '_');
    if identifier {
        format!(".{name}")
    } else {
        format!("[{name:?}]")
    }
}
