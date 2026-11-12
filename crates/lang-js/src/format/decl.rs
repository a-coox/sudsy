//! Variable declarations, functions and parameters, imports and exports.

use super::arrow::Flags;
use super::util::{
    R, braces, elem, inline, inline_split, kids, node_of, remove_lines, semi, shape_error,
    token_of, will_break,
};
use crate::generated::kind as k;
use core_format::doc::{self, Doc, concat, group, indent, line, lit, softline};
use core_format::{FormatCtx, FormatError, comments};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::VARIABLE_DECLARATION => variable_declaration(node, cx)?,
        k::FORMAL_PARAMETER | k::EXPORT_DECLARATION => inline(cx, &kids(cx, node))?,
        k::FUNCTION => {
            super::arrow::print_fn(cx, node, 0)?.ok_or_else(|| shape_error("function"))?
        }
        k::IMPORT_DECLARATION | k::EXPORT_NAMED_DECLARATION | k::EXPORT_FROM_DECLARATION => {
            module_declaration(node, cx)?
        }
        k::EXPORT_ALL_DECLARATION => concat(vec![inline(cx, &kids(cx, node))?, semi(cx, node)]),
        k::EXPORT_DEFAULT_DECLARATION => export_default(node, cx)?,
        k::WITH_CLAUSE => with_clause(node, cx)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// `const a = 1, b = 2;`: one declarator per line when any has a value.
fn variable_declaration(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let is_declarator =
        |e: &SyntaxElement| node_of(e).is_some_and(|n| n.kind() == k::VARIABLE_DECLARATOR);
    let first = kids
        .iter()
        .position(is_declarator)
        .ok_or_else(|| shape_error("variable declaration"))?;
    let in_for_head = node.parent().is_some_and(|p| {
        matches!(
            p.kind(),
            k::FOR_STATEMENT | k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT
        )
    }) && !kids
        .iter()
        .any(|e| token_of(e).is_some_and(|t| t.kind() == k::SEMICOLON));
    let asi = if in_for_head {
        None
    } else {
        super::util::detach_asi(cx, node)
    };
    let mut has_value = false;
    let mut declarators = Vec::new();
    let mut comma = None;
    for e in &kids[first..] {
        match e {
            SyntaxElement::Node(n) if n.kind() == k::VARIABLE_DECLARATOR => {
                has_value |= n
                    .children_with_tokens()
                    .any(|c| matches!(&c, SyntaxElement::Token(t) if t.text() == "="));
                declarators.push((comma.take(), cx.format(n)?));
            }
            SyntaxElement::Token(t) if t.kind() == k::COMMA => comma = Some(cx.token(t)),
            _ => {}
        }
    }
    let mut declarators = declarators.into_iter();
    let (_, head) = declarators
        .next()
        .ok_or_else(|| shape_error("variable declaration"))?;
    let sep = if has_value && !in_for_head {
        doc::hardline()
    } else {
        line()
    };
    let rest: Vec<Doc> = declarators
        .map(|(comma, d)| concat(vec![comma.unwrap_or_else(|| lit(",")), sep.clone(), d]))
        .collect();
    let (lead, prefix) = inline_split(cx, &kids[..first])?;
    let mut parts = vec![prefix, lit(" "), head];
    if !rest.is_empty() {
        parts.push(indent(concat(rest)));
    }
    let declaration = concat(vec![lead, group(concat(parts))]);
    Ok(if in_for_head {
        declaration
    } else {
        concat(vec![
            declaration,
            semi(cx, node),
            super::util::reattach_asi(cx, asi),
        ])
    })
}

/// `async function* name<T>(params): Type { body }`; a method's function has only the part
/// from its type parameters on, an overload ends in `;`.
pub(crate) fn function(
    node: &SyntaxNode,
    cx: &FormatCtx<'_>,
    flags: u32,
) -> Result<Option<Doc>, FormatError> {
    let kids = kids(cx, node);
    let prefix_end = kids
        .iter()
        .position(|e| node_of(e).is_some() || super::util::is_tok(e, "("))
        .unwrap_or(kids.len());
    let mut parts = Vec::new();
    for (i, e) in kids[..prefix_end].iter().enumerate() {
        // `function*`: the star sticks to the keyword.
        if i > 0 && !super::util::is_tok(e, "*") {
            parts.push(lit(" "));
        }
        parts.push(elem(cx, e)?);
    }
    if kids[..prefix_end]
        .iter()
        .any(|e| super::util::is_tok(e, "function"))
    {
        parts.push(lit(" "));
    }
    let mut at = prefix_end;
    if let Some(id) = kids.get(at).and_then(node_of)
        && id.kind() == k::BINDING_IDENTIFIER
    {
        parts.push(cx.format(id)?);
        at += 1;
    }
    let mut signature = Vec::new();
    if let Some(tp) = kids.get(at).and_then(node_of)
        && tp.kind() == k::TS_TYPE_PARAMETER_DECLARATION
    {
        signature.push(cx.format(tp)?);
        at += 1;
    }
    let object_return = kids.iter().filter_map(node_of).any(|n| {
        n.kind() == k::TS_TYPE_ANNOTATION
            && n.children()
                .next()
                .is_some_and(|t| matches!(t.kind(), k::TS_TYPE_LITERAL | k::TS_MAPPED_TYPE))
    });
    let hugged = flags & (Flags::EXPAND | Flags::TEST) != 0;
    // A function hugged as a call argument keeps its parameters on one line.
    let flatten = |params: Doc| -> Option<Doc> {
        if !hugged {
            return Some(params);
        }
        if flags & Flags::EXPAND != 0 && will_break(&params) {
            return None;
        }
        Some(remove_lines(&params))
    };
    let params = match kids.get(at) {
        Some(SyntaxElement::Node(fp)) if fp.kind() == k::FORMAL_PARAMETERS => {
            at += 1;
            match flatten(parameters_in(
                cx,
                &super::util::kids(cx, fp),
                Some(object_return),
            )?) {
                Some(p) => p,
                None => return Ok(None),
            }
        }
        // oxc leaves the parameters of a function with a `this` parameter without their node.
        Some(e) if super::util::is_tok(e, "(") => {
            let close = kids[at..]
                .iter()
                .position(|e| super::util::is_tok(e, ")"))
                .ok_or_else(|| shape_error("parameters"))?;
            let params = parameters_in(cx, &kids[at..=at + close], Some(object_return))?;
            at += close + 1;
            match flatten(params) {
                Some(p) => p,
                None => return Ok(None),
            }
        }
        _ => return Err(shape_error("function without parameters")),
    };
    signature.push(params);
    let mut body = None;
    for e in &kids[at..] {
        match e {
            SyntaxElement::Node(n) if n.kind() == k::FUNCTION_BODY => body = Some(cx.format(n)?),
            SyntaxElement::Token(t) if t.kind() == k::SEMICOLON => {}
            e => signature.push(elem(cx, e)?),
        }
    }
    parts.push(group(concat(signature)));
    match body {
        Some(body) => parts.extend([lit(" "), body]),
        None => parts.push(semi(cx, node)),
    }
    Ok(Some(concat(parts)))
}

/// A lone parameter that hugs the parentheses: a destructuring pattern, or a name with an object
/// type (`(options: { a: A }) => ..`).
fn hugs_parentheses(param: &SyntaxNode) -> bool {
    match (param.kind(), param.children().next()) {
        (k::FORMAL_PARAMETER, Some(c))
            if matches!(c.kind(), k::OBJECT_PATTERN | k::ARRAY_PATTERN) =>
        {
            super::object::hugs_only_param(param)
        }
        (k::FORMAL_PARAMETER | k::TS_THIS_PARAMETER, _) => param.children().any(|c| {
            c.kind() == k::TS_TYPE_ANNOTATION
                && c.children()
                    .next()
                    .is_some_and(|t| matches!(t.kind(), k::TS_TYPE_LITERAL | k::TS_MAPPED_TYPE))
        }),
        _ => false,
    }
}

/// `(a, b)`, one per line with a trailing comma when too long; a lone object pattern hugs the
/// parentheses. `elems` runs from `(` to `)`.
pub(crate) fn parameters(cx: &FormatCtx<'_>, elems: &[SyntaxElement]) -> R {
    parameters_in(cx, elems, None)
}

/// [`parameters`] for a signature that groups the parameters with its return type, as Prettier
/// does: `Some(object_return)` leaves them ungrouped, but for a lone parameter of a function
/// that returns an object type.
pub(crate) fn parameters_in(
    cx: &FormatCtx<'_>,
    elems: &[SyntaxElement],
    ungrouped: Option<bool>,
) -> R {
    let (Some(open), Some(close)) = (
        elems.first().and_then(token_of),
        elems.last().and_then(token_of),
    ) else {
        return Err(shape_error("parameters"));
    };
    // The comments of each comma stay after it.
    let mut items: Vec<&SyntaxNode> = Vec::new();
    let mut commas: Vec<Doc> = Vec::new();
    for e in &elems[1..elems.len() - 1] {
        match e {
            SyntaxElement::Node(n) => {
                items.push(n);
                commas.push(Doc::Nil);
            }
            SyntaxElement::Token(t) if t.text() == "," => {
                if let Some(c) = commas.last_mut() {
                    *c = concat(vec![
                        cx.leading_comments_doc(t),
                        cx.trailing_comments_doc(t),
                    ]);
                }
            }
            SyntaxElement::Token(_) => {}
        }
    }
    let hugs = matches!(&items[..], [only] if hugs_parentheses(only));
    let ends_in_rest = items
        .last()
        .is_some_and(|n| n.kind() == k::FORMAL_PARAMETER_REST);
    let docs = items
        .iter()
        .map(|n| cx.format(n))
        .collect::<Result<Vec<_>, _>>()?;
    if docs.is_empty() {
        return Ok(super::util::empty_pair(cx, open, close));
    }
    if hugs {
        return Ok(concat(vec![
            cx.token(open),
            concat(docs),
            concat(commas),
            cx.token(close),
        ]));
    }
    // Several parameters, one a parameter property (`private a: A`): one per line.
    let property = items.len() > 1
        && items.iter().any(|p| {
            p.children_with_tokens().any(|e| {
                matches!(&e, SyntaxElement::Token(t) if matches!(t.text(),
                "public" | "private" | "protected" | "readonly" | "override"))
            })
        });
    let grouped = ungrouped.is_none_or(|object_return| object_return && items.len() == 1);
    let last = docs.len() - 1;
    let mut list = Vec::new();
    for (i, (d, c)) in docs.into_iter().zip(commas).enumerate() {
        list.push(d);
        if i < last {
            list.extend([lit(","), c, line()]);
        } else {
            if !ends_in_rest {
                list.push(super::util::trailing_comma(cx, true));
            }
            list.push(c);
        }
    }
    let body = concat(vec![
        if property {
            doc::break_parent()
        } else {
            Doc::Nil
        },
        cx.token(open),
        indent(concat(vec![softline(), concat(list)])),
        softline(),
        cx.token(close),
    ]);
    Ok(if grouped { group(body) } else { body })
}

/// `import a, { b } from "x" with { type: "json" };`, `export { a } from "x";`, `export * ...`.
fn module_declaration(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let find = |kind| {
        kids.iter()
            .position(|e| token_of(e).is_some_and(|t| t.kind() == kind))
    };
    let (Some(open), Some(close)) = (find(k::L_BRACE), find(k::R_BRACE)) else {
        return Ok(concat(vec![inline(cx, &kids)?, semi(cx, node)]));
    };
    // A blank line in front of a specifier stays when the list breaks.
    let mut specifiers = Vec::new();
    for e in &kids[open + 1..close] {
        // The comments of a comma stay after the specifier before it.
        if let SyntaxElement::Token(t) = e {
            if let Some(prev) = specifiers.last_mut() {
                let with = concat(vec![
                    std::mem::replace(prev, Doc::Nil),
                    cx.leading_comments_doc(t),
                    cx.trailing_comments_doc(t),
                ]);
                *prev = with;
            }
            continue;
        }
        let Some(n) = node_of(e) else { continue };
        let blank = !specifiers.is_empty()
            && cx
                .first_token(n)
                .is_some_and(|t| comments::blank_lines_before(cx.grammar, &t) > 0);
        let printed = cx.format(n)?;
        specifiers.push(if blank {
            concat(vec![softline(), printed])
        } else {
            printed
        });
    }
    // A default or namespace import in front of the braces keeps the braces from breaking alone.
    let standalone = kids[..open].iter().any(|e| node_of(e).is_some());
    let can_break = specifiers.len() > 1 || standalone;
    let (Some(o), Some(c)) = (token_of(&kids[open]), token_of(&kids[close])) else {
        return Err(shape_error("module declaration"));
    };
    let mut parts = vec![
        inline(cx, &kids[..open])?,
        lit(" "),
        braces(cx, o, specifiers, c, can_break),
    ];
    if kids[close + 1..]
        .iter()
        .any(|e| !super::util::is_tok(e, ";"))
    {
        parts.push(lit(" "));
        parts.push(inline(cx, &kids[close + 1..])?);
    }
    parts.push(semi(cx, node));
    Ok(concat(parts))
}

/// `export default` and an expression, which ends in `;`, or a function or class, which does not.
fn export_default(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let declaration = kids.iter().rev().find_map(node_of);
    let ends = declaration.is_some_and(|d| {
        matches!(
            d.kind(),
            k::FUNCTION | k::CLASS | k::TS_INTERFACE_DECLARATION
        )
    });
    Ok(concat(vec![
        inline(cx, &kids)?,
        if ends { Doc::Nil } else { semi(cx, node) },
    ]))
}

/// `{ type: "json" }` of an import attribute list.
fn with_clause(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let (Some(open), Some(close)) = (
        kids.first().and_then(token_of),
        kids.last().and_then(token_of),
    ) else {
        return Err(shape_error("import attributes"));
    };
    let attributes = kids
        .iter()
        .filter_map(node_of)
        .map(|n| cx.format(n))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(braces(cx, open, attributes, close, false))
}
