use super::{
    BlankRule, Rendered, Slot, container, container_between, dangling_comments, gap, render_slots,
};
use crate::FormatCtx;
use crate::comments::{self, Comment};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxNode, SyntaxToken};

#[derive(Clone, Debug)]
pub struct BlockStyle {
    /// Blank lines between items kept from the source.
    pub max_blank_lines: usize,
    /// What stands between the delimiters of an empty block.
    pub empty: &'static str,
    /// Items between which a blank line is required.
    pub force_blank: Option<BlankRule>,
    /// An empty block puts the closing delimiter on a line of its own instead of `empty`.
    pub expand_empty: bool,
    /// Items that share a source line (no comment between) stay on one line, separated by a space.
    pub keep_same_line: bool,
}

impl Default for BlockStyle {
    fn default() -> Self {
        BlockStyle {
            max_blank_lines: 1,
            empty: "",
            force_blank: None,
            expand_empty: false,
            keep_same_line: false,
        }
    }
}

/// `open`, the items one per line and indented, `close`. An empty block is `open close`. Separator
/// tokens between items are kept as written.
pub fn block(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(cx, container(cx, node)?, style, false)
}

/// [`block`] where comments after the opening delimiter move inside, to lines of their own.
pub fn block_open_inside(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(cx, container(cx, node)?, style, true)
}

/// [`block`] for the delimiters `open` and `close` among the children of `node`.
pub fn block_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(
        cx,
        container_between(cx, node, Some((open, close)))?,
        style,
        false,
    )
}

fn block_of(
    cx: &FormatCtx<'_>,
    c: super::Container,
    style: &BlockStyle,
    open_inside: bool,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let dangling = comments::leading_comments(g, &c.close);
    let moved: Vec<Comment> = if open_inside {
        comments::trailing_comments(g, &c.open)
    } else {
        Vec::new()
    };
    let opening = if moved.is_empty() {
        cx.token(&c.open)
    } else {
        doc::concat(vec![
            cx.leading_comments_doc(&c.open),
            cx.token_text(&c.open),
        ])
    };
    let closing = doc::concat(vec![
        cx.token_text(&c.close),
        cx.trailing_comments_doc(&c.close),
    ]);
    if c.slots.is_empty() && dangling.is_empty() && moved.is_empty() {
        return Ok(doc::concat(vec![
            opening,
            if style.expand_empty {
                doc::hardline()
            } else {
                doc::lit(style.empty)
            },
            cx.token_text(&c.close),
            cx.trailing_comments_doc(&c.close),
        ]));
    }
    let mut inner = vec![sequence_items(
        cx,
        render_slots(cx, &c.slots)?,
        style.max_blank_lines,
        style.force_blank,
        style.keep_same_line,
    )];
    let blank_before = !c.slots.is_empty() && comments::blank_lines_before(g, &c.close) > 0;
    inner.push(dangling_comments(&dangling, blank_before));
    let first_hardline = if c.slots.is_empty() {
        Doc::Nil
    } else if !moved.is_empty()
        && cx
            .first_token(&c.slots[0].node)
            .is_some_and(|t| comments::blank_lines_before(g, &t) > 0)
    {
        // A blank line after the comment that moved inside stays.
        doc::concat(vec![doc::hardline(), doc::hardline()])
    } else {
        doc::hardline()
    };
    Ok(doc::concat(vec![
        opening,
        doc::indent(doc::concat(vec![
            dangling_comments(&moved, false),
            first_hardline,
            doc::concat(inner),
        ])),
        doc::hardline(),
        closing,
    ]))
}

/// Items one per line with the source's blank lines (at most `max_blank_lines`) between them.
pub fn statement_sequence(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    max_blank_lines: usize,
) -> Result<Doc, FormatError> {
    statement_sequence_with(cx, items, max_blank_lines, None, false)
}

/// [`statement_sequence`] with a rule for blank lines the source does not have.
pub fn statement_sequence_with(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    max_blank_lines: usize,
    force_blank: Option<BlankRule>,
    keep_same_line: bool,
) -> Result<Doc, FormatError> {
    let slots: Vec<Slot> = items
        .iter()
        .map(|n| Slot {
            node: n.clone(),
            sep: None,
        })
        .collect();
    Ok(sequence_items(
        cx,
        render_slots(cx, &slots)?,
        max_blank_lines,
        force_blank,
        keep_same_line,
    ))
}

fn sequence_items(
    cx: &FormatCtx<'_>,
    mut items: Vec<Rendered>,
    max_blank: usize,
    force_blank: Option<BlankRule>,
    keep_same_line: bool,
) -> Doc {
    let mut parts = Vec::new();
    for i in 0..items.len() {
        if i > 0 && keep_same_line && shares_line(cx, &items[i - 1], &items[i]) {
            parts.push(doc::lit(" "));
        } else if i > 0 {
            parts.push(gap(
                cx,
                &items[i - 1],
                &items[i],
                doc::hardline(),
                max_blank,
                force_blank,
            ));
        }
        parts.push(std::mem::replace(&mut items[i].doc, Doc::Nil));
        if let Some(sep) = &items[i].sep {
            parts.push(cx.token(sep));
        }
    }
    doc::concat(parts)
}

/// Do two neighbouring items stand on one source line, each of them written on one line, with
/// no comment or separator token between them?
fn shares_line(cx: &FormatCtx<'_>, prev: &Rendered, next: &Rendered) -> bool {
    let g = cx.grammar;
    let (Some(start), Some(end), Some(after)) = (
        cx.first_token(&prev.first),
        cx.last_token(&prev.last),
        cx.first_token(&next.first),
    ) else {
        return false;
    };
    let range = core_syntax::TextRange::new(start.text_range().start(), end.text_range().end());
    prev.sep.is_none()
        && !comments::break_after(g, &end)
        && !cx.has_line_break(&prev.first, range)
        && comments::trailing_comments(g, &end).is_empty()
        && comments::leading_comments(g, &after).is_empty()
}

/// A document of statements: the root's items as a [`statement_sequence_with`], then the comments
/// at the end of the file, which the zero-width end-of-file token owns.
pub fn document(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    max_blank_lines: usize,
    force_blank: Option<BlankRule>,
) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence_with(
        cx,
        &items,
        max_blank_lines,
        force_blank,
        false,
    )?];
    let eof = cx
        .last_token(node)
        .or_else(|| node.last_token())
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !items.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(crate::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}
