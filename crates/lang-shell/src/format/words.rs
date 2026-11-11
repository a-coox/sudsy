//! Word-like nodes (string, concatenation, test, substitution): written as they are, except that
//! the commands inside a `$(...)` are laid out like statements.

use super::{
    E, block_list, breaks, dangling, elems, inline_list, kw_lead, max_blank, opening_blank,
    unpadded,
};
use crate::generated::kind as k;
use core_format::comments;
use core_format::context::verbatim_text;
use core_format::{Doc, FormatCtx, FormatError, doc};
use core_syntax::{SyntaxElement, SyntaxNode, TextRange, TextSize};

/// The node as written when it holds no `$(...)`.
pub fn words(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    if node.kind() != k::TEST
        && !(node.descendants_with_tokens())
            .any(|e| matches!(e.kind(), k::SUBST | k::ARRAY | k::ARITH))
    {
        return Ok(cx.verbatim_node(node));
    }
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return Ok(Doc::Nil);
    };
    Ok(doc::concat(vec![
        cx.leading_comments_doc(&first),
        inner(
            cx,
            node,
            (first.text_range().start(), last.text_range().end()),
        )?,
        cx.trailing_comments_doc(&last),
    ]))
}

/// The children of `node` before `end`: text as written, substitutions laid out; the trivia after
/// the node's last token only when more of the parent follows.
fn inner(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    span: (TextSize, TextSize),
) -> Result<Doc, FormatError> {
    if matches!(node.kind(), k::SUBST | k::ARRAY) {
        let trivia = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| cx.grammar.is_trivia(t.kind()) && in_span(t, span))
            .filter(|t| {
                cx.last_token(node)
                    .is_some_and(|l| t.text_range().start() >= l.text_range().end())
            })
            .map(|t| t.text().to_string())
            .collect::<String>();
        let d = if node.kind() == k::SUBST {
            subst(cx, node)?
        } else {
            array(cx, node)?
        };
        return Ok(doc::concat(vec![d, verbatim_text(&trivia)]));
    }
    // A test: its blanks are one blank, and the lines after a line break are indented one level.
    let test = node.kind() == k::TEST;
    let (mut head, mut tail, mut broke, mut indentation) = (Vec::new(), Vec::new(), false, false);
    let mut els = Vec::new();
    for el in node.children_with_tokens() {
        if test && let SyntaxElement::Node(n) = &el {
            // The blanks around a child are its leading and trailing trivia: they belong to the test.
            let trivia =
                |c: SyntaxElement| c.into_token().filter(|t| cx.grammar.is_trivia(t.kind()));
            let lead: Vec<_> = n.children_with_tokens().map_while(trivia).collect();
            let mut trail: Vec<_> = n
                .children_with_tokens()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map_while(trivia)
                .collect();
            trail.reverse();
            let from = lead
                .last()
                .map_or(span.0, |t| t.text_range().end())
                .max(span.0);
            let to = trail
                .first()
                .map_or(span.1, |t| t.text_range().start())
                .min(span.1);
            els.extend(
                lead.into_iter()
                    .map(|t| (SyntaxElement::Token(t), span.0, span.1)),
            );
            els.push((el, from, to));
            els.extend(
                trail
                    .into_iter()
                    .map(|t| (SyntaxElement::Token(t), span.0, span.1)),
            );
        } else {
            els.push((el, span.0, span.1));
        }
    }
    for (el, from, to) in els {
        let span = (from, to);
        let d = match el {
            SyntaxElement::Token(t) if in_span(&t, span) => {
                let plain = t.kind() == k::WHITESPACE && !t.text().contains('\\');
                if test && t.kind() == k::NEWLINE {
                    (broke, indentation) = (true, true);
                    tail.push(doc::hardline());
                    continue;
                }
                if test && plain && std::mem::take(&mut indentation) {
                    continue;
                }
                indentation = false;
                verbatim_text(&if t.kind() == k::ARITH {
                    unpadded(t.text(), true)
                } else if test && plain {
                    " ".to_string()
                } else {
                    t.text().to_string()
                })
            }
            SyntaxElement::Node(n)
                if n.text_range().end() > span.0 && n.text_range().start() < span.1 =>
            {
                indentation = false;
                if matches!(
                    n.kind(),
                    k::STRING | k::CONCAT | k::SUBST | k::TEST | k::ARRAY
                ) {
                    inner(cx, &n, span)?
                } else {
                    let to = n.text_range().end().min(span.1) - n.text_range().start();
                    verbatim_text(&n.text().slice(TextRange::new(0.into(), to)).to_string())
                }
            }
            _ => continue,
        };
        if broke { &mut tail } else { &mut head }.push(d);
    }
    head.push(doc::indent(doc::concat(tail)));
    Ok(doc::concat(head))
}

/// `2> file` is `2>file`: no blanks between the operator and its target, except where the target
/// would run into the operator (`< <(cmd)`, `> -x`).
pub fn redirect(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    if es.iter().any(|e| e.cont) {
        return Ok(cx.verbatim_node(node));
    }
    let mut parts = Vec::new();
    let mut prev: Option<E> = None;
    for e in es {
        let operator = |e: &E| {
            e.tok()
                .is_some_and(|t| t.text().chars().all(|c| "<>&|-".contains(c)))
        };
        if let Some(p) = &prev {
            // The words after the target (`>&2 'a b'`) are arguments of the command.
            let blank = p.last(cx).and_then(|t| t.next_token()) != e.first(cx);
            let special = (e.first(cx)).is_some_and(|t| t.text().starts_with(['<', '>', '-', '(']));
            if blank && (special || !operator(p)) {
                parts.push(doc::lit(" "));
            }
        }
        parts.push(e.doc(cx)?);
        prev = Some(e);
    }
    Ok(doc::concat(parts))
}

/// `(a b)`: the elements on the lines of the source, those after the first line break indented;
/// a line break after `(` or before `)` stays. A continuation keeps the array as written.
fn array(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let (Some(open), Some(close)) = (es.first().and_then(E::tok), es.last().and_then(E::tok))
    else {
        return Ok(cx.verbatim_node(node));
    };
    if open.text() != "(" || close.text() != ")" || es.iter().any(|e| e.cont) {
        let to = close.text_range().end() - node.text_range().start();
        return Ok(verbatim_text(
            &node.text().slice(TextRange::new(0.into(), to)).to_string(),
        ));
    }
    let body = &es[1..es.len() - 1];
    let mut head = vec![cx.token_text(open)];
    let mut tail = Vec::new();
    let mut substs = false;
    let mut prev = open.clone();
    for (i, e) in body.iter().enumerate() {
        let brk = breaks(cx, &prev);
        let in_head = !brk && tail.is_empty();
        // shfmt indents the lines after a `$(...)` on the first line no deeper than the array.
        substs |= in_head
            && e.el
                .as_node()
                .is_some_and(|n| n.descendants().any(|d| d.kind() == k::SUBST));
        let target = if in_head { &mut head } else { &mut tail };
        if brk {
            let first = e.first(cx).expect("an element has a token");
            let blank = comments::blank_lines_before(cx.grammar, &first).min(max_blank(cx));
            target.extend((0..=blank).map(|_| doc::hardline()));
        } else if i > 0 {
            target.push(doc::lit(" "));
        }
        target.push(e.doc(cx)?);
        prev = e.last(cx).expect("an element has a token");
    }
    if breaks(cx, &prev) {
        tail.push(dangling(cx, Some(close.clone()), !body.is_empty()));
    }
    if substs {
        head.push(doc::concat(tail));
    } else if !tail.is_empty() {
        head.push(doc::indent(doc::concat(tail)));
    }
    if breaks(cx, &prev) {
        head.extend([doc::hardline(), kw_lead(cx, close)]);
    } else {
        head.push(cx.token_text(close));
    }
    Ok(doc::concat(head))
}

fn in_span(t: &core_syntax::SyntaxToken, span: (TextSize, TextSize)) -> bool {
    t.text_range().start() >= span.0 && t.text_range().start() < span.1
}

/// `$(...)`: the commands one per line and indented when there are several or the source breaks
/// the line after `$(` or before `)`; else on one line without the blanks inside. Backticks and
/// a substitution with a continuation or a heredoc are kept as written.
fn subst(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let es = elems(cx, node);
    let (Some(open), Some(close)) = (es.first().and_then(E::tok), es.last().and_then(E::tok))
    else {
        return Ok(cx.verbatim_node(node));
    };
    let body = &es[1..es.len() - 1];
    if !matches!(open.text(), "$(" | "<(" | ">(")
        || close.text() != ")"
        || es.iter().any(|e| e.cont)
    {
        let to = close.text_range().end() - node.text_range().start();
        return Ok(verbatim_text(
            &node.text().slice(TextRange::new(0.into(), to)).to_string(),
        ));
    }
    let last = body.iter().rev().find_map(|e| e.last(cx));
    let block = body.iter().filter(|e| e.el.as_node().is_some()).count() > 1
        || breaks(cx, open)
        || last.is_some_and(|t| breaks(cx, &t));
    let mut parts = vec![cx.token_text(open)];
    if block {
        let inner = vec![
            doc::hardline(),
            opening_blank(cx, body),
            block_list(cx, body)?,
            dangling(cx, Some(close.clone()), true),
        ];
        parts.push(doc::indent(doc::concat(inner)));
        parts.extend([doc::hardline(), kw_lead(cx, close)]);
    } else {
        parts.extend([inline_list(cx, body, false)?, cx.token_text(close)]);
    }
    Ok(doc::concat(parts))
}
