//! The oxc tree read the way Prettier's printers read ESTree: parentheses are transparent
//! (`un`, `parent`), operators and call parts are found by position.

use super::util::{kids, node_of};
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

/// The expression inside any number of parentheses.
pub(crate) fn un(n: &SyntaxNode) -> SyntaxNode {
    let mut n = n.clone();
    while n.kind() == k::PARENTHESIZED_EXPRESSION {
        match n.children().next() {
            Some(c) => n = c,
            None => break,
        }
    }
    n
}

/// `n` with the parentheses around it: the node as its parent holds it.
pub(crate) fn outer(n: &SyntaxNode) -> SyntaxNode {
    let mut n = n.clone();
    while let Some(p) = n.parent() {
        if p.kind() != k::PARENTHESIZED_EXPRESSION {
            break;
        }
        n = p;
    }
    n
}

/// The node that holds `n` (and its parentheses).
pub(crate) fn parent(n: &SyntaxNode) -> Option<SyntaxNode> {
    outer(n).parent()
}

pub(crate) fn is_member(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::STATIC_MEMBER_EXPRESSION | k::COMPUTED_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION
    )
}

pub(crate) fn is_binaryish(kind: SyntaxKind) -> bool {
    matches!(kind, k::BINARY_EXPRESSION | k::LOGICAL_EXPRESSION)
}

pub(crate) fn is_function(kind: SyntaxKind) -> bool {
    matches!(kind, k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION)
}

pub(crate) fn is_jsx(kind: SyntaxKind) -> bool {
    matches!(kind, k::JSX_ELEMENT | k::JSX_FRAGMENT)
}

pub(crate) fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::NUMERIC_LITERAL
            | k::STRING_LITERAL
            | k::BOOLEAN_LITERAL
            | k::NULL_LITERAL
            | k::BIG_INT_LITERAL
            | k::REG_EXP_LITERAL
    )
}

/// The first direct token that is not trivia: the operator of a binary, logical, assignment,
/// unary or update expression.
pub(crate) fn operator(cx: &FormatCtx<'_>, n: &SyntaxNode) -> Option<SyntaxToken> {
    n.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !cx.grammar.is_trivia(t.kind()) && !matches!(t.kind(), k::L_PAREN | k::R_PAREN))
}

pub(crate) fn operator_text(cx: &FormatCtx<'_>, n: &SyntaxNode) -> String {
    operator(cx, n)
        .map(|t| t.text().to_string())
        .unwrap_or_default()
}

/// The source text of a node without its comments, from its first to its last token.
pub(crate) fn text_of(cx: &FormatCtx<'_>, n: &SyntaxNode) -> String {
    match (cx.first_token(n), cx.last_token(n)) {
        (Some(a), Some(b)) => {
            let all = n.text().to_string();
            let from = usize::from(a.text_range().start() - n.text_range().start());
            let to = usize::from(b.text_range().end() - n.text_range().start());
            all[from..to].to_string()
        }
        _ => String::new(),
    }
}

/// The name of an identifier node.
pub(crate) fn name_of(cx: &FormatCtx<'_>, n: &SyntaxNode) -> Option<String> {
    matches!(
        n.kind(),
        k::IDENTIFIER_REFERENCE | k::IDENTIFIER_NAME | k::BINDING_IDENTIFIER
    )
    .then(|| text_of(cx, n))
}

/// Does the leftmost descendant of `node` (through the parts a statement or arrow body starts
/// with) satisfy `pred`? Prettier's `startsWithNoLookaheadToken`.
pub(crate) fn starts_with(node: &SyntaxNode, pred: &dyn Fn(&SyntaxNode) -> bool) -> bool {
    let n = un(node);
    let first = || n.children().next();
    match n.kind() {
        k::BINARY_EXPRESSION
        | k::LOGICAL_EXPRESSION
        | k::ASSIGNMENT_EXPRESSION
        | k::STATIC_MEMBER_EXPRESSION
        | k::COMPUTED_MEMBER_EXPRESSION
        | k::PRIVATE_FIELD_EXPRESSION
        | k::CONDITIONAL_EXPRESSION
        | k::SEQUENCE_EXPRESSION
        | k::CHAIN_EXPRESSION
        | k::TS_AS_EXPRESSION
        | k::TS_SATISFIES_EXPRESSION
        | k::TS_NON_NULL_EXPRESSION => first().is_some_and(|c| starts_with(&c, pred)),
        k::TAGGED_TEMPLATE_EXPRESSION | k::CALL_EXPRESSION => first().is_some_and(|c| {
            // An IIFE is always parenthesized already.
            un(&c).kind() != k::FUNCTION && starts_with(&c, pred)
        }),
        k::UPDATE_EXPRESSION => {
            first().is_some_and(|c| !is_prefix(&n, &c) && starts_with(&c, pred))
        }
        _ => pred(&n),
    }
}

/// Is the operator of the update expression `n` in front of its argument `arg`?
fn is_prefix(n: &SyntaxNode, arg: &SyntaxNode) -> bool {
    n.children_with_tokens().next().is_some_and(|e| {
        e.as_token().is_some() && e.text_range().start() < arg.text_range().start()
    })
}

pub(crate) fn nodes(n: &SyntaxNode) -> Vec<SyntaxNode> {
    n.children().collect()
}

/// Does any of the tokens of `n` (its own, not its children's) have a comment next to it?
pub(crate) fn token_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    use core_format::comments;
    !comments::leading_comments(cx.grammar, t).is_empty()
        || !comments::trailing_comments(cx.grammar, t).is_empty()
}

/// Does `n` have a comment in front of its first token?
pub(crate) fn has_leading_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.first_token(n)
        .is_some_and(|t| !core_format::comments::leading_comments(cx.grammar, &t).is_empty())
}

/// Does `n` have a comment after its last token?
pub(crate) fn has_trailing_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.last_token(n)
        .is_some_and(|t| core_format::comments::has_trailing_comment(cx.grammar, &t))
}

pub(crate) fn has_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    has_leading_comment(cx, n) || has_trailing_comment(cx, n)
}

/// The significant elements of `n` split at its first `(`: what comes before it and the
/// argument nodes with the comma after each, and the parentheses.
pub(crate) struct Call {
    pub callee: SyntaxNode,
    pub type_args: Option<SyntaxNode>,
    /// The `?.` in front of `(`.
    pub optional: Option<SyntaxToken>,
    pub open: Option<SyntaxToken>,
    pub args: Vec<(SyntaxNode, Option<SyntaxToken>)>,
    pub close: Option<SyntaxToken>,
}

impl Call {
    /// `None` for anything but a call or `new` expression of the usual shape.
    pub(crate) fn of(cx: &FormatCtx<'_>, n: &SyntaxNode) -> Option<Call> {
        let elems = kids(cx, n);
        let mut it = elems.iter().peekable();
        if n.kind() == k::NEW_EXPRESSION {
            it.next();
        }
        let callee = node_of(it.next()?)?.clone();
        let mut call = Call {
            callee,
            type_args: None,
            optional: None,
            open: None,
            args: Vec::new(),
            close: None,
        };
        for e in it {
            match e {
                SyntaxElement::Node(a) if call.open.is_none() => {
                    if a.kind() != k::TS_TYPE_PARAMETER_INSTANTIATION {
                        return None;
                    }
                    call.type_args = Some(a.clone());
                }
                SyntaxElement::Node(a) => call.args.push((a.clone(), None)),
                SyntaxElement::Token(t) if t.kind() == k::QUESTION_DOT => {
                    call.optional = Some(t.clone());
                }
                SyntaxElement::Token(t) if t.kind() == k::L_PAREN && call.open.is_none() => {
                    call.open = Some(t.clone());
                }
                SyntaxElement::Token(t) if t.kind() == k::R_PAREN => call.close = Some(t.clone()),
                SyntaxElement::Token(t) if t.kind() == k::COMMA => {
                    call.args.last_mut()?.1 = Some(t.clone());
                }
                SyntaxElement::Token(_) => return None,
            }
        }
        Some(call)
    }
}
