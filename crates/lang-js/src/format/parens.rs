//! Parentheses: which a node needs (Prettier's `needsParens`), and the printing of the ones the
//! source has. The tree keeps `ParenthesizedExpression` (`#transparent` for the safety check): a
//! kept pair is printed as written, an unneeded pair is left out, and a node that needs a pair the
//! source lacks gets one around its printed document.

use super::ast::{
    is_binaryish, is_member, operator_text, outer, parent, starts_with, token_comments, un,
};
use super::binary::{is_bitwise, precedence, should_flatten};
use super::util::{kids, shape_error, token_of};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, lit};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxKind, SyntaxNode};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Need {
    Yes,
    No,
    /// A kind this step does not decide on (TypeScript): the source's parentheses stay.
    Keep,
}

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::PARENTHESIZED_EXPRESSION {
        return Ok(None);
    }
    let elems = kids(cx, node);
    let (Some(open), Some(close)) = (
        elems.first().and_then(token_of),
        elems.last().and_then(token_of),
    ) else {
        return Err(shape_error("parentheses"));
    };
    let child = node
        .children()
        .next()
        .ok_or_else(|| shape_error("parentheses"))?;
    let commented = token_comments(cx, open) || token_comments(cx, close);
    let nested = node
        .parent()
        .is_some_and(|p| p.kind() == k::PARENTHESIZED_EXPRESSION);
    let keep = commented || (!nested && needs_parens(cx, &un(node)) != Need::No);
    if !keep {
        return Ok(Some(cx.format(&child)?));
    }
    Ok(Some(concat(vec![
        cx.token(open),
        cx.format(&child)?,
        cx.token(close),
    ])))
}

/// `doc` of `node` in the parentheses it needs when the source has none.
pub(crate) fn wrap(node: &SyntaxNode, cx: &FormatCtx<'_>, doc: Doc) -> Doc {
    let addable = matches!(
        node.kind(),
        k::BINARY_EXPRESSION
            | k::LOGICAL_EXPRESSION
            | k::UNARY_EXPRESSION
            | k::UPDATE_EXPRESSION
            | k::AWAIT_EXPRESSION
            | k::YIELD_EXPRESSION
            | k::SEQUENCE_EXPRESSION
            | k::ASSIGNMENT_EXPRESSION
            | k::CONDITIONAL_EXPRESSION
            | k::ARROW_FUNCTION_EXPRESSION
            | k::FUNCTION
            | k::CLASS
            | k::OBJECT_EXPRESSION
            | k::NUMERIC_LITERAL
            | k::STRING_LITERAL
            | k::CHAIN_EXPRESSION
            | k::CALL_EXPRESSION
            | k::STATIC_MEMBER_EXPRESSION
            | k::COMPUTED_MEMBER_EXPRESSION
            | k::TAGGED_TEMPLATE_EXPRESSION
            | k::JSX_ELEMENT
            | k::JSX_FRAGMENT
    );
    let held = node
        .parent()
        .is_none_or(|p| p.kind() == k::PARENTHESIZED_EXPRESSION);
    if !addable || held || needs_parens(cx, node) != Need::Yes {
        return doc;
    }
    concat(vec![lit("("), doc, lit(")")])
}

/// The first character the printed `node` starts with (a kept or added parenthesis counts).
pub(crate) fn first_char(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<char> {
    let mut n = node.clone();
    loop {
        if n.kind() == k::PARENTHESIZED_EXPRESSION {
            let inner = un(&n);
            let commented = n
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| token_comments(cx, &t));
            let nested = n
                .parent()
                .is_some_and(|p| p.kind() == k::PARENTHESIZED_EXPRESSION);
            if commented || (!nested && needs_parens(cx, &inner) != Need::No) {
                return Some('(');
            }
            n = n.first_child()?;
            continue;
        }
        if n.parent()
            .is_some_and(|p| p.kind() != k::PARENTHESIZED_EXPRESSION)
            && needs_parens(cx, &n) == Need::Yes
        {
            return Some('(');
        }
        match n
            .children_with_tokens()
            .find(|e| !cx.grammar.is_trivia(e.kind()))?
        {
            core_syntax::SyntaxElement::Node(c) => n = c,
            core_syntax::SyntaxElement::Token(t) => return t.text().chars().next(),
        }
    }
}

/// The kinds that are declarations when they stand where a statement does.
fn declaration_position(pk: SyntaxKind) -> bool {
    matches!(
        pk,
        k::PROGRAM
            | k::BLOCK_STATEMENT
            | k::FUNCTION_BODY
            | k::SWITCH_CASE
            | k::STATIC_BLOCK
            | k::TS_MODULE_BLOCK
            | k::EXPORT_NAMED_DECLARATION
            | k::EXPORT_DEFAULT_DECLARATION
            | k::LABELED_STATEMENT
            | k::IF_STATEMENT
            | k::FOR_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_WHILE_STATEMENT
            | k::WITH_STATEMENT
            | k::METHOD_DEFINITION
            | k::OBJECT_PROPERTY
            | k::TS_INTERFACE_BODY
            | k::TS_EXTERNAL_MODULE_DECLARATION
    )
}

/// Does `node` need parentheses where its parent holds it? (`node` is not a parenthesized
/// expression; parentheses around it do not count.)
pub(crate) fn needs_parens(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Need {
    let Some(p) = parent(node) else {
        return Need::No;
    };
    let c = outer(node);
    let (kind, pk) = (node.kind(), p.kind());
    if pk == k::DECORATOR {
        return Need::Keep;
    }
    let yes = |b: bool| if b { Need::Yes } else { Need::No };
    let first = p.first_child().as_ref() == Some(&c);
    let last = p.last_child().as_ref() == Some(&c);
    let member_p = is_member(pk);
    let is_callee = matches!(pk, k::CALL_EXPRESSION | k::NEW_EXPRESSION) && first;
    let is_object = member_p && first;
    let super_class = pk == k::CLASS
        && std::iter::successors(c.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
            .find(|e| !cx.grammar.is_trivia(e.kind()))
            .is_some_and(|e| e.as_token().is_some_and(|t| t.text() == "extends"));

    // A leading `{`, `function` or `class` of a statement, `{` of an arrow body or the
    // declaration of `export default` would be read as something else.
    if matches!(kind, k::OBJECT_EXPRESSION | k::FUNCTION | k::CLASS)
        && (c != *node || !declaration_position(pk))
    {
        let at = |n: &SyntaxNode| *n == *node;
        let statement = node
            .ancestors()
            .find(|a| a.kind() == k::EXPRESSION_STATEMENT);
        if statement
            .and_then(|s| s.children().next())
            .is_some_and(|e| starts_with(&e, &at))
        {
            return Need::Yes;
        }
        if kind == k::OBJECT_EXPRESSION
            && let Some(body) = node
                .ancestors()
                .find(|a| a.kind() == k::ARROW_FUNCTION_EXPRESSION)
                .and_then(|a| a.children().last())
            && un(&body).kind() != k::SEQUENCE_EXPRESSION
            && starts_with(&body, &at)
        {
            return Need::Yes;
        }
        let export = node
            .ancestors()
            .find(|a| a.kind() == k::EXPORT_DEFAULT_DECLARATION)
            .and_then(|a| a.children().last());
        if kind != k::OBJECT_EXPRESSION && export.is_some_and(|e| starts_with(&e, &at)) {
            return Need::Yes;
        }
    }
    if super_class
        && matches!(
            kind,
            k::ARROW_FUNCTION_EXPRESSION
                | k::ASSIGNMENT_EXPRESSION
                | k::AWAIT_EXPRESSION
                | k::BINARY_EXPRESSION
                | k::CONDITIONAL_EXPRESSION
                | k::LOGICAL_EXPRESSION
                | k::NEW_EXPRESSION
                | k::OBJECT_EXPRESSION
                | k::SEQUENCE_EXPRESSION
                | k::TAGGED_TEMPLATE_EXPRESSION
                | k::UNARY_EXPRESSION
                | k::UPDATE_EXPRESSION
                | k::YIELD_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
        )
    {
        return Need::Yes;
    }
    if pk == k::EXPORT_DEFAULT_DECLARATION && kind == k::SEQUENCE_EXPRESSION {
        return Need::Yes;
    }
    let is_cast = |kd: SyntaxKind| matches!(kd, k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION);
    match kind {
        k::UPDATE_EXPRESSION | k::UNARY_EXPRESSION => {
            let op = operator_text(cx, node);
            if kind == k::UPDATE_EXPRESSION && pk == k::UNARY_EXPRESSION {
                let prefix = node
                    .children_with_tokens()
                    .next()
                    .is_some_and(|e| e.as_token().is_some());
                let pop = operator_text(cx, &p);
                return yes(prefix && ((op == "++" && pop == "+") || (op == "--" && pop == "-")));
            }
            match pk {
                k::UNARY_EXPRESSION => yes(op == operator_text(cx, &p) && (op == "+" || op == "-")),
                k::TAGGED_TEMPLATE_EXPRESSION | k::TS_NON_NULL_EXPRESSION => Need::Yes,
                _ if member_p => yes(is_object),
                k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
                k::BINARY_EXPRESSION => yes(first && operator_text(cx, &p) == "**"),
                _ => Need::No,
            }
        }
        k::BINARY_EXPRESSION
        | k::LOGICAL_EXPRESSION
        | k::TS_AS_EXPRESSION
        | k::TS_SATISFIES_EXPRESSION
        | k::TS_TYPE_ASSERTION => {
            let op = operator_text(cx, node);
            if kind == k::BINARY_EXPRESSION {
                if pk == k::UPDATE_EXPRESSION {
                    return Need::Yes;
                }
                if op == "in" && in_for_init(node) {
                    return Need::Yes;
                }
            }
            match pk {
                k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION => yes(!is_cast(kind)),
                k::CONDITIONAL_EXPRESSION => {
                    if is_cast(kind) {
                        Need::Keep
                    } else {
                        Need::No
                    }
                }
                k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
                k::CLASS => yes(super_class),
                k::TS_TYPE_ASSERTION
                | k::TAGGED_TEMPLATE_EXPRESSION
                | k::UNARY_EXPRESSION
                | k::JSX_SPREAD_ATTRIBUTE
                | k::SPREAD_ELEMENT
                | k::AWAIT_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
                | k::UPDATE_EXPRESSION => Need::Yes,
                _ if member_p => yes(is_object),
                k::ASSIGNMENT_EXPRESSION => {
                    yes(first && (kind == k::TS_TYPE_ASSERTION || is_cast(kind)))
                }
                k::LOGICAL_EXPRESSION | k::BINARY_EXPRESSION => {
                    if kind == k::TS_TYPE_ASSERTION {
                        return Need::Keep;
                    }
                    if !is_binaryish(kind) {
                        return Need::Yes;
                    }
                    let pop = operator_text(cx, &p);
                    if pk == k::LOGICAL_EXPRESSION && kind == k::LOGICAL_EXPRESSION {
                        return yes(pop != op);
                    }
                    let (np, pp) = (precedence(&op), precedence(&pop));
                    // The left side of a chain of one precedence prints without its own parentheses.
                    if first && pp == np && should_flatten(&pop, &op) {
                        return Need::No;
                    }
                    yes(pp > np
                        || (pp == np && !first)
                        || (pp == np && !should_flatten(&pop, &op))
                        || (pp < np && op == "%" && (pop == "+" || pop == "-"))
                        || is_bitwise(&pop))
                }
                _ => Need::No,
            }
        }
        k::SEQUENCE_EXPRESSION => match pk {
            k::RETURN_STATEMENT | k::FOR_STATEMENT | k::EXPRESSION_STATEMENT => Need::No,
            k::ARROW_FUNCTION_EXPRESSION => yes(!last),
            _ => Need::Yes,
        },
        k::YIELD_EXPRESSION | k::AWAIT_EXPRESSION => {
            if kind == k::YIELD_EXPRESSION
                && matches!(pk, k::AWAIT_EXPRESSION | k::TS_TYPE_ASSERTION)
            {
                return Need::Yes;
            }
            match pk {
                k::TAGGED_TEMPLATE_EXPRESSION
                | k::UNARY_EXPRESSION
                | k::LOGICAL_EXPRESSION
                | k::SPREAD_ELEMENT
                | k::TS_AS_EXPRESSION
                | k::TS_SATISFIES_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
                | k::BINARY_EXPRESSION => Need::Yes,
                _ if member_p => yes(is_object),
                k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
                k::CONDITIONAL_EXPRESSION => yes(first),
                _ => Need::No,
            }
        }
        k::NUMERIC_LITERAL => yes(is_object),
        k::STRING_LITERAL => yes(pk == k::EXPRESSION_STATEMENT
            && p.parent().is_some_and(|g| {
                matches!(g.kind(), k::PROGRAM | k::BLOCK_STATEMENT | k::FUNCTION_BODY)
            })),
        k::ASSIGNMENT_EXPRESSION => match pk {
            k::ARROW_FUNCTION_EXPRESSION if last => Need::Yes,
            k::EXPRESSION_STATEMENT => yes(node
                .children()
                .next()
                .is_some_and(|l| l.kind() == k::OBJECT_ASSIGNMENT_TARGET)),
            k::FOR_STATEMENT | k::ASSIGNMENT_EXPRESSION => Need::No,
            k::SEQUENCE_EXPRESSION if p.parent().is_some_and(|g| g.kind() == k::FOR_STATEMENT) => {
                Need::No
            }
            _ => Need::Yes,
        },
        k::CONDITIONAL_EXPRESSION => match pk {
            k::TAGGED_TEMPLATE_EXPRESSION
            | k::UNARY_EXPRESSION
            | k::SPREAD_ELEMENT
            | k::BINARY_EXPRESSION
            | k::LOGICAL_EXPRESSION
            | k::AWAIT_EXPRESSION
            | k::JSX_SPREAD_ATTRIBUTE
            | k::TS_TYPE_ASSERTION
            | k::TS_AS_EXPRESSION
            | k::TS_SATISFIES_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION => Need::Yes,
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            k::CONDITIONAL_EXPRESSION => yes(first),
            _ if member_p => yes(is_object),
            _ => Need::No,
        },
        k::FUNCTION => match pk {
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            k::TAGGED_TEMPLATE_EXPRESSION => Need::Yes,
            _ => Need::No,
        },
        k::ARROW_FUNCTION_EXPRESSION => match pk {
            k::BINARY_EXPRESSION
            | k::TS_AS_EXPRESSION
            | k::TS_SATISFIES_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION
            | k::TAGGED_TEMPLATE_EXPRESSION
            | k::UNARY_EXPRESSION
            | k::LOGICAL_EXPRESSION
            | k::AWAIT_EXPRESSION
            | k::TS_TYPE_ASSERTION => Need::Yes,
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            k::CONDITIONAL_EXPRESSION => yes(first),
            _ if member_p => yes(is_object),
            _ => Need::No,
        },
        k::CLASS => {
            let decorated = node
                .children()
                .next()
                .is_some_and(|c| c.kind() == k::DECORATOR);
            yes(decorated || (pk == k::NEW_EXPRESSION && first))
        }
        k::OBJECT_EXPRESSION => Need::No,
        k::CHAIN_EXPRESSION => match pk {
            k::TS_NON_NULL_EXPRESSION => Need::Keep,
            k::TAGGED_TEMPLATE_EXPRESSION => Need::Yes,
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => yes(is_callee),
            _ if member_p => yes(is_object),
            _ => Need::No,
        },
        k::CALL_EXPRESSION
        | k::STATIC_MEMBER_EXPRESSION
        | k::COMPUTED_MEMBER_EXPRESSION
        | k::PRIVATE_FIELD_EXPRESSION
        | k::TAGGED_TEMPLATE_EXPRESSION
        | k::TS_NON_NULL_EXPRESSION => {
            yes(pk == k::NEW_EXPRESSION && first && has_call_in_object_chain(node))
        }
        k::JSX_ELEMENT | k::JSX_FRAGMENT => {
            yes(is_callee || (first && pk == k::BINARY_EXPRESSION && operator_text(cx, &p) == "<"))
        }
        k::IDENTIFIER_REFERENCE
        | k::THIS_EXPRESSION
        | k::ARRAY_EXPRESSION
        | k::TEMPLATE_LITERAL
        | k::REG_EXP_LITERAL
        | k::BOOLEAN_LITERAL
        | k::NULL_LITERAL
        | k::BIG_INT_LITERAL
        | k::SUPER
        | k::IMPORT_EXPRESSION
        | k::NEW_EXPRESSION => Need::No,
        _ => Need::Keep,
    }
}

/// `new (foo())`, `new (a.b().c)`: a call in the callee's object chain would take the arguments.
fn has_call_in_object_chain(node: &SyntaxNode) -> bool {
    let mut n = node.clone();
    loop {
        match n.kind() {
            k::CALL_EXPRESSION => return true,
            k::STATIC_MEMBER_EXPRESSION
            | k::COMPUTED_MEMBER_EXPRESSION
            | k::PRIVATE_FIELD_EXPRESSION
            | k::TAGGED_TEMPLATE_EXPRESSION
            | k::TS_NON_NULL_EXPRESSION => match n.children().next() {
                Some(c) => n = un(&c),
                None => return false,
            },
            _ => return false,
        }
    }
}

/// Is `node` inside the first part of a `for (;;)` head?
fn in_for_init(node: &SyntaxNode) -> bool {
    let mut child = outer(node);
    while let Some(p) = child.parent() {
        if p.kind() == k::FOR_STATEMENT
            && p.children_with_tokens()
                .find(|e| e.as_token().is_some_and(|t| t.kind() == k::SEMICOLON))
                .is_some_and(|semi| child.text_range().end() <= semi.text_range().start())
        {
            return true;
        }
        child = p;
    }
    false
}
