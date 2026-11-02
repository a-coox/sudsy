//! The lossless lexer: every byte of the input ends up in exactly one token.

use core_syntax::SyntaxKind;
use core_syntax::generated::json::kind as k;

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: SyntaxKind,
    pub start: usize,
    pub end: usize,
}

/// A problem found while lexing.
#[derive(Clone, Debug)]
pub struct LexError {
    pub start: usize,
    pub len: usize,
    pub code: &'static str,
    pub message: String,
}

pub fn lex(src: &str) -> (Vec<Token>, Vec<LexError>) {
    let mut lx = Lexer {
        src,
        pos: 0,
        tokens: Vec::new(),
        errors: Vec::new(),
    };
    while lx.pos < src.len() {
        lx.next();
    }
    let end = src.len();
    lx.tokens.push(Token {
        kind: k::EOF,
        start: end,
        end,
    });
    (lx.tokens, lx.errors)
}

struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(offset)
    }

    fn error(&mut self, start: usize, end: usize, code: &'static str, message: impl Into<String>) {
        self.errors.push(LexError {
            start,
            len: end - start,
            code,
            message: message.into(),
        });
    }

    fn emit(&mut self, kind: SyntaxKind, start: usize) {
        self.tokens.push(Token {
            kind,
            start,
            end: self.pos,
        });
    }

    fn eat_while(&mut self, f: impl Fn(char) -> bool) {
        while let Some(c) = self.peek() {
            if !f(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
    }

    fn next(&mut self) {
        let start = self.pos;
        let c = self.peek().expect("not at the end");
        match c {
            ' ' | '\t' => {
                self.eat_while(|c| c == ' ' || c == '\t');
                self.emit(k::WHITESPACE, start);
            }
            '\n' | '\r' => {
                self.pos += if self.src[start..].starts_with("\r\n") {
                    2
                } else {
                    1
                };
                self.emit(k::NEWLINE, start);
            }
            '{' | '}' | '[' | ']' | ':' | ',' => {
                self.pos += 1;
                self.emit(
                    match c {
                        '{' => k::L_BRACE,
                        '}' => k::R_BRACE,
                        '[' => k::L_BRACKET,
                        ']' => k::R_BRACKET,
                        ':' => k::COLON,
                        _ => k::COMMA,
                    },
                    start,
                );
            }
            '"' => self.string(),
            '/' if self.peek_at(1) == Some('/') => {
                self.eat_while(|c| c != '\n' && c != '\r');
                self.emit(k::LINE_COMMENT, start);
            }
            '/' if self.peek_at(1) == Some('*') => self.block_comment(),
            '-' | '0'..='9' => self.number(),
            c if c.is_ascii_alphabetic() => self.word(),
            c => {
                self.pos += c.len_utf8();
                self.emit(k::UNKNOWN, start);
                self.error(
                    start,
                    self.pos,
                    "unexpected-character",
                    format!("unexpected character `{c}`"),
                );
            }
        }
    }

    fn block_comment(&mut self) {
        let start = self.pos;
        match self.src[start + 2..].find("*/") {
            Some(i) => self.pos = start + 2 + i + 2,
            None => {
                self.pos = self.src.len();
                self.error(
                    start,
                    self.pos,
                    "unterminated-comment",
                    "unterminated block comment",
                );
            }
        }
        self.emit(k::BLOCK_COMMENT, start);
    }

    fn word(&mut self) {
        let start = self.pos;
        self.eat_while(|c| c.is_ascii_alphanumeric() || c == '_');
        let kind = match &self.src[start..self.pos] {
            "true" => k::TRUE,
            "false" => k::FALSE,
            "null" => k::NULL,
            w => {
                let message = format!("unexpected word `{w}`");
                self.error(start, self.pos, "unexpected-word", message);
                k::UNKNOWN
            }
        };
        self.emit(kind, start);
    }

    fn number(&mut self) {
        let start = self.pos;
        self.eat_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_'));
        if !is_valid_number(&self.src[start..self.pos]) {
            let message = format!("invalid number `{}`", &self.src[start..self.pos]);
            self.error(start, self.pos, "invalid-number", message);
        }
        self.emit(k::NUMBER, start);
    }

    fn string(&mut self) {
        let start = self.pos;
        self.pos += 1;
        loop {
            let Some(c) = self.peek() else {
                self.error(
                    start,
                    self.pos,
                    "unterminated-string",
                    "unterminated string",
                );
                break;
            };
            let at = self.pos;
            match c {
                '"' => {
                    self.pos += 1;
                    break;
                }
                '\n' | '\r' => {
                    self.error(
                        start,
                        self.pos,
                        "unterminated-string",
                        "unterminated string",
                    );
                    break;
                }
                '\\' => {
                    self.pos += 1;
                    self.escape(at);
                }
                c if (c as u32) < 0x20 => {
                    self.pos += 1;
                    self.error(
                        at,
                        self.pos,
                        "invalid-string",
                        "control character in string must be escaped",
                    );
                }
                c => self.pos += c.len_utf8(),
            }
        }
        self.emit(k::STRING, start);
    }

    /// After a backslash at `at`.
    fn escape(&mut self, at: usize) {
        match self.peek() {
            Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') => self.pos += 1,
            Some('u') => {
                self.pos += 1;
                for _ in 0..4 {
                    match self.peek() {
                        Some(h) if h.is_ascii_hexdigit() => self.pos += 1,
                        _ => {
                            self.error(at, self.pos, "invalid-string", "invalid \\u escape");
                            return;
                        }
                    }
                }
            }
            // The string ends (or the line does) right after the backslash.
            None | Some('\n' | '\r') => {}
            Some(c) => {
                self.pos += c.len_utf8();
                self.error(
                    at,
                    self.pos,
                    "invalid-string",
                    format!("invalid escape `\\{c}`"),
                );
            }
        }
    }
}

/// `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`
fn is_valid_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let from = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > from
    };
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(b'1'..=b'9') => {
            digits(&mut i);
        }
        _ => return false,
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        if !digits(&mut i) {
            return false;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        if !digits(&mut i) {
            return false;
        }
    }
    i == b.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        for ok in ["0", "-0", "12", "1.5", "1e5", "1E+5", "-1.25e-3", "0.0"] {
            assert!(is_valid_number(ok), "{ok}");
        }
        for bad in [
            "", "-", "01", "1.", ".5", "+1", "1e", "1e+", "0x1", "1_0", "--1", "1.5.2",
        ] {
            assert!(!is_valid_number(bad), "{bad}");
        }
    }

    #[test]
    fn every_byte_is_in_a_token() {
        let src = "{ \"a\\n\": [1, true] } // c\r\n/* d */ \u{ff}x";
        let (tokens, _) = lex(src);
        let joined: String = tokens.iter().map(|t| &src[t.start..t.end]).collect();
        assert_eq!(joined, src);
    }
}
