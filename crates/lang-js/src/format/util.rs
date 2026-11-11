//! What the formatting modules share: the options, a node's significant children, the
//! one-line "words" printer, statement endings and clauses, and brace lists.

use crate::generated::kind as k;
use core_config::OptionValue;
use core_format::doc::{self, Doc, concat, group, hardline, if_break, indent, line, lit};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub(crate) type R = Result<Doc, FormatError>;

/// The options the layout code reads, taken once per run.
pub(crate) struct Style {
    pub semi: bool,
    pub single_quote: bool,
}

pub(crate) fn style<'a>(cx: &'a FormatCtx<'_>) -> &'a Style {
    cx.cached(|| Style {
        semi: cx.options.bool("semi"),
        single_quote: matches!(cx.options.get("quote_style"),
            Some(OptionValue::String(s)) if s == "single"),
    })
}

/// The children of a node that are not trivia.
pub(crate) fn kids(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|e| !cx.grammar.is_trivia(e.kind()))
        .collect()
}

pub(crate) fn is_tok(e: &SyntaxElement, text: &str) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.text() == text)
}

pub(crate) fn node_of(e: &SyntaxElement) -> Option<&SyntaxNode> {
    match e {
        SyntaxElement::Node(n) => Some(n),
        SyntaxElement::Token(_) => None,
    }
}

pub(crate) fn token_of(e: &SyntaxElement) -> Option<&SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t),
        SyntaxElement::Node(_) => None,
    }
}

pub(crate) fn shape_error(what: &str) -> FormatError {
    FormatError::Shape(what.to_string())
}

/// The `i`th significant child, which must be a node.
pub(crate) fn node_at(kids: &[SyntaxElement], i: usize) -> Result<&SyntaxNode, FormatError> {
    kids.get(i)
        .and_then(node_of)
        .ok_or_else(|| shape_error("unexpected statement shape"))
}

pub(crate) fn elem(cx: &FormatCtx<'_>, e: &SyntaxElement) -> R {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(cx.token(t)),
    }
}

fn first_start(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<core_syntax::TextSize> {
    match e {
        SyntaxElement::Token(t) => Some(t.text_range().start()),
        SyntaxElement::Node(n) => cx.first_token(n).map(|t| t.text_range().start()),
    }
}

/// The separator between two neighbours of [`inline`].
fn gap(cx: &FormatCtx<'_>, a: &SyntaxElement, b: &SyntaxElement) -> Doc {
    if let SyntaxElement::Node(d) = a
        && d.kind() == k::DECORATOR
    {
        let broken = cx
            .last_token(d)
            .zip(first_start(cx, b))
            .is_some_and(|(l, s)| {
                s >= l.text_range().end()
                    && cx.has_line_break(d, TextRange::new(l.text_range().end(), s))
            });
        return if broken { hardline() } else { lit(" ") };
    }
    let tight_before = match b {
        SyntaxElement::Token(t) => matches!(t.text(), "," | ";" | ")" | "]" | "?" | "!" | ":"),
        SyntaxElement::Node(n) => match n.kind() {
            k::TS_TYPE_PARAMETER_DECLARATION
            | k::TS_TYPE_PARAMETER_INSTANTIATION
            | k::TS_TYPE_ANNOTATION => true,
            // A method's function follows its key: `m()`, `[k]()`, but `return function () {}`.
            k::FUNCTION => matches!(a, SyntaxElement::Node(_)) || is_tok(a, "]"),
            _ => false,
        },
    };
    let tight_after = match a {
        SyntaxElement::Token(t) => match t.text() {
            "(" | "[" | "..." | "@" => true,
            // A generator method's star: `async *m()`, but `* as ns`.
            "*" => matches!(b, SyntaxElement::Node(_)) || is_tok(b, "["),
            _ => false,
        },
        SyntaxElement::Node(_) => false,
    };
    if tight_before || tight_after {
        Doc::Nil
    } else {
        lit(" ")
    }
}

/// Children on one line, a space between them except where punctuation and annotations stick to
/// their neighbour; `;` tokens are left out (see [`semi`]).
pub(crate) fn inline(cx: &FormatCtx<'_>, elems: &[SyntaxElement]) -> R {
    let (lead, rest) = inline_split(cx, elems)?;
    Ok(concat(vec![lead, rest]))
}

/// [`inline`] with the comments in front of the first token apart from the rest: a statement's
/// leading comments go outside the group that lays out its header, or their line breaks would
/// break it.
pub(crate) fn inline_split(
    cx: &FormatCtx<'_>,
    elems: &[SyntaxElement],
) -> Result<(Doc, Doc), FormatError> {
    let mut lead = Doc::Nil;
    let mut parts = Vec::with_capacity(elems.len() * 2);
    let mut prev: Option<&SyntaxElement> = None;
    for e in elems.iter().filter(|e| !is_tok(e, ";")) {
        if let Some(p) = prev {
            parts.push(gap(cx, p, e));
        }
        match e {
            SyntaxElement::Token(t) if prev.is_none() => {
                lead = cx.leading_comments_doc(t);
                parts.extend([cx.token_text(t), cx.trailing_comments_doc(t)]);
            }
            e => parts.push(elem(cx, e)?),
        }
        prev = Some(e);
    }
    Ok((lead, concat(parts)))
}

/// A keyword with the comments in front of it apart (see [`inline_split`]).
pub(crate) fn keyword(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Result<(Doc, Doc), FormatError> {
    inline_split(cx, std::slice::from_ref(e))
}

/// The `;` that ends a statement: the node's own `;` token with its comments, printed when the
/// `semi` option asks for it.
pub(crate) fn semi(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let tok = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::SEMICOLON);
    let text = if style(cx).semi { lit(";") } else { Doc::Nil };
    match tok {
        Some(t) => concat(vec![
            cx.leading_comments_doc(&t),
            text,
            cx.trailing_comments_doc(&t),
        ]),
        None => text,
    }
}

/// The body of a control statement after its `)`: a block follows after a space, an empty
/// statement is `;`, any other statement goes on the next line when it does not fit.
pub(crate) fn clause(body: &SyntaxNode, printed: Doc, force_space: bool) -> Doc {
    match body.kind() {
        k::EMPTY_STATEMENT => printed,
        k::BLOCK_STATEMENT => concat(vec![lit(" "), printed]),
        _ if force_space => concat(vec![lit(" "), printed]),
        _ => indent(concat(vec![line(), printed])),
    }
}

/// `{ a, b }`; with `can_break` the items go one per line when the line is too long, with a
/// trailing comma. An empty list is `{}`.
pub(crate) fn braces(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    items: Vec<Doc>,
    close: &SyntaxToken,
    can_break: bool,
) -> Doc {
    let (open, close) = (cx.token(open), cx.token(close));
    if items.is_empty() {
        return concat(vec![open, close]);
    }
    let comma = concat(vec![lit(","), line()]);
    if !can_break {
        let all = doc::join(&lit(", "), items);
        return concat(vec![open, lit(" "), all, lit(" "), close]);
    }
    group(concat(vec![
        open,
        indent(concat(vec![line(), doc::join(&comma, items)])),
        if_break(lit(","), Doc::Nil),
        line(),
        close,
    ]))
}
