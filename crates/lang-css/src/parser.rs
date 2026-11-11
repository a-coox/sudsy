//! A lossless parser for CSS, SCSS and Less with per-statement error recovery.
//!
//! The three dialects share one grammar of statements. What ends a statement tells its kind: `{`
//! makes a `Rule`, `;` or `}` a `Declaration` (when a `:` came first) or a `Statement`; a statement
//! that starts with an at-keyword is an `AtRule`. Preludes and values are lists of tokens and
//! `Group`s (`()`, `[]`, `#{}`), as in the CSS Syntax spec, so dialect constructs need no rules of
//! their own. Every token is in the tree; trivia are placed by the shared attachment rule (see
//! `core_syntax::trivia`).

use crate::Dialect;
use crate::generated::kind as k;
use crate::lexer::{Token, lex};
use core_syntax::{SyntaxKind, SyntaxNode, TreeBuilder, trivia};
use provider_api::ParseError;
use std::ops::Range;

/// Deeper nesting of blocks or groups is an error: the rest of the file is one `ERROR` node. This
/// bounds recursion in the parser, the formatter and tree destruction.
pub const MAX_DEPTH: usize = 256;

pub struct Parsed {
    pub tree: SyntaxNode,
    pub errors: Vec<ParseError>,
}

pub fn parse(src: &str, dialect: Dialect) -> Parsed {
    let (tokens, lex_errors) = lex(src, dialect);
    let mut p = Parser::new(src, tokens);
    p.errors.extend(lex_errors.into_iter().map(|e| ParseError {
        start: e.start as u32,
        len: e.len as u32,
        code: e.code.into(),
        message: e.message.into(),
    }));
    p.stylesheet();
    p.errors.sort_by_key(|e| e.start);
    Parsed {
        tree: SyntaxNode::new_root(p.builder.finish()),
        errors: p.errors,
    }
}

/// How the statement that a scan started at ends.
#[derive(Clone, Copy, PartialEq)]
enum End {
    /// `{`: a block follows.
    Brace,
    /// `;`
    Semi,
    /// `}` (it belongs to the enclosing block) or the end of the file.
    Close,
    /// A closer that does not match, at the token before the end (not included).
    Unbalanced,
    /// An opener that is never closed, or nesting beyond [`MAX_DEPTH`]: the rest of the file.
    Unclosed,
}

struct Scan {
    /// Index (in `real`) of the token that ends the statement.
    end: usize,
    kind: End,
    /// The first `:` outside any group.
    colon: Option<usize>,
}

struct Parser<'a> {
    src: &'a str,
    tokens: Vec<Token>,
    /// Indices into `tokens` of the non-trivia tokens.
    real: Vec<usize>,
    leading: Vec<Range<usize>>,
    trailing: Vec<Range<usize>>,
    pos: usize,
    depth: usize,
    /// The first token of the parameters of `@include`, `@mixin` and `@function`: a name there is a
    /// call even when blank space separates it from its `(`.
    call_at: Option<usize>,
    /// The closers that the scan waits for, innermost last.
    closers: Vec<SyntaxKind>,
    builder: TreeBuilder,
    errors: Vec<ParseError>,
}

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHITESPACE | k::NEWLINE | k::COMMENT | k::LINE_COMMENT
    )
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, tokens: Vec<Token>) -> Self {
        let trivia::Attached {
            real,
            leading,
            trailing,
        } = trivia::attach(
            tokens.len(),
            |i| is_trivia(tokens[i].kind),
            |i| &src[tokens[i].start..tokens[i].end],
        );
        Parser {
            src,
            tokens,
            real,
            leading,
            trailing,
            pos: 0,
            depth: 0,
            call_at: None,
            closers: Vec::new(),
            builder: TreeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn kind_at(&self, i: usize) -> SyntaxKind {
        self.tokens[self.real[i]].kind
    }

    fn cur(&self) -> SyntaxKind {
        self.kind_at(self.pos)
    }

    fn error_at(&mut self, i: usize, code: &str, message: &str) {
        let t = &self.tokens[self.real[i]];
        let (start, end) = (t.start as u32, t.end as u32);
        // One report per place: a lexer error on the same token already said it.
        let end = end.max(start + 1);
        if self
            .errors
            .iter()
            .any(|e| e.start < end && start < e.start + e.len.max(1))
        {
            return;
        }
        self.errors.push(ParseError {
            start,
            len: end - start,
            code: code.into(),
            message: message.into(),
        });
    }

    /// Consumes the current token with its leading and trailing trivia.
    fn bump(&mut self) {
        let (lead, trail) = (
            self.leading[self.pos].clone(),
            self.trailing[self.pos].clone(),
        );
        for i in lead.chain([self.real[self.pos]]).chain(trail) {
            let t = self.tokens[i];
            self.builder.token(t.kind, &self.src[t.start..t.end]);
        }
        if self.cur() != k::EOF {
            self.pos += 1;
        }
    }

    /// Consumes tokens up to (not including) real token `end` into an `ERROR` node.
    fn error_until(&mut self, end: usize) {
        self.builder.start_node(SyntaxKind::ERROR);
        while self.pos < end {
            self.bump();
        }
        self.builder.finish_node();
    }

    fn stylesheet(&mut self) {
        self.builder.start_node(k::STYLESHEET);
        self.statements();
        // Only a `}` that closes nothing stops the loop before the end.
        while self.cur() != k::EOF {
            self.error_at(self.pos, "unexpected-brace", "unexpected `}`");
            self.error_until(self.pos + 1);
            self.statements();
        }
        self.bump(); // Eof
        self.builder.finish_node();
    }

    /// Statements up to a `}` or the end of the file.
    fn statements(&mut self) {
        loop {
            match self.cur() {
                k::R_BRACE | k::EOF => return,
                k::SEMICOLON if self.has_comment() => {
                    self.builder.start_node(k::EMPTY);
                    self.bump();
                    self.builder.finish_node();
                }
                k::SEMICOLON => self.bump(),
                _ => self.statement(),
            }
        }
    }

    /// Does the current token own a comment?
    fn has_comment(&self) -> bool {
        let (lead, trail) = (
            self.leading[self.pos].clone(),
            self.trailing[self.pos].clone(),
        );
        lead.chain(trail)
            .any(|i| matches!(self.tokens[i].kind, k::COMMENT | k::LINE_COMMENT))
    }

    fn statement(&mut self) {
        let start = self.pos;
        let scan = self.scan();
        match scan.kind {
            End::Unclosed => {
                let message = if self.closers.len() >= MAX_DEPTH {
                    "nesting is too deep"
                } else {
                    "unclosed bracket"
                };
                self.error_at(self.pos, "unclosed", message);
                self.error_until(scan.end);
            }
            End::Unbalanced => {
                self.error_at(scan.end, "unbalanced-bracket", "unmatched closing bracket");
                // A stray `)` or `]` is part of the error; a `}` ends the enclosing block.
                let stray = self.kind_at(scan.end) != k::R_BRACE;
                self.error_until(scan.end + usize::from(stray));
            }
            _ if self.cur() == k::AT_KEYWORD => self.at_rule(scan),
            End::Brace => {
                self.builder.start_node(k::RULE);
                self.list(k::PRELUDE, scan.end);
                self.finish_statement(End::Brace);
            }
            _ => match scan.colon {
                Some(colon) => {
                    self.builder.start_node(k::DECLARATION);
                    self.list(k::PROPERTY, colon);
                    self.bump(); // :
                    if self.braced_value(start, scan.end) {
                        self.builder.start_node(k::VALUE);
                        self.block();
                        self.builder.finish_node();
                    } else {
                        self.list(k::VALUE, scan.end);
                    }
                    self.finish_statement(scan.kind);
                }
                None => {
                    self.builder.start_node(k::STATEMENT);
                    self.list(k::VALUE, scan.end);
                    self.finish_statement(scan.kind);
                }
            },
        }
        self.closers.clear();
    }

    /// An at-rule; a `@else` right after its block (`@if` chains) is nested in it, so that the chain
    /// is one node. Each link counts as a level of nesting.
    fn at_rule(&mut self, mut scan: Scan) {
        let mut links = 0;
        loop {
            self.builder.start_node(k::AT_RULE);
            let name = &self.tokens[self.real[self.pos]];
            // shortcut: allocates a String per at-rule (about +0.6% of css parse allocations);
            // compare with `eq_ignore_ascii_case` instead.
            let name = self.src[name.start..name.end].to_ascii_lowercase();
            self.bump();
            if matches!(name.as_str(), "@include" | "@mixin" | "@function") {
                self.call_at = Some(self.pos);
            }
            self.list(k::PRELUDE, scan.end);
            self.call_at = None;
            match scan.kind {
                End::Brace => self.block(),
                End::Semi => self.bump(),
                _ => {}
            }
            if scan.kind != End::Brace || !self.else_follows() || self.depth + links >= MAX_DEPTH {
                break;
            }
            self.closers.clear();
            scan = self.scan();
            if !matches!(scan.kind, End::Brace | End::Semi | End::Close) {
                break;
            }
            links += 1;
        }
        for _ in 0..=links {
            self.builder.finish_node();
        }
    }

    /// Is the current token an `@else` with no comment between it and the `}` before it?
    fn else_follows(&self) -> bool {
        let comment = |r: &Range<usize>| {
            r.clone()
                .any(|i| matches!(self.tokens[i].kind, k::COMMENT | k::LINE_COMMENT))
        };
        let t = &self.tokens[self.real[self.pos]];
        t.kind == k::AT_KEYWORD
            && self.src[t.start..t.end].eq_ignore_ascii_case("@else")
            && !comment(&self.leading[self.pos])
            && !comment(&self.trailing[self.pos - 1])
    }

    /// Is the value of the custom property that starts at real token `start` one `{ }` block,
    /// as in `--x: { color: red }` (it is a block of declarations)?
    fn braced_value(&self, start: usize, end: usize) -> bool {
        let t = self.tokens[self.real[start]];
        if t.kind != k::IDENT
            || !self.src[t.start..t.end].starts_with("--")
            || self.cur() != k::L_BRACE
        {
            return false;
        }
        let mut depth = 0;
        for i in self.pos..end {
            match self.kind_at(i) {
                k::L_BRACE | k::INTERP_OPEN => depth += 1,
                k::R_BRACE => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1 == end;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// The end of a statement whose node is open: its block or `;`.
    fn finish_statement(&mut self, end: End) {
        match end {
            End::Brace => self.block(),
            End::Semi => self.bump(),
            _ => {}
        }
        self.builder.finish_node();
    }

    fn block(&mut self) {
        if self.depth >= MAX_DEPTH {
            self.error_at(self.pos, "nesting-too-deep", "nesting is too deep");
            let eof = self.real.len() - 1;
            return self.error_until(eof);
        }
        self.depth += 1;
        self.builder.start_node(k::BLOCK);
        self.bump(); // {
        self.statements();
        if self.cur() == k::R_BRACE {
            self.bump();
        } else {
            self.error_at(
                self.pos,
                "unclosed-block",
                "expected `}` before the end of the file",
            );
        }
        self.builder.finish_node();
        self.depth -= 1;
    }

    /// A node of component values up to real token `end`; nothing if there are none.
    fn list(&mut self, kind: SyntaxKind, end: usize) {
        if self.pos == end {
            return;
        }
        self.builder.start_node(kind);
        while self.pos < end {
            self.component();
        }
        self.builder.finish_node();
    }

    /// One token, or a group with what it holds. The scan found every group closed.
    fn component(&mut self) {
        match self.cur() {
            k::L_PAREN | k::L_BRACKET | k::L_BRACE | k::INTERP_OPEN => self.group(),
            k::IDENT
                if self.kind_at(self.pos + 1) == k::L_PAREN
                    && (self.adjacent() || self.call_at == Some(self.pos)) =>
            {
                self.builder.start_node(k::FUNCTION);
                self.bump();
                self.group();
                self.builder.finish_node();
            }
            _ => self.bump(),
        }
    }

    /// Whether the next token follows the current one with nothing in between.
    fn adjacent(&self) -> bool {
        self.tokens[self.real[self.pos]].end == self.tokens[self.real[self.pos + 1]].start
    }

    fn group(&mut self) {
        let closer = closer_of(self.cur()).expect("at an opener");
        self.builder.start_node(k::GROUP);
        self.bump();
        while self.cur() != closer && self.cur() != k::EOF {
            self.component();
        }
        self.bump();
        self.builder.finish_node();
    }

    /// Finds where the statement that starts at the current token ends, and checks that its
    /// brackets are balanced. A custom property (`--x: {...}`) may hold braces in its value.
    fn scan(&mut self) -> Scan {
        let start = self.pos;
        let mut colon = None;
        let mut custom = false;
        let mut i = start;
        let done = |end, kind, colon| Scan { end, kind, colon };
        loop {
            let kind = self.kind_at(i);
            if let Some(&top) = self.closers.last() {
                if kind == top {
                    self.closers.pop();
                } else if let Some(c) = closer_of(kind) {
                    if self.closers.len() >= MAX_DEPTH {
                        return done(self.real.len() - 1, End::Unclosed, colon);
                    }
                    self.closers.push(c);
                } else if matches!(kind, k::R_PAREN | k::R_BRACKET | k::R_BRACE) {
                    return done(i, End::Unbalanced, colon);
                } else if kind == k::EOF {
                    return done(i, End::Unclosed, colon);
                }
            } else {
                match kind {
                    k::EOF | k::R_BRACE => return done(i, End::Close, colon),
                    k::SEMICOLON => return done(i, End::Semi, colon),
                    k::R_PAREN | k::R_BRACKET => return done(i, End::Unbalanced, colon),
                    k::L_BRACE if !custom => return done(i, End::Brace, colon),
                    k::COLON if colon.is_none() => {
                        colon = Some(i);
                        let t = self.tokens[self.real[start]];
                        custom = i == start + 1
                            && t.kind == k::IDENT
                            && self.src[t.start..t.end].starts_with("--");
                    }
                    _ => {
                        if let Some(c) = closer_of(kind) {
                            self.closers.push(c);
                        }
                    }
                }
            }
            i += 1;
        }
    }
}

fn closer_of(open: SyntaxKind) -> Option<SyntaxKind> {
    match open {
        k::L_PAREN => Some(k::R_PAREN),
        k::L_BRACKET => Some(k::R_BRACKET),
        k::L_BRACE | k::INTERP_OPEN => Some(k::R_BRACE),
        _ => None,
    }
}
