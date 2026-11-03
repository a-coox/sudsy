//! Delimited lists: argument and parameter lists, initializers, collection expressions,
//! attribute lists, and the hook that keeps a lambda as the last argument on the call's line.

use super::options::{
    creation_brace_on_new_line, empty_braces, hug_closing_paren, hug_only_sole_lambda,
    initializer_layout, nested_brace_on_new_line, trailing_comma,
};
use super::seq::{R, Sep, direct_tokens, sep_doc, separator, seq_of, sig};
use crate::generated::kind as k;
use core_format::shapes::{ListStyle, TrailingComma, delimited_list, delimited_list_between};
use core_format::{Doc, FormatCtx, FormatError, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// Did the source put a comma after the last element, before `close`?
pub fn source_trailing_comma(cx: &FormatCtx<'_>, node: &SyntaxNode, close: &SyntaxToken) -> bool {
    let els = sig(cx, node);
    els.iter()
        .position(|e| matches!(e, SyntaxElement::Token(t) if t == close))
        .and_then(|i| i.checked_sub(1))
        .is_some_and(|i| matches!(&els[i], SyntaxElement::Token(t) if t.kind() == k::COMMA_TOKEN))
}

/// `x is { A: 1 }` and `x is T { A: 1 }`: the braces of a property pattern start on their own
/// line when the pattern breaks, but not in `{ A: 1 } => ...`, where nothing precedes them.
fn property_pattern_follows_something(clause: &SyntaxNode) -> bool {
    let Some(pattern) = clause.parent() else {
        return false;
    };
    clause.prev_sibling().is_some()
        || pattern.parent().map(|p| p.kind()) == Some(k::IS_PATTERN_EXPRESSION)
}

/// The trailing comma of a list, as `trailing_commas` says, except that a comma is not added after
/// a last element that ends in a line comment: the comma would end up in front of the comment,
/// and the comment would belong to the comma the next time (a different layout, so not stable).
pub fn list_trailing_comma(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    close: &SyntaxToken,
) -> TrailingComma {
    let source = source_trailing_comma(cx, node, close);
    let els = sig(cx, node);
    let last_item_comment = els
        .iter()
        .position(|e| matches!(e, SyntaxElement::Token(t) if t == close))
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| match &els[i] {
            SyntaxElement::Node(n) => cx.last_token(n),
            SyntaxElement::Token(_) => None,
        })
        .is_some_and(|t| {
            comments::trailing_comments(cx.grammar, &t)
                .iter()
                .any(|c| c.kind != comments::CommentKind::Block)
        });
    if last_item_comment && !source {
        return TrailingComma::Never;
    }
    trailing_comma(cx.options, source)
}

/// Parameters, arguments, type arguments, attributes, tuples, patterns: one line if it fits,
/// else one item per line.
pub fn plain_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let style = match node.kind() {
        k::PROPERTY_PATTERN_CLAUSE => ListStyle {
            padding: true,
            empty: empty_braces(cx.options),
            leading_line: nested_brace_on_new_line(cx.options)
                && property_pattern_follows_something(node),
            ..ListStyle::default()
        },
        _ => call_style(cx, node),
    };
    delimited_list(cx, node, &style)
}

/// Parameter and argument lists: `)` follows the last item with `closing_paren_line: same_line`.
fn call_style(cx: &FormatCtx<'_>, node: &SyntaxNode) -> ListStyle {
    ListStyle {
        hug_close: hug_closing_paren(cx.options)
            && matches!(
                node.kind(),
                k::ARGUMENT_LIST | k::PARAMETER_LIST | k::ATTRIBUTE_ARGUMENT_LIST
            ),
        ..ListStyle::default()
    }
}

/// A delimited list that is never broken: the items separated by `, ` between the delimiters.
pub fn unbreakable_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in sig(cx, node) {
        match &e {
            SyntaxElement::Token(t) if t.kind() == k::COMMA_TOKEN => {
                parts.push(cx.token(t));
                parts.push(doc::lit(" "));
            }
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
            SyntaxElement::Node(n) => parts.push(cx.format(n)?),
        }
    }
    Ok(doc::concat(parts))
}

fn creation_parent(kind: Option<SyntaxKind>) -> bool {
    matches!(
        kind,
        Some(
            k::OBJECT_CREATION_EXPRESSION
                | k::IMPLICIT_OBJECT_CREATION_EXPRESSION
                | k::ARRAY_CREATION_EXPRESSION
                | k::IMPLICIT_ARRAY_CREATION_EXPRESSION
                | k::STACK_ALLOC_ARRAY_CREATION_EXPRESSION
                | k::IMPLICIT_STACK_ALLOC_ARRAY_CREATION_EXPRESSION
                | k::WITH_EXPRESSION
        )
    )
}

fn brace_style(cx: &FormatCtx<'_>, node: &SyntaxNode, close: &SyntaxToken) -> ListStyle {
    ListStyle {
        padding: true,
        // `{ key, value }` as an element of a collection initializer takes no trailing comma.
        trailing_comma: if node.kind() == k::COMPLEX_ELEMENT_INITIALIZER_EXPRESSION {
            TrailingComma::Never
        } else {
            list_trailing_comma(cx, node, close)
        },
        layout: initializer_layout(cx.options),
        empty: empty_braces(cx.options),
        ..ListStyle::default()
    }
}

/// `{ a, b }`, or when it does not fit (or the source broke it) the braces on their own lines.
pub fn initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let close = direct_tokens(node, k::CLOSE_BRACE_TOKEN)
        .pop()
        .ok_or_else(|| core_format::FormatError::Shape("initializer without `}`".into()))?;
    let mut style = brace_style(cx, node, &close);
    style.leading_line = (creation_parent(node.parent().map(|p| p.kind()))
        && creation_brace_on_new_line(cx.options))
        || (nested_brace_on_new_line(cx.options)
            && node.parent().map(|p| p.kind()) == Some(k::SIMPLE_ASSIGNMENT_EXPRESSION));
    delimited_list(cx, node, &style)
}

pub fn collection_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let close = direct_tokens(node, k::CLOSE_BRACKET_TOKEN)
        .pop()
        .ok_or_else(|| core_format::FormatError::Shape("collection without `]`".into()))?;
    let style = ListStyle {
        padding: false,
        trailing_comma: list_trailing_comma(cx, node, &close),
        layout: initializer_layout(cx.options),
        ..ListStyle::default()
    };
    delimited_list(cx, node, &style)
}

/// Lists with other children around them: `new { A = 1 }`, `[1, 2] list`.
pub fn list_with_surroundings(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let (open_kind, close_kind) = if node.kind() == k::LIST_PATTERN {
        (k::OPEN_BRACKET_TOKEN, k::CLOSE_BRACKET_TOKEN)
    } else {
        (k::OPEN_BRACE_TOKEN, k::CLOSE_BRACE_TOKEN)
    };
    let els = sig(cx, node);
    let open = direct_tokens(node, open_kind).into_iter().next();
    let close = direct_tokens(node, close_kind).into_iter().last();
    let (Some(open), Some(close)) = (open, close) else {
        return seq_of(cx, node, &els);
    };
    let at = |t: &SyntaxToken| {
        els.iter()
            .position(|e| matches!(e, SyntaxElement::Token(x) if x == t))
    };
    let (Some(oi), Some(ci)) = (at(&open), at(&close)) else {
        return seq_of(cx, node, &els);
    };
    let mut style = if node.kind() == k::LIST_PATTERN {
        ListStyle::default()
    } else {
        brace_style(cx, node, &close)
    };
    style.trailing_comma = if node.kind() == k::LIST_PATTERN {
        TrailingComma::Never
    } else {
        style.trailing_comma
    };
    style.leading_line = node.kind() != k::LIST_PATTERN;
    let mut parts = Vec::new();
    if oi > 0 {
        parts.push(seq_of(cx, node, &els[..oi])?);
        let sep = if style.leading_line {
            Sep::None
        } else {
            separator(cx, node, &els[oi - 1], &els[oi])
        };
        parts.push(sep_doc(cx, sep));
    }
    parts.push(delimited_list_between(cx, node, &open, &close, &style)?);
    if ci + 1 < els.len() {
        parts.push(sep_doc(cx, separator(cx, node, &els[ci], &els[ci + 1])));
        parts.push(seq_of(cx, node, &els[ci + 1..])?);
    }
    Ok(doc::concat(parts))
}

fn has_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    !comments::leading_comments(cx.grammar, t).is_empty()
        || !comments::trailing_comments(cx.grammar, t).is_empty()
}

/// Is this argument a lambda or anonymous method with a block body?
fn block_lambda(arg: &SyntaxNode) -> bool {
    arg.children().any(|e| {
        matches!(
            e.kind(),
            k::SIMPLE_LAMBDA_EXPRESSION
                | k::PARENTHESIZED_LAMBDA_EXPRESSION
                | k::ANONYMOUS_METHOD_EXPRESSION
        ) && e.children().any(|c| c.kind() == k::BLOCK)
    })
}

fn is_lambda(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::SIMPLE_LAMBDA_EXPRESSION
            | k::PARENTHESIZED_LAMBDA_EXPRESSION
            | k::ANONYMOUS_METHOD_EXPRESSION
    )
}

/// `lambda_argument_hug: sole`: a call whose one argument is a lambda with an expression body keeps the lambda's
/// head on the call's line and puts the body on its own line when the call does not fit:
/// `Foo(x =>` / `    body` / `)`. `None` when the argument is anything else.
fn expression_lambda_hug(cx: &FormatCtx<'_>, arg: &SyntaxNode) -> Result<Option<Doc>, FormatError> {
    let mut children = arg.children();
    let (Some(lambda), None) = (children.next(), children.next()) else {
        return Ok(None);
    };
    if !is_lambda(lambda.kind()) {
        return Ok(None);
    }
    let els = sig(cx, &lambda);
    let Some((SyntaxElement::Node(body), head)) = els.split_last() else {
        return Ok(None);
    };
    if body.kind() == k::BLOCK {
        return Ok(None);
    }
    let head = doc::shared(seq_of(cx, &lambda, head)?);
    let body = doc::shared(cx.format(body)?);
    let flat = doc::concat(vec![
        doc::lit("("),
        head.clone(),
        doc::lit(" "),
        body.clone(),
        doc::lit(")"),
    ]);
    let hugged = doc::concat(vec![
        doc::lit("("),
        head.clone(),
        doc::indent(doc::concat(vec![doc::hardline(), body.clone()])),
        doc::hardline(),
        doc::lit(")"),
    ]);
    let expanded = doc::group(doc::concat(vec![
        doc::lit("("),
        doc::indent(doc::concat(vec![
            doc::softline(),
            head,
            doc::lit(" "),
            body,
        ])),
        doc::softline(),
        doc::lit(")"),
    ]));
    Ok(Some(doc::conditional_group(vec![flat, hugged, expanded])))
}

/// `(a, b, x => { ... })`: when the arguments before the last one fit on the call's line, the
/// lambda's body hangs from it; otherwise one argument per line.
pub fn argument_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let args: Vec<SyntaxNode> = node
        .children()
        .filter(|c| c.kind() == k::ARGUMENT)
        .collect();
    let simple_tokens = node
        .children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|t| !cx.grammar.is_trivia(t.kind()))
        .all(|t| !has_comments(cx, &t));
    if hug_only_sole_lambda(cx.options)
        && args.len() == 1
        && simple_tokens
        && cx
            .first_token(&args[0])
            .is_some_and(|t| comments::leading_comments(cx.grammar, &t).is_empty())
        && let Some(hugged) = expression_lambda_hug(cx, &args[0])?
    {
        return Ok(hugged);
    }
    // With `lambda_argument_hug: sole` a lambda hugs only when it is the one argument.
    let hug = args.last().is_some_and(block_lambda)
        && (args.len() == 1 || !hug_only_sole_lambda(cx.options))
        && simple_tokens
        && args.iter().all(|a| {
            cx.first_token(a)
                .is_some_and(|t| comments::leading_comments(cx.grammar, &t).is_empty())
        });
    if !hug {
        return delimited_list(cx, node, &call_style(cx, node));
    }
    // The two candidates below are built from the same argument documents: share them.
    let docs = args
        .iter()
        .map(|a| cx.format(a).map(doc::shared))
        .collect::<Result<Vec<Doc>, _>>()?;
    let (last, init) = docs.split_last().expect("at least one argument");
    if init.iter().any(|d| d.clone().propagate_breaks()) {
        return delimited_list(cx, node, &ListStyle::default());
    }
    let hugged = {
        let mut parts = vec![doc::lit("(")];
        for d in init {
            parts.push(d.clone());
            parts.push(doc::lit(", "));
        }
        parts.push(last.clone());
        parts.push(doc::lit(")"));
        doc::concat(parts)
    };
    let expanded = doc::group(doc::concat(vec![
        doc::lit("("),
        doc::indent(doc::concat(vec![
            doc::softline(),
            doc::join(&doc::concat(vec![doc::lit(","), doc::line()]), docs.clone()),
        ])),
        if hug_closing_paren(cx.options) {
            Doc::Nil
        } else {
            doc::softline()
        },
        doc::lit(")"),
    ]));
    Ok(doc::conditional_group(vec![hugged, expanded]))
}
