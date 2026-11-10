//! Shared shapes: the handful of layouts most syntax is made of. Each shape reads the node's
//! structure generically (delimiter tokens at both ends, child nodes in between, tokens tagged
//! `#separator`), merges its style with the node's layout directives, and returns a [`Doc`].

mod block;
mod chain;
mod key_value;
mod list;

pub use block::{
    BlockStyle, block, block_between, document, statement_sequence, statement_sequence_with,
};
pub use chain::{
    MemberChain, binary_chain, ends_in_line_comment, gap_after_operator, member_chain,
};
pub use key_value::{KeyValueStyle, key_value};
pub use list::{ListLayout, ListStyle, TrailingComma, delimited_list, delimited_list_between};

use crate::FormatCtx;
use crate::comments::{self, Comment, CommentKind};
use crate::directives::Directive;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::ignore::{self, IgnoreMark};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken};

/// Source text of a node exactly as written (plus its outer comments).
pub fn verbatim(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    cx.verbatim_node(node)
}

/// Decides whether a blank line must stand between two items (the previous item's last node and
/// the next item's first node) although the source has none.
pub type BlankRule = fn(&FormatCtx<'_>, &SyntaxNode, &SyntaxNode) -> bool;

/// A child node and the separator token that follows it, if any.
pub(crate) struct Slot {
    node: SyntaxNode,
    sep: Option<SyntaxToken>,
}

/// A node with open and close delimiter tokens and items between them.
pub(crate) struct Container {
    open: SyntaxToken,
    close: SyntaxToken,
    /// A node tagged `#list-prefix` before the first item (a C# attribute target: `[assembly: A]`).
    prefix: Option<SyntaxNode>,
    slots: Vec<Slot>,
}

/// The container of a node whose first and last significant elements are its delimiters.
pub(crate) fn container(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Container, FormatError> {
    container_between(cx, node, None)
}

/// The container formed by the elements of `node` from the delimiter token `open` to the
/// delimiter token `close` (the node's first and last when `None`); other children of the node
/// are the caller's business.
pub(crate) fn container_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    bounds: Option<(&SyntaxToken, &SyntaxToken)>,
) -> Result<Container, FormatError> {
    let g = cx.grammar;
    let name = g.name(node.kind());
    let shape = |m: &str| FormatError::Shape(format!("{name}: {m}"));
    let all: Vec<SyntaxElement> = node
        .children_with_tokens()
        .by_kind(|k| !g.is_trivia(k))
        .collect();
    let position = |t: &SyntaxToken| {
        all.iter()
            .position(|e| matches!(e, SyntaxElement::Token(x) if x == t))
    };
    let (first, last) = match bounds {
        Some((o, c)) => (position(o), position(c)),
        None => (Some(0), all.len().checked_sub(1)),
    };
    let (Some(first), Some(last)) = (first, last) else {
        return Err(shape("expected an opening and a closing token"));
    };
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (all.get(first), all.get(last))
    else {
        return Err(shape("expected an opening and a closing token"));
    };
    if first >= last {
        return Err(shape("expected an opening and a closing token"));
    }
    let mut slots: Vec<Slot> = Vec::new();
    let mut prefix = None;
    for e in &all[first + 1..last] {
        if slots.is_empty()
            && prefix.is_none()
            && let SyntaxElement::Node(n) = e
            && g.has_tag(n.kind(), "list-prefix")
        {
            prefix = Some(n.clone());
            continue;
        }
        place(g, &mut slots, e.clone()).map_err(|m| shape(&m))?;
    }
    Ok(Container {
        open: open.clone(),
        close: close.clone(),
        prefix,
        slots,
    })
}

/// Adds a child between the delimiters: a node starts a slot, a separator ends the last one.
fn place(g: &Grammar, slots: &mut Vec<Slot>, e: SyntaxElement) -> Result<(), String> {
    match e {
        SyntaxElement::Node(node) => slots.push(Slot { node, sep: None }),
        SyntaxElement::Token(t) if g.has_tag(t.kind(), "separator") => {
            match slots.last_mut().filter(|s| s.sep.is_none()) {
                Some(slot) => slot.sep = Some(t),
                None => return Err("separator without a preceding item".into()),
            }
        }
        SyntaxElement::Token(t) => return Err(format!("unexpected token `{}`", t.text())),
    }
    Ok(())
}

// A formatted item, or a run of items kept as written by an ignore region.
pub(crate) struct Rendered {
    /// First node of the item (directives are read from it).
    first: SyntaxNode,
    /// Last node of the item.
    last: SyntaxNode,
    /// Blank lines the source has before the item (see [`comments::blank_lines_before`]).
    blank_before: usize,
    doc: Doc,
    /// Separator after the last item of the run.
    sep: Option<SyntaxToken>,
}

fn no_tokens() -> FormatError {
    FormatError::Shape("item without tokens".into())
}

/// The blank lines before a slot and its ignore mark. Only a comment can carry a mark, so the
/// first token is looked at only when there is one.
fn leading_info(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
) -> Result<(usize, Option<IgnoreMark>), FormatError> {
    let g = cx.grammar;
    let lead = comments::node_leading(g, node);
    if !lead.has_tokens {
        return Err(no_tokens());
    }
    if !lead.has_comment {
        return Ok((lead.blank_lines, None));
    }
    let tok = cx.first_token(node).ok_or_else(no_tokens)?;
    Ok((
        comments::blank_lines_before(g, &tok),
        ignore::mark_in(g, &comments::leading_comments(g, &tok)),
    ))
}

/// Formats each slot; slots between `sudsy-ignore-start` and `sudsy-ignore-end` comments are
/// merged into one verbatim item.
pub(crate) fn render_slots(
    cx: &FormatCtx<'_>,
    slots: &[Slot],
) -> Result<Vec<Rendered>, FormatError> {
    let mut out = Vec::with_capacity(slots.len());
    let mut i = 0;
    while i < slots.len() {
        let (blank_before, mark) = leading_info(cx, &slots[i].node)?;
        if mark == Some(IgnoreMark::Start) {
            let mut j = i;
            while j + 1 < slots.len()
                && leading_info(cx, &slots[j + 1].node)?.1 != Some(IgnoreMark::End)
            {
                j += 1;
            }
            let tok = cx.first_token(&slots[i].node).ok_or_else(no_tokens)?;
            let last = cx.last_token(&slots[j].node).ok_or_else(no_tokens)?;
            out.push(Rendered {
                first: slots[i].node.clone(),
                last: slots[j].node.clone(),
                blank_before,
                doc: cx.verbatim_range(&tok, &last),
                sep: slots[j].sep.clone(),
            });
            i = j + 1;
        } else {
            out.push(Rendered {
                first: slots[i].node.clone(),
                last: slots[i].node.clone(),
                blank_before,
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
    force_blank: Option<BlankRule>,
) -> Doc {
    let (prev_d, next_d) = (cx.directives_of(&prev.first), cx.directives_of(&next.first));
    let required = next_d.blank_lines_before().or_else(|| {
        force_blank
            .is_some_and(|rule| rule(cx, &prev.last, &next.first))
            .then_some(1)
    });
    let forced = next_d.has(&Directive::BreakBefore)
        || prev_d.has(&Directive::BreakAfter)
        || required.is_some_and(|n| n > 0);
    let hard_base = matches!(base, Doc::HardLine);
    let blank = required.unwrap_or_else(|| next.blank_before.min(max_blank));
    // Blank lines the source had vanish when a list fits on one line; required ones do not.
    let blank_line = if hard_base || required.is_some() {
        doc::hardline()
    } else {
        doc::softline()
    };
    let first = if forced { doc::hardline() } else { base };
    if blank == 0 {
        return first;
    }
    let mut parts = vec![first];
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
    let own: Vec<Comment> = comments::leading_comments(g, sep)
        .into_iter()
        .chain(comments::trailing_comments(g, sep))
        .collect();
    if own.is_empty() {
        return Doc::Nil;
    }
    let mut line_comment_open = cx.last_token(&item.last).is_some_and(|last| {
        comments::trailing_comments(g, &last)
            .iter()
            .any(|c| c.kind == CommentKind::Line)
    });
    let mut parts = Vec::new();
    for c in own {
        if line_comment_open {
            parts.push(doc::hardline());
            parts.push(crate::context::verbatim_text(&c.text()));
        } else {
            parts.push(crate::context::trailing_comments_doc(
                &cx.comment_style,
                std::slice::from_ref(&c),
            ));
            line_comment_open = c.kind == CommentKind::Line;
        }
    }
    doc::concat(parts)
}

/// Comments in front of a closing delimiter, each on its own line inside the container. Blank
/// lines between them are kept, and one before the first when `blank_before`.
pub fn dangling_comments(cs: &[Comment], blank_before: bool) -> Doc {
    let mut parts = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        parts.push(doc::hardline());
        if (i == 0 && blank_before) || (i > 0 && cs[i - 1].breaks_after >= 2) {
            parts.push(doc::hardline());
        }
        parts.push(crate::context::comment_doc(c));
    }
    doc::concat(parts)
}
