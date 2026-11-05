//! Markdown formatting, line oriented and with prose kept as written (Prettier's `proseWrap:
//! preserve`). Blocks are rendered to lines recursively and joined by one blank line (none inside
//! a tight list item); only what the safety check can verify is restyled: heading and list
//! markers, emphasis delimiters, code fences, table alignment, `*` and `_` escapes, runs of blanks.
//! Code, HTML, code spans, destinations and everything else are kept verbatim.

mod blocks;
mod containers;
mod inline;
mod keep;
mod signature;
mod table;

use crate::generated::kind as k;
use signature::signature;

use core_config::{OptionSchema, Preset, shared};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use unicode_width::UnicodeWidthStr;

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::indent_size(2),
        shared::final_newline(true),
        shared::end_of_line(),
    ])
    .with_presets(vec![Preset::new(
        "markdown/prettier-compat",
        include_str!("../../presets/prettier-compat.yaml"),
    )])
}

pub struct MarkdownFormatter;

impl NodeFormatter for MarkdownFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        if node.kind() != k::DOCUMENT {
            return Ok(None);
        }
        let src = node.text().to_string();
        let r = Render {
            src: &src,
            tab: cx.options.int(shared::INDENT_SIZE).max(1) as usize,
        };
        // Line breaks are the printer's: \r stays out of the text.
        let as_written = src.replace('\r', "");
        if r.unformattable(node) {
            return Ok(Some(doc::verbatim(as_written.trim_end_matches('\n'))));
        }
        let out = r.blocks(node, false).join("\n").replace('\r', "");
        Ok(Some(doc::verbatim(out.trim_end_matches('\n'))))
    }

    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        let (a, b) = (signature(before), signature(after));
        a.iter()
            .zip(&b)
            .position(|(x, y)| x != y)
            .or((a.len() != b.len()).then_some(a.len().min(b.len())))
            .map_or(Ok(()), |i| {
                Err(format!(
                    "the meaning changed: input has {:?}, output has {:?}",
                    a.get(i),
                    b.get(i)
                ))
            })
    }
}

/// A container whose prefix every line of its content carries.
enum Cont {
    Quote,
    /// Width of the continuation indentation, relative to the container around it.
    Item(usize),
}

struct Render<'a> {
    src: &'a str,
    tab: usize,
}

fn is_trivia(kind: SyntaxKind) -> bool {
    [k::WHITESPACE, k::NEWLINE, k::PREFIX, k::MARKER].contains(&kind)
}

fn significant(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !is_trivia(t.kind()))
}

/// First and last significant token of an element.
fn elem_ends(e: &SyntaxElement) -> (Option<SyntaxToken>, Option<SyntaxToken>) {
    match e {
        SyntaxElement::Node(n) => (significant(n).next(), significant(n).last()),
        SyntaxElement::Token(t) if !is_trivia(t.kind()) => (Some(t.clone()), Some(t.clone())),
        SyntaxElement::Token(_) => (None, None),
    }
}

fn start(t: &SyntaxToken) -> usize {
    usize::from(t.text_range().start())
}

fn end(t: &SyntaxToken) -> usize {
    usize::from(t.text_range().end())
}

/// Does a line still belong to the containers (a quote needs its `>`, an item its indentation or a blank)?
fn in_container(line: &str, chain: &[Cont]) -> bool {
    let mut line = line;
    for c in chain {
        match c {
            Cont::Quote => {
                if !line.trim_start_matches(' ').starts_with('>') {
                    return false;
                }
            }
            Cont::Item(w) => {
                if !line.trim().is_empty() && line.bytes().take_while(|b| *b == b' ').count() < *w {
                    return false;
                }
            }
        }
        line = strip(line, std::slice::from_ref(c));
    }
    true
}

/// Removes the container prefix from a line.
fn strip<'a>(mut line: &'a str, chain: &[Cont]) -> &'a str {
    for c in chain {
        line = match c {
            Cont::Quote => {
                let t = line.trim_start_matches(' ');
                t.strip_prefix('>')
                    .map_or(line, |r| r.strip_prefix(' ').unwrap_or(r))
            }
            Cont::Item(w) => {
                let n = line.bytes().take(*w).take_while(|b| *b == b' ').count();
                &line[n..]
            }
        };
    }
    line
}

/// Could a line, once unindented, start a block that interrupts a paragraph (a heading, quote,
/// fence, HTML, rule, setext underline or list)?
fn starts_block(t: &str) -> bool {
    let first = t.chars().next();
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    t.starts_with(['#', '>', '<'])
        || t.starts_with("```")
        || t.starts_with("~~~")
        || first.is_some_and(|c| "-*_=+".contains(c))
        || digits > 0 && t[digits..].starts_with(['.', ')'])
}

/// Stands for a line break inside a token while a paragraph is joined and trimmed line by line.
const NEWLINE_IN_TOKEN: char = '\u{1}';

/// All lines of a token, container prefixes removed from all but the first.
fn raw_lines_all(text: &str, chain: &[Cont]) -> Vec<String> {
    text.replace('\r', "")
        .split('\n')
        .enumerate()
        .map(|(i, l)| if i == 0 { l } else { strip(l, chain) }.to_string())
        .collect()
}

/// The lines of a multi-line token: the first starts after its container's prefix, the others carry it.
fn raw_lines(text: &str, chain: &[Cont]) -> Vec<String> {
    let mut lines: Vec<String> = text
        .replace('\r', "")
        .split('\n')
        .enumerate()
        .map(|(i, l)| if i == 0 { l } else { strip(l, chain) }.to_string())
        .collect();
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// The level of an ATX heading: a run of `#` stands in front of its text on the same line (after
/// a container's marker or not). A setext heading has none.
fn atx_level(src: &str, h: &SyntaxNode) -> Option<usize> {
    let first = significant(h).next()?;
    let line = &src[src[..start(&first)].rfind('\n').map_or(0, |i| i + 1)..start(&first)];
    let line = line.trim_end();
    let hashes = line.chars().rev().take_while(|c| *c == '#').count();
    let rest = &line[..line.len() - hashes];
    ((1..=6).contains(&hashes) && (rest.is_empty() || rest.ends_with([' ', '\t', '>'])))
        .then_some(hashes)
}

/// From the heading's `#`s (or first word) to its last word, without the trivia around it.
pub(crate) fn heading_range(src: &str, h: &SyntaxNode) -> core_syntax::TextRange {
    let (Some(first), Some(last)) = (significant(h).next(), significant(h).last()) else {
        return h.text_range();
    };
    let line = src[..start(&first)].rfind('\n').map_or(0, |i| i + 1);
    let from = if atx_level(src, h).is_some() {
        src[line..start(&first)]
            .find('#')
            .map_or(start(&first), |i| line + i)
    } else {
        start(&first)
    };
    core_syntax::TextRange::new((from as u32).into(), (end(&last) as u32).into())
}

pub(crate) fn heading_level(src: &str, h: &SyntaxNode) -> usize {
    if let Some(level) = atx_level(src, h) {
        return level;
    }
    // Setext: the underline is the line after the last line of text.
    let from = significant(h).last().map_or(0, |t| end(&t));
    let rest = &src[from..];
    let line = rest.split('\n').nth(1).unwrap_or("");
    1 + usize::from(!line.trim_start_matches([' ', '>']).starts_with('='))
}

/// The text of an ATX heading without its closing sequence of `#`.
fn without_closing(text: &str) -> &str {
    let body = text.trim_end().trim_end_matches('#');
    if body.len() < text.trim_end().len() && (body.is_empty() || body.ends_with(' ')) {
        body.trim_end()
    } else {
        text
    }
}

fn word_char(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

fn is_ignore(comment: &str) -> bool {
    comment
        .trim_start_matches("<!--")
        .split_whitespace()
        .next()
        .is_some_and(|w| w == "sudsy-ignore")
}
