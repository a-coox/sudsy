//! A lossless recursive-descent parser for JSON and JSONC with error recovery.
//!
//! Every token ends up in the tree. Trivia are placed by the shared attachment rule (see
//! `core_syntax::trivia`): trailing trivia right after their token, leading trivia right before
//! theirs, inside the node that token belongs to. Unexpected tokens are wrapped in `ERROR` nodes.

use crate::generated::kind as k;
use crate::lexer::{LexError, Token, lex};
use core_syntax::trivia::is_line_break;
use core_syntax::{SyntaxKind, SyntaxNode, TreeBuilder};
use provider_api::ParseError;
use std::ops::Range;

/// Deeper nesting is an error: the rest of the input is kept as one `ERROR` node. This bounds
/// recursion in the parser, the formatter and tree destruction.
pub const MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Json,
    /// JSON with comments and trailing commas.
    Jsonc,
}

impl Dialect {
    pub fn from_name(name: &str) -> Option<Dialect> {
        match name {
            "json" => Some(Dialect::Json),
            "jsonc" => Some(Dialect::Jsonc),
            _ => None,
        }
    }
}

pub struct Parsed {
    pub tree: SyntaxNode,
    pub errors: Vec<ParseError>,
}

pub fn parse(src: &str, dialect: Dialect) -> Parsed {
    let (tokens, lex_errors) = lex(src);
    let mut p = Parser::new(src, tokens, dialect);
    p.errors
        .extend(lex_errors.into_iter().map(|e: LexError| ParseError {
            start: e.start as u32,
            len: e.len as u32,
            code: e.code.into(),
            message: e.message,
        }));
    p.document();
    p.errors.sort_by_key(|e| e.start);
    Parsed {
        tree: SyntaxNode::new_root(p.builder.finish()),
        errors: p.errors,
    }
}

struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    /// Indices into `tokens` of the non-trivia tokens.
    real: Vec<usize>,
    /// Trivia token ranges owned by each real token.
    leading: Vec<Range<usize>>,
    trailing: Vec<Range<usize>>,
    pos: usize,
    depth: usize,
    dialect: Dialect,
    builder: TreeBuilder,
    errors: Vec<ParseError>,
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHITESPACE | k::NEWLINE | k::LINE_COMMENT | k::BLOCK_COMMENT
    )
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, tokens: Vec<Token>, dialect: Dialect) -> Self {
        let real: Vec<usize> = (0..tokens.len())
            .filter(|&i| !is_trivia(tokens[i].kind))
            .collect();
        let mut leading = Vec::with_capacity(real.len());
        let mut trailing = Vec::with_capacity(real.len());
        let mut prev_end = 0; // index after the previous real token
        for &r in &real {
            // Trivia tokens prev_end..r: the rest of the previous token's line is trailing.
            let run = prev_end..r;
            let cut = if prev_end == 0 {
                run.start
            } else {
                let ends_line = |i: usize| {
                    src[tokens[i].start..tokens[i].end]
                        .chars()
                        .next_back()
                        .is_some_and(is_line_break)
                };
                run.clone()
                    .find(|&i| ends_line(i))
                    .map_or(run.end, |i| i + 1)
            };
            if let Some(last) = trailing.last_mut() {
                *last = run.start..cut;
            }
            leading.push(cut..r);
            trailing.push(r + 1..r + 1);
            prev_end = r + 1;
        }
        Parser {
            src,
            tokens,
            real,
            leading,
            trailing,
            pos: 0,
            depth: 0,
            dialect,
            builder: TreeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn cur(&self) -> SyntaxKind {
        self.tokens[self.real[self.pos]].kind
    }

    fn cur_range(&self) -> Range<usize> {
        let t = &self.tokens[self.real[self.pos]];
        t.start..t.end
    }

    fn at_eof(&self) -> bool {
        self.cur() == k::EOF
    }

    fn error(&mut self, code: &str, message: impl Into<String>) {
        let r = self.cur_range();
        self.error_at(r, code, message);
    }

    /// One diagnostic per error site: a report that overlaps an earlier one (a lexer error on
    /// the same token, or a second complaint about the same position) is dropped.
    fn error_at(&mut self, r: Range<usize>, code: &str, message: impl Into<String>) {
        let (start, end) = (r.start as u32, r.end as u32);
        if self
            .errors
            .iter()
            .any(|e| e.start == start || (e.start < end && start < e.start + e.len))
        {
            return;
        }
        self.errors.push(ParseError {
            start: r.start as u32,
            len: (r.end - r.start) as u32,
            code: code.into(),
            message: message.into(),
        });
    }

    fn emit(&mut self, i: usize) {
        let t = self.tokens[i];
        if t.kind == k::LINE_COMMENT || t.kind == k::BLOCK_COMMENT {
            self.comment_allowed(t);
        }
        self.builder.token(t.kind, &self.src[t.start..t.end]);
    }

    fn comment_allowed(&mut self, t: Token) {
        if self.dialect == Dialect::Json {
            self.error_at(
                t.start..t.end,
                "comment-in-json",
                "comments are not allowed in JSON (use the jsonc dialect)",
            );
        }
    }

    /// Consumes the current token with its leading and trailing trivia.
    fn bump(&mut self) {
        let (lead, trail) = (
            self.leading[self.pos].clone(),
            self.trailing[self.pos].clone(),
        );
        for i in lead {
            self.emit(i);
        }
        self.emit(self.real[self.pos]);
        for i in trail {
            self.emit(i);
        }
        if !self.at_eof() {
            self.pos += 1;
        }
    }

    fn error_node(&mut self) {
        self.builder.start_node(SyntaxKind::ERROR);
        self.bump();
        self.builder.finish_node();
    }

    fn starts_value(&self) -> bool {
        matches!(
            self.cur(),
            k::L_BRACE | k::L_BRACKET | k::STRING | k::NUMBER | k::TRUE | k::FALSE | k::NULL
        )
    }

    fn document(&mut self) {
        self.builder.start_node(k::DOCUMENT);
        if self.starts_value() {
            self.value();
        } else if self.at_eof() {
            self.error("expected-value", "expected a value");
        }
        if !self.at_eof() {
            let message = if self.starts_value() {
                "unexpected content after the top-level value"
            } else {
                "unexpected token"
            };
            self.error("unexpected-token", message);
            // Everything after the first stray token is one error site.
            self.builder.start_node(SyntaxKind::ERROR);
            while !self.at_eof() {
                self.bump();
            }
            self.builder.finish_node();
        }
        self.bump(); // Eof
        self.builder.finish_node();
    }

    /// At a token that starts a value.
    fn value(&mut self) {
        match self.cur() {
            k::L_BRACE | k::L_BRACKET => {
                if self.depth >= MAX_DEPTH {
                    self.too_deep();
                } else {
                    self.depth += 1;
                    if self.cur() == k::L_BRACE {
                        self.object();
                    } else {
                        self.array();
                    }
                    self.depth -= 1;
                }
            }
            _ => {
                self.builder.start_node(k::LITERAL);
                self.bump();
                self.builder.finish_node();
            }
        }
    }

    fn too_deep(&mut self) {
        self.error(
            "nesting-too-deep",
            format!("nesting is deeper than {MAX_DEPTH} levels"),
        );
        self.builder.start_node(SyntaxKind::ERROR);
        while !self.at_eof() {
            self.bump();
        }
        self.builder.finish_node();
    }

    fn object(&mut self) {
        self.builder.start_node(k::OBJECT);
        self.bump(); // {
        loop {
            match self.cur() {
                k::R_BRACE => break,
                k::EOF => {
                    self.error("unclosed-object", "expected `}` before the end of the file");
                    break;
                }
                k::STRING => {
                    self.member();
                    match self.cur() {
                        k::COMMA => self.comma(k::R_BRACE),
                        k::R_BRACE | k::EOF => {}
                        _ => self.error("expected-comma", "expected `,` or `}`"),
                    }
                }
                k::COMMA => {
                    self.error("unexpected-comma", "unexpected `,`");
                    self.error_node();
                }
                _ => {
                    self.error("expected-key", "expected a string key or `}`");
                    self.recover(k::R_BRACE);
                }
            }
        }
        if self.cur() == k::R_BRACE {
            self.bump();
        }
        self.builder.finish_node();
    }

    /// Consumes the `,` after an item.
    fn comma(&mut self, closer: SyntaxKind) {
        self.bump();
        if self.cur() == closer {
            self.trailing_comma_allowed();
        }
    }

    /// Skips the rest of a bad item without further reports: everything up to the next `,` or
    /// the closing delimiter at this level (brackets in between are balanced).
    fn recover(&mut self, closer: SyntaxKind) {
        self.builder.start_node(SyntaxKind::ERROR);
        let mut nesting = 0usize;
        while !self.at_eof() {
            match self.cur() {
                k::L_BRACE | k::L_BRACKET => nesting += 1,
                k::R_BRACE | k::R_BRACKET if nesting > 0 => nesting -= 1,
                k::COMMA if nesting == 0 => break,
                kind if kind == closer && nesting == 0 => break,
                _ => {}
            }
            self.bump();
        }
        self.builder.finish_node();
        if self.cur() == k::COMMA {
            self.comma(closer);
        }
    }

    fn member(&mut self) {
        self.builder.start_node(k::MEMBER);
        self.bump(); // key
        if self.cur() == k::COLON {
            self.bump();
        } else {
            self.error("expected-colon", "expected `:` after the key");
        }
        if self.starts_value() {
            self.value();
        } else {
            self.error("expected-value", "expected a value");
        }
        self.builder.finish_node();
    }

    fn array(&mut self) {
        self.builder.start_node(k::ARRAY);
        self.bump(); // [
        loop {
            match self.cur() {
                k::R_BRACKET => break,
                k::EOF => {
                    self.error("unclosed-array", "expected `]` before the end of the file");
                    break;
                }
                k::COMMA => {
                    self.error("unexpected-comma", "unexpected `,`");
                    self.error_node();
                }
                _ if self.starts_value() => {
                    self.value();
                    match self.cur() {
                        k::COMMA => self.comma(k::R_BRACKET),
                        k::R_BRACKET | k::EOF => {}
                        _ => self.error("expected-comma", "expected `,` or `]`"),
                    }
                }
                _ => {
                    self.error("expected-value", "expected a value or `]`");
                    self.recover(k::R_BRACKET);
                }
            }
        }
        if self.cur() == k::R_BRACKET {
            self.bump();
        }
        self.builder.finish_node();
    }

    /// Called after a comma that is followed by the closing delimiter. The comma is the previous
    /// real token.
    fn trailing_comma_allowed(&mut self) {
        if self.dialect == Dialect::Json {
            let t = self.tokens[self.real[self.pos - 1]];
            self.error_at(
                t.start..t.end,
                "trailing-comma",
                "trailing commas are not allowed in JSON (use the jsonc dialect)",
            );
        }
    }
}
