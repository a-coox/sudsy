//! Conditional expressions: `test ? a : b` on one line or one branch per line, chains of them
//! flat, and the JSX mode that wraps branches in parentheses (Prettier's `printTernary`).

use super::ast::{is_jsx, is_member, outer, parent, un};
use super::util::{R, shape_error};
use crate::generated::kind as k;
use core_format::doc::{
    Doc, align, concat, group, hardline, if_break, indent, line, lit, softline,
};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::CONDITIONAL_EXPRESSION {
        return Ok(None);
    }
    Ok(Some(ternary(node, cx)?))
}

fn is_cond(n: &SyntaxNode) -> bool {
    n.kind() == k::CONDITIONAL_EXPRESSION
}

fn chain_contains_jsx(top: &SyntaxNode) -> bool {
    let mut todo = vec![top.clone()];
    while let Some(c) = todo.pop() {
        for part in c.children() {
            let part = un(&part);
            if is_jsx(part.kind()) {
                return true;
            }
            if is_cond(&part) {
                todo.push(part);
            }
        }
    }
    false
}

/// The part of `parent` its text starts with (what Prettier calls its left side).
fn left_side(parent: &SyntaxNode) -> Option<SyntaxNode> {
    match parent.kind() {
        k::EXPRESSION_STATEMENT
        | k::SEQUENCE_EXPRESSION
        | k::BINARY_EXPRESSION
        | k::LOGICAL_EXPRESSION
        | k::ASSIGNMENT_EXPRESSION
        | k::CONDITIONAL_EXPRESSION
        | k::TAGGED_TEMPLATE_EXPRESSION
        | k::CALL_EXPRESSION
        | k::NEW_EXPRESSION
        | k::AWAIT_EXPRESSION
        | k::TS_AS_EXPRESSION
        | k::TS_SATISFIES_EXPRESSION
        | k::TS_NON_NULL_EXPRESSION
        | k::CHAIN_EXPRESSION => parent.first_child(),
        kind if is_member(kind) => parent.first_child(),
        k::UNARY_EXPRESSION | k::SPREAD_ELEMENT | k::YIELD_EXPRESSION => parent.last_child(),
        _ => None,
    }
}

/// `(a ? b : c).d()`: the parenthesized chain breaks inside its parentheses, indented.
fn should_extra_indent(node: &SyntaxNode) -> bool {
    let mut child = node.clone();
    let chain_parent = loop {
        let Some(p) = parent(&child) else {
            return false;
        };
        let first = p.first_child().as_ref() == Some(&outer(&child));
        match p.kind() {
            k::CHAIN_EXPRESSION | k::TS_NON_NULL_EXPRESSION => child = p,
            k::CALL_EXPRESSION if first => child = p,
            kind if is_member(kind) && first => child = p,
            k::NEW_EXPRESSION | k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION if first => {
                let gp = parent(&p);
                child = p;
                break gp;
            }
            _ => break Some(p),
        }
    };
    child != *node
        && chain_parent
            .and_then(|p| left_side(&p))
            .is_some_and(|l| l == outer(&child))
}

fn ternary(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let parts: Vec<SyntaxNode> = node.children().collect();
    let [test, cons, alt] = &parts[..] else {
        return Err(shape_error("conditional"));
    };
    let toks: Vec<_> = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !cx.grammar.is_trivia(t.kind()))
        .collect();
    let (Some(q), Some(colon)) = (toks.first(), toks.get(1)) else {
        return Err(shape_error("conditional"));
    };
    let p = parent(node);
    let held = outer(node);
    let same_parent = p.as_ref().is_some_and(is_cond);
    let is_parent_test =
        same_parent && p.as_ref().and_then(|p| p.first_child()).as_ref() == Some(&held);
    let is_alt_of_parent =
        same_parent && p.as_ref().and_then(|p| p.last_child()).as_ref() == Some(&held);
    let mut force_no_indent = false;
    // The top of the chain of conditionals this one is in (through branches, not tests).
    let mut top = node.clone();
    let first_non_cond = loop {
        match parent(&top) {
            Some(up) if is_cond(&up) && up.first_child().as_ref() != Some(&outer(&top)) => top = up,
            other => break other,
        }
    };
    let (cons_u, alt_u, test_u) = (un(cons), un(alt), un(test));
    let jsx_mode = is_jsx(test_u.kind())
        || is_jsx(cons_u.kind())
        || is_jsx(alt_u.kind())
        || chain_contains_jsx(&top);
    let tab = super::call::tab_width(cx);
    let mut out = Vec::new();
    let (cons_doc, alt_doc) = (cx.format(cons)?, cx.format(alt)?);
    if jsx_mode {
        force_no_indent = true;
        let wrap = |d: Doc| {
            concat(vec![
                if_break(lit("("), Doc::Nil),
                indent(concat(vec![softline(), d])),
                softline(),
                if_break(lit(")"), Doc::Nil),
            ])
        };
        let nil = |n: &SyntaxNode| {
            n.kind() == k::NULL_LITERAL
                || (n.kind() == k::IDENTIFIER_REFERENCE
                    && super::ast::name_of(cx, n).is_some_and(|x| x == "undefined"))
        };
        out.extend([lit(" "), cx.token(q), lit(" ")]);
        out.push(if nil(&cons_u) {
            cons_doc
        } else {
            wrap(cons_doc)
        });
        out.extend([lit(" "), cx.token(colon), lit(" ")]);
        out.push(if is_cond(&alt_u) || nil(&alt_u) {
            alt_doc
        } else {
            wrap(alt_doc)
        });
    } else {
        let cons_cond = is_cond(&cons_u);
        let paren = |open: bool| {
            if !cons_cond {
                Doc::Nil
            } else if open {
                if_break(Doc::Nil, lit("("))
            } else {
                if_break(Doc::Nil, lit(")"))
            }
        };
        let branches = concat(vec![
            line(),
            cx.token(q),
            lit(" "),
            paren(true),
            // A line comment after `?` ends the line: the branch starts on the next one.
            align(2, concat(vec![comment_break(cx, q), cons_doc])),
            paren(false),
            line(),
            cx.token(colon),
            lit(" "),
            if is_cond(&alt_u) {
                concat(vec![comment_break(cx, colon), alt_doc])
            } else {
                align(2, concat(vec![comment_break(cx, colon), alt_doc]))
            },
        ]);
        out.push(if !same_parent || is_alt_of_parent || is_parent_test {
            branches
        } else {
            align(tab.saturating_sub(2), branches)
        });
    }
    let test_doc = {
        let d = cx.format(test)?;
        if is_alt_of_parent { align(2, d) } else { d }
    };
    let break_closing_paren = !jsx_mode
        && p.as_ref().is_some_and(|p| {
            matches!(
                p.kind(),
                k::STATIC_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION
            )
        });
    let extra_indent = should_extra_indent(node);
    let body = concat(vec![
        test_doc,
        if force_no_indent {
            concat(out)
        } else {
            indent(concat(out))
        },
        if break_closing_paren && !extra_indent {
            softline()
        } else {
            Doc::Nil
        },
    ]);
    let result = if p == first_non_cond {
        group(body)
    } else {
        body
    };
    Ok(if is_parent_test || extra_indent {
        group(concat(vec![
            indent(concat(vec![line(), result])),
            softline(),
        ]))
    } else {
        result
    })
}

fn comment_break(cx: &FormatCtx<'_>, tok: &core_syntax::SyntaxToken) -> Doc {
    let line_comment = core_format::comments::trailing_comments(cx.grammar, tok)
        .iter()
        .any(|c| c.kind == core_format::comments::CommentKind::Line);
    if line_comment { hardline() } else { Doc::Nil }
}
