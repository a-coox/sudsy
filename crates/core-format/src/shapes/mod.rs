//! Shared shapes: the handful of layouts most syntax is made of. Each shape reads the node's
//! structure generically (delimiter tokens at both ends, child nodes in between, tokens tagged
//! `#separator`), merges its style with the node's layout directives, and returns a [`Doc`].

mod block;
mod key_value;
mod list;

pub use block::{BlockStyle, block, statement_sequence};
pub use key_value::{KeyValueStyle, key_value};
pub use list::{ListLayout, ListStyle, TrailingComma, delimited_list};

use crate::FormatCtx;
use crate::comments::{self, Comment, CommentKind};
use crate::directives::Directive;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::ignore::{self, IgnoreMark};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// Source text of a node exactly as written (plus its outer comments).
pub fn verbatim(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    cx.verbatim_node(node)
}

/// A child node and the separator token that follows it, if any.
pub(crate) struct Slot {
    node: SyntaxNode,
    sep: Option<SyntaxToken>,
}

/// A node with open and close delimiter tokens and items between them.
pub(crate) struct Container {
    open: SyntaxToken,
    close: SyntaxToken,
    slots: Vec<Slot>,
}

pub(crate) fn container(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Container, FormatError> {
    let g = cx.grammar;
    let name = g.name(node.kind());
    let shape = |m: &str| FormatError::Shape(format!("{name}: {m}"));
    let mut elements: Vec<SyntaxElement> = node
        .children_with_tokens()
        .filter(|e| e.as_token().is_none_or(|t| !g.is_trivia(t.kind())))
        .collect();
    let (Some(SyntaxElement::Token(close)), Some(SyntaxElement::Token(open))) = (
        elements.pop(),
        (!elements.is_empty()).then(|| elements.remove(0)),
    ) else {
        return Err(shape("expected an opening and a closing token"));
    };
    let mut slots: Vec<Slot> = Vec::new();
    for e in elements {
        match e {
            SyntaxElement::Node(node) => slots.push(Slot { node, sep: None }),
            SyntaxElement::Token(t) if g.has_tag(t.kind(), "separator") => {
                match slots.last_mut().filter(|s| s.sep.is_none()) {
                    Some(slot) => slot.sep = Some(t),
                    None => return Err(shape("separator without a preceding item")),
                }
            }
            SyntaxElement::Token(t) => {
                return Err(shape(&format!("unexpected token `{}`", t.text())));
            }
        }
    }
    Ok(Container { open, close, slots })
}

/// A formatted item, or a run of items kept as written by an ignore region.
pub(crate) struct Rendered {
    /// First node of the item (directives are read from it).
    first: SyntaxNode,
    first_token: SyntaxToken,
    last_token: SyntaxToken,
    doc: Doc,
    /// Separator after the last item of the run.
    sep: Option<SyntaxToken>,
}

/// Formats each slot; slots between `sudsy-ignore-start` and `sudsy-ignore-end` comments are
/// merged into one verbatim item.
pub(crate) fn render_slots(
    cx: &FormatCtx<'_>,
    slots: &[Slot],
) -> Result<Vec<Rendered>, FormatError> {
    let g = cx.grammar;
    let first_token = |slot: &Slot| {
        cx.first_token(&slot.node)
            .ok_or_else(|| FormatError::Shape("item without tokens".into()))
    };
    let mark = |slot: &Slot| -> Result<Option<IgnoreMark>, FormatError> {
        let tok = first_token(slot)?;
        Ok(ignore::mark_in(g, &comments::leading_comments(g, &tok)))
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < slots.len() {
        let tok = first_token(&slots[i])?;
        if mark(&slots[i])? == Some(IgnoreMark::Start) {
            let mut j = i;
            while j + 1 < slots.len() && mark(&slots[j + 1])? != Some(IgnoreMark::End) {
                j += 1;
            }
            let last = cx
                .last_token(&slots[j].node)
                .ok_or_else(|| FormatError::Shape("item without tokens".into()))?;
            out.push(Rendered {
                first: slots[i].node.clone(),
                first_token: tok.clone(),
                last_token: last.clone(),
                doc: cx.verbatim_range(&tok, &last),
                sep: slots[j].sep.clone(),
            });
            i = j + 1;
        } else {
            let last_token = cx
                .last_token(&slots[i].node)
                .ok_or_else(|| FormatError::Shape("item without tokens".into()))?;
            out.push(Rendered {
                first: slots[i].node.clone(),
                first_token: tok,
                last_token,
                doc: cx.format(&slots[i].node)?,
                sep: slots[i].sep.clone(),
            });
            i += 1;
        }
    }
    Ok(out)
}

/// The break between two items: `base` (a `line` or `hardline`), turned into a hard break by
/// `BreakBefore`/`BreakAfter`, followed by the blank lines the source had (at most
/// `max_blank`) or that a `BlankLineBefore` directive requires.
pub(crate) fn gap(
    cx: &FormatCtx<'_>,
    prev: &Rendered,
    next: &Rendered,
    base: Doc,
    max_blank: usize,
) -> Doc {
    let (prev_d, next_d) = (cx.directives_of(&prev.first), cx.directives_of(&next.first));
    let required = next_d.blank_lines_before();
    let forced = next_d.has(&Directive::BreakBefore)
        || prev_d.has(&Directive::BreakAfter)
        || required.is_some_and(|n| n > 0);
    let hard_base = matches!(base, Doc::HardLine);
    let blank = required.unwrap_or_else(|| {
        comments::blank_lines_before(cx.grammar, &next.first_token).min(max_blank)
    });
    // Blank lines the source had vanish when a list fits on one line; required ones do not.
    let blank_line = if hard_base || required.is_some() {
        doc::hardline()
    } else {
        doc::softline()
    };
    let mut parts = vec![if forced { doc::hardline() } else { base }];
    parts.extend((0..blank).map(|_| blank_line.clone()));
    doc::concat(parts)
}

/// Comments owned by the separator token after `item`, printed like trailing comments of the
/// item. Once a line comment is on the line, anything further must go on its own line, or it
/// would become part of that comment.
pub(crate) fn separator_comments(cx: &FormatCtx<'_>, item: &Rendered) -> Doc {
    let Some(sep) = item.sep.as_ref() else {
        return Doc::Nil;
    };
    let g = cx.grammar;
    let mut line_comment_open = comments::trailing_comments(g, &item.last_token)
        .iter()
        .any(|c| c.kind == CommentKind::Line);
    let mut parts = Vec::new();
    for c in comments::leading_comments(g, sep)
        .into_iter()
        .chain(comments::trailing_comments(g, sep))
    {
        if line_comment_open {
            parts.push(doc::hardline());
            parts.push(crate::context::verbatim_text(c.text()));
        } else {
            parts.push(crate::context::trailing_comments_doc(std::slice::from_ref(
                &c,
            )));
            line_comment_open = c.kind == CommentKind::Line;
        }
    }
    doc::concat(parts)
}

/// Comments in front of a closing delimiter, each on its own line inside the container.
pub(crate) fn dangling_comments(cs: &[Comment]) -> Doc {
    doc::concat(
        cs.iter()
            .flat_map(|c| [doc::hardline(), crate::context::verbatim_text(c.text())])
            .collect(),
    )
}
