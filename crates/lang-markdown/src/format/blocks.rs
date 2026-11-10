//! Block rendering: the blocks of a container, lists, fences.

use super::*;
use core_syntax::SyntaxNodeExt;

impl Render<'_> {
    pub(super) fn blocks(&self, parent: &SyntaxNode, tight: bool) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let (mut ignore, mut nth, mut prev_list) = (false, 0, None::<bool>);
        // The block after a list kept as written is kept too: a list's new marker could be the
        // other's, an indented block could join the list once unindented.
        let mut prev_kept = false;
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
            let after_list = ordered.is_some() && ordered == prev_list;
            nth = if after_list { nth + 1 } else { 0 };
            let after_kept_list = prev_list.is_some() && prev_kept;
            prev_list = ordered;
            let kept = ignore || self.keep(&e, top) || after_kept_list;
            prev_kept = kept;
            let mut lines = if kept {
                self.verbatim(&e)
            } else {
                match &e {
                    SyntaxElement::Node(n) => self.node_block(n, nth),
                    SyntaxElement::Token(t) => self.token_block(t, out.is_empty() && top),
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
    pub(super) fn indented(&self, t: &SyntaxToken) -> Vec<String> {
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

    pub(super) fn ordered(&self, list: &SyntaxNode) -> bool {
        significant(list)
            .find(|t| t.kind() == k::LIST_MARKER)
            .is_some_and(|t| t.text().trim().starts_with(|c: char| c.is_ascii_digit()))
    }

    pub(super) fn token_block(&self, t: &SyntaxToken, first: bool) -> Vec<String> {
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

    pub(super) fn node_block(&self, n: &SyntaxNode, nth: usize) -> Vec<String> {
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
                .child(k::INLINE)
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
    pub(super) fn blank_between(
        &self,
        a: &SyntaxElement,
        b: &SyntaxElement,
        chain: &[Cont],
    ) -> bool {
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
    pub(super) fn item_spread(&self, item: &SyntaxNode) -> bool {
        let inside = self.chain_in(item);
        let kids = Self::block_elems(item);
        kids.windows(2)
            .any(|w| self.blank_between(&w[0], &w[1], &inside))
    }

    /// A blank line after the item, or inside it: the next item is separated by one.
    pub(super) fn item_loose(&self, items: &[SyntaxNode], i: usize) -> bool {
        self.item_spread(&items[i])
            || items.get(i + 1).is_some_and(|n| {
                let (a, b) = (
                    SyntaxElement::Node(items[i].clone()),
                    SyntaxElement::Node(n.clone()),
                );
                self.blank_between(&a, &b, &self.chain(&items[i]))
            })
    }

    pub(super) fn list(&self, list: &SyntaxNode, nth: usize) -> Vec<String> {
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

    /// The content lines of a fenced block as `(start, prefix, end)`: where each stands in the
    /// source (without its line break) and how much of its start belongs to the containers and
    /// the fence's indentation; and whether the closing fence was found. The content is read from
    /// the source, line by line up to the closing fence: blank content has no token, and the
    /// closing fence may stand outside the node.
    pub(crate) fn fence_lines(&self, n: &SyntaxNode) -> (Vec<(usize, usize, usize)>, bool) {
        let chain = self.chain(n);
        let open = n.token(k::FENCE);
        let (mut lines, mut closed) = (Vec::new(), false);
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
            let mut at = from;
            while let Some(piece) = pieces.next() {
                let raw = piece.strip_suffix('\r').unwrap_or(piece);
                let line_start = at;
                at += piece.len() + 1;
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
                    closed = true;
                    break;
                }
                let cut = l
                    .bytes()
                    .take(fence_indent)
                    .take_while(|b| *b == b' ')
                    .count();
                lines.push((
                    line_start,
                    raw.len() - l.len() + cut,
                    line_start + raw.len(),
                ));
            }
        }
        (lines, closed)
    }

    pub(super) fn fenced(&self, n: &SyntaxNode) -> Vec<String> {
        let info = n
            .token(k::INFO)
            .map(|t| t.text().trim().to_string())
            .unwrap_or_default();
        let lines: Vec<String> = (self.fence_lines(n).0.into_iter())
            .map(|(start, prefix, end)| self.src[start + prefix..end].to_string())
            .collect();
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
}
