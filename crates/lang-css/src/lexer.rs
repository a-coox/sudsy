//! The lossless lexer: every byte of the input ends up in exactly one token. CSS Syntax Level 3
//! tokens, plus the extras of the dialects (see [`Dialect`]).

use crate::Dialect;
use crate::generated::kind as k;
use core_syntax::SyntaxKind;

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub start: usize,
    pub end: usize,
}

/// A problem found while lexing.
pub struct LexError {
    pub start: usize,
    pub len: usize,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn lex(src: &str, dialect: Dialect) -> (Vec<Token>, Vec<LexError>) {
    let mut lx = Lexer {
        src: src.as_bytes(),
        text: src,
        pos: 0,
        dialect,
        tokens: Vec::with_capacity(src.len() / 4),
        errors: Vec::new(),
    };
    while lx.pos < src.len() {
        lx.next();
    }
    lx.tokens.push(Token {
        kind: k::EOF,
        start: src.len(),
        end: src.len(),
    });
    (lx.tokens, lx.errors)
}

struct Lexer<'a> {
    src: &'a [u8],
    text: &'a str,
    pos: usize,
    dialect: Dialect,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_name(b: u8) -> bool {
    is_name_start(b) || b.is_ascii_digit() || b == b'-'
}

impl Lexer<'_> {
    fn at(&self, i: usize) -> u8 {
        self.src.get(self.pos + i).copied().unwrap_or(0)
    }

    fn emit(&mut self, kind: SyntaxKind, start: usize) {
        self.tokens.push(Token {
            kind,
            start,
            end: self.pos,
        });
    }

    fn error(&mut self, start: usize, code: &'static str, message: &'static str) {
        self.errors.push(LexError {
            start,
            len: self.pos - start,
            code,
            message,
        });
    }

    /// Whether a backslash at `i` starts an escape (it is not followed by a line break or the end).
    fn escape_at(&self, i: usize) -> bool {
        self.at(i) == b'\\' && !matches!(self.at(i + 1), 0 | b'\n' | b'\r' | 0x0c)
    }

    fn ident_at(&self, i: usize) -> bool {
        let c = self.at(i);
        is_name_start(c)
            || self.escape_at(i)
            || c == b'-'
                && (is_name_start(self.at(i + 1))
                    || self.at(i + 1) == b'-'
                    || self.escape_at(i + 1))
    }

    fn number_at(&self, i: usize) -> bool {
        let digit_or_dot = |j| {
            self.at(j).is_ascii_digit() || self.at(j) == b'.' && self.at(j + 1).is_ascii_digit()
        };
        digit_or_dot(i) || matches!(self.at(i), b'+' | b'-') && digit_or_dot(i + 1)
    }

    /// Does a number, dimension or identifier end right here? A sign after it is an operator
    /// (`n+3` is the same three tokens as `n + 3`), not the sign of a number.
    fn after_operand(&self) -> bool {
        self.tokens.last().is_some_and(|t| {
            t.end == self.pos
                && matches!(t.kind, k::IDENT | k::NUMBER | k::DIMENSION | k::PERCENTAGE)
        })
    }

    /// After a backslash: up to six hex digits and one blank, or any one character.
    fn escape(&mut self) {
        self.pos += 1;
        if self.at(0).is_ascii_hexdigit() {
            let mut n = 0;
            while n < 6 && self.at(0).is_ascii_hexdigit() {
                self.pos += 1;
                n += 1;
            }
            if self.text[self.pos..].starts_with("\r\n") {
                self.pos += 2;
            } else if matches!(self.at(0), b' ' | b'\t' | b'\n' | b'\r' | 0x0c) {
                self.pos += 1;
            }
        } else {
            self.pos += self.text[self.pos..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        }
    }

    fn name(&mut self) {
        while self.pos < self.src.len() {
            if is_name(self.at(0)) {
                self.pos += 1;
            } else if self.escape_at(0) {
                self.escape();
            } else {
                break;
            }
        }
    }

    fn next(&mut self) {
        let start = self.pos;
        let c = self.at(0);
        let kind = match c {
            b' ' | b'\t' | 0x0c => {
                while matches!(self.at(0), b' ' | b'\t' | 0x0c) {
                    self.pos += 1;
                }
                k::WHITESPACE
            }
            b'\n' | b'\r' => {
                self.pos += if self.at(0) == b'\r' && self.at(1) == b'\n' {
                    2
                } else {
                    1
                };
                k::NEWLINE
            }
            b'/' if self.at(1) == b'*' => match self.text[start + 2..].find("*/") {
                Some(i) => {
                    self.pos = start + 2 + i + 2;
                    k::COMMENT
                }
                None => {
                    self.pos = self.src.len();
                    self.error(start, "unterminated-comment", "unterminated comment");
                    k::UNKNOWN
                }
            },
            b'/' if self.at(1) == b'/' && self.dialect != Dialect::Css => {
                while self.pos < self.src.len() && !matches!(self.at(0), b'\n' | b'\r') {
                    self.pos += 1;
                }
                k::LINE_COMMENT
            }
            b'"' | b'\'' => self.string(c),
            b'#' if self.at(1) == b'{' && self.dialect == Dialect::Scss => {
                self.pos += 2;
                k::INTERP_OPEN
            }
            b'#' if is_name(self.at(1)) || self.escape_at(1) => {
                self.pos += 1;
                self.name();
                k::HASH
            }
            b'@' if self.at(1) == b'{' && self.dialect == Dialect::Less => {
                self.pos += 2;
                k::INTERP_OPEN
            }
            b'@' if self.ident_at(1) => {
                self.pos += 1;
                self.name();
                k::AT_KEYWORD
            }
            b'$' if self.dialect == Dialect::Scss && self.ident_at(1) => {
                self.pos += 1;
                self.name();
                k::VARIABLE
            }
            _ if self.number_at(0) && !(matches!(c, b'+' | b'-') && self.after_operand()) => {
                self.number()
            }
            b'u' | b'U'
                if self.at(1) == b'+' && (self.at(2).is_ascii_hexdigit() || self.at(2) == b'?') =>
            {
                self.pos += 2;
                while self.at(0).is_ascii_hexdigit() || matches!(self.at(0), b'?' | b'-') {
                    self.pos += 1;
                }
                k::UNICODE_RANGE
            }
            _ if self.ident_at(0) => {
                self.name();
                if self.at(0) == b'(' && self.text[start..self.pos].eq_ignore_ascii_case("url") {
                    self.url()
                } else {
                    k::IDENT
                }
            }
            b':' => self.one(k::COLON),
            b';' => self.one(k::SEMICOLON),
            b',' => self.one(k::COMMA),
            b'{' => self.one(k::L_BRACE),
            b'}' => self.one(k::R_BRACE),
            b'(' => self.one(k::L_PAREN),
            b')' => self.one(k::R_PAREN),
            b'[' => self.one(k::L_BRACKET),
            b']' => self.one(k::R_BRACKET),
            _ => {
                self.pos += self.text[start..].chars().next().map_or(1, char::len_utf8);
                k::DELIM
            }
        };
        self.emit(kind, start);
    }

    fn one(&mut self, kind: SyntaxKind) -> SyntaxKind {
        self.pos += 1;
        kind
    }

    /// A quoted string; one that is not closed on its line is `Unknown` up to the line's end.
    fn string(&mut self, quote: u8) -> SyntaxKind {
        let start = self.pos;
        self.pos += 1;
        loop {
            match self.at(0) {
                _ if self.pos >= self.src.len() => break,
                b'\\' => {
                    self.pos += 1;
                    if self.text[self.pos..].starts_with("\r\n") {
                        self.pos += 2;
                    } else if self.pos < self.src.len() {
                        self.pos += self.text[self.pos..]
                            .chars()
                            .next()
                            .map_or(1, char::len_utf8);
                    }
                }
                c if c == quote => {
                    self.pos += 1;
                    return k::STRING;
                }
                b'\n' | b'\r' => break,
                _ => self.pos += 1,
            }
        }
        self.error(start, "unterminated-string", "unterminated string");
        k::UNKNOWN
    }

    /// `url(` and an unquoted body: one token. Anything else (a quoted string, a function call,
    /// an interpolation, a `$variable`) is a function call of the ordinary tokens.
    fn url(&mut self) -> SyntaxKind {
        let mut i = 1;
        while self.pos + i < self.src.len()
            && !matches!(self.at(i), b')' | b'"' | b'\'' | b'(' | b'{')
        {
            i += if self.at(i) == b'\\' { 2 } else { 1 };
        }
        let end = (self.pos + i).min(self.src.len());
        if self.src.get(end) == Some(&b')')
            && self.at(1) != b'$'
            && !self.text[self.pos + 1..end]
                .trim()
                .contains(char::is_whitespace)
        {
            self.pos = end + 1;
            return k::URL;
        }
        k::IDENT
    }

    fn number(&mut self) -> SyntaxKind {
        if matches!(self.at(0), b'+' | b'-') {
            self.pos += 1;
        }
        while self.at(0).is_ascii_digit() {
            self.pos += 1;
        }
        if self.at(0) == b'.' && self.at(1).is_ascii_digit() {
            self.pos += 1;
            while self.at(0).is_ascii_digit() {
                self.pos += 1;
            }
        }
        if matches!(self.at(0), b'e' | b'E') {
            let sign = usize::from(matches!(self.at(1), b'+' | b'-'));
            if self.at(1 + sign).is_ascii_digit() {
                self.pos += 1 + sign;
                while self.at(0).is_ascii_digit() {
                    self.pos += 1;
                }
            }
        }
        if self.at(0) == b'%' {
            self.pos += 1;
            k::PERCENTAGE
        } else if self.ident_at(0) {
            self.name();
            k::DIMENSION
        } else {
            k::NUMBER
        }
    }
}
