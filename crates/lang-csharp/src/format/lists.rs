//! Delimited lists: argument and parameter lists, initializers, collection expressions,
//! attribute lists, and the hook that keeps a lambda as the last argument on the call's line.

use super::options::{csharpier, initializer_layout, trailing_comma};
use super::seq::{R, Sep, direct_tokens, sep_doc, separator, seq_of, sig};
use crate::generated::kind as k;
use core_format::shapes::{ListStyle, TrailingComma, delimited_list, delimited_list_between};
use core_format::{Doc, FormatCtx, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// Did the source put a comma after the last element, before `close`?
pub fn source_trailing_comma(cx: &FormatCtx<'_>, node: &SyntaxNode, close: &SyntaxToken) -> bool {
    let els = sig(cx, node);
    els.iter()
        .position(|e| matches!(e, SyntaxElement::Token(t) if t == close))
        .and_then(|i| i.checked_sub(1))
        .is_some_and(|i| matches!(&els[i], SyntaxElement::Token(t) if t.kind() == k::COMMA_TOKEN))
}

/// Parameters, arguments, type arguments, attributes, tuples, patterns: one line if it fits,
/// else one item per line.
pub fn plain_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let style = match node.kind() {
        k::PROPERTY_PATTERN_CLAUSE => ListStyle {
            padding: true,
            empty: " ",
            ..ListStyle::default()
        },
        _ => ListStyle::default(),
    };
    delimited_list(cx, node, &style)
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
            trailing_comma(cx.options, source_trailing_comma(cx, node, close))
        },
        layout: initializer_layout(cx.options),
        empty: " ",
        ..ListStyle::default()
    }
}

/// `{ a, b }`, or when it does not fit (or the source broke it) the braces on their own lines.
pub fn initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let close = direct_tokens(node, k::CLOSE_BRACE_TOKEN)
        .pop()
        .ok_or_else(|| core_format::FormatError::Shape("initializer without `}`".into()))?;
    let mut style = brace_style(cx, node, &close);
    style.leading_line = creation_parent(node.parent().map(|p| p.kind()));
    delimited_list(cx, node, &style)
}

pub fn collection_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let close = direct_tokens(node, k::CLOSE_BRACKET_TOKEN)
        .pop()
        .ok_or_else(|| core_format::FormatError::Shape("collection without `]`".into()))?;
    let style = ListStyle {
        padding: false,
        trailing_comma: trailing_comma(cx.options, source_trailing_comma(cx, node, &close)),
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
    // CSharpier hugs a lambda only when it is the one argument.
    let hug = args.last().is_some_and(block_lambda)
        && (args.len() == 1 || !csharpier(cx.options))
        && simple_tokens
        && args.iter().all(|a| {
            cx.first_token(a)
                .is_some_and(|t| comments::leading_comments(cx.grammar, &t).is_empty())
        });
    if !hug {
        return delimited_list(cx, node, &ListStyle::default());
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
    let (open, close) = (
        direct_tokens(node, k::OPEN_PAREN_TOKEN).pop(),
        direct_tokens(node, k::CLOSE_PAREN_TOKEN).pop(),
    );
    let (Some(open), Some(close)) = (open, close) else {
        return delimited_list(cx, node, &ListStyle::default());
    };
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
        doc::softline(),
        doc::lit(")"),
    ]));
    let _ = (open, close);
    Ok(doc::conditional_group(vec![hugged, expanded]))
}
