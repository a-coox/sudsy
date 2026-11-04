//! The stream, its documents, and the `---` and `...` markers.

use super::block::{Ctx, node_after, property_root};
use super::comments::{gap, has_remaining_comments, leading, tok};
use super::{DocumentEnd, DocumentStart, Style, scalars};
use crate::generated::kind as k;
use crate::tree::{blank_lines_after, first_token, prev_significant, token_of};
use core_format::comments;
use core_format::{Doc, FormatCtx, FormatError, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

type R = Result<Doc, FormatError>;

/// The documents one under the other, then the comments at the end of the file.
pub(crate) fn stream(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let mut out = Vec::new();
    let documents: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == k::DOCUMENT)
        .collect();
    for (i, d) in documents.iter().enumerate() {
        if i > 0 {
            let first =
                first_token(d).ok_or_else(|| FormatError::Shape("an empty document".into()))?;
            out.push(gap(cx, st, &first));
        }
        // A directive can only follow a `...`.
        let before_directive = documents
            .get(i + 1)
            .is_some_and(|n| token_of(n, k::DIRECTIVE).is_some());
        out.push(document(cx, st, d, i, before_directive)?);
    }
    if let Some(eof) = token_of(node, k::EOF) {
        if has_remaining_comments(cx, &eof) {
            if !out.is_empty() {
                out.push(gap(cx, st, &eof));
            }
            out.push(leading(cx, &eof));
        } else if let Some(n) = keep_blank_lines(&eof) {
            // The blank lines at the end of a `keep` scalar are part of its value.
            out.push(doc::verbatim(
                "
"
                .repeat(n + 1),
            ));
        }
    }
    Ok(doc::concat(out))
}

fn document(
    cx: &FormatCtx<'_>,
    st: &Style,
    node: &SyntaxNode,
    index: usize,
    before_directive: bool,
) -> R {
    let directives: Vec<SyntaxToken> = node
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|t| t.kind() == k::DIRECTIVE)
        .collect();
    let start = token_of(node, k::DOC_START);
    let end = token_of(node, k::DOC_END);
    let body = node.children().next();
    let marker = match st.document_start {
        DocumentStart::Preserve => start.is_some(),
        DocumentStart::Always => start.is_some() || body.is_some() || !directives.is_empty(),
        // The marker of the first document can go unless it carries a directive or the document
        // would vanish.
        DocumentStart::Never => {
            start.is_some() && (index > 0 || !directives.is_empty() || body.is_none())
        }
    };
    let mut out = Vec::new();
    for d in &directives {
        out.push(tok(cx, d));
        out.push(doc::hardline());
    }
    match (&start, marker) {
        (Some(s), true) => {
            out.push(tok(cx, s));
            if let Some(b) = &body {
                out.push(node_after(cx, st, s, b, Ctx::Root, false)?);
            }
        }
        (None, true) => {
            out.push(doc::lit("---"));
            if let Some(b) = &body {
                out.push(doc::hardline());
                out.push(root_body(cx, st, b)?);
            }
        }
        (start, false) => {
            if let Some(s) = start {
                out.push(dropped_marker(cx, s));
            }
            if let Some(b) = &body {
                out.push(root_body(cx, st, b)?);
            }
        }
    }
    let keep_end = match st.document_end {
        DocumentEnd::Preserve => end.is_some(),
        DocumentEnd::Always => true,
        DocumentEnd::Never => before_directive,
    };
    match (end, keep_end) {
        (Some(e), true) => {
            if !out.is_empty() {
                out.push(gap(cx, st, &e));
            }
            out.push(tok(cx, &e));
        }
        (Some(e), false) => {
            // Only its comments remain, below the document.
            if !out.is_empty()
                && (has_remaining_comments(cx, &e)
                    || !comments::trailing_comments(cx.grammar, &e).is_empty())
            {
                out.push(gap(cx, st, &e));
            }
            out.push(dropped_marker(cx, &e));
        }
        (None, true) if !out.is_empty() => {
            out.push(doc::hardline());
            out.push(doc::lit("..."));
        }
        (None, _) => {}
    }
    Ok(doc::concat(out))
}

/// The blank lines that end the file after a `keep` block scalar.
fn keep_blank_lines(eof: &SyntaxToken) -> Option<usize> {
    let last = prev_significant(eof)?;
    let scalar = last.parent().filter(|n| n.kind() == k::BLOCK_SCALAR)?;
    first_token(&scalar)
        .is_some_and(|h| h.text().contains('+'))
        .then(|| blank_lines_after(&last))
        .filter(|n| *n > 0)
}

/// A document body without a marker in front of it.
fn root_body(cx: &FormatCtx<'_>, st: &Style, body: &SyntaxNode) -> R {
    match body.kind() {
        k::BLOCK_SCALAR => scalars::block_scalar(cx, st, body, 1),
        k::PROPERTY_NODE => property_root(cx, st, body),
        _ => cx.format(body),
    }
}

/// The comments of a `---` that is left out stay, each on its own line.
fn dropped_marker(cx: &FormatCtx<'_>, marker: &SyntaxToken) -> Doc {
    let mut out = vec![leading(cx, marker)];
    for c in comments::trailing_comments(cx.grammar, marker) {
        out.push(core_format::context::comment_doc(&c));
        out.push(doc::hardline());
    }
    doc::concat(out)
}
