//! XML formatting. Whitespace in character data is content, so an element that holds text, a
//! reference, a CDATA section or a processing instruction (or has `xml:space="preserve"`, or only
//! blanks without a line break) keeps its content exactly as written. The other elements put each
//! child element on its own line, indented. A tag that does not fit breaks its attributes.

use crate::generated::kind as k;
use core_config::{OptionSchema, Preset, shared};
use core_format::shapes::{dangling_comments, statement_sequence};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, context, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken};

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(100),
        shared::indent_size(2),
        shared::indent_style(),
        shared::final_newline(true),
        shared::end_of_line(),
        shared::max_blank_lines(
            1,
            "Largest number of blank lines kept between elements, comments and declarations.",
        ),
    ])
    .with_presets(vec![
        Preset::new("xml/default", include_str!("../presets/default.yaml")),
        Preset::new(
            "xml/csharpier-compat",
            include_str!("../presets/csharpier-compat.yaml"),
        ),
    ])
}

pub struct XmlFormatter;

/// The inner text of every attribute value: quotes may change, nothing else.
fn values(tree: &SyntaxNode) -> Vec<String> {
    tree.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::ATT_VALUE)
        .map(|t| t.text()[1..t.text().len() - 1].to_string())
        .collect()
}

impl NodeFormatter for XmlFormatter {
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        (values(before) == values(after))
            .then_some(())
            .ok_or_else(|| "an attribute value changed".into())
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::ELEMENT => element(cx, node)?,
            k::ATTRIBUTE => attribute(cx, node),
            _ => return Ok(None),
        }))
    }
}

fn blank_lines(cx: &FormatCtx<'_>) -> usize {
    cx.options.max_blank_lines()
}

fn tokens(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Vec<SyntaxToken> {
    node.tokens()
        .filter(|t| !cx.grammar.is_trivia(t.kind()))
        .collect()
}

fn inner(t: &SyntaxToken) -> &str {
    &t.text()[1..t.text().len() - 1]
}

/// `name="value"`; single quotes become double quotes unless the value holds a double quote.
fn attribute(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let parts = tokens(node, cx)
        .iter()
        .map(|t| {
            if t.kind() == k::ATT_VALUE && t.text().starts_with('\'') && !inner(t).contains('"') {
                doc::text(format!("\"{}\"", inner(t)))
            } else {
                cx.token(t)
            }
        })
        .collect();
    doc::concat(parts)
}

/// `<name attrs` and, for an empty tag, ` />`; the closing `>` of a start tag is `element`'s. The
/// attributes go one per line when the tag is too long. A single attribute stays on the line when
/// the element is empty or holds elements. `borrowed`: the `>` goes with the text that follows,
/// so no line break precedes it.
fn tag(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    element_children: bool,
    borrowed: bool,
) -> Result<Doc, FormatError> {
    let parts = tokens(node, cx);
    let (open, name, close) = (&parts[0], &parts[1], &parts[parts.len() - 1]);
    let attrs: Vec<SyntaxNode> = node.children().collect();
    let empty = close.kind() == k::SLASH_GT;
    let mut d = vec![cx.token_text(open), cx.token(name)];
    if !attrs.is_empty() {
        let single =
            attrs.len() == 1 && !attrs[0].text().contains_char('\n') && (element_children || empty);
        let formatted = attrs
            .iter()
            .map(|a| cx.format(a))
            .collect::<Result<Vec<_>, _>>()?;
        let end = match (single || borrowed, empty) {
            (true, true) => doc::lit(" "),
            (true, false) => doc::nil(),
            (false, true) => doc::line(),
            (false, false) => doc::softline(),
        };
        d.push(doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![
                if single { doc::lit(" ") } else { doc::line() },
                doc::join(&doc::line(), formatted),
            ])),
            end,
        ])));
    } else if empty {
        d.push(doc::lit(" "));
    }
    if empty {
        d.push(cx.token_text(close));
    }
    Ok(doc::concat(d))
}

fn preserves_space(cx: &FormatCtx<'_>, start: &SyntaxNode) -> bool {
    start.children().any(|a| {
        let t = tokens(&a, cx);
        t.len() == 3 && t[0].text() == "xml:space" && inner(&t[2]) == "preserve"
    })
}

/// The comments in front of an element stay out of the group that decides its layout, and a
/// comment after its last token on the same line goes on a line of its own.
fn element(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut d = Vec::new();
    // Every comment is on a line of its own.
    for c in cx
        .first_token(node)
        .map_or_else(Vec::new, |t| comments::leading_comments(cx.grammar, &t))
    {
        d.push(context::comment_doc(&c));
        d.push(doc::hardline());
        if c.breaks_after >= 2 && blank_lines(cx) > 0 {
            d.push(doc::hardline());
        }
    }
    d.push(element_inner(cx, node)?);
    if let Some(last) = cx.last_token(node) {
        for c in comments::trailing_comments(cx.grammar, &last) {
            d.extend([doc::hardline(), context::comment_doc(&c)]);
        }
    }
    Ok(doc::concat(d))
}

fn element_inner(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let (mut start, mut end, mut children, mut mixed) = (None, None, Vec::new(), false);
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Node(n) => match n.kind() {
                k::S_TAG | k::EMPTY_TAG => start = Some(n),
                k::E_TAG => end = Some(n),
                k::CHUNK => mixed = true,
                _ => children.push(n),
            },
            SyntaxElement::Token(_) => {}
        }
    }
    let start = start.ok_or_else(|| FormatError::Shape("Element without a tag".into()))?;
    let Some(end) = end else {
        return tag(cx, &start, false, false);
    };
    let close = tokens(&end, cx);
    let open_gt = tokens(&start, cx).pop().expect("a tag has tokens");
    let (lt_slash, name, gt) = (&close[0], &close[1], &close[2]);
    // The text between the tags, exactly as written. Sliced from the element: a slice of the
    // root walks every token of the file.
    let base = node.text_range().start();
    let text = node
        .text()
        .slice(core_syntax::TextRange::new(
            open_gt.text_range().end() - base,
            lt_slash.text_range().start() - base,
        ))
        .to_string();
    let blank_only = text.chars().all(char::is_whitespace);
    let end_doc = doc::concat(vec![
        cx.token_text(lt_slash),
        cx.token_text(name),
        cx.token_text(gt),
    ]);
    let has_attrs = start.children().next().is_some();
    if mixed
        || preserves_space(cx, &start)
        || (!text.is_empty() && blank_only && !text.contains('\n'))
    {
        // Text that starts right after the tag takes the `>` with it, so a tag that must break
        // keeps the text on the line of the `>`.
        let borrowed = has_attrs && !text.starts_with(['\n', '\r', '<']);
        let start_doc = tag(cx, &start, !children.is_empty(), borrowed)?;
        return Ok(if borrowed {
            doc::group(doc::concat(vec![
                start_doc,
                doc::indent(doc::concat(vec![
                    doc::softline(),
                    cx.token_text(&open_gt),
                    doc::text(&text),
                ])),
                end_doc,
            ]))
        } else {
            doc::concat(vec![
                start_doc,
                cx.token_text(&open_gt),
                context::verbatim_text(&text),
                end_doc,
            ])
        });
    }
    let start_doc = tag(cx, &start, !children.is_empty(), false)?;
    let open = doc::concat(vec![start_doc, cx.token(&open_gt)]);
    let dangling = comments::leading_comments(g, lt_slash);
    if children.is_empty() && dangling.is_empty() {
        return Ok(doc::concat(vec![open, end_doc]));
    }
    let mut body = Vec::new();
    if !children.is_empty() {
        body.push(doc::hardline());
        body.push(statement_sequence(cx, &children, blank_lines(cx))?);
    }
    body.push(dangling_comments(
        &dangling,
        !children.is_empty() && comments::blank_lines_before(g, lt_slash) > 0,
    ));
    Ok(doc::concat(vec![
        open,
        doc::indent(doc::concat(body)),
        doc::hardline(),
        end_doc,
    ]))
}

/// Prolog tokens and the root element one per line, blank lines kept; the comments after the last
/// item belong to the end-of-file token.
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let mut parts = Vec::new();
    let mut eof = None;
    for e in node.children_with_tokens() {
        let (first, d) = match e {
            SyntaxElement::Node(n) => (cx.first_token(&n), cx.format(&n)?),
            SyntaxElement::Token(t) if g.is_trivia(t.kind()) => continue,
            SyntaxElement::Token(t) if t.kind() == k::EOF => {
                eof = Some(t);
                continue;
            }
            SyntaxElement::Token(t) => (Some(t.clone()), cx.token(&t)),
        };
        if !parts.is_empty() {
            parts.push(doc::hardline());
            let blank = first.map_or(0, |t| comments::blank_lines_before(g, &t));
            if blank > 0 && blank_lines(cx) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(d);
    }
    if let Some(eof) = eof {
        let trailing = comments::leading_comments(g, &eof);
        if !trailing.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(g, &eof) > 0 && blank_lines(cx) > 0 {
                parts.push(doc::hardline());
            }
            parts.push(context::leading_comments_doc(&trailing));
        }
    }
    Ok(doc::concat(parts))
}
