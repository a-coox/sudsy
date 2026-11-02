//! Character-level helpers: positions and columns, trivia, node properties, and the scanners
//! for plain, single-quoted and double-quoted scalars.

use super::{Parser, R};
use crate::generated::kind as k;

pub(crate) fn is_blank(c: u8) -> bool {
    matches!(c, b' ' | b'\t')
}

pub(crate) fn is_break(c: u8) -> bool {
    matches!(c, b'\n' | b'\r')
}

pub(crate) fn is_flow_indicator(c: u8) -> bool {
    matches!(c, b',' | b'[' | b']' | b'{' | b'}')
}

/// Where the next content is, after skipping blanks, comments and line breaks.
pub(crate) struct Look {
    /// A line break was crossed.
    pub nl: bool,
    /// Column of the content, in characters.
    pub col: usize,
    /// The blanks before the content on its line contain a tab.
    pub tab: bool,
    pub eof: bool,
    /// The content is a `---` or `...` document marker.
    pub marker: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Marker {
    Start,
    End,
}

impl Parser<'_> {
    pub(crate) fn at(&self, i: usize) -> u8 {
        self.b.get(i).copied().unwrap_or(0)
    }

    pub(crate) fn cur(&self) -> u8 {
        self.at(self.pos)
    }

    pub(crate) fn eof(&self) -> bool {
        self.pos >= self.b.len()
    }

    /// Blank, line break or end of input at `i`.
    pub(crate) fn is_bb(&self, i: usize) -> bool {
        i >= self.b.len() || is_blank(self.b[i]) || is_break(self.b[i])
    }

    pub(crate) fn line_start(&self, p: usize) -> usize {
        self.b[..p]
            .iter()
            .rposition(|c| is_break(*c))
            .map_or(0, |i| i + 1)
    }

    pub(crate) fn col(&self, p: usize) -> usize {
        self.src[self.line_start(p)..p].chars().count()
    }

    /// Is a comment allowed to start at `i`: after a blank, or at the start of a line.
    pub(crate) fn comment_at(&self, i: usize) -> bool {
        self.at(i) == b'#' && (i == 0 || is_blank(self.b[i - 1]) || is_break(self.b[i - 1]))
    }

    pub(crate) fn marker_at(&self, p: usize) -> Option<Marker> {
        if p != 0 && !is_break(self.b[p - 1]) {
            return None;
        }
        if !self.is_bb(p + 3) {
            return None;
        }
        match self.b.get(p..p + 3) {
            Some(b"---") => Some(Marker::Start),
            Some(b"...") => Some(Marker::End),
            _ => None,
        }
    }

    /// Position after the line break at `i` (`\r\n` counts once).
    pub(crate) fn skip_break(&self, i: usize) -> usize {
        if self.at(i) == b'\r' && self.at(i + 1) == b'\n' {
            i + 2
        } else {
            i + 1
        }
    }

    /// Skips spaces and tabs; returns whether a tab was among them.
    pub(crate) fn skip_inline_ws(&mut self) -> bool {
        let mut tab = false;
        while !self.eof() && is_blank(self.cur()) {
            tab |= self.cur() == b'\t';
            self.pos += 1;
        }
        tab
    }

    /// Is there content on the rest of this line (not a comment, not the end)?
    pub(crate) fn at_content(&self) -> bool {
        !self.eof() && !is_break(self.cur()) && !self.comment_at(self.pos)
    }

    /// Skips blanks, comments and line breaks up to the next content.
    pub(crate) fn skip_trivia(&mut self) -> Look {
        let mut nl = false;
        let mut tab = false;
        while !self.eof() {
            let c = self.cur();
            if c == b' ' {
                self.pos += 1;
            } else if c == b'\t' {
                tab = true;
                self.pos += 1;
            } else if is_break(c) {
                self.pos = self.skip_break(self.pos);
                nl = true;
                tab = false;
            } else if self.comment_at(self.pos) {
                while !self.eof() && !is_break(self.cur()) {
                    self.pos += 1;
                }
            } else if self.src[self.pos..].starts_with('\u{feff}')
                && (self.pos == 0 || is_break(self.b[self.pos - 1]))
            {
                self.pos += 3;
            } else {
                break;
            }
        }
        let eof = self.eof();
        let ls = self.line_start(self.pos);
        // Content that is the first on its line counts as being on a new line.
        nl = nl || self.b[ls..self.pos].iter().all(|c| is_blank(*c));
        let col = self.col(self.pos);
        let marker = !eof && col == 0 && self.marker_at(self.pos).is_some();
        Look {
            nl,
            col,
            tab,
            eof,
            marker,
        }
    }

    /// The rest of the line must be blanks, then a comment or the end of the line.
    pub(crate) fn expect_eol(&mut self) -> R<()> {
        self.skip_inline_ws();
        if self.eof() || is_break(self.cur()) || self.comment_at(self.pos) {
            Ok(())
        } else {
            self.fail(
                "unexpected-content",
                "unexpected content at the end of the line",
            )
        }
    }

    /// Blanks, comments and line breaks inside a flow collection. Lines with content must be
    /// indented at least `min_col` and must not be document markers.
    pub(crate) fn skip_flow_ws(&mut self) -> R<()> {
        loop {
            self.skip_inline_ws();
            if self.comment_at(self.pos) {
                while !self.eof() && !is_break(self.cur()) {
                    self.pos += 1;
                }
            }
            if self.eof() || !is_break(self.cur()) {
                return Ok(());
            }
            self.pos = self.skip_break(self.pos);
            if self.marker_at(self.pos).is_some() {
                return self.fail(
                    "marker-in-flow",
                    "a document marker inside a flow collection",
                );
            }
            let mut j = self.pos;
            while self.at(j) == b' ' {
                j += 1;
            }
            let spaces = j - self.pos;
            while is_blank(self.at(j)) {
                j += 1;
            }
            let blank_line = j >= self.b.len() || is_break(self.at(j)) || self.comment_at(j);
            if !blank_line && spaces < self.min_col {
                self.pos = j;
                return self.fail("bad-indentation", "bad indentation inside a flow node");
            }
        }
    }

    // ---- properties -------------------------------------------------------------------------

    fn is_uri_char(c: u8) -> bool {
        c.is_ascii_alphanumeric() || b"-#;/?:@&=+$_.~*'()%".contains(&c)
    }

    /// `!`, `!!suffix`, `!handle!suffix`, `!suffix` or `!<uri>`.
    fn scan_tag(&mut self) -> R<()> {
        let s = self.pos;
        let mut i = s + 1;
        if self.at(i) == b'<' {
            i += 1;
            let body = i;
            while self.at(i) != b'>'
                && (Self::is_uri_char(self.at(i)) || b"!,[]".contains(&self.at(i)))
            {
                i += 1;
            }
            if self.at(i) != b'>' || i == body {
                self.pos = i;
                return self.fail("bad-tag", "a verbatim tag needs a URI and a closing `>`");
            }
            i += 1;
        } else {
            let mut j = i;
            while self.at(j).is_ascii_alphanumeric() || self.at(j) == b'-' {
                j += 1;
            }
            let named = self.at(j) == b'!';
            if named {
                if j > i && !self.handles.iter().any(|h| *h == self.src[s..=j]) {
                    self.pos = j;
                    return self.fail("undefined-tag-handle", "the tag handle is not declared");
                }
                i = j + 1;
            }
            let suffix = i;
            while Self::is_uri_char(self.at(i)) {
                if self.at(i) == b'%'
                    && !(self.at(i + 1).is_ascii_hexdigit() && self.at(i + 2).is_ascii_hexdigit())
                {
                    self.pos = i;
                    return self.fail("bad-tag", "`%` in a tag must be followed by two hex digits");
                }
                i += 1;
            }
            if named && i == suffix {
                self.pos = i;
                return self.fail("bad-tag", "a tag handle needs a suffix");
            }
        }
        self.tok(k::TAG, s, i);
        Ok(())
    }

    fn scan_name(&mut self, kind: core_syntax::SyntaxKind, what: &str) -> R<()> {
        let s = self.pos;
        let mut i = s + 1;
        while !self.is_bb(i) && !is_flow_indicator(self.at(i)) {
            i += 1;
        }
        while !self.src.is_char_boundary(i) {
            i += 1;
        }
        if i == s + 1 {
            return self.fail("bad-name", &format!("{what} needs a name"));
        }
        self.tok(kind, s, i);
        Ok(())
    }

    pub(crate) fn scan_anchor(&mut self) -> R<()> {
        self.scan_name(k::ANCHOR, "an anchor")
    }

    pub(crate) fn scan_alias(&mut self) -> R<()> {
        self.scan_name(k::ALIAS, "an alias")
    }

    /// At most one tag and one anchor, in any order, separated by blanks. Returns which were
    /// found: bit 0 the tag, bit 1 the anchor.
    pub(crate) fn properties(&mut self) -> R<u8> {
        let (mut tag, mut anchor) = (false, false);
        loop {
            match self.cur() {
                b'!' if !tag => {
                    self.scan_tag()?;
                    tag = true;
                }
                b'&' if !anchor => {
                    self.scan_anchor()?;
                    anchor = true;
                }
                b'!' | b'&' => {
                    return self.fail("duplicate-property", "a node has one tag and one anchor");
                }
                _ => break,
            }
            if !self.is_bb(self.pos) && !(self.in_flow > 0 && is_flow_indicator(self.cur())) {
                return self.fail("bad-property", "unexpected character after a node property");
            }
            let before = self.pos;
            if self.in_flow > 0 {
                self.skip_flow_ws()?;
            } else {
                self.skip_inline_ws();
            }
            if self.pos == before {
                break;
            }
        }
        Ok(u8::from(tag) | u8::from(anchor) << 1)
    }

    // ---- scalars ----------------------------------------------------------------------------

    /// Can a plain scalar start here?
    pub(crate) fn plain_start(&self) -> bool {
        let c = self.cur();
        if self.eof() || is_blank(c) || is_break(c) {
            return false;
        }
        match c {
            b'-' | b'?' | b':' => {
                !self.is_bb(self.pos + 1)
                    && !(self.in_flow > 0 && is_flow_indicator(self.at(self.pos + 1)))
            }
            b',' | b'[' | b']' | b'{' | b'}' | b'#' | b'&' | b'*' | b'!' | b'|' | b'>' | b'\''
            | b'"' | b'%' | b'@' | b'`' => false,
            _ => true,
        }
    }

    /// May the line whose first non-blank character is at `p` continue a plain scalar?
    fn plain_continues(&self, p: usize) -> bool {
        let in_flow = self.in_flow > 0;
        match self.at(p) {
            b':' => !(self.is_bb(p + 1) || in_flow && is_flow_indicator(self.at(p + 1))),
            c if in_flow && is_flow_indicator(c) => false,
            _ => true,
        }
    }

    /// End of the plain scalar content on the line that continues at `p`.
    fn plain_line(&self, p: usize) -> usize {
        let in_flow = self.in_flow > 0;
        let mut last = p;
        let mut i = p;
        while i < self.b.len() {
            let c = self.b[i];
            if is_break(c) {
                break;
            }
            if is_blank(c) {
                let mut j = i;
                while j < self.b.len() && is_blank(self.b[j]) {
                    j += 1;
                }
                if j >= self.b.len() || is_break(self.b[j]) || self.b[j] == b'#' {
                    break;
                }
                i = j;
                continue;
            }
            if c == b':' && (self.is_bb(i + 1) || in_flow && is_flow_indicator(self.at(i + 1))) {
                break;
            }
            if in_flow && is_flow_indicator(c) {
                break;
            }
            i += 1;
            while i < self.b.len() && (self.b[i] & 0xC0) == 0x80 {
                i += 1;
            }
            last = i;
        }
        last
    }

    /// Scans a plain scalar and emits its token; returns whether it spans lines.
    pub(crate) fn scan_plain(&mut self) -> bool {
        let start = self.pos;
        let mut p = start;
        let mut multiline = false;
        let end = loop {
            let end = self.plain_line(p);
            let mut q = end;
            while is_blank(self.at(q)) {
                q += 1;
            }
            if q < self.b.len() && !is_break(self.b[q]) {
                break end;
            }
            // Look for a continuation line past this break and any blank lines.
            let mut r = q;
            let next = loop {
                if r >= self.b.len() {
                    break None;
                }
                r = self.skip_break(r);
                let ls = r;
                while self.at(r) == b' ' {
                    r += 1;
                }
                let spaces = r - ls;
                while is_blank(self.at(r)) {
                    r += 1;
                }
                if r >= self.b.len() {
                    break None;
                }
                if is_break(self.b[r]) {
                    continue;
                }
                break Some((ls, r, spaces));
            };
            let Some((ls, f, spaces)) = next else {
                break end;
            };
            if spaces < self.min_col
                || (spaces == 0 && self.marker_at(ls).is_some())
                || self.b[f] == b'#'
                || !self.plain_continues(f)
            {
                break end;
            }
            multiline = true;
            p = f;
        };
        self.tok(k::PLAIN, start, end);
        multiline
    }

    /// At a line break inside a quoted scalar at `i`: skips blank lines. The next line's
    /// indentation must reach `min_col` and it must not be a document marker. Returns the
    /// position of its first non-blank character.
    fn quoted_newline(&mut self, mut i: usize) -> R<usize> {
        loop {
            i = self.skip_break(i);
            let ls = i;
            while self.at(i) == b' ' {
                i += 1;
            }
            let spaces = i - ls;
            while is_blank(self.at(i)) {
                i += 1;
            }
            if i >= self.b.len() {
                self.pos = i;
                return self.fail("unterminated-scalar", "unterminated quoted scalar");
            }
            if is_break(self.b[i]) {
                continue;
            }
            if spaces == 0 && self.marker_at(ls).is_some() {
                self.pos = ls;
                return self.fail(
                    "marker-in-scalar",
                    "a document marker inside a quoted scalar",
                );
            }
            if spaces < self.min_col {
                self.pos = i;
                return self.fail("bad-indentation", "bad indentation of a quoted scalar line");
            }
            return Ok(i);
        }
    }

    /// Scans a double-quoted scalar and emits its token; returns whether it spans lines.
    pub(crate) fn scan_double(&mut self) -> R<bool> {
        let start = self.pos;
        let mut i = start + 1;
        let mut multiline = false;
        loop {
            if i >= self.b.len() {
                self.pos = i;
                return self.fail("unterminated-scalar", "unterminated double-quoted scalar");
            }
            match self.b[i] {
                b'"' => {
                    i += 1;
                    break;
                }
                b'\\' => match self.at(i + 1) {
                    b'0' | b'a' | b'b' | b't' | b'\t' | b'n' | b'v' | b'f' | b'r' | b'e' | b' '
                    | b'"' | b'/' | b'\\' | b'N' | b'_' | b'L' | b'P' => i += 2,
                    c @ (b'x' | b'u' | b'U') => {
                        let digits = match c {
                            b'x' => 2,
                            b'u' => 4,
                            _ => 8,
                        };
                        if !(0..digits).all(|d| self.at(i + 2 + d).is_ascii_hexdigit()) {
                            self.pos = i;
                            return self.fail("bad-escape", "an escape needs hexadecimal digits");
                        }
                        i += 2 + digits;
                    }
                    b'\n' | b'\r' => {
                        multiline = true;
                        i = self.quoted_newline(i + 1)?;
                    }
                    _ => {
                        self.pos = i;
                        return self.fail("bad-escape", "an invalid escape sequence");
                    }
                },
                b'\n' | b'\r' => {
                    multiline = true;
                    i = self.quoted_newline(i)?;
                }
                _ => i += 1,
            }
        }
        self.tok(k::DOUBLE_QUOTED, start, i);
        Ok(multiline)
    }

    /// Scans a single-quoted scalar and emits its token; returns whether it spans lines.
    pub(crate) fn scan_single(&mut self) -> R<bool> {
        let start = self.pos;
        let mut i = start + 1;
        let mut multiline = false;
        loop {
            if i >= self.b.len() {
                self.pos = i;
                return self.fail("unterminated-scalar", "unterminated single-quoted scalar");
            }
            match self.b[i] {
                b'\'' if self.at(i + 1) == b'\'' => i += 2,
                b'\'' => {
                    i += 1;
                    break;
                }
                b'\n' | b'\r' => {
                    multiline = true;
                    i = self.quoted_newline(i)?;
                }
                _ => i += 1,
            }
        }
        self.tok(k::SINGLE_QUOTED, start, i);
        Ok(multiline)
    }
}
