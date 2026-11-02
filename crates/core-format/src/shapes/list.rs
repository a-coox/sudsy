use super::{container, dangling_comments, gap, render_slots, separator_comments};
use crate::FormatCtx;
use crate::comments;
use crate::directives::Directive;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxNode, SyntaxToken, TextRange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingComma {
    Never,
    /// Only when the list is laid out on several lines.
    WhenBroken,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListLayout {
    /// One line if it fits, else one item per line.
    Fit,
    /// Like `Fit`, but a list with a line break anywhere between its delimiters in the source
    /// (nested items included) stays expanded.
    PreserveBreak,
    /// Always one item per line.
    Break,
}

#[derive(Clone, Debug)]
pub struct ListStyle {
    pub separator: &'static str,
    /// Space inside the delimiters when the list is on one line: `{ a }` rather than `{a}`.
    pub padding: bool,
    pub trailing_comma: TrailingComma,
    pub layout: ListLayout,
    /// Blank lines between items kept from the source.
    pub max_blank_lines: usize,
}

impl Default for ListStyle {
    fn default() -> Self {
        ListStyle {
            separator: ",",
            padding: false,
            trailing_comma: TrailingComma::Never,
            layout: ListLayout::Fit,
            max_blank_lines: 1,
        }
    }
}

/// `open item, item, item close` in a group; the node's directives `BreakAll` and `KeepTogether`
/// override the style's layout. Separator tokens are re-created from `style.separator`; their
/// comments are kept.
pub fn delimited_list(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &ListStyle,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let c = container(cx, node)?;
    let items = render_slots(cx, &c.slots)?;
    let dangling = comments::leading_comments(g, &c.close);
    let open_comments = !comments::trailing_comments(g, &c.open).is_empty();
    // Comments before the opening and after the closing delimiter belong outside the group: a
    // comment must not force the list itself to break.
    let before = cx.leading_comments_doc(&c.open);
    let after = cx.trailing_comments_doc(&c.close);
    let opening = doc::concat(vec![
        cx.token_text(&c.open),
        cx.trailing_comments_doc(&c.open),
    ]);
    let closing = cx.token_text(&c.close);

    if items.is_empty() && dangling.is_empty() && !open_comments {
        return Ok(doc::concat(vec![before, opening, closing, after]));
    }

    let pad = if style.padding {
        doc::line()
    } else {
        doc::softline()
    };
    let sep = doc::text(style.separator);
    let last = items.len().saturating_sub(1);
    let mut inner = Vec::new();
    if !items.is_empty() {
        inner.push(pad.clone());
    }
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            inner.push(gap(
                cx,
                &items[i - 1],
                item,
                doc::line(),
                style.max_blank_lines,
            ));
        }
        inner.push(item.doc.clone());
        inner.push(match (i == last, style.trailing_comma) {
            (false, _) => sep.clone(),
            (true, TrailingComma::Never) => Doc::Nil,
            (true, TrailingComma::WhenBroken) => doc::if_break(sep.clone(), Doc::Nil),
        });
        inner.push(separator_comments(cx, item));
    }
    inner.push(dangling_comments(&dangling));
    let body = doc::concat(vec![opening, doc::indent(doc::concat(inner)), pad, closing]);

    let d = cx.directives_of(node);
    let broken = if d.has(&Directive::BreakAll) {
        true
    } else if d.has(&Directive::KeepTogether) {
        false
    } else {
        match style.layout {
            ListLayout::Fit => false,
            ListLayout::Break => !items.is_empty(),
            ListLayout::PreserveBreak => !items.is_empty() && spans_lines(node, &c.open, &c.close),
        }
    };
    let list = if broken {
        doc::group_broken(body)
    } else {
        doc::group(body)
    };
    Ok(doc::concat(vec![before, list, after]))
}

/// Does the source between the opening and the closing delimiter contain a line break? Trivia
/// outside the delimiters (the node's own leading and trailing trivia) do not count.
fn spans_lines(node: &SyntaxNode, open: &SyntaxToken, close: &SyntaxToken) -> bool {
    let base = node.text_range().start();
    let range = TextRange::new(
        open.text_range().start() - base,
        close.text_range().start() - base,
    );
    let text = node.text().slice(range);
    text.contains_char(char::from(10)) || text.contains_char(char::from(13))
}
