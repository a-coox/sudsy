//! Block collections and their entries, flow collections, node properties, and where a value
//! goes after the indicator in front of it (`key:`, `-`, `?`, `---`).

use super::comments::{
    blank_lines_before, breaks, dangling, gap, has_leading_comments, has_trailing_comments, tok,
};
use super::{SequenceIndent, Style, scalars};
use crate::generated::kind as k;
use crate::tree::{column, first_token, is_trivia, last_token, line_break_between};
use core_format::comments;
use core_format::ignore::{self, IgnoreMark};
use core_format::shapes::{ListLayout, ListStyle, TrailingComma, delimited_list};
use core_format::{Doc, FormatCtx, FormatError, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

type R = Result<Doc, FormatError>;

fn shape(what: &str) -> FormatError {
    FormatError::Shape(what.into())
}

/// The pieces of a mapping entry: `? key : value`, each part optional.
struct Parts {
    question: Option<SyntaxToken>,
    key: Option<SyntaxNode>,
    colon: Option<SyntaxToken>,
    value: Option<SyntaxNode>,
}

fn parts(node: &SyntaxNode) -> Parts {
    let mut p = Parts {
        question: None,
        key: None,
        colon: None,
        value: None,
    };
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::QUESTION => p.question = Some(t),
            SyntaxElement::Token(t) if t.kind() == k::COLON => p.colon = Some(t),
            SyntaxElement::Token(_) => {}
            SyntaxElement::Node(n) if p.colon.is_some() => p.value = Some(n),
            SyntaxElement::Node(n) => p.key = Some(n),
        }
    }
    p
}

/// Where a value sits relative to the indicator before it.
#[derive(Clone, Copy)]
pub(crate) enum Ctx {
    /// After `key:` in a block mapping whose entries start at `key_col`.
    Mapping { key_col: usize },
    /// After `-`, `?` or an explicit `:`, where a collection may start on the same line.
    Compact,
    /// After `---` (or nothing) at the document level.
    Root,
}

/// Entries one under the other with the blank lines of the source, then the comments that
/// belong at the end of the collection. Entries between `sudsy-ignore-start` and
/// `sudsy-ignore-end` comments are kept as written.
pub(crate) fn collection(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let g = cx.grammar;
    let entries: Vec<SyntaxNode> = node.children().collect();
    let mark = |entry: &SyntaxNode| {
        // Only a comment can carry a mark.
        if !comments::node_leading(g, entry).has_comment {
            return None;
        }
        let first = first_token(entry)?;
        ignore::mark_in(g, &comments::leading_comments(g, &first))
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < entries.len() {
        let first = first_token(&entries[i]).ok_or_else(|| shape("an empty entry"))?;
        if i > 0 {
            out.push(gap(cx, st, &first));
        }
        if mark(&entries[i]) == Some(IgnoreMark::Start) {
            let mut j = i + 1;
            while j < entries.len() && mark(&entries[j]) != Some(IgnoreMark::End) {
                j += 1;
            }
            let last = last_token(&entries[j - 1]).ok_or_else(|| shape("an empty entry"))?;
            out.push(cx.verbatim_range(&first, &last));
            i = j;
        } else {
            out.push(cx.format(&entries[i])?);
            i += 1;
        }
    }
    out.push(dangling(cx, st, node));
    Ok(doc::concat(out))
}

pub(crate) fn sequence_entry(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let dash = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::DASH)
        .ok_or_else(|| shape("a sequence entry without `-`"))?;
    let mut out = vec![tok(cx, &dash)];
    if let Some(value) = node.children().next() {
        out.push(node_after(cx, st, &dash, &value, Ctx::Compact, false)?);
    }
    Ok(doc::concat(out))
}

pub(crate) fn mapping_entry(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let p = parts(node);
    if let Some(question) = &p.question {
        return explicit_entry(cx, st, &p, question);
    }
    let colon = p
        .colon
        .as_ref()
        .ok_or_else(|| shape("a mapping entry without `:`"))?;
    let key_col = first_token(node).map_or(0, |t| column(&t));
    let mut out = Vec::new();
    if let Some(key) = &p.key {
        out.push(key_doc(cx, key)?);
        if space_before_colon(key) {
            out.push(doc::lit(" "));
        }
    }
    out.push(tok(cx, colon));
    if let Some(value) = &p.value {
        out.push(node_after(
            cx,
            st,
            colon,
            value,
            Ctx::Mapping { key_col },
            false,
        )?);
    }
    Ok(doc::concat(out))
}

/// `? key` and, on the next line, `: value`.
fn explicit_entry(cx: &FormatCtx<'_>, st: &Style, p: &Parts, question: &SyntaxToken) -> R {
    let mut out = vec![tok(cx, question)];
    if let Some(key) = &p.key {
        out.push(node_after(cx, st, question, key, Ctx::Compact, false)?);
    }
    if let Some(colon) = &p.colon {
        out.push(doc::hardline());
        out.push(tok(cx, colon));
        if let Some(value) = &p.value {
            out.push(node_after(cx, st, colon, value, Ctx::Compact, false)?);
        }
    }
    Ok(doc::concat(out))
}

/// A key is one line. A flow collection as a key is printed as written so that it never breaks.
fn key_doc(cx: &FormatCtx<'_>, key: &SyntaxNode) -> R {
    let is_flow = |n: &SyntaxNode| matches!(n.kind(), k::FLOW_MAPPING | k::FLOW_SEQUENCE);
    if is_flow(key) || key.kind() == k::PROPERTY_NODE && key.children().any(|c| is_flow(&c)) {
        Ok(cx.verbatim_node(key))
    } else {
        cx.format(key)
    }
}

/// Aliases and empty nodes with properties would swallow the `:` into their name.
fn space_before_colon(key: &SyntaxNode) -> bool {
    key.kind() == k::ALIAS_NODE || key.kind() == k::PROPERTY_NODE && key.children().next().is_none()
}

// ---- values after an indicator --------------------------------------------------------------

/// Puts `doc` on the line after the indicator, one level in.
fn next_line(ctx: Ctx, doc: Doc) -> Doc {
    match ctx {
        Ctx::Mapping { .. } => doc::indent(doc::concat(vec![doc::hardline(), doc])),
        Ctx::Compact => doc::align(2, doc::concat(vec![doc::hardline(), doc])),
        Ctx::Root => doc::concat(vec![doc::hardline(), doc]),
    }
}

/// How far `next_line` moves what follows, and the extra indentation of a document-level block
/// scalar (whose parent is at -1).
fn block_scalar_extra(ctx: Ctx, st: &Style, moved: bool) -> usize {
    match (ctx, moved) {
        (Ctx::Mapping { .. }, true) => st.indent,
        (Ctx::Compact, true) => 2,
        (Ctx::Root, false) => 1,
        (Ctx::Root, true) | (Ctx::Mapping { .. }, false) | (Ctx::Compact, false) => 0,
    }
}

/// The layout of `value` after `indicator`, starting right after the indicator's own text: a
/// space and the value, or a line break and the value one level in. `after_props` says the line
/// already holds properties of this node, so a block collection cannot start on it.
pub(crate) fn node_after(
    cx: &FormatCtx<'_>,
    st: &Style,
    indicator: &SyntaxToken,
    value: &SyntaxNode,
    ctx: Ctx,
    after_props: bool,
) -> R {
    let first = first_token(value).ok_or_else(|| shape("an empty value"))?;
    // A comment after the indicator, or in front of the value, keeps the value on its own line.
    let force = has_trailing_comments(cx, indicator) || has_leading_comments(cx, &first);
    match value.kind() {
        k::PROPERTY_NODE => property_after(cx, st, indicator, value, ctx),
        k::BLOCK_MAPPING | k::BLOCK_SEQUENCE => {
            let body = cx.format(value)?;
            Ok(match ctx {
                Ctx::Mapping { key_col } => {
                    let lead = breaks(1 + blank_lines_before(cx, &first, st));
                    let flush = value.kind() == k::BLOCK_SEQUENCE
                        && match st.sequence {
                            SequenceIndent::Flush => true,
                            SequenceIndent::Indented => false,
                            SequenceIndent::Preserve => column(&first) == key_col,
                        };
                    let inner = doc::concat(vec![lead, body]);
                    if flush { inner } else { doc::indent(inner) }
                }
                Ctx::Compact => {
                    // Only a comment in front of the first entry, or properties on the line,
                    // keep a collection from starting on the indicator's line.
                    if force || after_props {
                        next_line(ctx, body)
                    } else {
                        doc::concat(vec![doc::lit(" "), doc::align(2, body)])
                    }
                }
                Ctx::Root => {
                    doc::concat(vec![breaks(1 + blank_lines_before(cx, &first, st)), body])
                }
            })
        }
        k::BLOCK_SCALAR => {
            let next = force || matches!(ctx, Ctx::Root) && line_break_between(indicator, &first);
            let body = scalars::block_scalar(cx, st, value, block_scalar_extra(ctx, st, next))?;
            Ok(if next {
                next_line(ctx, body)
            } else {
                doc::concat(vec![doc::lit(" "), body])
            })
        }
        kind => {
            let body = cx.format(value)?;
            let flow = matches!(kind, k::FLOW_MAPPING | k::FLOW_SEQUENCE);
            if force {
                return Ok(next_line(ctx, body));
            }
            Ok(match ctx {
                // A flow collection that does not fit moves to its own line, indented.
                Ctx::Mapping { .. } if flow => {
                    doc::group(doc::indent(doc::concat(vec![doc::line(), body])))
                }
                Ctx::Compact if flow => doc::concat(vec![doc::lit(" "), doc::align(2, body)]),
                // A scalar that was on its own line stays there.
                Ctx::Mapping { .. } | Ctx::Root if line_break_between(indicator, &first) => {
                    next_line(ctx, body)
                }
                _ => doc::concat(vec![doc::lit(" "), body]),
            })
        }
    }
}

// ---- node properties -------------------------------------------------------------------------

fn properties(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !is_trivia(t.kind()))
        .collect()
}

fn property_head(cx: &FormatCtx<'_>, props: &[SyntaxToken]) -> Doc {
    let docs: Vec<Doc> = props.iter().map(|p| tok(cx, p)).collect();
    doc::join(&doc::lit(" "), docs)
}

/// Properties that a layout can handle: comments between them need the node as written.
fn props_are_simple(cx: &FormatCtx<'_>, props: &[SyntaxToken]) -> bool {
    props.iter().enumerate().all(|(i, p)| {
        (i == 0 || !has_leading_comments(cx, p))
            && (i + 1 == props.len() || !has_trailing_comments(cx, p))
    })
}

/// Properties after an indicator, then their content.
fn property_after(
    cx: &FormatCtx<'_>,
    st: &Style,
    indicator: &SyntaxToken,
    node: &SyntaxNode,
    ctx: Ctx,
) -> R {
    let props = properties(node);
    let content = node.children().next();
    if props.is_empty() {
        return Err(shape("a property node without properties"));
    }
    // Properties that were on their own line after `---` stay there.
    let own_line = matches!(ctx, Ctx::Root) && line_break_between(indicator, &props[0]);
    let force =
        has_trailing_comments(cx, indicator) || has_leading_comments(cx, &props[0]) || own_line;
    let block_scalar = content
        .as_ref()
        .is_some_and(|c| c.kind() == k::BLOCK_SCALAR);
    if !props_are_simple(cx, &props) || force && block_scalar {
        return Ok(doc::concat(vec![doc::lit(" "), cx.verbatim_node(node)]));
    }
    let mut out = vec![property_head(cx, &props)];
    if let Some(c) = &content {
        let last = props.last().ok_or_else(|| shape("no properties"))?;
        out.push(node_after(cx, st, last, c, ctx, true)?);
    }
    let body = doc::concat(out);
    Ok(if force {
        next_line(ctx, body)
    } else {
        doc::concat(vec![doc::lit(" "), body])
    })
}

/// A document-level property node without a `---` in front of it.
pub(crate) fn property_root(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let props = properties(node);
    let Some(last) = props.last() else {
        return Err(shape("a property node without properties"));
    };
    if !props_are_simple(cx, &props) {
        return Ok(cx.verbatim_node(node));
    }
    let mut out = vec![property_head(cx, &props)];
    if let Some(c) = node.children().next() {
        out.push(node_after(cx, st, last, &c, Ctx::Root, true)?);
    }
    Ok(doc::concat(out))
}

/// Properties and the inline node they belong to (keys, flow collections).
pub(crate) fn property_inline(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let props = properties(node);
    let mut out = vec![property_head(cx, &props)];
    if let Some(c) = node.children().next() {
        out.push(doc::lit(" "));
        out.push(cx.format(&c)?);
    }
    Ok(doc::concat(out))
}

// ---- flow collections -------------------------------------------------------------------------

/// Flow collections stay on one line if they were on one line and fit; otherwise one item per
/// line.
pub(crate) fn flow_collection(cx: &FormatCtx<'_>, st: &Style, node: &SyntaxNode) -> R {
    let style = ListStyle {
        separator: ",",
        padding: node.kind() == k::FLOW_MAPPING,
        trailing_comma: TrailingComma::Never,
        layout: ListLayout::PreserveBreak,
        max_blank_lines: st.max_blank,
    };
    delimited_list(cx, node, &style)
}

/// `key: value` in `{}` or a single pair in `[]`; `? key : value` when written that way.
pub(crate) fn flow_pair(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let p = parts(node);
    let mut out = Vec::new();
    let value_doc = |cx: &FormatCtx<'_>, v: &SyntaxNode| -> R {
        Ok(doc::concat(vec![doc::lit(" "), cx.format(v)?]))
    };
    if let Some(q) = &p.question {
        out.push(tok(cx, q));
        if let Some(key) = &p.key {
            out.push(doc::lit(" "));
            out.push(cx.format(key)?);
        }
        if let Some(colon) = &p.colon {
            out.push(doc::lit(" "));
            out.push(tok(cx, colon));
            if let Some(v) = &p.value {
                out.push(value_doc(cx, v)?);
            }
        }
        return Ok(doc::concat(out));
    }
    if let Some(key) = &p.key {
        out.push(key_doc(cx, key)?);
        if p.colon.is_some() && space_before_colon(key) {
            out.push(doc::lit(" "));
        }
    }
    if let Some(colon) = &p.colon {
        out.push(tok(cx, colon));
        if let Some(v) = &p.value {
            out.push(value_doc(cx, v)?);
        }
    }
    Ok(doc::concat(out))
}
