//! ruff's AST and tokens as our lossless tree.
//!
//! ruff's AST has no punctuation nodes, no parenthesis information and no trivia. The adapter
//! takes the lexer's tokens (every token of the file, with its span), the AST nodes' spans (one
//! `SourceOrderVisitor` walk), and the text between two tokens, which holds only whitespace,
//! comments and line continuations and is split into trivia tokens here, the way
//! `core-treesitter` does. A node opens before the first token that starts in it and closes after
//! its last, so a token belongs to the innermost node that contains it; trivia follow the shared
//! attachment rule (`core_syntax::trivia`).
//!
//! - Logical and non-logical newlines, indents and dedents are not tokens of the tree: the bytes
//!   they cover are `Newline` and `Whitespace` trivia, comments are `Comment` trivia.
//! - ruff's expression ranges exclude parentheses: a `Parenthesized` node is made around each
//!   parenthesized expression or pattern, one per pair, nested for `((x))`. The parentheses of a
//!   call or a class's bases belong to their `Arguments` node and make none.
//! - A node with no token (`lambda:` has an empty `Parameters`, error recovery adds names of no
//!   length) is an empty node, so a kind is present wherever ruff's AST has it.
//! - ruff's errors cascade: a file reports the first error of each statement.

use crate::generated::kind as k;
use core_syntax::trivia::is_line_break;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TreeBuilder, grammars};
use provider_api::{ParseError, ParseResult};
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{AnyNodeRef, NodeKind, PythonVersion};
use ruff_python_parser::{Mode, ParseOptions, parse_unchecked};
use ruff_text_size::Ranged;

pub fn parse(src: &str, target: PythonVersion) -> ParseResult {
    let parsed = parse_unchecked(
        src,
        ParseOptions::from(Mode::Module).with_target_version(target),
    );
    let mut names = Names::new(grammars::PYTHON.get());
    let tokens: Vec<Tok> = (parsed.tokens().iter())
        .filter(|t| {
            t.start() != t.end()
                && !matches!(
                    t.kind(),
                    TokenKind::Comment
                        | TokenKind::Newline
                        | TokenKind::NonLogicalNewline
                        | TokenKind::Indent
                        | TokenKind::Dedent
                        | TokenKind::EndOfFile
                )
        })
        .map(|t| Tok {
            kind: names.token(t.kind()),
            start: t.start().to_u32(),
            end: t.end().to_u32(),
        })
        .collect();
    let mut collector = Collector {
        names,
        tokens: &tokens,
        nodes: Vec::with_capacity(tokens.len() / 2),
        open: Vec::new(),
        stmts: Vec::new(),
    };
    collector.visit_mod(parsed.syntax());
    let (nodes, stmts) = (collector.nodes, collector.stmts);
    let mut errors = first_per_statement(
        (parsed.errors().iter())
            .map(|e| (e.range(), e.error.to_string()))
            .chain(
                (parsed.unsupported_syntax_errors().iter())
                    .map(|e| (e.range(), e.to_string())),
            ),
        &stmts,
    );
    let mut build = Build {
        src,
        builder: TreeBuilder::new(),
        errors: Vec::new(),
    };
    build.run(&tokens, &nodes);
    errors.append(&mut build.errors);
    errors.sort_by_key(|e| e.start);
    ParseResult {
        tree: SyntaxNode::new_root(build.builder.finish()),
        errors,
    }
}

/// The errors of the parse, one per statement (the innermost one that contains it): the first
/// error of a statement makes the rest of its line noise.
fn first_per_statement(
    errors: impl Iterator<Item = (ruff_text_size::TextRange, String)>,
    stmts: &[(u32, u32)],
) -> Vec<ParseError> {
    let mut all: Vec<_> = errors.collect();
    all.sort_by_key(|(r, _)| r.start());
    let (mut open, mut next): (Vec<(u32, u32)>, usize) = (Vec::new(), 0);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (range, message) in all {
        let at = range.start().to_u32();
        while let Some(&s) = stmts.get(next).filter(|s| s.0 <= at) {
            next += 1;
            while open.last().is_some_and(|o| o.1 <= s.0) {
                open.pop();
            }
            open.push(s);
        }
        while open.last().is_some_and(|o| o.1 <= at) {
            open.pop();
        }
        if open.last().is_none_or(|o| seen.insert(o.0)) {
            out.push(ParseError {
                start: at,
                len: range.len().to_u32(),
                code: "syntax-error".into(),
                message,
            });
        }
    }
    out
}

#[derive(Clone, Copy)]
struct Tok {
    kind: SyntaxKind,
    start: u32,
    end: u32,
}

/// Our kinds for ruff's, found by name on first use.
struct Names {
    grammar: &'static Grammar,
    tokens: [Option<SyntaxKind>; 128],
    nodes: [Option<Option<SyntaxKind>>; 128],
}

impl Names {
    fn new(grammar: &'static Grammar) -> Self {
        Names {
            grammar,
            tokens: [None; 128],
            nodes: [None; 128],
        }
    }

    fn token(&mut self, kind: TokenKind) -> SyntaxKind {
        let grammar = self.grammar;
        *self.tokens[kind as usize].get_or_insert_with(|| {
            let found = grammar.kind_by_name(&format!("{kind:?}"));
            debug_assert!(found.is_some(), "{kind:?} has no token in grammars/python.grammar");
            found.filter(|k| k.is_token()).unwrap_or(k::UNKNOWN)
        })
    }

    /// `None` for a kind with no node (IPython escape commands, the root).
    fn node(&mut self, kind: NodeKind) -> Option<SyntaxKind> {
        let grammar = self.grammar;
        *self.nodes[kind as usize].get_or_insert_with(|| {
            let name = format!("{kind:?}");
            let found = grammar.kind_by_name(&name).filter(|k| !k.is_token());
            debug_assert!(
                found.is_some() || name.contains("Ipy") || name.starts_with("Mod"),
                "{name} has no node in grammars/python.grammar"
            );
            found.filter(|_| name != "ModModule")
        })
    }
}

/// An AST node: its span and our kind.
struct Node {
    start: u32,
    end: u32,
    kind: SyntaxKind,
}

/// The nodes of the AST in pre-order, and the spans of its statements.
struct Collector<'t> {
    names: Names,
    tokens: &'t [Tok],
    nodes: Vec<Node>,
    /// The enclosing AST nodes: whether each owns the parentheses at the ends of its range
    /// (`Arguments`), and its end.
    open: Vec<(bool, u32)>,
    stmts: Vec<(u32, u32)>,
}

impl Collector<'_> {
    /// The ranges of the parentheses around the expression or pattern `start..end`, outermost
    /// last. The closing parenthesis that ends a parent that owns its parentheses is not one.
    fn parens(&self, start: u32, end: u32, limit: Option<u32>) -> Vec<(u32, u32)> {
        let toks = self.tokens;
        let first = toks.partition_point(|t| t.start < start);
        let last = toks.partition_point(|t| t.end <= end);
        let left = toks[..first]
            .iter()
            .rev()
            .take_while(|t| t.kind == k::LPAR);
        let right = toks[last..]
            .iter()
            .take_while(|t| t.kind == k::RPAR && limit.is_none_or(|l| t.end < l));
        left.zip(right).map(|(l, r)| (l.start, r.end)).collect()
    }
}

impl<'a> SourceOrderVisitor<'a> for Collector<'_> {
    fn enter_node(&mut self, node: AnyNodeRef<'a>) -> TraversalSignal {
        let kind = node.kind();
        let range = node.range();
        let (start, end) = (range.start().to_u32(), range.end().to_u32());
        let name = format!("{kind:?}");
        let owns = matches!(kind, NodeKind::Arguments | NodeKind::PatternArguments);
        let parent_limit = self.open.last().filter(|p| p.0).map(|p| p.1);
        self.open.push((owns, end));
        let Some(ours) = self.names.node(kind) else {
            return TraversalSignal::Traverse;
        };
        if name.starts_with("Stmt") {
            self.stmts.push((start, end));
        }
        if start < end && (name.starts_with("Expr") || name.starts_with("PatternMatch")) {
            for (s, e) in self.parens(start, end, parent_limit).into_iter().rev() {
                self.nodes.push(Node {
                    start: s,
                    end: e,
                    kind: k::PARENTHESIZED,
                });
            }
        }
        self.nodes.push(Node {
            start,
            end,
            kind: ours,
        });
        TraversalSignal::Traverse
    }

    fn leave_node(&mut self, _: AnyNodeRef<'a>) {
        self.open.pop();
    }
}

struct Build<'a> {
    src: &'a str,
    builder: TreeBuilder,
    errors: Vec<ParseError>,
}

impl Build<'_> {
    fn emit(&mut self, t: Tok) {
        self.builder
            .token(t.kind, &self.src[t.start as usize..t.end as usize]);
    }

    /// Replays tokens and nodes: each token takes its leading trivia, itself and its trailing
    /// trivia, inside the innermost node that contains it.
    fn run(&mut self, tokens: &[Tok], nodes: &[Node]) {
        let len = self.src.len() as u32;
        self.builder.start_node(k::MOD_MODULE);
        // The zero-width end of file token closes every node and owns the trivia of the tail.
        let eof = Tok {
            kind: k::END_OF_FILE,
            start: len,
            end: len,
        };
        let mut nodes = nodes.iter().peekable();
        // The end of each open node, innermost last.
        let mut open: Vec<u32> = Vec::new();
        let mut carry: Vec<Tok> = Vec::new();
        let mut gap: Vec<Tok> = Vec::new();
        let first = tokens.first().map_or(len, |t| t.start);
        self.gap(0, first, &mut carry);
        let mut real = tokens.iter().copied().chain(Some(eof)).peekable();
        while let Some(t) = real.next() {
            while open.last().is_some_and(|&end| end <= t.start) {
                open.pop();
                self.builder.finish_node();
            }
            while let Some(n) = nodes.next_if(|n| n.start <= t.start) {
                if n.start == n.end {
                    self.builder.start_node(n.kind);
                    self.builder.finish_node();
                } else if n.end > t.start {
                    // A node that overlaps its parent ends with it.
                    let end = open.last().map_or(n.end, |&p| p.min(n.end));
                    self.builder.start_node(n.kind);
                    open.push(end);
                }
            }
            for c in carry.drain(..) {
                self.emit(c);
            }
            self.emit(t);
            if let Some(next) = real.peek() {
                gap.clear();
                self.gap(t.end, next.start, &mut gap);
                let ends_line = |c: &Tok| {
                    self.src[..c.end as usize]
                        .chars()
                        .next_back()
                        .is_some_and(is_line_break)
                };
                let cut = gap.iter().position(ends_line).map_or(gap.len(), |i| i + 1);
                for c in gap[..cut].iter().copied() {
                    self.emit(c);
                }
                carry.extend_from_slice(&gap[cut..]);
            }
        }
        self.builder.finish_node();
    }

    /// Splits the text `a..b` between two tokens into trivia tokens.
    fn gap(&mut self, a: u32, b: u32, out: &mut Vec<Tok>) {
        let src = self.src;
        let (mut pos, b) = (a as usize, b as usize);
        while pos < b {
            let rest = &src[pos..b];
            let c = rest.chars().next().unwrap_or(' ');
            let blank = |c: char| matches!(c, ' ' | '\t' | '\x0c' | '\u{feff}');
            let (kind, len) = if c == '#' {
                (k::COMMENT, rest.find(['\n', '\r']).unwrap_or(rest.len()))
            } else if rest.starts_with("\r\n") {
                (k::NEWLINE, 2)
            } else if c == '\n' || c == '\r' {
                (k::NEWLINE, 1)
            } else if let Some(after) = rest.strip_prefix('\\')
                && after.starts_with(['\n', '\r'])
            {
                (k::CONTINUATION, if after.starts_with("\r\n") { 3 } else { 2 })
            } else if blank(c) {
                (k::WHITESPACE, rest.find(|c| !blank(c)).unwrap_or(rest.len()))
            } else {
                let n = rest
                    .find(|c: char| blank(c) || c == '\n' || c == '\r')
                    .unwrap_or(rest.len());
                self.errors.push(ParseError {
                    start: pos as u32,
                    len: n as u32,
                    code: "syntax-error".into(),
                    message: "unexpected text".into(),
                });
                (k::UNKNOWN, n)
            };
            out.push(Tok {
                kind,
                start: pos as u32,
                end: (pos + len) as u32,
            });
            pos += len;
        }
    }
}
