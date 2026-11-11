//! The small expressions: unary, update, `await`, `yield`, spread and sequence expressions, and
//! `return` / `throw` with their argument.

use super::ast::{has_leading_comment, is_binaryish, is_member, outer, parent, starts_with, un};
use super::util::{R, elem, keyword, kids, node_of, semi, shape_error, token_of};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, group, hardline, if_break, indent, line, lit, softline};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::UNARY_EXPRESSION => unary(node, cx)?,
        k::UPDATE_EXPRESSION | k::SPREAD_ELEMENT => cx.concat_children(node)?,
        k::AWAIT_EXPRESSION => await_expression(node, cx)?,
        k::YIELD_EXPRESSION => yield_expression(node, cx)?,
        k::SEQUENCE_EXPRESSION => sequence(node, cx)?,
        k::RETURN_STATEMENT | k::THROW_STATEMENT => return_or_throw(node, cx)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

fn unary(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let (Some(op), Some(arg)) = (elems.first().and_then(token_of), node.last_child()) else {
        return Err(shape_error("unary"));
    };
    let word = op.text().ends_with(|c: char| c.is_ascii_alphabetic());
    let arg_doc = if has_leading_comment(cx, &arg) {
        group(concat(vec![
            lit("("),
            indent(concat(vec![softline(), cx.format(&arg)?])),
            softline(),
            lit(")"),
        ]))
    } else {
        cx.format(&arg)?
    };
    Ok(concat(vec![
        cx.token(op),
        if word { lit(" ") } else { Doc::Nil },
        arg_doc,
    ]))
}

fn await_expression(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let [kw, arg] = &elems[..] else {
        return cx.concat_children(node);
    };
    let parts = concat(vec![elem(cx, kw)?, lit(" "), elem(cx, arg)?]);
    let held = outer(node);
    let p = parent(node);
    let head = p.as_ref().is_some_and(|p| {
        p.first_child().as_ref() == Some(&held)
            && (matches!(p.kind(), k::CALL_EXPRESSION) || is_member(p.kind()))
    });
    if !head {
        return Ok(parts);
    }
    let indented = concat(vec![indent(concat(vec![softline(), parts])), softline()]);
    let enclosing = node
        .ancestors()
        .skip(1)
        .find(|a| matches!(a.kind(), k::AWAIT_EXPRESSION | k::BLOCK_STATEMENT));
    let leftmost_of_await = enclosing.as_ref().is_some_and(|a| {
        a.kind() == k::AWAIT_EXPRESSION
            && a.last_child()
                .is_some_and(|arg| starts_with(&arg, &|n| n == node))
    });
    Ok(if leftmost_of_await {
        indented
    } else {
        group(indented)
    })
}

fn yield_expression(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let mut parts = Vec::new();
    for e in &elems {
        if node_of(e).is_some() {
            parts.push(lit(" "));
        }
        parts.push(elem(cx, e)?);
    }
    Ok(concat(parts))
}

fn sequence(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let in_statement = parent(node)
        .is_some_and(|p| matches!(p.kind(), k::EXPRESSION_STATEMENT | k::FOR_STATEMENT));
    let mut items = Vec::new();
    let mut comma = None;
    for e in &elems {
        match e {
            e if token_of(e).is_some() => comma = Some(elem(cx, e)?),
            e => items.push((comma.take(), elem(cx, e)?)),
        }
    }
    let mut it = items.into_iter();
    let Some((_, head)) = it.next() else {
        return Err(shape_error("sequence"));
    };
    if in_statement {
        let mut parts = vec![head];
        for (comma, d) in it {
            parts.push(comma.unwrap_or_else(|| lit(",")));
            parts.push(indent(concat(vec![line(), d])));
        }
        return Ok(group(concat(parts)));
    }
    let sep = |c: Option<Doc>| concat(vec![c.unwrap_or_else(|| lit(",")), line()]);
    let mut parts = vec![head];
    for (comma, d) in it {
        parts.push(sep(comma));
        parts.push(d);
    }
    Ok(group(concat(parts)))
}

/// `return a;`, `throw b;`: a binary or sequence argument that does not fit goes in parentheses
/// on its own lines.
fn return_or_throw(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let (lead, kw) = keyword(cx, &elems[0])?;
    let arg = elems.get(1).and_then(node_of);
    let Some(arg) = arg else {
        return Ok(concat(vec![lead, kw, semi(cx, node)]));
    };
    let arg_u = un(arg);
    let asi = super::util::detach_asi(cx, node);
    let printed = cx.format(arg)?;
    let body = if has_leading_own_line_comment(cx, &arg_u) {
        concat(vec![
            lit(" ("),
            indent(concat(vec![hardline(), printed])),
            hardline(),
            lit(")"),
        ])
    } else if is_binaryish(arg_u.kind()) || arg_u.kind() == k::SEQUENCE_EXPRESSION {
        group(concat(vec![
            lit(" "),
            if_break(lit("("), Doc::Nil),
            indent(concat(vec![softline(), printed])),
            softline(),
            if_break(lit(")"), Doc::Nil),
        ]))
    } else {
        concat(vec![lit(" "), printed])
    };
    let end = super::util::reattach_asi(cx, asi);
    Ok(concat(vec![lead, kw, body, semi(cx, node), end]))
}

fn has_leading_own_line_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.first_token(n).is_some_and(|t| {
        core_format::comments::leading_comments(cx.grammar, &t)
            .iter()
            .any(|c| c.breaks_after > 0)
    })
}
