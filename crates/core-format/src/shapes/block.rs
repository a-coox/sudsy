use super::{Rendered, Slot, container, container_between, dangling_comments, gap, render_slots};
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxNode, SyntaxToken};

#[derive(Clone, Debug)]
pub struct BlockStyle {
    /// Blank lines between items kept from the source.
    pub max_blank_lines: usize,
    /// What stands between the delimiters of an empty block.
    pub empty: &'static str,
}

impl Default for BlockStyle {
    fn default() -> Self {
        BlockStyle {
            max_blank_lines: 1,
            empty: "",
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
    block_of(cx, container(cx, node)?, style)
}

/// [`block`] for the delimiters `open` and `close` among the children of `node`.
pub fn block_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    block_of(cx, container_between(cx, node, Some((open, close)))?, style)
}

fn block_of(
    cx: &FormatCtx<'_>,
    c: super::Container,
    style: &BlockStyle,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let dangling = comments::leading_comments(g, &c.close);
    let opening = cx.token(&c.open);
    let closing = doc::concat(vec![
        cx.token_text(&c.close),
        cx.trailing_comments_doc(&c.close),
    ]);
    if c.slots.is_empty() && dangling.is_empty() {
        return Ok(doc::concat(vec![
            opening,
            doc::lit(style.empty),
            cx.token_text(&c.close),
            cx.trailing_comments_doc(&c.close),
        ]));
    }
    let mut inner = vec![sequence_items(
        cx,
        render_slots(cx, &c.slots)?,
        style.max_blank_lines,
    )];
    inner.push(dangling_comments(&dangling));
    let first_hardline = if c.slots.is_empty() {
        Doc::Nil
    } else {
        doc::hardline()
    };
    Ok(doc::concat(vec![
        opening,
        doc::indent(doc::concat(vec![first_hardline, doc::concat(inner)])),
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
    ))
}

fn sequence_items(cx: &FormatCtx<'_>, mut items: Vec<Rendered>, max_blank: usize) -> Doc {
    let mut parts = Vec::new();
    for i in 0..items.len() {
        if i > 0 {
            parts.push(gap(
                cx,
                &items[i - 1],
                &items[i],
                doc::hardline(),
                max_blank,
            ));
        }
        parts.push(std::mem::replace(&mut items[i].doc, Doc::Nil));
        if let Some(sep) = &items[i].sep {
            parts.push(cx.token(sep));
        }
    }
    doc::concat(parts)
}
