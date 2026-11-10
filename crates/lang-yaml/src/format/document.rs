//! The stream, its documents, and the `---` and `...` markers.

use super::block::{Ctx, node_after, property_root};
use super::comments::{gap, has_remaining_comments, leading, tok};
use super::{DocumentEnd, DocumentStart, Style, scalars};
use crate::generated::kind as k;
use crate::tree::{blank_lines_after, first_token, is_trivia, prev_significant};
use core_format::comments;
use core_format::{Doc, FormatCtx, FormatError, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken};

type R = Result<Doc, FormatError>;

/// The documents one under the other, then the comments at the end of the file.
pub(crate) fn stream(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let mut out = Vec::new();
    let documents: Vec<SyntaxNode> = node
        .children()
        .filter(|n| n.kind() == k::DOCUMENT)
        .collect();
    let mut last_ended = false;
    for (i, d) in documents.iter().enumerate() {
        if i > 0 {
            let first =
                first_token(d).ok_or_else(|| FormatError::Shape("an empty document".into()))?;
            out.push(gap(cx, st, &first));
        }
        // A directive, or a document without its own `---`, can only follow a `...`.
        let end_needed = documents
            .get(i + 1)
            .is_some_and(|n| n.token(k::DIRECTIVE).is_some() || !has_start_marker(st, n, i + 1));
        let (doc, ended) = document(cx, st, d, i, end_needed)?;
        last_ended = ended;
        out.push(doc);
    }
    if let Some(eof) = node.token(k::EOF) {
        if has_remaining_comments(cx, &eof) {
            if !out.is_empty() {
                out.push(gap(cx, st, &eof));
            }
            out.push(leading(cx, &eof));
        } else if let Some(n) = keep_blank_lines(&eof).filter(|_| !last_ended) {
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

/// Whether the formatted document starts with `---`.
fn has_start_marker(st: &Style, node: &SyntaxNode, index: usize) -> bool {
    let start = node.token(k::DOC_START).is_some();
    let body = node.children().next().is_some();
    let directives = node.token(k::DIRECTIVE).is_some();
    match st.document_start {
        DocumentStart::Preserve => start,
        DocumentStart::Always => start || body || directives,
        // The marker of the first document can go unless it carries a directive or the document
        // would vanish.
        DocumentStart::Never => start && (index > 0 || directives || !body),
    }
}

fn document(
    cx: &FormatCtx<'_>,
    st: &Style,
    node: &SyntaxNode,
    index: usize,
    end_needed: bool,
) -> Result<(Doc, bool), FormatError> {
    let directives: Vec<SyntaxToken> = node
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|t| t.kind() == k::DIRECTIVE)
        .collect();
    let start = node.token(k::DOC_START);
    let end = node.token(k::DOC_END);
    let body = node.children().next();
    let marker = has_start_marker(st, node, index);
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
        // An empty document is nothing but its `...`.
        DocumentEnd::Never => end_needed || (body.is_none() && start.is_none()),
    };
    // The blank lines at the end of a `keep` scalar are part of its value: they stay above `...`.
    let blanks = body.as_ref().and_then(|b| {
        let last = b.last_token()?;
        let last = if is_trivia(last.kind()) {
            prev_significant(&last)?
        } else {
            last
        };
        keep_blanks_after(&last)
    });
    let kept = |n: usize| {
        doc::verbatim(
            "
"
            .repeat(n + 1),
        )
    };
    let ended = keep_end && (end.is_some() || !out.is_empty());
    match (end, keep_end) {
        (Some(e), true) => {
            if !out.is_empty() {
                out.push(blanks.map_or_else(|| gap(cx, st, &e), kept));
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
            out.push(blanks.map_or_else(doc::hardline, kept));
            out.push(doc::lit("..."));
        }
        (None, _) => {}
    }
    Ok((doc::concat(out), ended))
}

/// The blank lines that end the file after a `keep` block scalar.
fn keep_blank_lines(eof: &SyntaxToken) -> Option<usize> {
    keep_blanks_after(&prev_significant(eof)?)
}

/// The blank lines after `last`, the end of a `keep` block scalar.
fn keep_blanks_after(last: &SyntaxToken) -> Option<usize> {
    let scalar = last.parent().filter(|n| n.kind() == k::BLOCK_SCALAR)?;
    first_token(&scalar)
        .is_some_and(|h| h.text().contains('+'))
        .then(|| blank_lines_after(last))
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
