//! Type nodes: annotations, references, type parameters and arguments, unions, intersections,
//! function, conditional, mapped and tuple types, and the type operators.

use super::{glued, words};
use crate::format::decl;
use crate::format::util::{R, elem, is_tok, kids, shape_error, token_of};
use crate::generated::kind as k;
use core_format::doc::{
    self, Doc, GroupId, align, concat, group, group_with_id, if_break, indent, line, lit, softline,
};
use core_format::shapes::{self, ListStyle, TrailingComma};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

pub(super) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::TS_TYPE_ANNOTATION => annotation(node, cx)?,
        k::TS_TYPE_PARAMETER_INSTANTIATION | k::TS_TYPE_PARAMETER_DECLARATION => {
            type_params(node, cx, None)?
        }
        k::TS_TYPE_PARAMETER | k::TS_TYPE_PREDICATE => words(cx, &kids(cx, node), false)?,
        k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION => cast(node, cx)?,
        k::TS_INFER_TYPE | k::TS_TYPE_QUERY | k::TS_TYPE_OPERATOR | k::TS_THIS_PARAMETER => {
            words(cx, &kids(cx, node), true)?
        }
        k::TS_TYPE_REFERENCE
        | k::TS_ARRAY_TYPE
        | k::TS_INDEXED_ACCESS_TYPE
        | k::TS_OPTIONAL_TYPE
        | k::TS_REST_TYPE
        | k::TS_NON_NULL_EXPRESSION
        | k::TS_TYPE_ASSERTION
        | k::TS_INSTANTIATION_EXPRESSION
        | k::TS_INTERFACE_HERITAGE
        | k::TS_CLASS_IMPLEMENTS
        | k::TS_PARENTHESIZED_TYPE => glued(cx, node)?,
        // The enclosing signature groups the parameters with the return type.
        k::FORMAL_PARAMETERS => {
            let object_return = node.next_sibling().is_some_and(|n| {
                n.kind() == k::TS_TYPE_ANNOTATION
                    && n.children().next().is_some_and(|t| is_object_type(&t))
            });
            if node.parent().is_some_and(|p| {
                matches!(
                    p.kind(),
                    k::TS_FUNCTION_TYPE
                        | k::TS_CONSTRUCTOR_TYPE
                        | k::TS_METHOD_SIGNATURE
                        | k::TS_CALL_SIGNATURE_DECLARATION
                        | k::TS_CONSTRUCT_SIGNATURE_DECLARATION
                )
            }) {
                decl::parameters_in(cx, &kids(cx, node), Some(object_return))?
            } else {
                decl::parameters(cx, &kids(cx, node))?
            }
        }
        k::TS_NAMED_TUPLE_MEMBER => named_tuple_member(node, cx)?,
        k::TS_TUPLE_TYPE => tuple(node, cx)?,
        k::TS_UNION_TYPE => union(node, cx)?,
        k::TS_INTERSECTION_TYPE => intersection(node, cx)?,
        k::TS_FUNCTION_TYPE | k::TS_CONSTRUCTOR_TYPE => function_type(node, cx)?,
        k::TS_CONDITIONAL_TYPE => conditional(node, cx)?,
        k::TS_MAPPED_TYPE => mapped(node, cx)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// `: T`, `=> T` of a function type, or the bare type of a predicate.
fn annotation(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    match kids(cx, node).as_slice() {
        [tok @ SyntaxElement::Token(_), ty] => {
            Ok(concat(vec![elem(cx, tok)?, lit(" "), elem(cx, ty)?]))
        }
        [ty] => elem(cx, ty),
        _ => Err(shape_error("type annotation")),
    }
}

const KEYWORD_TYPES: &[SyntaxKind] = &[
    k::TS_ANY_KEYWORD,
    k::TS_STRING_KEYWORD,
    k::TS_BOOLEAN_KEYWORD,
    k::TS_NUMBER_KEYWORD,
    k::TS_NEVER_KEYWORD,
    k::TS_UNKNOWN_KEYWORD,
    k::TS_NULL_KEYWORD,
    k::TS_UNDEFINED_KEYWORD,
    k::TS_VOID_KEYWORD,
    k::TS_SYMBOL_KEYWORD,
    k::TS_THIS_TYPE,
    k::TS_OBJECT_KEYWORD,
    k::TS_BIG_INT_KEYWORD,
];

pub(super) fn is_object_type(n: &SyntaxNode) -> bool {
    matches!(n.kind(), k::TS_TYPE_LITERAL | k::TS_MAPPED_TYPE)
}

/// A keyword type or a reference without type arguments.
fn is_simple(n: &SyntaxNode) -> bool {
    KEYWORD_TYPES.contains(&n.kind())
        || n.kind() == k::TS_TYPE_REFERENCE
            && !n
                .children()
                .any(|c| c.kind() == k::TS_TYPE_PARAMETER_INSTANTIATION)
}

/// Types that stay next to their brackets: simple ones, object types, and `{ .. } | null`.
pub(super) fn should_hug(n: &SyntaxNode) -> bool {
    if is_simple(n) || is_object_type(n) {
        return true;
    }
    if n.kind() != k::TS_UNION_TYPE {
        return false;
    }
    let members: Vec<_> = n.children().collect();
    let voids = members
        .iter()
        .filter(|m| matches!(m.kind(), k::TS_VOID_KEYWORD | k::TS_NULL_KEYWORD))
        .count();
    members.len() - 1 == voids && members.iter().any(is_object_type)
}

/// `<A, B>`: on one line, or one per line with a trailing comma (declarations only); a lone
/// simple or object type never breaks.
pub(super) fn type_params(node: &SyntaxNode, cx: &FormatCtx<'_>, id: Option<GroupId>) -> R {
    let ks = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (ks.first(), ks.last())
    else {
        return Err(shape_error("type parameters"));
    };
    let items: Vec<SyntaxNode> = node.children().collect();
    let docs = items
        .iter()
        .map(|n| cx.format(n))
        .collect::<Result<Vec<_>, _>>()?;
    // Each comma keeps the comments after it (also a trailing comma's).
    let commas: Vec<Doc> = ks
        .iter()
        .filter_map(token_of)
        .filter(|t| t.text() == ",")
        .map(|t| cx.trailing_comments_doc(t))
        .collect();
    let joined = |gap: Doc, docs: Vec<Doc>| {
        let mut parts = Vec::new();
        let last = docs.len().saturating_sub(1);
        for (i, d) in docs.into_iter().enumerate() {
            parts.push(d);
            if i < last {
                parts.push(lit(","));
                parts.push(commas.get(i).cloned().unwrap_or(Doc::Nil));
                parts.push(gap.clone());
            } else {
                parts.push(commas.get(i).cloned().unwrap_or(Doc::Nil));
            }
        }
        concat(parts)
    };
    // `<T,>(x) => x` in TSX: the comma tells a type parameter from a JSX tag.
    if items.len() == 1
        && node
            .parent()
            .is_some_and(|p| p.kind() == k::ARROW_FUNCTION_EXPRESSION)
        && ks.iter().any(|e| is_tok(e, ","))
        && !items[0]
            .children()
            .any(|c| c.kind() != k::BINDING_IDENTIFIER)
    {
        return Ok(concat(vec![cx.token(open), docs[0].clone(), lit(",>")]));
    }
    if docs.is_empty() || (items.len() == 1 && should_hug(&items[0])) {
        return Ok(concat(vec![
            cx.token(open),
            joined(lit(" "), docs),
            cx.token(close),
        ]));
    }
    let comma = if node.kind() == k::TS_TYPE_PARAMETER_DECLARATION {
        if_break(lit(","), Doc::Nil)
    } else {
        Doc::Nil
    };
    let body = concat(vec![
        cx.token(open),
        indent(concat(vec![softline(), joined(line(), docs)])),
        comma,
        softline(),
        cx.token(close),
    ]);
    Ok(match id {
        Some(id) => group_with_id(id, body),
        None => group(body),
    })
}

/// `x as T`, `x satisfies T`; as the object of a member or callee of a call it breaks inside
/// (parenthesized, as the source must have it).
fn cast(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let doc = words(cx, &kids(cx, node), false)?;
    let paren = node
        .parent()
        .filter(|p| p.kind() == k::PARENTHESIZED_EXPRESSION);
    let outer = paren
        .as_ref()
        .or(Some(node))
        .and_then(|n| n.parent().map(|p| (n.clone(), p)));
    let operand = outer.is_some_and(|(n, p)| {
        matches!(
            p.kind(),
            k::CALL_EXPRESSION
                | k::STATIC_MEMBER_EXPRESSION
                | k::COMPUTED_MEMBER_EXPRESSION
                | k::PRIVATE_FIELD_EXPRESSION
        ) && p.children().next().is_some_and(|c| c == n)
    });
    Ok(if operand {
        group(concat(vec![
            indent(concat(vec![softline(), doc])),
            softline(),
        ]))
    } else {
        doc
    })
}

/// `[a, b?: T, ...rest]`.
fn tuple(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let style = ListStyle {
        trailing_comma: TrailingComma::WhenBroken,
        ..ListStyle::default()
    };
    shapes::delimited_list(cx, node, &style)
}

/// `label?: T`.
fn named_tuple_member(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        parts.push(elem(cx, &e)?);
        if is_tok(&e, ":") {
            parts.push(lit(" "));
        }
    }
    Ok(concat(parts))
}

/// The members of a union or intersection, each with the operator token in front of it.
fn operands(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<(Option<SyntaxToken>, SyntaxNode)> {
    let mut out = Vec::new();
    let mut op = None;
    for e in kids(cx, node) {
        match e {
            SyntaxElement::Token(t) => op = Some(t),
            SyntaxElement::Node(n) => out.push((op.take(), n)),
        }
    }
    out
}

/// An operator with its comments, `text` when the source has none (the leading one).
fn operator(cx: &FormatCtx<'_>, op: &Option<SyntaxToken>, text: &'static str) -> Doc {
    op.as_ref().map_or_else(|| lit(text), |t| cx.token(t))
}

/// `A | B`: one line, or one `| member` per line with a leading bar.
fn union(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let members = operands(cx, node);
    // A lone member with its bar: the tree would lose a node without it.
    if members.len() < 2 {
        return words(cx, &kids(cx, node), false);
    }
    let parent = node.parent().map(|p| p.kind());
    let hug = should_hug(node);
    if hug {
        let mut parts = Vec::new();
        for (i, (op, m)) in members.iter().enumerate() {
            if i > 0 {
                parts.extend([lit(" "), operator(cx, op, "|"), lit(" ")]);
            }
            parts.push(cx.format(m)?);
        }
        return Ok(concat(parts));
    }
    let indented = !matches!(
        parent,
        Some(
            k::TS_TYPE_PARAMETER_INSTANTIATION
                | k::TS_TYPE_REFERENCE
                | k::TS_TYPE_ASSERTION
                | k::TS_TUPLE_TYPE
        )
    );
    let start = if indented { line() } else { Doc::Nil };
    let mut code = Vec::new();
    for (i, (op, m)) in members.iter().enumerate() {
        let d = align(2, cx.format(m)?);
        if i == 0 {
            let flat = match op {
                Some(t) => concat(vec![
                    cx.leading_comments_doc(t),
                    cx.trailing_comments_doc(t),
                ]),
                None => Doc::Nil,
            };
            code.push(if_break(
                concat(vec![start.clone(), operator(cx, op, "|"), lit(" ")]),
                flat,
            ));
        } else {
            code.extend([line(), operator(cx, op, "|"), lit(" ")]);
        }
        code.push(d);
    }
    let code = concat(code);
    Ok(if parent == Some(k::TS_PARENTHESIZED_TYPE) {
        group(concat(vec![indent(code), softline()]))
    } else if indented {
        group(indent(code))
    } else {
        group(code)
    })
}

/// `A & B`: object types stay on the line of their neighbour, other members wrap with `&` last.
fn intersection(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let members = operands(cx, node);
    // A leading `&` is a token the tree keeps.
    if members.first().is_some_and(|(op, _)| op.is_some()) {
        return words(cx, &kids(cx, node), false);
    }
    let mut parts = Vec::new();
    let mut was_indented = false;
    for (i, (op, m)) in members.iter().enumerate() {
        let d = cx.format(m)?;
        if i == 0 {
            parts.push(d);
            continue;
        }
        let amp = operator(cx, op, "&");
        let (a, b) = (is_object_type(&members[i - 1].1), is_object_type(m));
        if !a && !b {
            parts.push(indent(concat(vec![lit(" "), amp, line(), d])));
        } else if a && b {
            parts.push(concat(vec![
                lit(" "),
                amp,
                lit(" "),
                if was_indented { indent(d) } else { d },
            ]));
        } else {
            if i > 1 {
                was_indented = true;
            }
            parts.push(concat(vec![
                lit(" "),
                amp,
                lit(" "),
                if i > 1 { indent(d) } else { d },
            ]));
        }
    }
    Ok(group(concat(parts)))
}

/// `<T>(a: A) => R`, `abstract new () => R`.
fn function_type(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let mut head = Vec::new();
    let mut signature = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(_) => {
                head.push(elem(cx, &e)?);
                head.push(lit(" "));
            }
            SyntaxElement::Node(n) if n.kind() == k::TS_TYPE_ANNOTATION => {
                signature.push(lit(" "));
                signature.push(cx.format(n)?);
            }
            e => signature.push(elem(cx, e)?),
        }
    }
    Ok(concat(vec![concat(head), group(concat(signature))]))
}

/// `A extends B ? X : Y`; a chain in the false branch breaks together with the first.
fn conditional(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let parts: Vec<SyntaxNode> = node.children().collect();
    let [check, ext, yes, no] = parts.as_slice() else {
        return Err(shape_error("conditional type"));
    };
    let alt = cx.format(no)?;
    let tok = |text: &'static str| {
        kids(cx, node)
            .iter()
            .find_map(|e| {
                token_of(e)
                    .filter(|t| t.text() == text)
                    .map(|t| cx.token(t))
            })
            .unwrap_or_else(|| lit(text))
    };
    let body = concat(vec![
        cx.format(check)?,
        lit(" "),
        tok("extends"),
        lit(" "),
        cx.format(ext)?,
        indent(concat(vec![
            line(),
            tok("?"),
            lit(" "),
            align(2, cx.format(yes)?),
            line(),
            tok(":"),
            lit(" "),
            if no.kind() == k::TS_CONDITIONAL_TYPE {
                alt
            } else {
                align(2, alt)
            },
        ])),
    ]);
    // A conditional in a branch breaks with the one around it; one in a check or extends type
    // is a group of its own.
    let in_chain = node.parent().is_some_and(|p| {
        p.kind() == k::TS_CONDITIONAL_TYPE && p.children().take(2).all(|c| c != *node)
    });
    Ok(if in_chain { body } else { group(body) })
}

/// `{ readonly [K in keyof T]?: T[K] }`; stays broken when the source breaks after `{`.
fn mapped(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let ks = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (ks.first(), ks.last())
    else {
        return Err(shape_error("mapped type"));
    };
    let mut inner = Vec::new();
    // The key `[K in C as N]` breaks inside its brackets when it is long.
    let mut key: Option<(Doc, Vec<Doc>)> = None;
    for e in &ks[1..ks.len().saturating_sub(1)] {
        let target = match &mut key {
            Some((_, parts)) => parts,
            None => &mut inner,
        };
        match e {
            SyntaxElement::Token(t) if t.text() == "[" => key = Some((cx.token(t), Vec::new())),
            SyntaxElement::Token(t) if t.text() == "]" => {
                if let Some((open, parts)) = key.take() {
                    inner.push(group(concat(vec![
                        open,
                        indent(concat(vec![softline(), concat(parts)])),
                        softline(),
                        cx.token(t),
                    ])));
                }
            }
            SyntaxElement::Token(t) => match t.text() {
                "in" | "as" => target.extend([lit(" "), cx.token(t), lit(" ")]),
                "readonly" | ":" => target.extend([cx.token(t), lit(" ")]),
                ";" | "," => {
                    target.extend([cx.leading_comments_doc(t), cx.trailing_comments_doc(t)])
                }
                _ => target.push(cx.token(t)),
            },
            e => target.push(elem(cx, e)?),
        }
    }
    let brace_end = ks[0].text_range().end();
    let key = ks.get(1).map_or(brace_end, |e| e.text_range().start());
    let broken = cx.has_line_break(node, TextRange::new(brace_end, key.max(brace_end)));
    let body = concat(vec![
        cx.token(open),
        indent(concat(vec![
            line(),
            group(concat(inner)),
            if_break(lit(";"), Doc::Nil),
        ])),
        line(),
        cx.token(close),
    ]);
    Ok(if broken {
        doc::group_broken(body)
    } else {
        group(body)
    })
}
