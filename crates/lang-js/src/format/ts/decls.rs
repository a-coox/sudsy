//! Interfaces, type aliases, enums, namespaces and modules, signatures, and the TypeScript-only
//! statements.

use super::words;
use crate::format::util::{R, elem, is_tok, kids, node_of, semi, shape_error};
use crate::generated::kind as k;
use core_format::doc::{
    self, Doc, concat, group, group_with_id, indent, indent_if_break, line, line_suffix_boundary,
    lit, new_group_id,
};
use core_format::shapes::{self, BlockStyle, ListLayout, ListStyle, TrailingComma};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(super) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::TS_INTERFACE_DECLARATION => interface(node, cx)?,
        k::TS_TYPE_ALIAS_DECLARATION => type_alias(node, cx)?,
        k::TS_INTERFACE_BODY => members(node, cx, ListLayout::Break)?,
        k::TS_TYPE_LITERAL => members(node, cx, ListLayout::PreserveOpenBreak)?,
        k::TS_ENUM_BODY => {
            let style = ListStyle {
                padding: false,
                trailing_comma: TrailingComma::WhenBroken,
                layout: ListLayout::Break,
                ..ListStyle::default()
            };
            shapes::delimited_list(cx, node, &style)?
        }
        k::TS_INDEX_SIGNATURE if node.parent().is_some_and(|p| p.kind() == k::CLASS_BODY) => {
            let no_semi: Vec<_> = kids(cx, node)
                .into_iter()
                .filter(|e| !is_tok(e, ";"))
                .collect();
            concat(vec![words(cx, &no_semi, true)?, semi(cx, node)])
        }
        k::TS_METHOD_SIGNATURE
        | k::TS_CALL_SIGNATURE_DECLARATION
        | k::TS_CONSTRUCT_SIGNATURE_DECLARATION => signature(node, cx)?,
        k::TS_PROPERTY_SIGNATURE
        | k::TS_INDEX_SIGNATURE
        | k::TS_INDEX_SIGNATURE_NAME
        | k::TS_ENUM_MEMBER => words(cx, &kids(cx, node), true)?,
        k::TS_ENUM_DECLARATION | k::TS_GLOBAL_DECLARATION => words(cx, &kids(cx, node), false)?,
        k::TS_NAMESPACE_DECLARATION => words(cx, &kids(cx, node), true)?,
        k::TS_EXTERNAL_MODULE_DECLARATION => {
            let body = node.children().any(|c| c.kind() == k::TS_MODULE_BLOCK);
            let head = words(cx, &kids(cx, node), false)?;
            if body {
                head
            } else {
                concat(vec![head, semi(cx, node)])
            }
        }
        k::TS_MODULE_BLOCK => shapes::block_open_inside(cx, node, &BlockStyle::default())?,
        k::TS_IMPORT_EQUALS_DECLARATION
        | k::TS_EXPORT_ASSIGNMENT
        | k::TS_NAMESPACE_EXPORT_DECLARATION => {
            concat(vec![words(cx, &kids(cx, node), false)?, semi(cx, node)])
        }
        k::TS_EXTERNAL_MODULE_REFERENCE => super::glued(cx, node)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// `name<T>(a: A): R`: the type parameters, parameters and return type group; the name does not
/// (a comment in front of it must not break the parameters).
fn signature(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let ks = kids(cx, node);
    let at = ks
        .iter()
        .position(|e| {
            matches!(e, SyntaxElement::Node(n)
                if matches!(n.kind(), k::TS_TYPE_PARAMETER_DECLARATION | k::FORMAL_PARAMETERS))
        })
        .unwrap_or(ks.len());
    let gap = if at > 0 && is_tok(&ks[at - 1], "new") {
        lit(" ")
    } else {
        Doc::Nil
    };
    Ok(concat(vec![
        words(cx, &ks[..at], true)?,
        gap,
        group(words(cx, &ks[at..], true)?),
    ]))
}

/// `{ a: T; b(): U }`: members separated by `;`, one per line when broken.
fn members(node: &SyntaxNode, cx: &FormatCtx<'_>, layout: ListLayout) -> R {
    let style = ListStyle {
        separator: ";",
        broken_separator: Some(if super::super::util::style(cx).semi {
            ";"
        } else {
            ""
        }),
        padding: true,
        trailing_comma: TrailingComma::WhenBroken,
        layout,
        ..ListStyle::default()
    };
    shapes::delimited_list(cx, node, &style)
}

/// `interface A<T> extends B, C { .. }`; a long heritage list goes on the next line, indented.
fn interface(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let ks = kids(cx, node);
    let ext = ks.iter().position(|e| is_tok(e, "extends"));
    let body = ks.last().ok_or_else(|| shape_error("interface"))?;
    let head_end = ext.unwrap_or(ks.len() - 1);
    let mut head = Vec::new();
    for (i, e) in ks[..head_end].iter().enumerate() {
        if i > 0
            && !matches!(e, SyntaxElement::Node(n) if n.kind() == k::TS_TYPE_PARAMETER_DECLARATION)
        {
            head.push(lit(" "));
        }
        head.push(elem(cx, e)?);
    }
    let mut parts = Vec::new();
    match ext {
        None => parts.extend(head),
        Some(at) => {
            let nodes: Vec<&SyntaxNode> = ks[at + 1..]
                .iter()
                .filter_map(node_of)
                .filter(|n| n.kind() == k::TS_INTERFACE_HERITAGE)
                .collect();
            let heritage = nodes
                .iter()
                .map(|n| cx.format(n))
                .collect::<Result<Vec<_>, _>>()?;
            // A lone name, or one with type arguments (which break instead), stays on the line.
            let hugs = nodes.len() == 1
                && nodes[0].children().any(|c| {
                    matches!(
                        c.kind(),
                        k::TS_TYPE_PARAMETER_INSTANTIATION | k::IDENTIFIER_REFERENCE
                    )
                });
            let list = doc::join(&concat(vec![lit(","), line()]), heritage);
            let extends = elem(cx, &ks[at])?;
            let tail = if hugs {
                concat(vec![lit(" "), extends, lit(" "), list])
            } else if nodes.len() == 1 {
                concat(vec![line(), extends, lit(" "), list])
            } else {
                concat(vec![
                    line(),
                    group(concat(vec![extends, indent(concat(vec![line(), list]))])),
                ])
            };
            let has_params = ks[..at].iter().any(|e| matches!(e, SyntaxElement::Node(n) if n.kind() == k::TS_TYPE_PARAMETER_DECLARATION));
            // With type parameters only the heritage is indented, not their list.
            parts.push(if hugs {
                group(concat(vec![concat(head), tail]))
            } else if has_params {
                group(concat(vec![concat(head), indent(tail)]))
            } else {
                group(indent(concat(vec![concat(head), tail])))
            });
        }
    }
    parts.extend([lit(" "), elem(cx, body)?]);
    Ok(group(concat(parts)))
}

/// `type A<T> = B;`: the right side stays on the line unless it cannot fit.
fn type_alias(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let ks = kids(cx, node);
    let eq = ks
        .iter()
        .position(|e| is_tok(e, "="))
        .ok_or_else(|| shape_error("type alias"))?;
    let left = words(cx, &ks[..eq], true)?;
    let right = ks.get(eq + 1).ok_or_else(|| shape_error("type alias"))?;
    let id = new_group_id();
    Ok(concat(vec![
        group(concat(vec![
            left,
            lit(" "),
            elem(cx, &ks[eq])?,
            group_with_id(id, indent(line())),
            line_suffix_boundary(),
            indent_if_break(id, elem(cx, right)?),
        ])),
        semi(cx, node),
    ]))
}
