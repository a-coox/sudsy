use super::{
    BlankRule, container, container_between, dangling_comments, gap, render_slots,
    separator_comments,
};
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
    /// Like `Fit`, but a line break between the opening delimiter and the first item keeps the
    /// list expanded.
    PreserveOpenBreak,
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
    /// A `line` before the opening delimiter, inside the group: a space when the list is on one
    /// line and a line break (at the current indentation) when it is expanded. Braces that sit
    /// on their own line only when the list breaks.
    pub leading_line: bool,
    /// What stands between the delimiters of an empty list.
    pub empty: &'static str,
    /// Items between which a blank line is required.
    pub force_blank: Option<BlankRule>,
}

impl Default for ListStyle {
    fn default() -> Self {
        ListStyle {
            separator: ",",
            padding: false,
            trailing_comma: TrailingComma::Never,
            layout: ListLayout::Fit,
            max_blank_lines: 1,
            leading_line: false,
            empty: "",
            force_blank: None,
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
    list_of(cx, node, container(cx, node)?, style)
}

/// [`delimited_list`] for the delimiters `open` and `close` among the children of `node`; the
/// other children are the caller's business.
pub fn delimited_list_between(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
    style: &ListStyle,
) -> Result<Doc, FormatError> {
    list_of(
        cx,
        node,
        container_between(cx, node, Some((open, close)))?,
        style,
    )
}

fn list_of(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    c: super::Container,
    style: &ListStyle,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let mut items = render_slots(cx, &c.slots)?;
    let dangling = comments::leading_comments(g, &c.close);
    let open_comments = !comments::trailing_comments(g, &c.open).is_empty();
    // Comments before the opening and after the closing delimiter belong outside the group: a
    // comment must not force the list itself to break.
    let before = cx.leading_comments_doc(&c.open);
    let after = cx.trailing_comments_doc(&c.close);
    let opening = doc::concat(vec![
        cx.token_text(&c.open),
        open_comments_doc(
            cx,
            &comments::trailing_comments(g, &c.open),
            !items.is_empty(),
        ),
    ]);
    let closing = cx.token_text(&c.close);

    if items.is_empty() && dangling.is_empty() && !open_comments && c.prefix.is_none() {
        return Ok(doc::concat(vec![
            before,
            if style.leading_line {
                doc::lit(" ")
            } else {
                Doc::Nil
            },
            opening,
            doc::lit(style.empty),
            closing,
            after,
        ]));
    }

    let pad = if style.padding {
        doc::line()
    } else {
        doc::softline()
    };
    let sep = doc::lit(style.separator);
    let last = items.len().saturating_sub(1);
    let mut inner = Vec::new();
    if !items.is_empty() || c.prefix.is_some() {
        inner.push(pad.clone());
    }
    if let Some(prefix) = &c.prefix {
        inner.push(cx.format(prefix)?);
        inner.push(doc::lit(" "));
    }
    for i in 0..items.len() {
        if i > 0 {
            inner.push(gap(
                cx,
                &items[i - 1],
                &items[i],
                doc::line(),
                style.max_blank_lines,
                style.force_blank,
            ));
        }
        inner.push(std::mem::replace(&mut items[i].doc, Doc::Nil));
        inner.push(match (i == last, style.trailing_comma) {
            (false, _) => sep.clone(),
            (true, TrailingComma::Never) => Doc::Nil,
            (true, TrailingComma::WhenBroken) => doc::if_break(sep.clone(), Doc::Nil),
        });
        inner.push(separator_comments(cx, &items[i]));
    }
    inner.push(dangling_comments(cx, &dangling, false));
    let mut body_parts = Vec::new();
    if style.leading_line {
        body_parts.push(doc::line());
    }
    body_parts.extend([opening, doc::indent(doc::concat(inner)), pad, closing]);
    let body = doc::concat(body_parts);

    let d = cx.directives_of(node);
    let broken = if d.has(&Directive::BreakAll) {
        true
    } else if d.has(&Directive::KeepTogether) {
        false
    } else {
        match style.layout {
            ListLayout::Fit => false,
            ListLayout::Break => !items.is_empty(),
            ListLayout::PreserveBreak => {
                !items.is_empty() && spans_lines(cx, node, &c.open, &c.close)
            }
            ListLayout::PreserveOpenBreak => {
                !items.is_empty() && breaks_after_open(cx, node, &c.open)
            }
        }
    };
    let list = if broken {
        doc::group_broken(body)
    } else {
        doc::group(body)
    };
    Ok(doc::concat(vec![before, list, after]))
}

/// Comments after the opening delimiter. A block comment sits against the delimiter and is
/// followed by a space when items follow it on the same line: `(/* a */ 1)`.
fn open_comments_doc(cx: &FormatCtx<'_>, cs: &[comments::Comment], items_follow: bool) -> Doc {
    let mut parts = Vec::new();
    for c in cs {
        if c.kind == comments::CommentKind::Block {
            parts.push(crate::context::verbatim_text(
                &c.styled_text(&cx.comment_style),
            ));
            if items_follow {
                parts.push(doc::if_break(Doc::Nil, doc::lit(" ")));
            }
        } else {
            parts.push(crate::context::trailing_comments_doc(
                &cx.comment_style,
                std::slice::from_ref(c),
            ));
        }
    }
    doc::concat(parts)
}

/// Does the source between the opening and the closing delimiter contain a line break? Trivia
/// outside the delimiters (the node's own leading and trailing trivia) do not count.
fn spans_lines(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    open: &SyntaxToken,
    close: &SyntaxToken,
) -> bool {
    let range = TextRange::new(open.text_range().start(), close.text_range().start());
    cx.has_line_break(node, range)
}

/// Is there a line break between the opening delimiter and the first token after it that is not
/// trivia?
fn breaks_after_open(cx: &FormatCtx<'_>, node: &SyntaxNode, open: &SyntaxToken) -> bool {
    let g = cx.grammar;
    let mut next = open.next_token();
    while let Some(t) = &next {
        if !g.is_trivia(t.kind()) {
            break;
        }
        next = t.next_token();
    }
    let end = next.map_or(node.text_range().end(), |t| t.text_range().start());
    cx.has_line_break(node, TextRange::new(open.text_range().end(), end))
}
