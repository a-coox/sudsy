//! What the formatting modules share: the options, a node's significant children, the
//! one-line "words" printer, statement endings and clauses, and brace lists.

use crate::generated::kind as k;
use core_config::OptionValue;
use core_format::doc::{self, Doc, concat, group, hardline, if_break, indent, line, lit, softline};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub(crate) type R = Result<Doc, FormatError>;

/// The options the layout code reads, taken once per run.
pub(crate) struct Style {
    pub semi: bool,
    pub single_quote: bool,
    /// `trailing_commas`: 0 none, 1 es5, 2 all.
    pub commas: u8,
    pub bracket_spacing: bool,
}

pub(crate) fn style<'a>(cx: &'a FormatCtx<'_>) -> &'a Style {
    cx.cached(|| Style {
        semi: cx.options.bool("semi"),
        single_quote: matches!(cx.options.get("quote_style"),
            Some(OptionValue::String(s)) if s == "single"),
        commas: match cx.options.get("trailing_commas") {
            Some(OptionValue::String(s)) if s == "none" => 0,
            Some(OptionValue::String(s)) if s == "es5" => 1,
            _ => 2,
        },
        bracket_spacing: cx.options.bool("bracket_spacing"),
    })
}

/// The comma a broken list ends with: `all_only` for lists that take it only at `all`
/// (parameters, arguments), otherwise from `es5` on (objects, arrays, import lists).
pub(crate) fn trailing_comma(cx: &FormatCtx<'_>, all_only: bool) -> Doc {
    if style(cx).commas >= if all_only { 2 } else { 1 } {
        if_break(lit(","), Doc::Nil)
    } else {
        Doc::Nil
    }
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
            k::FUNCTION => matches!(a, SyntaxElement::Node(_)) || is_tok(a, "]") || is_tok(a, "?"),
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
    let spaced = style(cx).bracket_spacing;
    if !can_break {
        let all = doc::join(&lit(", "), items);
        let pad = if spaced { lit(" ") } else { Doc::Nil };
        return concat(vec![open, pad.clone(), all, pad, close]);
    }
    let edge = if spaced { line() } else { softline() };
    group(concat(vec![
        open,
        indent(concat(vec![edge.clone(), doc::join(&comma, items)])),
        trailing_comma(cx, false),
        edge,
        close,
    ]))
}

/// A statement without its own `;` holds a trailing comment on its last token; it is printed
/// after the `;` the output adds (`reattach_asi`), not inside the expression, whose groups it
/// would break.
pub(crate) fn detach_asi(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<SyntaxToken> {
    let has_semi = node
        .children_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::SEMICOLON));
    if has_semi {
        return None;
    }
    let last = cx.last_token(node)?;
    cx.detach_trailing(&last);
    Some(last)
}

pub(crate) fn reattach_asi(cx: &FormatCtx<'_>, token: Option<SyntaxToken>) -> Doc {
    token.map_or(Doc::Nil, |t| cx.reattach_trailing(&t))
}

/// Does printing `d` contain a forced line break (a hard line, a broken group)?
pub(crate) fn will_break(d: &Doc) -> bool {
    match d {
        Doc::HardLine | Doc::BreakParent | Doc::LineStart(_) | Doc::NewlineIfNeeded => true,
        Doc::Outdent(..) => true,
        Doc::Shared(s) => s.forced,
        Doc::Concat(parts) | Doc::Fill(parts) => parts.iter().any(will_break),
        Doc::Group(g) => g.broken || will_break(&g.contents),
        Doc::Indent(x)
        | Doc::Align(_, x)
        | Doc::Label(_, x)
        | Doc::IndentIfBreak(_, x)
        | Doc::LineSuffix(x) => will_break(x),
        Doc::IfBreak { broken, flat, .. } => will_break(broken) || will_break(flat),
        _ => false,
    }
}

/// `d` on one line: lines become spaces or nothing, groups never break (hard lines stay).
pub(crate) fn remove_lines(d: &Doc) -> Doc {
    match d {
        Doc::Line => lit(" "),
        Doc::SoftLine => Doc::Nil,
        Doc::IfBreak { flat, .. } => remove_lines(flat),
        Doc::Concat(p) => Doc::Concat(p.iter().map(remove_lines).collect()),
        Doc::Fill(p) => Doc::Fill(p.iter().map(remove_lines).collect()),
        Doc::Group(g) => Doc::Group(Box::new(doc::Group {
            contents: remove_lines(&g.contents),
            states: Vec::new(),
            broken: false,
            id: g.id,
        })),
        Doc::Indent(x) => Doc::Indent(Box::new(remove_lines(x))),
        Doc::Align(n, x) => Doc::Align(*n, Box::new(remove_lines(x))),
        Doc::Label(t, x) => Doc::Label(t, Box::new(remove_lines(x))),
        Doc::LineSuffix(x) => Doc::LineSuffix(Box::new(remove_lines(x))),
        Doc::IndentIfBreak(id, x) => Doc::IndentIfBreak(*id, Box::new(remove_lines(x))),
        Doc::Shared(s) => remove_lines(&s.doc),
        other => other.clone(),
    }
}
