//! JSX (Prettier's `printJsxElement` and friends): elements and fragments with their children
//! in a `fill` (words, `{" "}` where a break would lose a space), opening tags with one
//! attribute per line when broken, expression containers, spreads, and the parentheses around
//! a multi-line element. The `{" "}` children are `JSXText` nodes (see the adapter), so text and
//! jsx whitespace are one kind here.

use super::ast::{has_trailing_comment, is_binaryish, is_jsx, parent, un};
use super::parens::{Need, needs_parens};
use super::util::{R, kids, node_of, shape_error, token_of, will_break};
use crate::generated::kind as k;
use core_format::doc::{
    Doc, concat, conditional_group, fill, group, group_broken, hardline, if_break, indent, line,
    line_suffix_boundary, lit, shared, softline, text,
};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    Ok(Some(match node.kind() {
        k::JSX_ELEMENT | k::JSX_FRAGMENT => {
            let elem = element(node, cx)?;
            wrap_in_parens(node, cx, elem)
        }
        k::JSX_OPENING_ELEMENT => opening(node, cx)?,
        k::JSX_ATTRIBUTE => attribute(node, cx)?,
        k::JSX_SPREAD_ATTRIBUTE | k::JSX_SPREAD_CHILD => spread(node, cx)?,
        k::JSX_EXPRESSION_CONTAINER => container(node, cx)?,
        _ => return Ok(None),
    }))
}

/// `{...expr}`.
fn spread(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        parts.push(super::util::elem(cx, &e)?);
    }
    Ok(concat(parts))
}

/// `{expr}`: hugged when the expression is a kind that breaks inside its own delimiters.
fn container(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let (Some(open), Some(close)) = (
        elems.first().and_then(token_of),
        elems.last().and_then(token_of),
    ) else {
        return Err(shape_error("JSX expression container"));
    };
    let Some(expr) = elems.get(1).and_then(node_of) else {
        // `{}` or `{/* comment */}`, as written.
        return Ok(cx.verbatim_node(node));
    };
    let held = un(expr);
    let in_element = node.parent().is_some_and(|p| is_jsx(p.kind()));
    let inline = !super::ast::has_comment(cx, expr)
        && (hugs(&held)
            || (in_element
                && (held.kind() == k::CONDITIONAL_EXPRESSION || is_binaryish(held.kind()))));
    let body = cx.format(expr)?;
    Ok(group(if inline {
        concat(vec![
            cx.token(open),
            body,
            line_suffix_boundary(),
            cx.token(close),
        ])
    } else {
        concat(vec![
            cx.token(open),
            indent(concat(vec![softline(), body])),
            softline(),
            line_suffix_boundary(),
            cx.token(close),
        ])
    }))
}

/// Expressions that break inside their own delimiters, so `{` stays next to them.
fn hugs(e: &SyntaxNode) -> bool {
    match e.kind() {
        k::ARRAY_EXPRESSION
        | k::OBJECT_EXPRESSION
        | k::ARROW_FUNCTION_EXPRESSION
        | k::CALL_EXPRESSION
        | k::FUNCTION
        | k::TEMPLATE_LITERAL
        | k::TAGGED_TEMPLATE_EXPRESSION => true,
        k::CHAIN_EXPRESSION => e
            .first_child()
            .is_some_and(|c| c.kind() == k::CALL_EXPRESSION),
        k::AWAIT_EXPRESSION => e
            .first_child()
            .map(|a| un(&a))
            .is_some_and(|a| hugs(&a) || a.kind() == k::JSX_ELEMENT),
        _ => false,
    }
}

/// `name`, `name="value"`, `name={expr}`; a string value takes the preferred quote, entities
/// stand for the other quote.
fn attribute(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match &e {
            core_syntax::SyntaxElement::Node(v) if v.kind() == k::STRING_LITERAL => {
                let Some(tok) = cx.first_token(v) else {
                    return Err(shape_error("JSX attribute"));
                };
                let raw = tok.text();
                let inner = raw[1..raw.len() - 1]
                    .replace("&apos;", "'")
                    .replace("&quot;", "\"");
                let single = cx.options.bool("jsx_single_quote");
                let (preferred, other) = if single { ('\'', '"') } else { ('"', '\'') };
                let count = |q: char| inner.matches(q).count();
                let q = if count(preferred) > count(other) {
                    other
                } else {
                    preferred
                };
                let escaped = if q == '"' { "&quot;" } else { "&apos;" };
                parts.push(concat(vec![
                    cx.leading_comments_doc(&tok),
                    text(format!("{q}{}{q}", inner.replace(q, escaped))),
                    cx.trailing_comments_doc(&tok),
                ]));
            }
            e => parts.push(super::util::elem(cx, e)?),
        }
    }
    Ok(concat(parts))
}

/// `<name attr attr>`, `<name />`: attributes one per line when the tag does not fit.
fn opening(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let nodes: Vec<&SyntaxNode> = elems.iter().filter_map(node_of).collect();
    let open = elems.first().and_then(token_of);
    let (Some(open), Some(name)) = (open, nodes.first()) else {
        return Err(shape_error("JSX opening element"));
    };
    let attrs: Vec<&SyntaxNode> = nodes
        .iter()
        .copied()
        .filter(|n| matches!(n.kind(), k::JSX_ATTRIBUTE | k::JSX_SPREAD_ATTRIBUTE))
        .collect();
    let type_args = nodes
        .iter()
        .find(|n| n.kind() == k::TS_TYPE_PARAMETER_INSTANTIATION);
    let self_closing = elems.iter().any(|e| super::util::is_tok(e, "/"));
    let name_comments = super::ast::has_comment(cx, name)
        || type_args.is_some_and(|t| super::ast::has_comment(cx, t));
    let close_tokens = |cx: &FormatCtx<'_>| -> Vec<Doc> {
        elems
            .iter()
            .rev()
            .take(if self_closing { 2 } else { 1 })
            .rev()
            .filter_map(token_of)
            .map(|t| cx.token(t))
            .collect()
    };
    let mut head = vec![cx.token(open), cx.format(name)?];
    if let Some(t) = type_args {
        head.push(cx.format(t)?);
    }
    if self_closing && attrs.is_empty() && !name_comments {
        head.push(lit(" "));
        head.extend(close_tokens(cx));
        return Ok(concat(head));
    }
    let value_string = |a: &SyntaxNode| {
        a.children()
            .find(|c| c.kind() == k::STRING_LITERAL)
            .map(|s| s.text().to_string())
    };
    if let [a] = attrs[..]
        && let Some(s) = value_string(a)
        && !s.contains('\n')
        && !name_comments
        && !super::ast::has_comment(cx, a)
    {
        head.extend([lit(" "), cx.format(a)?]);
        if self_closing {
            head.push(lit(" "));
        }
        head.extend(close_tokens(cx));
        return Ok(group(concat(head)));
    }
    let should_break = attrs
        .iter()
        .any(|a| value_string(a).is_some_and(|s| s.contains('\n')));
    let mut printed = Vec::new();
    let source = node.text().to_string();
    let base = node.text_range().start();
    for (i, a) in attrs.iter().enumerate() {
        printed.push(line());
        printed.push(cx.format(a)?);
        // A blank line between attributes stays.
        if let Some(next) = attrs.get(i + 1) {
            // The blank line is trivia of a token: in the attribute's text or between the two.
            let (a_text, next_text) = (a.text().to_string(), next.text().to_string());
            let from = usize::from(a.text_range().start() - base) + a_text.trim_end().len();
            let to = usize::from(next.text_range().start() - base)
                + (next_text.len() - next_text.trim_start().len());
            if source[from..to].matches('\n').count() > 1 {
                printed.push(hardline());
            }
        }
    }
    head.push(indent(concat(printed)));
    let mut tail = close_tokens(cx);
    let same_line = (attrs.is_empty() && !name_comments)
        || (cx.options.bool("bracket_same_line")
            && (!name_comments || !attrs.is_empty())
            && !attrs.last().is_some_and(|a| has_trailing_comment(cx, a)));
    if self_closing {
        head.push(line());
    } else if !same_line {
        head.push(softline());
    }
    head.append(&mut tail);
    let d = concat(head);
    Ok(if should_break {
        group_broken(d)
    } else {
        group(d)
    })
}

/// One piece of an element's children.
enum Child {
    Text(String),
    Node(SyntaxNode),
}

/// A part of the children list: a word or printed node (content), or what separates them.
#[derive(Clone)]
enum P {
    Empty,
    Hard,
    Soft,
    Line,
    /// Jsx whitespace: a space, or `{" "}` and a line when the line breaks.
    Ws,
    Word(String),
    Doc(Doc),
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\r' | '\t')
}

/// Text with a word or a newline-free space in it: whitespace between lines is not.
fn meaningful(t: &str) -> bool {
    t.contains(|c| !is_space(c)) || !t.contains('\n')
}

/// The text split at runs of whitespace, the runs kept: words at even places.
fn split_words(t: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_space = false;
    for c in t.chars() {
        if is_space(c) != in_space {
            in_space = !in_space;
            out.push(String::new());
        }
        out.last_mut().expect("never empty").push(c);
    }
    if in_space {
        out.push(String::new());
    }
    out
}

/// `{/* prettier-ignore */}`: a container with a comment and no expression.
fn is_ignore_marker(n: &SyntaxNode) -> bool {
    n.kind() == k::JSX_EXPRESSION_CONTAINER
        && n.children().next().is_none()
        && n.text().to_string().contains("prettier-ignore")
}

fn is_self_closing(n: &SyntaxNode) -> bool {
    n.kind() == k::JSX_ELEMENT && !n.children().any(|c| c.kind() == k::JSX_CLOSING_ELEMENT)
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Between a word or node and the next: a softline, except next to a self-closing element.
fn sep_no_ws(word: &str, this: &Child, next: Option<&Child>) -> P {
    let closes = |c: &Child| matches!(c, Child::Node(n) if is_self_closing(n));
    if closes(this) || next.is_some_and(closes) {
        return if utf16_len(word) == 1 {
            P::Soft
        } else {
            P::Hard
        };
    }
    P::Soft
}

fn sep_with_ws(word: &str, this: &Child) -> P {
    if utf16_len(word) == 1 {
        return match this {
            Child::Text(t) if !t.contains('\n') => P::Hard,
            _ => P::Soft,
        };
    }
    P::Hard
}

fn children_parts(cx: &FormatCtx<'_>, children: &[Child]) -> Result<Vec<P>, FormatError> {
    let mut parts = Vec::new();
    for (i, child) in children.iter().enumerate() {
        let next = children.get(i + 1);
        match child {
            Child::Text(t) if meaningful(t) => {
                let mut words = split_words(t);
                if words[0].is_empty() {
                    parts.push(P::Empty);
                    words.remove(0);
                    if words[0].contains('\n') {
                        parts.push(sep_with_ws(words.get(1).map_or("", |w| w), child));
                    } else {
                        parts.push(P::Ws);
                    }
                    words.remove(0);
                }
                let mut end_ws = None;
                if words.last().is_some_and(|w| w.is_empty()) {
                    words.pop();
                    end_ws = words.pop();
                }
                if words.is_empty() {
                    continue;
                }
                let last = words.last().cloned().unwrap_or_default();
                for (j, w) in words.into_iter().enumerate() {
                    parts.push(if j % 2 == 1 { P::Line } else { P::Word(w) });
                }
                parts.push(match end_ws {
                    Some(e) if e.contains('\n') => sep_with_ws(&last, child),
                    Some(_) => P::Ws,
                    None => sep_no_ws(&last, child, next),
                });
            }
            Child::Text(t) if t.contains('\n') => {
                if t.matches('\n').count() > 1 {
                    parts.extend([P::Empty, P::Hard]);
                }
            }
            Child::Text(_) => parts.extend([P::Empty, P::Ws]),
            Child::Node(n) => {
                // `{/* prettier-ignore */}` keeps the next element as written.
                let ignored = children[..i].iter().rev().find_map(|c| match c {
                    Child::Text(t) if !meaningful(t) => None,
                    Child::Node(p) => Some(is_ignore_marker(p)),
                    Child::Text(_) => Some(false),
                });
                parts.push(P::Doc(if ignored == Some(true) && is_jsx(n.kind()) {
                    cx.verbatim_node(n)
                } else {
                    cx.format(n)?
                }));
                match next {
                    Some(nx @ Child::Text(t)) if meaningful(t) => {
                        let first = t
                            .trim_matches(is_space)
                            .split(is_space)
                            .next()
                            .unwrap_or("");
                        parts.push(sep_no_ws(first, child, Some(nx)));
                    }
                    _ => parts.push(P::Hard),
                }
            }
        }
    }
    Ok(parts)
}

/// Removes the doubled and misplaced separators `children_parts` leaves between text and nodes.
fn clean(parts: &mut Vec<P>, contains_text: bool) {
    use P::{Empty as E, Hard as H, Soft as S, Ws as W};
    let mut i = parts.len() as isize - 2;
    while i >= 0 {
        let u = i as usize;
        let at = |j: usize| parts.get(j);
        let (a, b, c) = (at(u), at(u + 1), at(u + 2));
        let b_empty = matches!(b, Some(E));
        let pair_hard = matches!((a, c), (Some(H), Some(H))) && b_empty;
        let pair_empty = matches!(a, Some(E)) && b_empty;
        let line_ws = matches!(a, Some(H | S)) && b_empty && matches!(c, Some(W));
        let ws_line = matches!(a, Some(W)) && b_empty && matches!(c, Some(H | S));
        let double_ws = matches!(a, Some(W)) && b_empty && matches!(c, Some(W));
        // The hard line wins over a soft one.
        let soft_hard = b_empty && matches!((a, c), (Some(S), Some(H)));
        let hard_soft = b_empty && matches!((a, c), (Some(H), Some(S)));
        if (pair_hard && contains_text) || pair_empty || line_ws || double_ws || soft_hard {
            parts.drain(u..u + 2);
        } else if ws_line || hard_soft {
            parts.drain(u + 1..u + 3);
        }
        i -= 1;
    }
}

fn is_blank(p: &P) -> bool {
    matches!(p, P::Empty | P::Hard | P::Soft | P::Line)
}

fn element(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let parts: Vec<SyntaxNode> = node.children().collect();
    let is_elem = node.kind() == k::JSX_ELEMENT;
    let Some(open) = parts.first() else {
        return Err(shape_error("JSX element"));
    };
    let closing = parts
        .iter()
        .skip(1)
        .find(|c| matches!(c.kind(), k::JSX_CLOSING_ELEMENT | k::JSX_CLOSING_FRAGMENT));
    let inner: Vec<&SyntaxNode> = parts
        .iter()
        .skip(1)
        .filter(|c| !matches!(c.kind(), k::JSX_CLOSING_ELEMENT | k::JSX_CLOSING_FRAGMENT))
        .collect();
    let opening_doc = cx.format(open)?;
    let Some(closing) = closing else {
        return Ok(opening_doc);
    };
    let closing_doc = cx.format(closing)?;
    let only_blank = |n: &SyntaxNode| n.kind() == k::JSX_TEXT && !meaningful(&n.text().to_string());
    if is_elem && (inner.is_empty() || (inner.len() == 1 && only_blank(inner[0]))) {
        return Ok(concat(vec![opening_doc, closing_doc]));
    }
    if let [only] = inner[..]
        && only.kind() == k::JSX_EXPRESSION_CONTAINER
        && only.children().next().is_some_and(|e| {
            matches!(
                e.kind(),
                k::TEMPLATE_LITERAL | k::TAGGED_TEMPLATE_EXPRESSION
            )
        })
    {
        return Ok(concat(vec![opening_doc, cx.format(only)?, closing_doc]));
    }
    let children: Vec<Child> = inner
        .iter()
        .map(|n| {
            let raw = n.text().to_string();
            match n.kind() {
                k::JSX_TEXT if raw.starts_with('{') => Child::Text(" ".into()),
                k::JSX_TEXT => Child::Text(raw),
                _ => Child::Node((*n).clone()),
            }
        })
        .collect();
    let contains_tag = inner.iter().any(|n| is_jsx(n.kind()));
    let many_expressions = inner
        .iter()
        .filter(|n| n.kind() == k::JSX_EXPRESSION_CONTAINER)
        .count()
        > 1;
    let many_attributes = is_elem
        && open
            .children()
            .filter(|c| matches!(c.kind(), k::JSX_ATTRIBUTE | k::JSX_SPREAD_ATTRIBUTE))
            .count()
            > 1;
    let mut forced_break =
        will_break(&opening_doc) || contains_tag || many_attributes || many_expressions;
    let raw_ws = if cx
        .options
        .get("quote_style")
        .is_some_and(|v| matches!(v, core_config::OptionValue::String(s) if s == "single"))
    {
        "{' '}"
    } else {
        "{\" \"}"
    };
    let contains_text = children
        .iter()
        .any(|c| matches!(c, Child::Text(t) if meaningful(t)));
    let mut parts = children_parts(cx, &children)?;
    clean(&mut parts, contains_text);
    while parts.last().is_some_and(is_blank) {
        parts.pop();
    }
    while parts.len() > 1 && is_blank(&parts[0]) && is_blank(&parts[1]) {
        parts.drain(..2);
    }
    let ws_doc = || if_break(concat(vec![lit(raw_ws), softline()]), lit(" "));
    let to_doc = |p: &P| match p {
        P::Empty => Doc::Nil,
        P::Hard => hardline(),
        P::Soft => softline(),
        P::Line => line(),
        P::Ws => ws_doc(),
        P::Word(w) => text(w.clone()),
        P::Doc(d) => d.clone(),
    };
    let mut multiline = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        if matches!(p, P::Ws) {
            let before_empty = i == 1 && matches!(parts[0], P::Empty);
            if before_empty && parts.len() == 2 {
                multiline.push(lit(raw_ws));
                continue;
            }
            if before_empty {
                multiline.push(concat(vec![lit(raw_ws), hardline()]));
                continue;
            }
            if i + 1 == parts.len() {
                // Trailing: measured with the content before it.
                let last = multiline.pop().unwrap_or(Doc::Nil);
                multiline.push(concat(vec![last, lit(raw_ws)]));
                continue;
            }
            if i + 2 == parts.len()
                && matches!(parts[i + 1], P::Empty)
                && i > 0
                && matches!(parts[i - 1], P::Hard)
            {
                multiline.push(lit(raw_ws));
                continue;
            }
        }
        multiline.push(to_doc(p));
        forced_break |= match p {
            P::Hard => true,
            P::Doc(d) => will_break(d),
            _ => false,
        };
    }
    let content = if contains_text {
        fill(multiline)
    } else {
        group_broken(concat(multiline))
    };
    let (opening_doc, closing_doc) = (shared(opening_doc), shared(closing_doc));
    let multi_line = group(concat(vec![
        opening_doc.clone(),
        indent(concat(vec![hardline(), content])),
        hardline(),
        closing_doc.clone(),
    ]));
    if forced_break {
        return Ok(multi_line);
    }
    let mut flat = vec![opening_doc];
    flat.extend(parts.iter().map(to_doc));
    flat.push(closing_doc);
    Ok(conditional_group(vec![group(concat(flat)), multi_line]))
}

/// A multi-line element in parentheses, `(` and `)` only when it breaks, unless the parent
/// takes it as it is.
fn wrap_in_parens(node: &SyntaxNode, cx: &FormatCtx<'_>, elem: Doc) -> Doc {
    let Some(p) = parent(node) else {
        return elem;
    };
    if matches!(
        p.kind(),
        k::ARRAY_EXPRESSION
            | k::JSX_ATTRIBUTE
            | k::JSX_ELEMENT
            | k::JSX_EXPRESSION_CONTAINER
            | k::JSX_FRAGMENT
            | k::EXPRESSION_STATEMENT
            | k::CALL_EXPRESSION
            | k::CONDITIONAL_EXPRESSION
    ) {
        return elem;
    }
    // An arrow that is the argument of a call in a container: `{items.map((x) => <li />)}`.
    let in_call_in_container = p.kind() == k::ARROW_FUNCTION_EXPRESSION
        && parent(&p).is_some_and(|c| {
            matches!(
                c.kind(),
                k::CALL_EXPRESSION | k::NEW_EXPRESSION | k::IMPORT_EXPRESSION
            ) && parent(&c).is_some_and(|g| g.kind() == k::JSX_EXPRESSION_CONTAINER)
        });
    let needs = needs_parens(cx, &un(node)) == Need::Yes;
    let paren = |s: &'static str| {
        if needs {
            Doc::Nil
        } else {
            if_break(lit(s), Doc::Nil)
        }
    };
    let d = concat(vec![
        paren("("),
        indent(concat(vec![softline(), elem])),
        softline(),
        paren(")"),
    ]);
    if in_call_in_container {
        group_broken(d)
    } else {
        group(d)
    }
}
