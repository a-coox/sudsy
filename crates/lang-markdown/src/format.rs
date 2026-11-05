//! Markdown formatting, line oriented and with prose kept as written (Prettier's `proseWrap:
//! preserve`). Blocks are rendered to lines recursively and joined by one blank line (none inside
//! a tight list item); only what the safety check can verify is restyled: heading and list
//! markers, emphasis delimiters, code fences, table alignment, `*` and `_` escapes, runs of blanks.
//! Code, HTML, code spans, destinations and everything else are kept verbatim.

use crate::generated::kind as k;
use crate::mapping;
use core_config::{OptionSchema, Preset, shared};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, grammars};
use core_treesitter::Engine;
use std::sync::OnceLock;
use unicode_width::UnicodeWidthStr;

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::indent_size(2),
        shared::final_newline(true),
        shared::end_of_line(),
    ])
    .with_presets(vec![Preset::new(
        "markdown/prettier-compat",
        include_str!("../presets/prettier-compat.yaml"),
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
        let mut out = r.blocks(node, false).join("\n").replace('\r', "");
        out.push('\n');
        // Release builds skip the safety check: a rendering the parser reads differently stays as written.
        if !r.same_document(node, &out) {
            out = as_written;
        }
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

/// Parses text the way the provider does, for the check of a rendered block.
fn reparse(text: &str) -> Option<SyntaxNode> {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    let engine = ENGINE.get_or_init(|| {
        Engine::with_composite(
            tree_sitter_md::LANGUAGE.into(),
            grammars::markdown(),
            &mapping::BLOCK,
            Some(mapping::COMPOSITE),
        )
    });
    let parsed = engine.parse(text);
    parsed.errors.is_empty().then_some(parsed.tree)
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

impl Render<'_> {
    fn line_start(&self, off: usize) -> usize {
        self.src[..off].rfind('\n').map_or(0, |i| i + 1)
    }

    /// Absolute column where the content of a quote or list item starts.
    fn content_col(&self, n: &SyntaxNode) -> usize {
        let marker = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::QUOTE_MARKER || t.kind() == k::LIST_MARKER);
        marker.map_or(0, |t| end(&t) - self.line_start(start(&t)))
    }

    /// The containers around `node`, outermost first.
    fn chain(&self, node: &SyntaxNode) -> Vec<Cont> {
        let mut out = Vec::new();
        let mut parent_col = None;
        let mut ancestors: Vec<SyntaxNode> = node
            .ancestors()
            .skip(1)
            .filter(|a| a.kind() == k::BLOCK_QUOTE || a.kind() == k::LIST_ITEM)
            .collect();
        ancestors.reverse();
        for a in ancestors {
            let col = self.content_col(&a);
            if a.kind() == k::BLOCK_QUOTE {
                out.push(Cont::Quote);
            } else {
                out.push(Cont::Item(col.saturating_sub(parent_col.unwrap_or(0))));
            }
            parent_col = Some(col);
        }
        out
    }

    /// Chain for the content of `node` itself when it is a container.
    fn chain_in(&self, node: &SyntaxNode) -> Vec<Cont> {
        let mut c = self.chain(node);
        let parent_col = node
            .ancestors()
            .skip(1)
            .find(|a| a.kind() == k::BLOCK_QUOTE || a.kind() == k::LIST_ITEM)
            .map_or(0, |a| self.content_col(&a));
        c.push(if node.kind() == k::BLOCK_QUOTE {
            Cont::Quote
        } else {
            Cont::Item(self.content_col(node).saturating_sub(parent_col))
        });
        c
    }

    /// The blocks of a container, as elements.
    fn block_elems(parent: &SyntaxNode) -> Vec<SyntaxElement> {
        parent
            .children_with_tokens()
            .filter(|e| match e {
                SyntaxElement::Node(_) => true,
                SyntaxElement::Token(t) => [
                    k::INDENTED_CODE,
                    k::THEMATIC_BREAK,
                    k::LINK_DEF,
                    k::FRONT_MATTER,
                ]
                .contains(&t.kind()),
            })
            .collect()
    }

    /// The element as written, minus the prefixes of the containers around it.
    fn verbatim(&self, e: &SyntaxElement) -> Vec<String> {
        let (Some(f), Some(l)) = elem_ends(e) else {
            return Vec::new();
        };
        let chain = match e {
            SyntaxElement::Node(n) => self.chain(n),
            SyntaxElement::Token(t) => self.chain_of_token(t),
        };
        let (mut from, mut to) = (start(&f), end(&l));
        if let SyntaxElement::Node(n) = e
            && n.kind() == k::HEADING
        {
            // The markers are trivia outside the text: an ATX heading's `#`s, a setext underline.
            if atx_level(self.src, n).is_some() {
                from = self.src[..from].trim_end_matches([' ', '\t', '#']).len();
            } else {
                to += self.src[to..].find('\n').map_or(0, |i| i + 1);
                to += self.src[to..].find('\n').unwrap_or(self.src.len() - to);
            }
        }
        // A top-level block keeps the indentation of its first line.
        let line = self.line_start(from);
        if chain.is_empty() && self.src[line..from].trim().is_empty() {
            from = line;
        }
        raw_lines(&self.src[from..to], &chain)
    }

    /// Blocks kept as written: a tab's width depends on its column, and a fence with no closing
    /// fence would gain one.
    fn keep(&self, e: &SyntaxElement, top: bool) -> bool {
        let range = e.text_range();
        let from = self.line_start(usize::from(range.start()));
        let open_fence = matches!(e, SyntaxElement::Node(n) if n.kind() == k::FENCED_CODE
            && n.children_with_tokens().filter(|c| c.kind() == k::FENCE).count() < 2);
        // Prettier keeps setext headings as written.
        let setext = matches!(e, SyntaxElement::Node(n) if n.kind() == k::HEADING
            && atx_level(self.src, n).is_none());
        open_fence
            || setext
            || top && self.src[from..usize::from(range.end())].contains('\t')
            || top
                && matches!(e, SyntaxElement::Node(n)
                if (n.kind() == k::BLOCK_QUOTE || n.kind() == k::LIST)
                    && significant(n).any(|t| t.kind() == k::INDENTED_CODE))
    }

    /// Files whose blocks cannot all be told apart or rendered again: an empty heading has no
    /// token to find its level in, and an HTML block of the kinds that run on past blank lines
    /// but was cut short by the parser (what follows is its text, not Markdown).
    fn unformattable(&self, doc: &SyntaxNode) -> bool {
        doc.descendants()
            .any(|n| n.kind() == k::HEADING && significant(&n).next().is_none())
            || doc
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| {
                    t.kind() == k::HTML_BLOCK
                        && t.parent().is_some_and(|p| p.kind() == k::HTML)
                        && {
                            let low = t.text().to_ascii_lowercase();
                            ["<script", "<pre", "<style", "<textarea"].iter().any(|o| {
                                low.starts_with(o) && !low.contains(&format!("</{}", &o[1..]))
                            })
                        }
                })
    }

    /// Does the rendered block parse to what the block was? A restyle the parser reads
    /// differently (odd list indentation, a lazy line that becomes a heading) is not made.
    fn same_meaning(&self, e: &SyntaxElement, lines: &[String]) -> bool {
        let text = format!("{}\n", lines.join("\n"));
        let Some(doc) = reparse(&text) else {
            return false;
        };
        let after = Render {
            src: &text,
            tab: self.tab,
        };
        let mut new = Vec::new();
        for b in Self::block_elems(&doc) {
            after.sig(&b, &mut new);
        }
        let mut old = Vec::new();
        self.sig(e, &mut old);
        old == new
    }

    fn same_document(&self, doc: &SyntaxNode, out: &str) -> bool {
        let Some(new) = reparse(out) else {
            return false;
        };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        self.sig(&SyntaxElement::Node(doc.clone()), &mut a);
        Render {
            src: out,
            tab: self.tab,
        }
        .sig(&SyntaxElement::Node(new), &mut b);
        a == b
    }

    fn blocks(&self, parent: &SyntaxNode, tight: bool) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let (mut ignore, mut nth, mut prev_list) = (false, 0, None::<bool>);
        let mut prev: Option<SyntaxElement> = None;
        let chain = if parent.kind() == k::BLOCK_QUOTE || parent.kind() == k::LIST_ITEM {
            self.chain_in(parent)
        } else {
            self.chain(parent)
        };
        let top = parent.kind() == k::DOCUMENT;
        for e in Self::block_elems(parent) {
            let ordered = match &e {
                SyntaxElement::Node(n) if n.kind() == k::LIST => Some(self.ordered(n)),
                _ => None,
            };
            nth = if ordered.is_some() && ordered == prev_list {
                nth + 1
            } else {
                0
            };
            prev_list = ordered;
            let mut lines = if ignore || self.keep(&e, top) {
                self.verbatim(&e)
            } else {
                let lines = match &e {
                    SyntaxElement::Node(n) => self.node_block(n, nth),
                    SyntaxElement::Token(t) => self.token_block(t, out.is_empty() && top),
                };
                if top && !self.same_meaning(&e, &lines) {
                    self.verbatim(&e)
                } else {
                    lines
                }
            };
            ignore = matches!(&e, SyntaxElement::Node(n) if n.kind() == k::HTML
                && significant(n).next().is_some_and(|t| t.kind() == k::HTML_COMMENT && is_ignore(t.text())));
            if lines.is_empty() {
                continue;
            }
            // Consecutive link reference definitions stay together.
            let defs = matches!((&prev, &e), (Some(SyntaxElement::Token(a)), SyntaxElement::Token(b))
                if a.kind() == k::LINK_DEF && b.kind() == k::LINK_DEF)
                && prev
                    .as_ref()
                    .is_some_and(|p| !self.blank_between(p, &e, &chain));
            if !out.is_empty() && !tight && !defs {
                out.push(String::new());
            }
            out.append(&mut lines);
            prev = Some(e);
        }
        out
    }

    /// Indented code from the source: its first line has lost its indentation in the token.
    fn indented(&self, t: &SyntaxToken) -> Vec<String> {
        let chain = self.chain_of_token(t);
        let from = self.line_start(start(t));
        let text = &self.src[from..end(t)];
        let mut lines = raw_lines_all(text, &chain);
        // A container's marker on the first line (`-     code`) cannot be told from the code.
        lines[0] = strip(&text[..text.find('\n').unwrap_or(text.len())], &chain).to_string();
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        lines
    }

    fn chain_of_token(&self, t: &SyntaxToken) -> Vec<Cont> {
        t.parent().map_or(Vec::new(), |p| {
            let mut c = self.chain(&p);
            if p.kind() == k::BLOCK_QUOTE || p.kind() == k::LIST_ITEM {
                c = self.chain_in(&p);
            }
            c
        })
    }

    fn ordered(&self, list: &SyntaxNode) -> bool {
        significant(list)
            .find(|t| t.kind() == k::LIST_MARKER)
            .is_some_and(|t| t.text().trim().starts_with(|c: char| c.is_ascii_digit()))
    }

    fn token_block(&self, t: &SyntaxToken, first: bool) -> Vec<String> {
        let kind = t.kind();
        if kind == k::THEMATIC_BREAK {
            // `---` at the top of a document, or as the first thing in an item, would start front matter or a list.
            let in_item = t.parent().is_some_and(|p| p.kind() == k::LIST_ITEM);
            vec![if first || in_item { "***" } else { "---" }.into()]
        } else if kind == k::INDENTED_CODE {
            self.indented(t)
        } else if kind == k::HTML_BLOCK || kind == k::HTML_COMMENT {
            // The text of an HTML block ends where its last line does.
            let mut lines = raw_lines(t.text(), &self.chain_of_token(t));
            if let Some(l) = lines.last_mut() {
                *l = l.trim_end().to_string();
            }
            lines
        } else if kind == k::LINK_DEF && !t.text().trim().contains('\n') {
            // Blanks between the parts collapse; a title keeps its text.
            let text = t.text().trim();
            let at = text
                .char_indices()
                .zip(text.chars().skip(1))
                .find(|((i, c), n)| *i > 0 && c.is_whitespace() && matches!(n, '"' | '\'' | '('))
                .map_or(text.len(), |((i, _), _)| i + 1);
            let (head, title) = text.split_at(at);
            let parts: Vec<&str> = head.split_whitespace().collect();
            vec![
                format!("{} {title}", parts.join(" "))
                    .trim_end()
                    .to_string(),
            ]
        } else {
            raw_lines(t.text(), &self.chain_of_token(t))
        }
    }

    fn node_block(&self, n: &SyntaxNode, nth: usize) -> Vec<String> {
        let kind = n.kind();
        if kind == k::HTML {
            significant(n)
                .next()
                .map_or(Vec::new(), |t| self.token_block(&t, false))
        } else if kind == k::INLINE {
            self.inline_lines(n)
        } else if kind == k::HEADING {
            let level = heading_level(self.src, n);
            let text = n
                .children()
                .find(|c| c.kind() == k::INLINE)
                .map(|i| self.inline_lines(&i).join(" "))
                .unwrap_or_default();
            let text = without_closing(&text).to_string();
            vec![
                format!("{} {text}", "#".repeat(level))
                    .trim_end()
                    .to_string(),
            ]
        } else if kind == k::BLOCK_QUOTE {
            let lines = self.blocks(n, false);
            if lines.is_empty() {
                return vec![">".into()];
            }
            lines
                .into_iter()
                .map(|l| {
                    if l.is_empty() {
                        ">".into()
                    } else {
                        format!("> {l}")
                    }
                })
                .collect()
        } else if kind == k::LIST {
            self.list(n, nth)
        } else if kind == k::FENCED_CODE {
            self.fenced(n)
        } else if kind == k::TABLE {
            self.table(n)
        } else {
            Vec::new()
        }
    }

    /// Is there a blank line between the end of `a` and the start of `b`?
    fn blank_between(&self, a: &SyntaxElement, b: &SyntaxElement, chain: &[Cont]) -> bool {
        let (_, Some(last)) = elem_ends(a) else {
            return false;
        };
        let (Some(first), _) = elem_ends(b) else {
            return false;
        };
        let from = start(&last) + last.text().trim_end().len();
        let gap = &self.src[from.min(start(&first))..start(&first)];
        let parts: Vec<&str> = gap.split('\n').collect();
        parts.len() > 2
            && parts[1..parts.len() - 1]
                .iter()
                .any(|l| strip(l, chain).trim().is_empty())
    }

    /// A blank line between the blocks of the item: they are separated by one in the output.
    fn item_spread(&self, item: &SyntaxNode) -> bool {
        let inside = self.chain_in(item);
        let kids = Self::block_elems(item);
        kids.windows(2)
            .any(|w| self.blank_between(&w[0], &w[1], &inside))
    }

    /// A blank line after the item, or inside it: the next item is separated by one.
    fn item_loose(&self, items: &[SyntaxNode], i: usize) -> bool {
        self.item_spread(&items[i])
            || items.get(i + 1).is_some_and(|n| {
                let (a, b) = (
                    SyntaxElement::Node(items[i].clone()),
                    SyntaxElement::Node(n.clone()),
                );
                self.blank_between(&a, &b, &self.chain(&items[i]))
            })
    }

    fn list(&self, list: &SyntaxNode, nth: usize) -> Vec<String> {
        let items: Vec<SyntaxNode> = list.children().collect();
        let ordered = self.ordered(list);
        let number = |i: usize| -> String {
            let num = |n: &SyntaxNode| {
                significant(n)
                    .find(|t| t.kind() == k::LIST_MARKER)
                    .and_then(|t| {
                        t.text()
                            .trim()
                            .trim_end_matches(['.', ')'])
                            .parse::<u64>()
                            .ok()
                    })
            };
            let first = num(&items[0]).unwrap_or(1);
            // `0. 0. 0.` and `1. 1. 1.` keep their number.
            let step = match items.get(1).and_then(num) {
                Some(second) if second == first && first <= 1 => 0,
                _ => 1,
            };
            format!(
                "{}{}",
                first + step * i as u64,
                if nth.is_multiple_of(2) { "." } else { ")" }
            )
        };
        let mut out: Vec<String> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let loose = self.item_spread(item);
            let raw = if ordered {
                format!("{} ", number(i))
            } else if nth.is_multiple_of(2) {
                "- ".into()
            } else {
                "* ".into()
            };
            let pad = self.tab.saturating_sub(raw.len()).min(3);
            let mut marker = format!("{raw}{}", " ".repeat(pad));
            if let Some(t) = significant(item).find(|t| t.kind() == k::TASK_MARKER) {
                marker.push_str(&format!(
                    "{} ",
                    if t.text().contains(['x', 'X']) {
                        "[x]"
                    } else {
                        "[ ]"
                    }
                ));
            }
            let body = self.blocks(item, !loose);
            if i > 0 && self.item_loose(&items, i - 1) {
                out.push(String::new());
            }
            let indent = " ".repeat(marker.len());
            if body.is_empty() {
                out.push(marker.trim_end().to_string());
            }
            for (j, l) in body.iter().enumerate() {
                out.push(match (j, l.is_empty()) {
                    (0, _) => format!("{marker}{l}"),
                    (_, true) => String::new(),
                    _ => format!("{indent}{l}"),
                });
            }
        }
        out
    }

    fn fenced(&self, n: &SyntaxNode) -> Vec<String> {
        let chain = self.chain(n);
        let info = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::INFO)
            .map(|t| t.text().trim().to_string())
            .unwrap_or_default();
        // The content is read from the source, line by line up to the closing fence: blank
        // content has no token, and the closing fence may stand outside the node.
        let open = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::FENCE);
        let mut lines: Vec<String> = Vec::new();
        if let Some(open) = open {
            let fence = open.text().trim();
            let (fch, flen) = (fence.chars().next().unwrap_or('`'), fence.len());
            // Indentation of the opening fence beyond its containers: content loses as much.
            let lead = open.text().len() - open.text().trim_start().len();
            let before = strip(
                &self.src[self.line_start(start(&open))..start(&open)],
                &chain,
            );
            let fence_indent = lead
                + if before.trim().is_empty() {
                    before.len()
                } else {
                    0
                };
            let from = self.src[end(&open)..]
                .find('\n')
                .map_or(self.src.len(), |i| end(&open) + i + 1);
            let mut pieces = self.src[from..].split('\n').peekable();
            while let Some(raw) = pieces.next() {
                // The empty piece after the file's last line break is not a line.
                if raw.is_empty() && pieces.peek().is_none() && self.src.ends_with('\n') {
                    break;
                }
                if !in_container(raw, &chain) {
                    break;
                }
                let l = strip(raw, &chain);
                let t = l.trim_start();
                if l.len() - t.len() < 4
                    && t.len() >= flen
                    && t.chars().take_while(|c| *c == fch).count() >= flen
                    && t.trim_end_matches(fch).trim().is_empty()
                {
                    break;
                }
                let cut = l
                    .bytes()
                    .take(fence_indent)
                    .take_while(|b| *b == b' ')
                    .count();
                lines.push(l[cut..].to_string());
            }
        }
        let tilde = info.contains('`');
        let ch = if tilde { '~' } else { '`' };
        let longest = lines
            .iter()
            .map(|l| {
                let t = l.trim_start();
                t.chars().take_while(|c| *c == ch).count()
            })
            .max()
            .unwrap_or(0);
        let fence = ch.to_string().repeat((longest + 1).max(3));
        let mut out = vec![format!("{fence}{info}")];
        out.extend(lines);
        out.push(fence);
        out
    }

    fn table(&self, n: &SyntaxNode) -> Vec<String> {
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut align: Vec<char> = Vec::new();
        for e in n.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) if t.kind() == k::TABLE_DELIMITER => {
                    align = t
                        .text()
                        .trim()
                        .trim_matches('|')
                        .split('|')
                        .map(|c| {
                            let c = c.trim();
                            match (c.starts_with(':'), c.ends_with(':')) {
                                (true, true) => 'c',
                                (true, false) => 'l',
                                (false, true) => 'r',
                                _ => 'n',
                            }
                        })
                        .collect();
                }
                SyntaxElement::Node(r) if r.kind() == k::TABLE_ROW => rows.push(self.row(&r)),
                _ => {}
            }
        }
        let cols = rows
            .iter()
            .map(Vec::len)
            .chain([align.len()])
            .max()
            .unwrap_or(0);
        let widths: Vec<usize> = (0..cols)
            .map(|c| {
                rows.iter()
                    .filter_map(|r| r.get(c))
                    .map(|s| s.width())
                    .max()
                    .unwrap_or(0)
                    .max(3)
            })
            .collect();
        let cell = |text: &str, c: usize| {
            let pad = widths[c] - text.width();
            match align.get(c).copied().unwrap_or('n') {
                'r' => format!("{}{text}", " ".repeat(pad)),
                'c' => format!("{}{text}{}", " ".repeat(pad / 2), " ".repeat(pad - pad / 2)),
                _ => format!("{text}{}", " ".repeat(pad)),
            }
        };
        let line = |cells: Vec<String>| format!("| {} |", cells.join(" | "));
        let delim: Vec<String> = (0..cols)
            .map(|c| {
                let w = widths[c];
                match align.get(c).copied().unwrap_or('n') {
                    'l' => format!(":{}", "-".repeat(w - 1)),
                    'r' => format!("{}:", "-".repeat(w - 1)),
                    'c' => format!(":{}:", "-".repeat(w - 2)),
                    _ => "-".repeat(w),
                }
            })
            .collect();
        let mut out = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            out.push(line(
                r.iter().enumerate().map(|(c, t)| cell(t, c)).collect(),
            ));
            if i == 0 {
                out.push(line(delim.clone()));
            }
        }
        out
    }

    /// The cells of a table row; an empty cell has no node, and the outer pipes are optional.
    fn row(&self, r: &SyntaxNode) -> Vec<String> {
        let mut cells: Vec<Option<String>> = vec![None];
        let (mut first, mut last_pipe) = (true, false);
        for e in r.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) if t.kind() == k::PIPE => {
                    if first {
                        cells.clear();
                    }
                    cells.push(None);
                    last_pipe = true;
                    first = false;
                }
                SyntaxElement::Node(c) if c.kind() == k::CELL => {
                    let text = self.inline_lines(&c).join(" ");
                    if let Some(slot) = cells.last_mut() {
                        *slot = Some(text);
                    }
                    last_pipe = false;
                    first = false;
                }
                _ => {}
            }
        }
        if last_pipe {
            cells.pop();
        }
        cells.into_iter().map(Option::unwrap_or_default).collect()
    }

    /// Prose lines: blanks collapsed, indentation dropped, a hard break (two trailing blanks) kept.
    fn inline_lines(&self, n: &SyntaxNode) -> Vec<String> {
        let mut s = String::new();
        self.inline(n, false, &mut s);
        let mut lines: Vec<String> = s.split('\n').map(str::to_string).collect();
        let count = lines.len();
        let sentinel = |l: &str| l.contains(NEWLINE_IN_TOKEN);
        for (i, l) in lines.iter_mut().enumerate() {
            let hard = i + 1 < count && l.ends_with("  ") && !l.trim().is_empty();
            // Blanks inside a token that spans lines (a code span) are content: only the ends trim.
            *l = if sentinel(l) {
                l.trim_start().to_string()
            } else {
                l.trim().to_string()
            };
            if hard {
                l.push_str("  ");
            }
        }
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        while lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        lines
            .iter()
            .flat_map(|l| l.split(NEWLINE_IN_TOKEN))
            .map(str::to_string)
            .collect()
    }

    fn inline(&self, n: &SyntaxNode, in_emph: bool, out: &mut String) {
        let kids: Vec<SyntaxElement> = n.children_with_tokens().collect();
        self.run(&kids, in_emph, out);
    }

    /// Inline elements as one run of text.
    fn run(&self, kids: &[SyntaxElement], in_emph: bool, out: &mut String) {
        for (i, e) in kids.iter().enumerate() {
            match e {
                SyntaxElement::Token(t) => {
                    let kind = t.kind();
                    if kind == k::WHITESPACE {
                        // Next to a backtick the parser may have missed a code span: keep the blanks.
                        let tick = |x: Option<SyntaxToken>| {
                            x.is_some_and(|x| x.kind() == k::TEXT && x.text().contains('`'))
                        };
                        if tick(t.prev_token()) || tick(t.next_token()) {
                            out.push_str(t.text());
                            continue;
                        }
                        let hard = t.text().len() >= 2
                            && matches!(kids.get(i + 1), Some(SyntaxElement::Token(x)) if x.kind() == k::NEWLINE);
                        out.push_str(if hard { "  " } else { " " });
                    } else if kind == k::NEWLINE {
                        out.push('\n');
                    } else if kind == k::TEXT {
                        out.push_str(&self.escape(t));
                    } else if !is_trivia(kind) {
                        // A line break inside a token (code span) is kept, minus the container prefix.
                        let chain = t.parent().map(|p| self.chain(&p)).unwrap_or_default();
                        let text = raw_lines_all(t.text(), &chain);
                        out.push_str(&text.join(&NEWLINE_IN_TOKEN.to_string()));
                    }
                }
                SyntaxElement::Node(c) if c.kind() == k::EMPHASIS || c.kind() == k::STRONG => {
                    self.emphasis(c, in_emph, out)
                }
                SyntaxElement::Node(c) => self.inline(c, in_emph, out),
            }
        }
    }

    fn emphasis(&self, n: &SyntaxNode, in_emph: bool, out: &mut String) {
        let strong = n.kind() == k::STRONG;
        let delims: Vec<SyntaxToken> = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::EMPHASIS_DELIM)
            .collect();
        let w = if strong { 2 } else { 1 };
        let (Some(open), Some(close)) =
            (delims.get(w - 1), delims.get(delims.len().wrapping_sub(w)))
        else {
            return self.inline(n, true, out);
        };
        let (a, b) = (start(&delims[0]), end(&delims[delims.len() - 1]));
        let inner_src = &self.src[end(open)..start(close).max(end(open))];
        let before = self.src[..a].chars().next_back();
        let after = self.src[b..].chars().next();
        let plain = |c: Option<char>| !word_char(c) && !matches!(c, Some('*' | '_'));
        let edge = |c: Option<char>| !matches!(c, Some('*' | '_'));
        // Labels of reference links are matched as written.
        let in_link = n
            .ancestors()
            .any(|a| a.kind() == k::LINK || a.kind() == k::IMAGE);
        let safe = !in_link
            && plain(before)
            && plain(after)
            && inner_src.chars().next().is_some_and(|c| !c.is_whitespace())
            && inner_src
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_whitespace())
            && edge(inner_src.chars().next())
            && edge(inner_src.chars().next_back())
            && !inner_src.contains(['_', '*']);
        let style = if strong {
            "**"
        } else if in_emph {
            "*"
        } else {
            "_"
        };
        let original: String = delims.iter().map(|t| t.text().to_string()).collect();
        let (o, c) = if safe {
            (style.to_string(), style.to_string())
        } else {
            let half = original.len() / 2;
            (original[..half].to_string(), original[half..].to_string())
        };
        // Trivia stands before the opening and after the closing delimiter as well.
        let kids: Vec<SyntaxElement> = n
            .children_with_tokens()
            .filter(|e| !matches!(e, SyntaxElement::Token(t) if t.kind() == k::EMPHASIS_DELIM))
            .collect();
        let (opens, closes) = (delims[0].text_range().start(), close.text_range().start());
        let head = kids
            .iter()
            .take_while(|e| e.text_range().end() <= opens)
            .count();
        let body = kids
            .iter()
            .take_while(|e| e.text_range().end() <= closes)
            .count();
        self.run(&kids[..head], in_emph, out);
        out.push_str(&o);
        self.run(&kids[head..body], !strong || in_emph, out);
        out.push_str(&c);
        self.run(&kids[body..], in_emph, out);
    }

    /// The text of a prose token with runs of blanks collapsed (kept next to a backtick, where
    /// the parser may have missed a code span). Escapes are the author's: Prettier 3.9 adds none.
    fn escape(&self, t: &SyntaxToken) -> String {
        let text = t.text();
        if text.contains('`') {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        for (i, w) in text
            .split([' ', '\t'])
            .filter(|w| !w.is_empty())
            .enumerate()
        {
            if i > 0 || text.starts_with([' ', '\t']) {
                out.push(' ');
            }
            out.push_str(w);
        }
        if text.ends_with([' ', '\t']) && !out.is_empty() {
            out.push(' ');
        }
        out
    }
}

fn is_ignore(comment: &str) -> bool {
    comment
        .trim_start_matches("<!--")
        .split_whitespace()
        .next()
        .is_some_and(|w| w == "sudsy-ignore")
}

/// What the formatter must not change, for the whole tree.
fn signature(tree: &SyntaxNode) -> Vec<String> {
    let src = tree.text().to_string();
    let mut out = Vec::new();
    Render { src: &src, tab: 2 }.sig(&SyntaxElement::Node(tree.clone()), &mut out);
    out
}

impl Render<'_> {
    /// What formatting must not change, one entry per element: structure, levels, list kinds,
    /// prose with escapes undone and blanks collapsed, and raw text without container prefixes.
    fn sig(&self, elem: &SyntaxElement, out: &mut Vec<String>) {
        let all: Vec<SyntaxElement> = match elem {
            SyntaxElement::Node(n) => n.descendants_with_tokens().collect(),
            SyntaxElement::Token(_) => vec![elem.clone()],
        };
        for e in all {
            match e {
                SyntaxElement::Node(n) => {
                    let kind = n.kind();
                    out.push(if kind == k::HEADING {
                        format!("H{}", heading_level(self.src, &n))
                    } else if kind == k::LIST {
                        let first = significant(&n).find(|t| t.kind() == k::LIST_MARKER);
                        let marker = first.map_or(String::new(), |t| t.text().trim().to_string());
                        let number = marker.trim_end_matches(['.', ')']).parse::<u64>().ok();
                        format!("L{}", number.map_or(String::new(), |n| n.to_string()))
                    } else {
                        format!("{:?}", kind.0)
                    });
                }
                SyntaxElement::Token(t) => self.sig_token(&t, out),
            }
        }
    }

    fn sig_token(&self, t: &SyntaxToken, out: &mut Vec<String>) {
        let kind = t.kind();
        let text = t.text().replace('\r', "");
        let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        if kind == k::TEXT {
            let in_heading = t
                .parent()
                .and_then(|p| p.parent())
                .is_some_and(|h| h.kind() == k::HEADING);
            let text = if in_heading {
                without_closing(&text).to_string()
            } else {
                text
            };
            out.push(words(&text.replace("\\*", "*").replace("\\_", "_")));
        } else if kind == k::TASK_MARKER {
            out.push(text.to_lowercase());
        } else if kind == k::THEMATIC_BREAK {
            out.push("---".into());
        } else if kind == k::INDENTED_CODE {
            out.push(
                self.indented(t)
                    .iter()
                    .map(|l| l.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        } else if [k::HTML_BLOCK, k::HTML_COMMENT, k::CODE_TEXT, k::CODE_SPAN].contains(&kind) {
            let lines = raw_lines(&text, &self.chain_of_token(t));
            out.push(
                lines
                    .iter()
                    .map(|l| l.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        } else if kind == k::TABLE_DELIMITER {
            out.push(
                text.split('|')
                    .filter(|c| !c.trim().is_empty())
                    .map(|c| format!("{:?}", (c.trim().starts_with(':'), c.trim().ends_with(':'))))
                    .collect(),
            );
        } else if kind == k::LINK_DEF || kind == k::INFO {
            out.push(words(&text));
        } else if !is_trivia(kind)
            && ![
                k::EMPHASIS_DELIM,
                k::LIST_MARKER,
                k::QUOTE_MARKER,
                k::PIPE,
                k::FENCE,
            ]
            .contains(&kind)
        {
            out.push(text);
        }
    }
}
