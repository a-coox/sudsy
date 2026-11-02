//! Documents, block collections and block scalars.

use super::scan::{Marker, is_blank, is_break};
use super::{Parser, R};
use crate::generated::kind as k;

/// Where a block node appears; decides whether a sequence may sit at the parent's indentation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ctx {
    BlockIn,
    /// A mapping value: `key:` followed by `- item` at the key's own column is allowed.
    BlockOut,
}

impl Parser<'_> {
    pub(crate) fn stream(&mut self) {
        self.start(k::STREAM);
        let mut bare_ok = true;
        loop {
            let look = self.skip_trivia();
            if look.eof {
                break;
            }
            self.document(&mut bare_ok);
        }
        let end = self.b.len();
        self.tok(k::EOF, end, end);
        self.finish();
    }

    fn document(&mut self, bare_ok: &mut bool) {
        let depth = self.open + 1;
        self.start(k::DOCUMENT);
        if self.document_inner(bare_ok).is_err() {
            self.recover(depth);
        }
        self.finish();
    }

    fn document_inner(&mut self, bare_ok: &mut bool) -> R<()> {
        let mut yaml_directive = false;
        let mut directives = 0;
        self.handles.clear();
        while self.col(self.pos) == 0 && self.cur() == b'%' {
            if !*bare_ok {
                return self.fail("directive-position", "a directive needs a preceding `...`");
            }
            self.directive(&mut yaml_directive)?;
            directives += 1;
            self.skip_trivia();
        }
        let marker = self.marker_at(self.pos);
        if marker == Some(Marker::Start) {
            let p = self.pos;
            self.tok(k::DOC_START, p, p + 3);
            *bare_ok = false;
            self.block_node(-1, Ctx::BlockIn, false)?;
        } else if directives > 0 {
            return self.fail(
                "directive-without-document",
                "a directive must be followed by `---`",
            );
        } else if marker.is_none() {
            *bare_ok = false;
            let look = self.skip_trivia();
            // The blanks may have been skipped before: look at the line again.
            let ls = self.line_start(self.pos);
            let tab = self.b[ls..self.pos].contains(&9);
            self.block_content(-1, Ctx::BlockIn, true, look.col, tab, 0)?;
        }
        let look = self.skip_trivia();
        if look.eof {
            return Ok(());
        }
        match self.marker_at(self.pos) {
            Some(Marker::End) => {
                let p = self.pos;
                self.tok(k::DOC_END, p, p + 3);
                *bare_ok = true;
                self.expect_eol()
            }
            Some(Marker::Start) => Ok(()),
            None => self.fail(
                "unexpected-content",
                "unexpected content after the document",
            ),
        }
    }

    /// `%YAML 1.2` or `%TAG !h! prefix`; reserved directives are accepted as they are.
    fn directive(&mut self, seen_yaml: &mut bool) -> R<()> {
        let s = self.pos;
        let mut e = s;
        while e < self.b.len() && !is_break(self.b[e]) {
            if self.comment_at(e) {
                break;
            }
            e += 1;
        }
        while e > s && is_blank(self.b[e - 1]) {
            e -= 1;
        }
        let text = &self.src[s..e];
        let words: Vec<&str> = text.split([' ', '\t']).filter(|w| !w.is_empty()).collect();
        let bad = match words[0] {
            "%YAML" => {
                let dup = std::mem::replace(seen_yaml, true);
                let version_ok = words.len() == 2
                    && words[1].split_once('.').is_some_and(|(a, b)| {
                        !a.is_empty()
                            && !b.is_empty()
                            && a.bytes().all(|c| c.is_ascii_digit())
                            && b.bytes().all(|c| c.is_ascii_digit())
                    });
                dup || !version_ok
            }
            "%TAG" => {
                words.len() != 3 || {
                    let h = words[1];
                    !(h == "!"
                        || h == "!!"
                        || h.len() > 2
                            && h.starts_with('!')
                            && h.ends_with('!')
                            && h[1..h.len() - 1]
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || c == b'-'))
                }
            }
            _ => false,
        };
        if bad {
            self.error_at(s, e - s, "bad-directive", "malformed or repeated directive");
            return Err(super::Abort);
        }
        if words[0] == "%TAG" {
            self.handles.push(words[1].to_string());
        }
        self.tok(k::DIRECTIVE, s, e);
        self.expect_eol()
    }

    /// The node after an indicator (`-`, `?`, `:`, `---`) on the same line or the lines below.
    /// `compact_ok`: a block collection may start on this line (after `-` and `?`).
    pub(crate) fn block_node(&mut self, n: i64, ctx: Ctx, compact_ok: bool) -> R<()> {
        let tab = self.skip_inline_ws();
        if self.at_content() {
            let col = self.col(self.pos);
            self.block_content(n, ctx, compact_ok && !tab, col, false, 0)
        } else {
            self.next_line_node(n, ctx, 0)
        }
    }

    /// A node that starts on a later line, if there is one indented more than `n`.
    /// `outer_props`: the properties (see `Cand::prop_mask`) on the previous line apply to it.
    fn next_line_node(&mut self, n: i64, ctx: Ctx, outer_props: u8) -> R<()> {
        let look = self.skip_trivia();
        if look.eof || look.marker {
            return Ok(());
        }
        let col = look.col as i64;
        let seq_here = ctx == Ctx::BlockOut && self.cur() == b'-' && self.is_bb(self.pos + 1);
        if col > n || (col == n && seq_here) {
            return self.block_content(n, ctx, true, look.col, look.tab, outer_props);
        }
        Ok(())
    }

    /// The node at the current position, in a block context with indentation `n`. `col` is the
    /// column it starts at; `allow_coll`: a block collection may start here.
    /// `tab`: a tab is among the blanks before it (it cannot start a block collection).
    pub(crate) fn block_content(
        &mut self,
        n: i64,
        ctx: Ctx,
        allow_coll: bool,
        col: usize,
        tab: bool,
        outer_props: u8,
    ) -> R<()> {
        let c = self.cur();
        if (c == b'-' || c == b'?') && self.is_bb(self.pos + 1) {
            if tab {
                return self.fail("tab-indentation", "a tab cannot indent block content");
            }
            if !allow_coll {
                return self.fail(
                    "collection-not-allowed",
                    "a block collection cannot start on this line",
                );
            }
            if c == b'-' {
                return self.block_sequence(col);
            }
            let cp = self.events.len();
            self.wrap_from(cp, &[k::BLOCK_MAPPING, k::BLOCK_MAPPING_ENTRY]);
            self.explicit_entry(col)?;
            return self.mapping_rest(col);
        }
        if c == b'|' || c == b'>' {
            return self.block_scalar(n);
        }
        let cp = self.events.len();
        let cand = self.flow_node_cand(n)?;
        if cand.block_scalar_next {
            self.block_scalar(n)?;
            self.finish();
            return self.merge_props(cp, outer_props, cand.prop_mask);
        }
        self.skip_inline_ws();
        if self.cur() == b':' && self.is_bb(self.pos + 1) {
            if cand.open_prop {
                self.finish();
            }
            if !allow_coll {
                return self.fail("mapping-not-allowed", "a mapping value is not allowed here");
            }
            if tab {
                return self.fail("tab-indentation", "a tab cannot indent block content");
            }
            if cand.multiline {
                return self.fail("multiline-key", "an implicit key must be on one line");
            }
            self.wrap_from(cp, &[k::BLOCK_MAPPING, k::BLOCK_MAPPING_ENTRY]);
            self.implicit_value(col)?;
            self.finish();
            return self.mapping_rest(col);
        }
        if cand.open_prop {
            if !cand.eol_after_props {
                return self.fail(
                    "unexpected-character",
                    "unexpected character after properties",
                );
            }
            self.next_line_node(n, ctx, cand.prop_mask)?;
            self.finish();
            return self.merge_props(cp, outer_props, cand.prop_mask);
        }
        if !cand.has_content {
            return self.fail("unexpected-character", "unexpected character");
        }
        self.merge_props(cp, outer_props, cand.prop_mask)?;
        self.expect_eol()
    }

    /// The node starting at event `cp` has its own properties and the previous line's `outer`
    /// ones apply to it as well: they must differ, and become one `PropertyNode`.
    fn merge_props(&mut self, cp: usize, outer: u8, inner: u8) -> R<()> {
        if outer == 0 || inner == 0 {
            return Ok(());
        }
        if outer & inner != 0 {
            return self.fail("duplicate-property", "a node has one tag and one anchor");
        }
        // The inner wrapper is the one opened at `cp` and closed by the last event.
        self.events.pop();
        self.events.remove(cp);
        Ok(())
    }

    /// `:` of an implicit key, and the value after it.
    fn implicit_value(&mut self, m: usize) -> R<()> {
        let p = self.pos;
        self.tok(k::COLON, p, p + 1);
        self.block_node(m as i64, Ctx::BlockOut, false)
    }

    /// Entries of a block mapping at column `m` after the first one.
    fn mapping_rest(&mut self, m: usize) -> R<()> {
        loop {
            let look = self.skip_trivia();
            if look.eof || look.marker || look.col < m && look.nl {
                break;
            }
            if !look.nl {
                return self.fail(
                    "unexpected-content",
                    "unexpected content after a mapping entry",
                );
            }
            if look.col > m {
                return self.fail("bad-indentation", "bad indentation of a mapping entry");
            }
            if look.tab {
                return self.fail("tab-indentation", "a tab cannot indent block content");
            }
            if self.cur() == b'-' && self.is_bb(self.pos + 1) {
                return self.fail("unexpected-sequence", "a sequence entry inside a mapping");
            }
            self.start(k::BLOCK_MAPPING_ENTRY);
            if self.cur() == b'?' && self.is_bb(self.pos + 1) {
                self.explicit_entry(m)?;
            } else {
                let cand = self.flow_node_cand(m as i64 - 1)?;
                if cand.block_scalar_next {
                    return self.fail("expected-key", "expected a mapping key");
                }
                self.skip_inline_ws();
                if !(self.cur() == b':' && self.is_bb(self.pos + 1)) {
                    return self.fail("expected-colon", "expected `:` after a mapping key");
                }
                if cand.open_prop {
                    self.finish();
                }
                if cand.multiline {
                    return self.fail("multiline-key", "an implicit key must be on one line");
                }
                self.implicit_value(m)?;
                self.finish();
            }
        }
        self.finish();
        Ok(())
    }

    /// `? key` and the optional `: value` on a later line at the mapping's column; the entry node
    /// is already started and is finished here.
    fn explicit_entry(&mut self, m: usize) -> R<()> {
        let p = self.pos;
        self.tok(k::QUESTION, p, p + 1);
        self.block_node(m as i64, Ctx::BlockOut, true)?;
        let look = self.skip_trivia();
        if !look.eof
            && !look.marker
            && look.nl
            && look.col == m
            && self.cur() == b':'
            && self.is_bb(self.pos + 1)
        {
            let p = self.pos;
            self.tok(k::COLON, p, p + 1);
            self.block_node(m as i64, Ctx::BlockOut, true)?;
        }
        self.finish();
        Ok(())
    }

    /// Entries of a block sequence at column `s`; the cursor is at the first `-`.
    fn block_sequence(&mut self, s: usize) -> R<()> {
        self.start(k::BLOCK_SEQUENCE);
        loop {
            self.start(k::BLOCK_SEQUENCE_ENTRY);
            let p = self.pos;
            self.tok(k::DASH, p, p + 1);
            self.block_node(s as i64, Ctx::BlockIn, true)?;
            self.finish();
            let look = self.skip_trivia();
            if look.eof || look.marker {
                break;
            }
            if !look.nl {
                return self.fail(
                    "unexpected-content",
                    "unexpected content after a sequence entry",
                );
            }
            if look.col < s {
                break;
            }
            if look.col > s {
                return self.fail("bad-indentation", "bad indentation of a sequence entry");
            }
            if !(self.cur() == b'-' && self.is_bb(self.pos + 1)) {
                break;
            }
            if look.tab {
                return self.fail("tab-indentation", "a tab cannot indent block content");
            }
        }
        self.finish();
        Ok(())
    }

    // ---- block scalars ----------------------------------------------------------------------

    /// `|` or `>` with indicators, then the lines indented more than `n`.
    fn block_scalar(&mut self, n: i64) -> R<()> {
        self.start(k::BLOCK_SCALAR);
        let hs = self.pos;
        self.pos += 1;
        let mut explicit = None;
        let mut seen_chomp = false;
        for _ in 0..2 {
            match self.cur() {
                b'+' | b'-' if !seen_chomp => {
                    seen_chomp = true;
                    self.pos += 1;
                }
                d @ b'1'..=b'9' if explicit.is_none() => {
                    explicit = Some((d - b'0') as i64);
                    self.pos += 1;
                }
                b'0' => {
                    return self.fail(
                        "bad-indentation-indicator",
                        "an indentation indicator is 1-9",
                    );
                }
                _ => {}
            }
        }
        let he = self.pos;
        self.skip_inline_ws();
        if !(self.eof() || is_break(self.cur()) || self.comment_at(self.pos)) {
            return self.fail(
                "bad-block-header",
                "unexpected character in a block scalar header",
            );
        }
        self.tok(k::BLOCK_HEADER, hs, he);
        self.block_scalar_content(n, explicit)?;
        self.finish();
        Ok(())
    }

    /// Start of the line break ending the line at `p`, whether the line is blank (only spaces)
    /// and how many spaces it starts with.
    fn line_info(&self, p: usize) -> (usize, bool, usize) {
        let mut e = p;
        while e < self.b.len() && !is_break(self.b[e]) {
            e += 1;
        }
        let spaces = self.b[p..e].iter().take_while(|c| **c == b' ').count();
        (e, spaces == e - p, spaces)
    }

    /// The lines of a block scalar. The token starts at the first line and ends with the last
    /// line of text, never with a line break. Trailing blank lines are left to the trivia (the
    /// data model counts them for `keep` chomping).
    fn block_scalar_content(&mut self, n: i64, explicit: Option<i64>) -> R<()> {
        let mut q = self.pos;
        while q < self.b.len() && !is_break(self.b[q]) {
            q += 1;
        }
        if q >= self.b.len() {
            return Ok(());
        }
        let content_start = self.skip_break(q);
        let mut indent: Option<usize> = explicit.map(|d| (n + d).max(0) as usize);
        if indent.is_none() {
            // The first line with content decides; blank lines before it may not be longer.
            let mut p = content_start;
            let mut longest_blank = 0;
            while p < self.b.len() {
                let (e, blank, spaces) = self.line_info(p);
                if !blank {
                    if spaces == 0 && self.b[p] == 9 && n >= 0 {
                        self.pos = p;
                        return self.fail(
                            "tab-indentation",
                            "a tab cannot indent block scalar content",
                        );
                    }
                    if (spaces as i64) > n {
                        indent = Some(spaces);
                        if longest_blank > spaces {
                            self.pos = p;
                            return self.fail(
                                "bad-indentation",
                                "leading blank lines are indented more than the first line",
                            );
                        }
                    }
                    break;
                }
                longest_blank = longest_blank.max(spaces);
                p = if e >= self.b.len() {
                    e
                } else {
                    self.skip_break(e)
                };
            }
        }
        // Without content lines no text line belongs to the scalar.
        let m = indent.unwrap_or(usize::MAX);
        let mut p = content_start;
        let mut last_text_end = None;
        while p < self.b.len() {
            let (e, blank, spaces) = self.line_info(p);
            let next = if e >= self.b.len() {
                e
            } else {
                self.skip_break(e)
            };
            // Spaces beyond the indentation are content.
            let blank = blank && (m == usize::MAX || spaces <= m);
            if !blank {
                if spaces < m || (spaces == 0 && self.marker_at(p).is_some()) {
                    break;
                }
                last_text_end = Some(e);
            }
            if next == e {
                break;
            }
            p = next;
        }
        if let Some(end) = last_text_end
            && end > content_start
        {
            self.tok(k::BLOCK_CONTENT, content_start, end);
        }
        Ok(())
    }
}
