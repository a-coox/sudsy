//! Classes and their members.

use super::util::{R, elem, inline, kids, node_of, semi, shape_error, style, token_of};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, hardline, lit};
use core_format::shapes::{self, BlockStyle};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::CLASS => class(node, cx)?,
        k::CLASS_BODY => shapes::block(cx, node, &BlockStyle::default())?,
        k::METHOD_DEFINITION => inline(cx, &kids(cx, node))?,
        k::PROPERTY_DEFINITION | k::ACCESSOR_PROPERTY => property(node, cx)?,
        k::STATIC_BLOCK => static_block(node, cx)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// `class A<T> extends B implements C { members }`, each decorator on its own line.
fn class(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let decorators = kids
        .iter()
        .take_while(|e| node_of(e).is_some_and(|n| n.kind() == k::DECORATOR))
        .count();
    let mut parts = Vec::new();
    for e in &kids[..decorators] {
        parts.extend([elem(cx, e)?, hardline()]);
    }
    let rest = &kids[decorators..];
    let body = rest
        .last()
        .and_then(node_of)
        .filter(|b| b.kind() == k::CLASS_BODY);
    let header = match body {
        Some(b) => {
            let printed = cx.format(b)?;
            super::ts::class_header(cx, rest, &printed, b.children().next().is_none())?
        }
        None => None,
    };
    parts.push(match header {
        Some(doc) => doc,
        None => inline(cx, rest)?,
    });
    Ok(concat(parts))
}

/// `static readonly x?: T = value;`.
fn property(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let mut parts = vec![super::assign::property_value(cx, node, &kids)?];
    if !style(cx).semi && needs_semicolon(node, cx) {
        parts.push(lit(";"));
    } else {
        parts.push(semi(cx, node));
    }
    Ok(concat(parts))
}

/// Without semicolons, a property keeps its `;` when the next member would continue it: one that
/// starts with `[` or `*`, or is named `in` or `instanceof`; or when it is a bare `static`, `get`
/// or `set`.
fn needs_semicolon(node: &SyntaxNode, cx: &FormatCtx<'_>) -> bool {
    let next = node
        .next_sibling()
        .and_then(|n| cx.first_token(&n))
        .is_some_and(|t| matches!(t.text(), "[" | "*" | "in" | "instanceof"));
    let bare = !node.children_with_tokens().any(|e| {
        matches!(&e, core_syntax::SyntaxElement::Token(t) if t.text() == "=")
            || matches!(&e, core_syntax::SyntaxElement::Node(n) if n.kind() == k::TS_TYPE_ANNOTATION)
    }) && node
        .children()
        .find(|n| n.kind() != k::DECORATOR)
        .and_then(|key| cx.first_token(&key))
        .is_some_and(|t| matches!(t.text(), "static" | "get" | "set"));
    next || bare
}

/// `static { statements }`.
fn static_block(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let (Some(kw), Some(open), Some(close)) = (
        kids.first(),
        kids.get(1).and_then(token_of),
        kids.last().and_then(token_of),
    ) else {
        return Err(shape_error("static block"));
    };
    let body = shapes::block_between(cx, node, open, close, &BlockStyle::default())?;
    Ok(concat(vec![elem(cx, kw)?, lit(" "), body]))
}
