//! EditorConfig formatting: `[glob]` headers, `key = value` pairs one per line, a blank line before
//! each section. Keys, globs, values and comments are kept as written.

use crate::generated::kind as k;
use core_config::shared;
use core_format::shapes::statement_sequence_with;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;

include!("schema.generated.rs");

pub struct EditorConfigFormatter;

impl NodeFormatter for EditorConfigFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::SECTION => section(cx, node)?,
            k::PAIR => pair(cx, node),
            _ => return Ok(None),
        }))
    }
}

/// Every section starts after a blank line, the first one after the preamble only.
fn blank_before_section(_: &FormatCtx<'_>, _: &SyntaxNode, next: &SyntaxNode) -> bool {
    next.kind() == k::SECTION
}

fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    core_format::shapes::document(
        cx,
        node,
        cx.options.max_blank_lines(),
        Some(blank_before_section),
    )
}

/// `[glob]` then its pairs, one per line.
fn section(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let pairs: Vec<SyntaxNode> = node.children().filter(|n| n.kind() == k::PAIR).collect();
    for n in node.children().filter(|n| n.kind() == k::SECTION_HEADER) {
        parts.push(cx.format(&n)?);
    }
    if !pairs.is_empty() {
        parts.push(doc::hardline());
        parts.push(statement_sequence_with(
            cx,
            &pairs,
            cx.options.max_blank_lines(),
            None,
            false,
        )?);
    }
    Ok(doc::concat(parts))
}

/// `key = value`; a pair without a value is `key =`.
fn pair(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let mut parts = Vec::new();
    for t in node.tokens().filter(|t| !cx.grammar.is_trivia(t.kind())) {
        if t.kind() != k::KEY {
            parts.push(doc::lit(" "));
        }
        parts.push(cx.token(&t));
    }
    doc::concat(parts)
}
