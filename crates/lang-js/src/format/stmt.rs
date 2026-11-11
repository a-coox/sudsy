//! Statement sequences, blocks and control flow.

use super::util::{
    R, clause, elem, inline, inline_split, is_tok, keyword, kids, node_at, node_of, semi,
    shape_error, style, token_of,
};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, group, hardline, indent, line, lit, softline};
use core_format::shapes::{self, BlockStyle};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::PROGRAM => program(node, cx)?,
        k::BLOCK_STATEMENT => shapes::block(cx, node, &block_style(node))?,
        k::FUNCTION_BODY => shapes::block(cx, node, &BlockStyle::default())?,
        k::EXPRESSION_STATEMENT => expression_statement(node, cx)?,
        k::EMPTY_STATEMENT => empty_statement(node, cx)?,
        k::IF_STATEMENT => if_statement(node, cx)?,
        k::FOR_STATEMENT => for_statement(node, cx)?,
        k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT => for_in_of(node, cx)?,
        k::WHILE_STATEMENT => condition_clause(node, cx, true)?,
        k::WITH_STATEMENT => condition_clause(node, cx, false)?,
        k::DO_WHILE_STATEMENT => do_while(node, cx)?,
        k::SWITCH_STATEMENT => switch(node, cx)?,
        k::SWITCH_CASE => switch_case(node, cx)?,
        k::TRY_STATEMENT | k::CATCH_CLAUSE => inline(cx, &kids(cx, node))?,
        k::RETURN_STATEMENT
        | k::THROW_STATEMENT
        | k::BREAK_STATEMENT
        | k::CONTINUE_STATEMENT
        | k::DEBUGGER_STATEMENT => concat(vec![inline(cx, &kids(cx, node))?, semi(cx, node)]),
        k::LABELED_STATEMENT => labeled(node, cx)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// An empty block is `{}` as a function, loop or `catch` body and `{` `}` on two lines anywhere
/// else (Prettier's rule).
fn block_style(node: &SyntaxNode) -> BlockStyle {
    let collapsed = node.parent().is_some_and(|p| match p.kind() {
        k::FOR_STATEMENT | k::WHILE_STATEMENT | k::DO_WHILE_STATEMENT => true,
        // A `catch` without a `finally` after it.
        k::CATCH_CLAUSE => p.parent().is_some_and(|t| {
            !t.children_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.text() == "finally"))
        }),
        _ => false,
    });
    BlockStyle {
        expand_empty: !collapsed,
        ..BlockStyle::default()
    }
}

/// The statements of a file, one per line with the blank lines of the source, then the comments
/// at the end. A token between statements (oxc leaves the decorators of `@dec export class`
/// outside every node) leaves the file as written.
fn program(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let stray = node.children_with_tokens().any(|e| {
        matches!(&e, SyntaxElement::Token(t) if !cx.grammar.is_trivia(t.kind()) && t.kind() != k::EOF)
    });
    if stray {
        return Ok(cx.verbatim_node(node));
    }
    shapes::document(cx, node, 1, None)
}

/// `;` on a line of its own: the output keeps the statement, as its syntax tree has one (a `;`
/// after a block is taken out before the tree is read, see `JsFormatter::rewrite_source`).
fn empty_statement(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    kids(cx, node).first().map_or(Ok(lit(";")), |t| elem(cx, t))
}

fn expression_statement(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let expr = node_at(&kids, 0)?;
    let mut parts = Vec::new();
    // Without `;` at the end of the previous statement, a line starting like this would continue it.
    if !style(cx).semi
        && cx.first_token(expr).is_some_and(|t| {
            matches!(
                t.text().get(..1),
                Some("(" | "[" | "`" | "+" | "-" | "/" | "<")
            )
        })
    {
        parts.push(lit(";"));
    }
    parts.push(cx.format(expr)?);
    parts.push(semi(cx, node));
    Ok(concat(parts))
}

/// `if (test) body else body`.
fn if_statement(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let [kw, open, _, close, _, rest @ ..] = &kids[..] else {
        return Err(shape_error("if"));
    };
    let (test, body) = (node_at(&kids, 2)?, node_at(&kids, 4)?);
    let (lead, kw) = keyword(cx, kw)?;
    let opening = group(concat(vec![
        kw,
        lit(" "),
        elem(cx, open)?,
        parenthesized(cx.format(test)?, true),
        elem(cx, close)?,
        clause(body, cx.format(body)?, false),
    ]));
    let mut parts = vec![lead, opening];
    if let [els, alt] = rest
        && let Some(alt) = node_of(alt)
    {
        parts.push(if body.kind() == k::BLOCK_STATEMENT {
            lit(" ")
        } else {
            hardline()
        });
        parts.push(elem(cx, els)?);
        let printed = cx.format(alt)?;
        parts.push(group(clause(alt, printed, alt.kind() == k::IF_STATEMENT)));
    }
    Ok(concat(parts))
}

/// `while (test) body` and `with (object) body`; `soft` lets the parenthesis content break.
fn condition_clause(node: &SyntaxNode, cx: &FormatCtx<'_>, soft: bool) -> R {
    let kids = kids(cx, node);
    let [kw, open, _, close, _] = &kids[..] else {
        return Err(shape_error("condition"));
    };
    let (test, body) = (node_at(&kids, 2)?, node_at(&kids, 4)?);
    let (lead, kw) = keyword(cx, kw)?;
    Ok(concat(vec![
        lead,
        group(concat(vec![
            kw,
            lit(" "),
            elem(cx, open)?,
            parenthesized(cx.format(test)?, soft),
            elem(cx, close)?,
            clause(body, cx.format(body)?, false),
        ])),
    ]))
}

/// The content between parentheses, indented on its own lines when it does not fit.
fn parenthesized(inner: Doc, soft: bool) -> Doc {
    if soft {
        group(concat(vec![
            indent(concat(vec![softline(), inner])),
            softline(),
        ]))
    } else {
        inner
    }
}

fn do_while(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let [kw, _, wh, open, _, close, ..] = &kids[..] else {
        return Err(shape_error("do while"));
    };
    let (body, test) = (node_at(&kids, 1)?, node_at(&kids, 4)?);
    let (lead, kw) = keyword(cx, kw)?;
    Ok(concat(vec![
        lead,
        group(concat(vec![kw, clause(body, cx.format(body)?, false)])),
        if body.kind() == k::BLOCK_STATEMENT {
            lit(" ")
        } else {
            hardline()
        },
        elem(cx, wh)?,
        lit(" "),
        elem(cx, open)?,
        parenthesized(cx.format(test)?, true),
        elem(cx, close)?,
        semi(cx, node),
    ]))
}

/// `for (init; test; update) body`.
fn for_statement(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let close = kids
        .iter()
        .position(|e| token_of(e).is_some_and(|t| t.kind() == k::R_PAREN))
        .ok_or_else(|| shape_error("for"))?;
    let body = node_at(&kids, close + 1)?;
    let body = clause(body, cx.format(body)?, false);
    let (lead, kw) = keyword(cx, &kids[0])?;
    let open = elem(cx, &kids[1])?;
    // The three parts, told apart by the `;` before them.
    let mut parts: [Doc; 3] = [Doc::Nil, Doc::Nil, Doc::Nil];
    let mut semis = Vec::new();
    let mut at = 0;
    for e in &kids[2..close] {
        if is_tok(e, ";") {
            semis.push(elem(cx, e)?);
            at += 1;
        } else if at < 3 {
            parts[at] = elem(cx, e)?;
        }
    }
    let [init, test, update] = parts;
    let [s1, s2] = <[Doc; 2]>::try_from(semis).map_err(|_| shape_error("for"))?;
    let close = elem(cx, &kids[close])?;
    if init.is_nil() && test.is_nil() && update.is_nil() {
        let head = concat(vec![kw, lit(" "), open, s1, s2, close, body]);
        return Ok(concat(vec![lead, group(head)]));
    }
    let inner = concat(vec![init, s1, line(), test, s2, line(), update]);
    Ok(concat(vec![
        lead,
        group(concat(vec![
            kw,
            lit(" "),
            open,
            group(concat(vec![
                indent(concat(vec![softline(), inner])),
                softline(),
            ])),
            close,
            body,
        ])),
    ]))
}

/// `for (left in right) body`, `for await (left of right) body`.
fn for_in_of(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let find = |kind, rev: bool| {
        let is = |e: &SyntaxElement| token_of(e).is_some_and(|t| t.kind() == kind);
        if rev {
            kids.iter().rposition(is)
        } else {
            kids.iter().position(is)
        }
        .ok_or_else(|| shape_error("for in"))
    };
    let (open, close) = (find(k::L_PAREN, false)?, find(k::R_PAREN, true)?);
    let body = node_at(&kids, close + 1)?;
    let (lead, head) = inline_split(cx, &kids[..open])?;
    Ok(concat(vec![
        lead,
        group(concat(vec![
            head,
            lit(" "),
            elem(cx, &kids[open])?,
            inline(cx, &kids[open + 1..close])?,
            elem(cx, &kids[close])?,
            clause(body, cx.format(body)?, false),
        ])),
    ]))
}

/// `switch (discriminant) { cases }`.
fn switch(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let [kw, open, disc, close, lbrace, .., rbrace] = &kids[..] else {
        return Err(shape_error("switch"));
    };
    let (Some(lbrace), Some(rbrace)) = (token_of(lbrace), token_of(rbrace)) else {
        return Err(shape_error("switch"));
    };
    let (lead, kw) = keyword(cx, kw)?;
    let head = group(concat(vec![
        kw,
        lit(" "),
        elem(cx, open)?,
        indent(concat(vec![softline(), elem(cx, disc)?])),
        softline(),
        elem(cx, close)?,
    ]));
    let style = BlockStyle {
        expand_empty: true,
        ..BlockStyle::default()
    };
    let body = shapes::block_between(cx, node, lbrace, rbrace, &style)?;
    Ok(concat(vec![lead, head, lit(" "), body]))
}

/// `case test:` or `default:` and the statements under it.
fn switch_case(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let colon = kids
        .iter()
        .position(|e| token_of(e).is_some_and(|t| t.kind() == k::COLON))
        .ok_or_else(|| shape_error("case"))?;
    let head = inline(cx, &kids[..=colon])?;
    let body: Vec<SyntaxNode> = kids[colon + 1..]
        .iter()
        .filter_map(|e| node_of(e).cloned())
        .collect();
    Ok(match &body[..] {
        [] => head,
        [only] if only.kind() == k::BLOCK_STATEMENT => {
            concat(vec![head, lit(" "), cx.format(only)?])
        }
        _ => concat(vec![
            head,
            indent(concat(vec![
                hardline(),
                shapes::statement_sequence(cx, &body, 1)?,
            ])),
        ]),
    })
}

/// `label: statement`.
fn labeled(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let kids = kids(cx, node);
    let [label, colon, body] = &kids[..] else {
        return Err(shape_error("label"));
    };
    let body_node = node_at(&kids, 2)?;
    let sep = if body_node.kind() == k::EMPTY_STATEMENT {
        cx.format(body_node)?
    } else {
        concat(vec![lit(" "), elem(cx, body)?])
    };
    Ok(concat(vec![elem(cx, label)?, elem(cx, colon)?, sep]))
}
