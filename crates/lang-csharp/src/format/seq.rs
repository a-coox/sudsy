//! The generic layout: a node's significant children one after the other, separated by the
//! space, line break or nothing that [`Sep`] decides.

use super::options::{brace_on_new_line, csharpier};
use super::spacing;
use super::tags::{self, Tag};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

pub type R = Result<Doc, FormatError>;

/// The non-trivia children of a node.
pub fn sig(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .by_kind(|kind| !cx.grammar.is_trivia(kind))
        .collect()
}

pub fn element_doc(cx: &FormatCtx<'_>, e: &SyntaxElement) -> R {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(cx.token(t)),
    }
}

pub fn first_tok(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.first_token(n),
    }
}

pub fn last_tok(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.last_token(n),
    }
}

/// What separates two elements.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sep {
    None,
    Space,
    Hard,
    /// A hard line break, one indentation level deeper.
    HardIndented,
    /// The brace of a body: a line break, or a space with `brace_newline: none`.
    Brace,
    /// The next element is an embedded statement: on its own line, indented.
    Embedded,
}

/// Does a node of this kind have children that are statements taken as the body of a
/// compound statement?
fn has_embedded_body(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::IF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_STATEMENT
            | k::FOR_STATEMENT
            | k::FOR_EACH_STATEMENT
            | k::FOR_EACH_VARIABLE_STATEMENT
            | k::USING_STATEMENT
            | k::FIXED_STATEMENT
            | k::LOCK_STATEMENT
            | k::ELSE_CLAUSE
    )
}

/// Attribute lists of these nodes sit on the same line as what they annotate.
fn inline_attributes(parent: SyntaxKind) -> bool {
    matches!(
        parent,
        k::PARAMETER
            | k::TYPE_PARAMETER
            | k::SIMPLE_LAMBDA_EXPRESSION
            | k::PARENTHESIZED_LAMBDA_EXPRESSION
            | k::FUNCTION_POINTER_PARAMETER
    )
}

/// Is a body in braces empty: nothing between the braces, and no comments?
pub fn is_empty_braces(cx: &FormatCtx<'_>, open: &SyntaxToken, close: &SyntaxToken) -> bool {
    let g = cx.grammar;
    let Some(parent) = open.parent() else {
        return false;
    };
    let next = parent
        .children_with_tokens()
        .by_kind(|kind| !g.is_trivia(kind))
        .skip_while(|e| !matches!(e, SyntaxElement::Token(t) if t == open))
        .nth(1);
    matches!(next, Some(SyntaxElement::Token(t)) if t == *close)
        && core_format::comments::leading_comments(g, close).is_empty()
        && core_format::comments::trailing_comments(g, open).is_empty()
}

/// Direct token children of kind `kind`.
pub fn direct_tokens(node: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|t| t.kind() == kind)
        .collect()
}

fn block_is_empty(cx: &FormatCtx<'_>, block: &SyntaxNode) -> bool {
    let opens = direct_tokens(block, k::OPEN_BRACE_TOKEN);
    let closes = direct_tokens(block, k::CLOSE_BRACE_TOKEN);
    match (opens.first(), closes.last()) {
        (Some(o), Some(c)) => is_empty_braces(cx, o, c),
        _ => false,
    }
}

/// The separator between `prev` and `next`, children of a node of kind `parent`.
pub fn separator(
    cx: &FormatCtx<'_>,
    parent: &SyntaxNode,
    prev: &SyntaxElement,
    next: &SyntaxElement,
) -> Sep {
    let pk = parent.kind();
    if let SyntaxElement::Node(n) = next {
        let nk = n.kind();
        if csharpier(cx.options)
            && matches!(
                nk,
                k::BASE_CONSTRUCTOR_INITIALIZER | k::THIS_CONSTRUCTOR_INITIALIZER
            )
        {
            return Sep::None; // the initializer starts its own line
        }
        let bare_property_pattern = nk == k::RECURSIVE_PATTERN
            && pk == k::IS_PATTERN_EXPRESSION
            && matches!(n.children().next(), Some(c) if c.kind() == k::PROPERTY_PATTERN_CLAUSE);
        if csharpier(cx.options)
            && (matches!(nk, k::BASE_LIST | k::PROPERTY_PATTERN_CLAUSE) || bare_property_pattern)
        {
            return Sep::None; // these start with a `line` of their own
        }
        if nk == k::BLOCK {
            return if block_is_empty(cx, n) {
                Sep::Space
            } else {
                Sep::Brace
            };
        }
        if tags::has(cx.grammar, nk, Tag::InitializerExpression) {
            return match pk {
                k::OBJECT_CREATION_EXPRESSION
                | k::IMPLICIT_OBJECT_CREATION_EXPRESSION
                | k::ARRAY_CREATION_EXPRESSION
                | k::IMPLICIT_ARRAY_CREATION_EXPRESSION
                | k::STACK_ALLOC_ARRAY_CREATION_EXPRESSION
                | k::IMPLICIT_STACK_ALLOC_ARRAY_CREATION_EXPRESSION
                | k::WITH_EXPRESSION => Sep::None,
                _ => Sep::Space,
            };
        }
        if nk == k::ACCESSOR_LIST {
            return if super::braces::accessors_inline(cx, n) {
                Sep::Space
            } else {
                Sep::Brace
            };
        }
        if has_embedded_body(pk)
            && tags::has(cx.grammar, nk, Tag::Statement)
            && !(pk == k::ELSE_CLAUSE && nk == k::IF_STATEMENT)
        {
            return Sep::Embedded;
        }
        if matches!(nk, k::ELSE_CLAUSE | k::CATCH_CLAUSE | k::FINALLY_CLAUSE) {
            return Sep::Hard;
        }
        if nk == k::TYPE_PARAMETER_CONSTRAINT_CLAUSE {
            return Sep::HardIndented;
        }
        if pk == k::LABELED_STATEMENT && tags::has(cx.grammar, nk, Tag::Statement) {
            return Sep::Hard;
        }
    }
    if let SyntaxElement::Node(p) = prev
        && p.kind() == k::ATTRIBUTE_LIST
    {
        return if inline_attributes(pk) {
            Sep::Space
        } else {
            Sep::Hard
        };
    }
    if pk == k::DO_STATEMENT
        && matches!(next, SyntaxElement::Token(t) if t.kind() == k::WHILE_KEYWORD)
    {
        // CSharpier keeps `} while (...)` on one line.
        return if csharpier(cx.options) {
            Sep::Space
        } else {
            Sep::Hard
        };
    }
    match (last_tok(cx, prev), first_tok(cx, next)) {
        (Some(a), Some(b)) => {
            if spacing::space(&a, &b, pk) {
                Sep::Space
            } else {
                Sep::None
            }
        }
        _ => Sep::None,
    }
}

pub fn sep_doc(cx: &FormatCtx<'_>, sep: Sep) -> Doc {
    match sep {
        Sep::None => Doc::Nil,
        Sep::Space => doc::lit(" "),
        Sep::Hard | Sep::Embedded => doc::hardline(),
        Sep::HardIndented => doc::indent(doc::hardline()),
        Sep::Brace => {
            if brace_on_new_line(cx.options) {
                doc::hardline()
            } else {
                doc::lit(" ")
            }
        }
    }
}

/// Statements whose parenthesized condition may break after `(` and before `)`.
fn has_paren_condition(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::IF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_STATEMENT
            | k::LOCK_STATEMENT
            | k::SWITCH_STATEMENT
    )
}

/// `( expr )` where the expression may move to its own, indented line.
pub fn paren_group(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    expr: &SyntaxNode,
    close: &SyntaxToken,
) -> R {
    // A comment after the closing parenthesis must not make the group break.
    Ok(doc::concat(vec![
        doc::group(doc::concat(vec![
            cx.token(open),
            doc::indent(doc::concat(vec![doc::softline(), cx.format(expr)?])),
            doc::softline(),
            cx.leading_comments_doc(close),
            cx.token_text(close),
        ])),
        cx.trailing_comments_doc(close),
    ]))
}

pub fn seq(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    seq_of(cx, node, &els)
}

/// `els`, children of `node`, laid out in order.
pub fn seq_of(cx: &FormatCtx<'_>, node: &SyntaxNode, els: &[SyntaxElement]) -> R {
    if let [only] = els {
        return element_doc(cx, only);
    }
    let pk = node.kind();
    let paren_condition = has_paren_condition(pk);
    let mut parts = Vec::with_capacity(els.len() * 2);
    let mut i = 0;
    while i < els.len() {
        let e = &els[i];
        // `( condition )` of if, while, ...
        let condition = match (paren_condition, e, els.get(i + 1), els.get(i + 2)) {
            (
                true,
                SyntaxElement::Token(open),
                Some(SyntaxElement::Node(expr)),
                Some(SyntaxElement::Token(close)),
            ) if open.kind() == k::OPEN_PAREN_TOKEN && close.kind() == k::CLOSE_PAREN_TOKEN => {
                Some((open, expr, close))
            }
            _ => None,
        };
        let sep = if i > 0 {
            separator(cx, node, &els[i - 1], e)
        } else {
            Sep::None
        };
        let body = match condition {
            Some((open, expr, close)) => paren_group(cx, open, expr, close)?,
            None => element_doc(cx, e)?,
        };
        if sep == Sep::Embedded {
            parts.push(doc::indent(doc::concat(vec![doc::hardline(), body])));
        } else {
            parts.push(sep_doc(cx, sep));
            parts.push(body);
        }
        i += if condition.is_some() { 3 } else { 1 };
    }
    Ok(doc::concat(parts))
}

/// `-x`, `!x`, `++x`: the operator right against its operand (a space only where the two would
/// become one token).
pub fn prefix_unary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Option<Doc>, FormatError> {
    let g = cx.grammar;
    let is_unary = matches!(
        node.kind(),
        k::UNARY_PLUS_EXPRESSION
            | k::UNARY_MINUS_EXPRESSION
            | k::BITWISE_NOT_EXPRESSION
            | k::LOGICAL_NOT_EXPRESSION
            | k::PRE_INCREMENT_EXPRESSION
            | k::PRE_DECREMENT_EXPRESSION
            | k::ADDRESS_OF_EXPRESSION
            | k::POINTER_INDIRECTION_EXPRESSION
            | k::INDEX_EXPRESSION
    );
    if !is_unary {
        return Ok(None);
    }
    let els = sig(cx, node);
    let [SyntaxElement::Token(op), SyntaxElement::Node(operand)] = els.as_slice() else {
        return Ok(None);
    };
    let merges = cx.first_token(operand).is_some_and(|f| {
        let (a, b) = (op.text().chars().next_back(), f.text().chars().next());
        matches!(
            (a, b),
            (Some('+'), Some('+')) | (Some('-'), Some('-')) | (Some('&'), Some('&'))
        )
    });
    let _ = g;
    Ok(Some(doc::concat(vec![
        cx.token(op),
        if merges { doc::lit(" ") } else { Doc::Nil },
        cx.format(operand)?,
    ])))
}
