//! TOML formatting: the shared shapes with TOML's layout. A table is its header and then its pairs
//! one per line, without indentation (there are no braces); an inline table never breaks (TOML 1.0
//! forbids line breaks in it); arrays follow the list shape.

use crate::align::{self, AlignTable};
use crate::generated::kind as k;
use core_config::{ResolvedOptions, shared};
use core_format::shapes::{
    KeyValueStyle, ListLayout, ListStyle, TrailingComma, delimited_list, key_value,
    statement_sequence,
};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct TomlFormatter;

fn array_style(options: &ResolvedOptions) -> ListStyle {
    ListStyle {
        padding: options.bool(shared::ARRAY_BRACKET_SPACING),
        trailing_comma: if options.str(TRAILING_COMMAS) == "multiline" {
            TrailingComma::WhenBroken
        } else {
            TrailingComma::Never
        },
        layout: match options.str(COLLAPSE) {
            "always" => ListLayout::Fit,
            "never" => ListLayout::Break,
            _ => ListLayout::PreserveBreak,
        },
        ..ListStyle::default()
    }
}

impl NodeFormatter for TomlFormatter {
    /// `end_of_line` rewrites the line breaks inside multi-line strings; nothing else may change.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => t.text().replace("\r\n", "\n").into(),
            SyntaxElement::Node(n) => n.text().to_string().into(),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::PAIR => pair(cx, node)?,
            k::ARRAY => delimited_list(cx, node, &array_style(cx.options))?,
            k::INLINE_TABLE => delimited_list(
                cx,
                node,
                &ListStyle {
                    padding: cx.options.bool(shared::BRACKET_SPACING),
                    layout: ListLayout::Flat,
                    ..ListStyle::default()
                },
            )?,
            k::TABLE | k::ARRAY_TABLE => table(cx, node)?,
            k::DOCUMENT => document(cx, node)?,
            _ => return Ok(None),
        };
        Ok(Some(doc))
    }
}

/// `key = value`, with its trailing comment padded to the column of its run.
fn pair(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut d = key_value(
        cx,
        node,
        &KeyValueStyle {
            space_before_separator: true,
            ..KeyValueStyle::default()
        },
    )?;
    if cx.options.bool(ALIGN_COMMENTS) {
        let extra = cx.cached(AlignTable::default).extra(node);
        if extra > 0 {
            align::pad_comment(&mut d, extra);
        }
    }
    Ok(d)
}

/// `[header]` then its pairs, one per line.
fn table(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if !cx.grammar.is_trivia(t.kind()) => parts.push(cx.token(&t)),
            SyntaxElement::Node(n) if n.kind() == k::KEY => parts.push(cx.format(&n)?),
            _ => {}
        }
    }
    let pairs: Vec<SyntaxNode> = node.children().filter(|n| n.kind() == k::PAIR).collect();
    if !pairs.is_empty() {
        parts.push(doc::hardline());
        parts.push(statement_sequence(
            cx,
            &pairs,
            cx.options.max_blank_lines(),
        )?);
    }
    Ok(doc::concat(parts))
}

/// The items, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    if cx.options.bool(ALIGN_COMMENTS) {
        cx.cached(|| AlignTable::build(cx, node));
    }
    core_format::shapes::document(cx, node, cx.options.max_blank_lines(), None)
}
