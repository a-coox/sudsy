//! The generic layout: a node's significant children one after the other, separated by the
//! space, line break or nothing that [`Sep`] decides.

use super::flat;
use super::options::{
    brace_on_new_line, clauses_on_closing_line, constructor_initializer_on_new_line,
    creation_brace_on_new_line, do_while_on_closing_line, expand_empty_bodies, hanging_base_list,
    keeps_attribute_line, label_outdent, nested_brace_on_new_line, space_after_delegate,
    space_after_empty_last_for_clause, space_between_empty_for_clauses,
};
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
    /// The brace of a body of this element (a name of `brace_same_line`): a line break, or a space
    /// with `brace_newline: none`.
    Brace(&'static str),
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

/// The `brace_same_line` element of a block whose parent has this kind.
pub fn block_element(parent: SyntaxKind) -> &'static str {
    match parent {
        k::METHOD_DECLARATION
        | k::CONSTRUCTOR_DECLARATION
        | k::DESTRUCTOR_DECLARATION
        | k::OPERATOR_DECLARATION
        | k::CONVERSION_OPERATOR_DECLARATION
        | k::LOCAL_FUNCTION_STATEMENT => "methods",
        k::GET_ACCESSOR_DECLARATION
        | k::SET_ACCESSOR_DECLARATION
        | k::INIT_ACCESSOR_DECLARATION
        | k::ADD_ACCESSOR_DECLARATION
        | k::REMOVE_ACCESSOR_DECLARATION
        | k::UNKNOWN_ACCESSOR_DECLARATION => "accessors",
        k::SIMPLE_LAMBDA_EXPRESSION | k::PARENTHESIZED_LAMBDA_EXPRESSION => "lambdas",
        k::ANONYMOUS_METHOD_EXPRESSION => "anonymous_methods",
        _ => "control_blocks",
    }
}

/// The `brace_same_line` element of a node that holds its own braces (a type, namespace, `switch`).
pub fn braced_element(kind: SyntaxKind) -> &'static str {
    match kind {
        k::SWITCH_STATEMENT | k::SWITCH_EXPRESSION => "control_blocks",
        _ => "types",
    }
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

/// Does the empty body in braces of `node` stay on the line of its head? Not with
/// `empty_braces: expanded`, except for the body of a lambda or an anonymous method.
pub fn keeps_empty_inline(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    !expand_empty_bodies(cx.options)
        || node.parent().is_some_and(|p| {
            matches!(
                p.kind(),
                k::SIMPLE_LAMBDA_EXPRESSION
                    | k::PARENTHESIZED_LAMBDA_EXPRESSION
                    | k::ANONYMOUS_METHOD_EXPRESSION
            )
        })
}

pub fn block_is_empty(cx: &FormatCtx<'_>, block: &SyntaxNode) -> bool {
    let opens = direct_tokens(block, k::OPEN_BRACE_TOKEN);
    let closes = direct_tokens(block, k::CLOSE_BRACE_TOKEN);
    match (opens.first(), closes.last()) {
        (Some(o), Some(c)) => is_empty_braces(cx, o, c),
        _ => false,
    }
}

/// The separator between `prev` and `next`, children of a node of kind `parent`. Inside what
/// `preserve_single_line_*` keeps on one line, line breaks become spaces; so does the break in
/// front of the one-line body of a member, accessor or lambda.
pub fn separator(
    cx: &FormatCtx<'_>,
    parent: &SyntaxNode,
    prev: &SyntaxElement,
    next: &SyntaxElement,
) -> Sep {
    let sep = separator_raw(cx, parent, prev, next);
    if matches!(
        sep,
        Sep::Hard | Sep::HardIndented | Sep::Embedded | Sep::Brace(_)
    ) {
        let body = matches!(next, SyntaxElement::Node(n) if n.kind() == k::BLOCK
            && (block_element(parent.kind()) != "control_blocks"
                || matches!(parent.kind(), k::CATCH_CLAUSE | k::FINALLY_CLAUSE))
            && flat::is_root(cx, n));
        if body || flat::inside(cx, parent) {
            return Sep::Space;
        }
    }
    sep
}

fn separator_raw(
    cx: &FormatCtx<'_>,
    parent: &SyntaxNode,
    prev: &SyntaxElement,
    next: &SyntaxElement,
) -> Sep {
    let pk = parent.kind();
    if let SyntaxElement::Node(n) = next {
        let nk = n.kind();
        if constructor_initializer_on_new_line(cx.options)
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
        let property_pattern = nk == k::PROPERTY_PATTERN_CLAUSE || bare_property_pattern;
        if (hanging_base_list(cx.options) && nk == k::BASE_LIST)
            || (nested_brace_on_new_line(cx.options) && property_pattern)
        {
            return Sep::None; // these start with a `line` of their own
        }
        if nk == k::BLOCK {
            return if block_is_empty(cx, n) && keeps_empty_inline(cx, n) {
                Sep::Space
            } else {
                Sep::Brace(block_element(pk))
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
                | k::WITH_EXPRESSION => {
                    if !creation_brace_on_new_line(cx.options) {
                        Sep::Space
                    } else {
                        Sep::None
                    }
                }
                _ => Sep::Space,
            };
        }
        if nk == k::ACCESSOR_LIST {
            return if super::braces::accessors_inline(cx, n) {
                Sep::Space
            } else {
                Sep::Brace("properties")
            };
        }
        if has_embedded_body(pk)
            && tags::has(cx.grammar, nk, Tag::Statement)
            && !(pk == k::ELSE_CLAUSE && nk == k::IF_STATEMENT)
        {
            return Sep::Embedded;
        }
        if matches!(nk, k::ELSE_CLAUSE | k::CATCH_CLAUSE | k::FINALLY_CLAUSE) {
            return if clauses_on_closing_line(cx.options, nk) && clause_can_cuddle(cx, prev, n) {
                Sep::Space
            } else {
                Sep::Hard
            };
        }
        if nk == k::TYPE_PARAMETER_CONSTRAINT_CLAUSE {
            return Sep::HardIndented;
        }
        // A line comment after `)` stays there: `=>` starts the next line.
        if nk == k::ARROW_EXPRESSION_CLAUSE
            && match prev {
                SyntaxElement::Node(p) => cx.last_token(p),
                SyntaxElement::Token(t) => Some(t.clone()),
            }
            .is_some_and(|t| core_format::shapes::ends_in_line_comment(cx, &t))
        {
            return Sep::HardIndented;
        }
        if pk == k::LABELED_STATEMENT && tags::has(cx.grammar, nk, Tag::Statement) {
            return Sep::Hard;
        }
    }
    if let SyntaxElement::Node(p) = prev
        && p.kind() == k::ATTRIBUTE_LIST
    {
        // `member_attribute_line: keep`: an attribute list the source leaves on its member's line
        // stays there.
        let first_of_next = match next {
            SyntaxElement::Node(n) => cx.first_token(n),
            SyntaxElement::Token(t) => Some(t.clone()),
        };
        let kept = keeps_attribute_line(cx.options)
            && matches!((cx.last_token(p), first_of_next), (Some(a), Some(b))
            if !cx.has_line_break(
                parent,
                core_syntax::TextRange::new(a.text_range().end(), b.text_range().start()),
            ));
        return if inline_attributes(pk) || kept {
            Sep::Space
        } else {
            Sep::Hard
        };
    }
    if pk == k::DO_STATEMENT
        && matches!(next, SyntaxElement::Token(t) if t.kind() == k::WHILE_KEYWORD)
    {
        // `do_while_line: same_line` keeps `} while (...)` on one line.
        return if do_while_on_closing_line(cx.options) {
            Sep::Space
        } else {
            Sep::Hard
        };
    }
    if pk == k::ANONYMOUS_METHOD_EXPRESSION
        && !space_after_delegate(cx.options)
        && matches!(prev, SyntaxElement::Token(t) if t.kind() == k::DELEGATE_KEYWORD)
        && matches!(next, SyntaxElement::Node(n) if n.kind() == k::PARAMETER_LIST)
    {
        return Sep::None; // `delegate(int x)`
    }
    if pk == k::FOR_STATEMENT
        && matches!(prev, SyntaxElement::Token(t) if t.kind() == k::SEMICOLON_TOKEN)
        && match next {
            SyntaxElement::Token(t) if t.kind() == k::SEMICOLON_TOKEN => {
                space_between_empty_for_clauses(cx.options) // `for (; ;)`
            }
            SyntaxElement::Token(t) if t.kind() == k::CLOSE_PAREN_TOKEN => {
                space_after_empty_last_for_clause(cx.options) // `for (;; )`
            }
            _ => false,
        }
    {
        return Sep::Space;
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
        Sep::Brace(element) => {
            if brace_on_new_line(cx.options, element) {
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

/// `label:` moved out by `label_indent`, then the labeled statement on the next line.
pub fn labeled_statement(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    if els.len() < 3 {
        return seq_of(cx, node, &els);
    }
    Ok(doc::concat(vec![
        doc::outdent(label_outdent(cx.options), seq_of(cx, node, &els[..2])?),
        doc::hardline(),
        seq_of(cx, node, &els[2..])?,
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
        // `foreach ( ... )`: everything up to the closing parenthesis may move below `(`.
        let foreach_close = match e {
            SyntaxElement::Token(open)
                if open.kind() == k::OPEN_PAREN_TOKEN
                    && matches!(pk, k::FOR_EACH_STATEMENT | k::FOR_EACH_VARIABLE_STATEMENT) =>
            {
                els[i..].iter().position(
                    |x| matches!(x, SyntaxElement::Token(t) if t.kind() == k::CLOSE_PAREN_TOKEN),
                )
            }
            _ => None,
        };
        let body = match (condition, foreach_close) {
            (Some((open, expr, close)), _) => paren_group(cx, open, expr, close)?,
            (None, Some(n)) => {
                let (SyntaxElement::Token(open), SyntaxElement::Token(close)) = (e, &els[i + n])
                else {
                    unreachable!("a parenthesis token")
                };
                doc::concat(vec![
                    doc::group(doc::concat(vec![
                        cx.token(open),
                        doc::indent(doc::concat(vec![
                            doc::softline(),
                            seq_of(cx, node, &els[i + 1..i + n])?,
                        ])),
                        doc::softline(),
                        cx.leading_comments_doc(close),
                        cx.token_text(close),
                    ])),
                    cx.trailing_comments_doc(close),
                ])
            }
            (None, None) => element_doc(cx, e)?,
        };
        if sep == Sep::Embedded {
            parts.push(doc::indent(doc::concat(vec![doc::hardline(), body])));
        } else {
            parts.push(sep_doc(cx, sep));
            parts.push(body);
        }
        i += match (condition.is_some(), foreach_close) {
            (true, _) => 3,
            (_, Some(n)) => n + 1,
            _ => 1,
        };
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

/// `} else {`: only after a block, and only when no comment sits between the `}` and the keyword
/// (a line comment there would swallow the clause).
fn clause_can_cuddle(cx: &FormatCtx<'_>, prev: &SyntaxElement, clause: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let ends_in_block = match prev {
        SyntaxElement::Node(n) => n.kind() == k::BLOCK || n.kind() == k::CATCH_CLAUSE,
        SyntaxElement::Token(_) => false,
    };
    let (Some(last), Some(first)) = (last_tok(cx, prev), cx.first_token(clause)) else {
        return false;
    };
    ends_in_block
        && core_format::comments::trailing_comments(g, &last).is_empty()
        && core_format::comments::leading_comments(g, &first).is_empty()
}
