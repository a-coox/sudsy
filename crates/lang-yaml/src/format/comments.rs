//! Printing tokens with their comments, and the comment placement hook for the end of block
//! collections.
//!
//! A comment belongs to the token after it (shared trivia rule). The comments between the end of
//! a nested block collection and the next token therefore belong to that next token, which
//! usually sits at a shallower indentation. The hook gives them back: a block collection that
//! ends right before the token takes the leading comments that are indented at least as far as
//! the collection itself (inner collections first), and prints them after its last entry.

use super::Style;
use crate::generated::kind as k;
use crate::tree::{column, first_token, last_token, next_significant, prev_significant};
use core_format::comments::{self, Comment};
use core_format::context::leading_comments_doc;
use core_format::{Doc, FormatCtx, doc};
use core_syntax::{SyntaxNode, SyntaxToken};

/// A token with the comments around it.
pub(crate) fn tok(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    with_text(cx, t, cx.token_text(t))
}

/// `text` standing in for the token's own text, with the comments around it.
pub(crate) fn with_text(cx: &FormatCtx<'_>, t: &SyntaxToken, text: Doc) -> Doc {
    let (leading, trailing) = (leading(cx, t), cx.trailing_comments_doc(t));
    if leading.is_nil() && trailing.is_nil() {
        return text;
    }
    doc::concat(vec![leading, text, trailing])
}

/// The leading comments the collections before the token have not taken.
pub(crate) fn leading(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    let cs = comments::leading_comments(cx.grammar, t);
    if cs.is_empty() {
        return Doc::Nil;
    }
    let skip = taken_before(&cs, t);
    leading_comments_doc(&cs[skip..])
}

/// Does the token have leading comments the collections before it have not taken?
pub(crate) fn has_remaining_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    let cs = comments::leading_comments(cx.grammar, t);
    taken_before(&cs, t) < cs.len()
}

pub(crate) fn has_leading_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    !comments::leading_comments(cx.grammar, t).is_empty()
}

pub(crate) fn has_trailing_comments(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    !comments::trailing_comments(cx.grammar, t).is_empty()
}

/// Blank lines to print before the token (or its first remaining leading comment).
pub(crate) fn blank_lines_before(cx: &FormatCtx<'_>, t: &SyntaxToken, st: &Style) -> usize {
    let cs = comments::leading_comments(cx.grammar, t);
    let skip = taken_before(&cs, t);
    let blank = match skip {
        0 => comments::blank_lines_before(cx.grammar, t),
        n => cs[n - 1].breaks_after.saturating_sub(1),
    };
    // The blank lines after a `keep` block scalar are part of its value.
    if blank <= st.max_blank || skip != 0 || !after_keep_scalar(t) {
        blank.min(st.max_blank)
    } else {
        blank
    }
}

fn after_keep_scalar(t: &SyntaxToken) -> bool {
    prev_significant(t)
        .and_then(|p| p.parent())
        .filter(|n| n.kind() == k::BLOCK_SCALAR)
        .and_then(|n| first_token(&n))
        .is_some_and(|h| h.text().contains('+'))
}

/// A line break and the blank lines the source had before `t`, at most `max_blank_lines`.
pub(crate) fn gap(cx: &FormatCtx<'_>, st: &Style, t: &SyntaxToken) -> Doc {
    breaks(1 + blank_lines_before(cx, t, st))
}

pub(crate) fn breaks(n: usize) -> Doc {
    doc::concat((0..n).map(|_| doc::hardline()).collect())
}

/// The block collections that end right before `t`, innermost first, with the range of `cs` each
/// one takes.
fn takers(cs: &[Comment], t: &SyntaxToken) -> Vec<(SyntaxNode, usize, usize)> {
    let Some(prev) = prev_significant(t) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut start = 0;
    for a in prev.parent_ancestors() {
        if last_token(&a).as_ref() != Some(&prev) {
            break;
        }
        if matches!(a.kind(), k::BLOCK_MAPPING | k::BLOCK_SEQUENCE) {
            let col = first_token(&a).map_or(0, |f| column(&f));
            let take = cs[start..]
                .iter()
                .take_while(|c| column(c.token()) >= col)
                .count();
            out.push((a, start, start + take));
            start += take;
        }
    }
    out
}

/// How many leading comments of `t` the collections before it took.
fn taken_before(cs: &[Comment], t: &SyntaxToken) -> usize {
    if cs.is_empty() {
        return 0;
    }
    takers(cs, t).last().map_or(0, |(_, _, end)| *end)
}

/// The comments a block collection takes from the token after it, printed after its last entry.
pub(crate) fn dangling(cx: &FormatCtx<'_>, st: &Style, collection: &SyntaxNode) -> Doc {
    let Some(next) = last_token(collection).and_then(|l| next_significant(&l)) else {
        return Doc::Nil;
    };
    let cs = comments::leading_comments(cx.grammar, &next);
    if cs.is_empty() {
        return Doc::Nil;
    }
    let Some((_, start, end)) = takers(&cs, &next)
        .into_iter()
        .find(|(n, _, _)| n == collection)
    else {
        return Doc::Nil;
    };
    let mut parts = Vec::new();
    for i in start..end {
        let blank = match i {
            0 => comments::blank_lines_before(cx.grammar, &next),
            _ => cs[i - 1].breaks_after.saturating_sub(1),
        };
        let blank = if blank > st.max_blank && i == 0 && after_keep_scalar(&next) {
            blank
        } else {
            blank.min(st.max_blank)
        };
        parts.push(breaks(1 + blank));
        parts.push(core_format::context::comment_doc(&cs[i]));
    }
    doc::concat(parts)
}
