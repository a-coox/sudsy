//! INI formatting: `[section]` headers, `key = value` settings one per line, a blank line before
//! each section. Names, keys, values and comments are kept as written.

use crate::generated::kind as k;
use core_config::shared;
use core_format::shapes::statement_sequence_with;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::SyntaxNodeExt;
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct IniFormatter;

impl NodeFormatter for IniFormatter {
    /// The formatter trims the blanks around a value.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => t.text().trim().into(),
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::SECTION => section(cx, node)?,
            k::SETTING => setting(cx, node),
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

/// `[name]` then its settings, one per line.
fn section(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let settings: Vec<SyntaxNode> = node.children().filter(|n| n.kind() == k::SETTING).collect();
    for n in node.children().filter(|n| n.kind() == k::SECTION_HEADER) {
        parts.push(cx.format(&n)?);
    }
    if !settings.is_empty() {
        parts.push(doc::hardline());
        parts.push(statement_sequence_with(
            cx,
            &settings,
            cx.options.max_blank_lines(),
            None,
            false,
        )?);
    }
    Ok(doc::concat(parts))
}

/// `key = value`; a setting without a value is `key =`, a bare key stays as it is. Continuation
/// lines stay on their own lines with their indentation as written.
fn setting(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let space = cx.options.bool(SPACE_AROUND_EQUALS);
    let max_blank = cx.options.max_blank_lines();
    let mut parts = Vec::new();
    // Line breaks and the indentation seen since the last token of text, for the own-line parts.
    let (mut breaks, mut indent) = (0usize, String::new());
    let mut started = false;
    for t in node.tokens() {
        match t.kind() {
            k::NEWLINE => {
                breaks += 1;
                indent.clear();
            }
            k::WHITESPACE => indent = t.text().to_string(),
            // Comments in front of the key are the key's own (`cx.token`).
            k::CONTINUATION | k::COMMENT if started => {
                // Blank lines between continuation lines are part of the value: kept, capped.
                for _ in 0..=breaks.saturating_sub(1).min(max_blank) {
                    parts.push(doc::hardline());
                }
                // A continuation's token holds its indentation; a comment's is the blanks before it.
                if t.kind() == k::COMMENT {
                    parts.push(doc::text(indent.as_str()));
                }
                parts.push(doc::text(t.text().trim_end()));
                breaks = 0;
            }
            kind if cx.grammar.is_trivia(kind) => {}
            kind => {
                started |= kind == k::KEY;
                if space && kind != k::KEY {
                    parts.push(doc::lit(" "));
                }
                // The parser's value token keeps the blanks around it.
                parts.push(if kind == k::VALUE {
                    doc::text(t.text().trim())
                } else {
                    cx.token(&t)
                });
            }
        }
    }
    doc::concat(parts)
}
