//! Template literals: the chunks stay as written and each `${}` expression is printed on one line
//! (Prettier prints it with unlimited width). An interpolation with a line break, or
//! one whose layout forces a break (a function body), keeps that expression as written. A tagged
//! template is its tag, type arguments and template.

use crate::generated::kind as k;
use core_format::context::verbatim_text;
use core_format::doc::{Doc, concat, text};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match node.kind() {
        k::TAGGED_TEMPLATE_EXPRESSION => Ok(Some(cx.concat_children(node)?)),
        k::TEMPLATE_LITERAL => template(node, cx),
        _ => Ok(None),
    }
}

fn template(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return Ok(None);
    };
    let mut parts = vec![cx.leading_comments_doc(&first)];
    // Trivia around the literal belong to the parent.
    let (start, end) = (first.text_range().start(), last.text_range().end());
    for e in node.children_with_tokens() {
        let range = e.text_range();
        if range.end() <= start || range.start() >= end {
            continue;
        }
        match e {
            SyntaxElement::Token(t) if cx.grammar.is_trivia(t.kind()) => {
                if t.text().contains(['\n', '\r', '/']) {
                    return Ok(None);
                }
            }
            SyntaxElement::Token(t) => parts.push(verbatim_text(t.text())),
            SyntaxElement::Node(n) => {
                let src = n.text().to_string();
                // A name or a chain of names (most interpolations) prints as it is.
                let simple = src
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'.'));
                let one_line = if simple {
                    Some(text(src))
                } else if src.contains('\n') {
                    None
                } else {
                    cx.format(&n).ok().and_then(flat)
                };
                parts.push(one_line.unwrap_or_else(|| super::literal::verbatim(cx, &n)));
            }
        }
    }
    parts.push(cx.trailing_comments_doc(&last));
    Ok(Some(concat(parts)))
}

/// The document on one line, `None` when it holds a forced break or a comment.
fn flat(d: Doc) -> Option<Doc> {
    Some(match d {
        Doc::Nil | Doc::Text(_) | Doc::LineSuffixBoundary => d,
        Doc::Line => text(" "),
        Doc::SoftLine => Doc::Nil,
        Doc::Verbatim(_)
        | Doc::LineStart(_)
        | Doc::Outdent(..)
        | Doc::NewlineIfNeeded
        | Doc::HardLine
        | Doc::BreakParent
        | Doc::LineSuffix(_) => return None,
        Doc::Concat(parts) | Doc::Fill(parts) => {
            Doc::Concat(parts.into_iter().map(flat).collect::<Option<_>>()?)
        }
        Doc::Group(g) if !g.broken => flat(g.contents)?,
        Doc::Group(_) => return None,
        Doc::Indent(d) | Doc::Align(_, d) | Doc::Label(_, d) | Doc::IndentIfBreak(_, d) => {
            flat(*d)?
        }
        Doc::IfBreak { flat: f, .. } => flat(*f)?,
        Doc::Shared(s) if !s.forced => flat(s.doc.clone())?,
        Doc::Shared(_) => return None,
    })
}
