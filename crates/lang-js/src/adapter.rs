//! oxc's AST and tokens as our lossless tree.
//!
//! oxc has no punctuation nodes and no trivia. The adapter takes the lexer's tokens (every token
//! of the file, with its span), the AST nodes' spans (one `Visit` walk over every `AstKind`),
//! and the text between two tokens, which holds only whitespace and comments and is split into
//! trivia tokens here, the way `core-treesitter` does. A node opens before the first token that
//! starts in it and closes after its last, so a token belongs to the innermost node that
//! contains it; trivia follow the shared attachment rule (`core_syntax::trivia`). Nodes with no
//! token (`JSXEmptyExpression`) and kinds without a grammar node (`Program`, which is the root
//! node of the tree, and `Hashbang`, a comment) are left out.

use crate::generated::kind as k;
use core_syntax::trivia::is_line_break;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TreeBuilder, grammars};
use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_ast::AstKind;
use oxc_ast::ast::{JSXChild, JSXExpression, TemplateElement};
use oxc_ast_visit::Visit;
use oxc_parser::config::TokensParserConfig;
use oxc_parser::{Kind, ParseOptions, Parser};
use oxc_span::{GetSpan, SourceType};
use provider_api::{ParseError, ParseResult};

/// The source type of a file name; JavaScript has JSX whatever its extension (as in Babel), and
/// text of no known extension (a Markdown fence) is JavaScript.
pub fn source_type(path: &str) -> SourceType {
    let st = SourceType::from_path(path).unwrap_or_else(|_| SourceType::jsx());
    if st.is_javascript() {
        st.with_jsx(true)
    } else {
        st
    }
}

pub fn parse(src: &str, source_type: SourceType) -> ParseResult {
    let allocator = Allocator::default();
    let options = ParseOptions {
        allow_return_outside_function: true,
        ..ParseOptions::default()
    };
    let parsed = Parser::new(&allocator, src, source_type)
        .with_options(options)
        .with_config(TokensParserConfig)
        .parse();
    let mut errors: Vec<ParseError> = parsed
        .diagnostics
        .errors()
        .map(|d| {
            let (start, len) = d.labels.first().map_or((0, 0), |l| (l.offset(), l.len()));
            ParseError {
                start,
                len,
                code: "syntax-error".into(),
                message: d.message.to_string(),
            }
        })
        .collect();
    errors.sort_by_key(|e| e.start);
    // A fatal error leaves nothing to take tokens and nodes from: the file is one unknown token.
    let (tokens, nodes) = if parsed.fatal_error {
        let whole = Tok {
            kind: k::UNKNOWN,
            start: 0,
            end: src.len() as u32,
        };
        (vec![whole], Vec::new())
    } else {
        let mut collector = Collector::new(grammars::JS.get());
        collector.comment_starts = parsed
            .program
            .comments
            .iter()
            .map(|c| c.span.start)
            .collect();
        collector.visit_program(&parsed.program);
        let tokens = (parsed.tokens.iter())
            .filter(|t| t.end() > t.start())
            .map(|t| Tok {
                kind: token_kind(t.kind()),
                start: t.start(),
                end: t.end(),
            })
            .collect();
        (tokens, collector.nodes)
    };
    let mut build = Build::new(src, tokens, nodes);
    build.run();
    errors.append(&mut build.errors);
    errors.sort_by_key(|e| e.start);
    ParseResult {
        tree: SyntaxNode::new_root(build.builder.finish()),
        errors,
    }
}

#[derive(Clone, Copy)]
struct Tok {
    kind: SyntaxKind,
    start: u32,
    end: u32,
}

/// Our token kind for oxc's.
fn token_kind(kind: Kind) -> SyntaxKind {
    match kind {
        Kind::LParen => k::L_PAREN,
        Kind::RParen => k::R_PAREN,
        Kind::LCurly => k::L_BRACE,
        Kind::RCurly => k::R_BRACE,
        Kind::LBrack => k::L_BRACKET,
        Kind::RBrack => k::R_BRACKET,
        Kind::LAngle => k::L_ANGLE,
        Kind::RAngle => k::R_ANGLE,
        Kind::Comma => k::COMMA,
        Kind::Semicolon => k::SEMICOLON,
        Kind::Colon => k::COLON,
        Kind::Dot => k::DOT,
        Kind::Dot3 => k::DOT3,
        Kind::Arrow => k::ARROW,
        Kind::Question => k::QUESTION,
        Kind::QuestionDot => k::QUESTION_DOT,
        Kind::At => k::AT,
        Kind::Pipe => k::PIPE,
        Kind::Ident => k::IDENTIFIER,
        Kind::Str => k::STRING,
        Kind::RegExp => k::REG_EXP,
        Kind::PrivateIdentifier => k::PRIVATE_NAME,
        Kind::JSXText => k::JSX_TEXT_RUN,
        Kind::NoSubstitutionTemplate
        | Kind::TemplateHead
        | Kind::TemplateMiddle
        | Kind::TemplateTail => k::TEMPLATE_CHUNK,
        Kind::Decimal
        | Kind::Float
        | Kind::Binary
        | Kind::Octal
        | Kind::Hex
        | Kind::PositiveExponential
        | Kind::NegativeExponential => k::NUMBER,
        Kind::DecimalBigInt | Kind::BinaryBigInt | Kind::OctalBigInt | Kind::HexBigInt => {
            k::BIG_INT
        }
        kind if kind.is_reserved_keyword() => k::KEYWORD,
        kind if kind.is_any_keyword() => k::IDENTIFIER,
        _ => k::OPERATOR,
    }
}

/// An AST node: its span and our kind.
struct Node {
    start: u32,
    end: u32,
    kind: SyntaxKind,
}

/// The nodes of the AST in pre-order.
struct Collector {
    grammar: &'static Grammar,
    /// Our node kind by `AstType`, resolved by name on first use; `None` is a kind with no node.
    kinds: Vec<Option<Option<SyntaxKind>>>,
    nodes: Vec<Node>,
    /// Whether each open AST node made a node.
    made: Vec<bool>,
    /// Starts of the `{" "}` children of elements and of their strings (see `enter_node`).
    jsx_space: std::collections::HashSet<u32>,
    /// Where the comments start: a `{" "}` with a comment inside is an expression like any other.
    comment_starts: Vec<u32>,
}

impl Collector {
    fn new(grammar: &'static Grammar) -> Self {
        Collector {
            grammar,
            kinds: vec![None; usize::from(oxc_ast::ast_kind::AST_TYPE_MAX) + 1],
            nodes: Vec::new(),
            made: Vec::new(),
            jsx_space: std::collections::HashSet::new(),
            comment_starts: Vec::new(),
        }
    }

    fn note_jsx_space(&mut self, children: &[JSXChild<'_>]) {
        for child in children {
            if let JSXChild::ExpressionContainer(c) = child
                && let JSXExpression::StringLiteral(s) = &c.expression
                && s.value == " "
                && !self
                    .comment_starts
                    .iter()
                    .any(|&p| p > c.span.start && p < c.span.end)
            {
                self.jsx_space.extend([c.span.start, s.span.start]);
            }
        }
    }
}

impl<'a> Visit<'a> for Collector {
    /// The quasis of a template lie inside its chunk tokens and never become nodes; visited
    /// before the expressions they would hold back the nodes of those (which are in source order).
    fn visit_template_elements(&mut self, _: &ArenaVec<'a, TemplateElement<'a>>) {}

    fn enter_node(&mut self, kind: AstKind<'a>) {
        let ty = kind.ty();
        let grammar = self.grammar;
        let ours = *self.kinds[ty as usize].get_or_insert_with(|| {
            let name = format!("{ty:?}");
            // `Program` is the root and `Hashbang` a comment; a new oxc kind has no row yet.
            let ours = grammar
                .kind_by_name(&name)
                .filter(|k| !k.is_token() && name != "Program");
            debug_assert!(
                ours.is_some() || matches!(name.as_str(), "Program" | "Hashbang"),
                "{name} has no node in grammars/js.grammar"
            );
            ours
        });
        let span = kind.span();
        // A `{" "}` child of an element is JSX whitespace, which the formatter moves between
        // text and `{" "}` like text: it is a `JSXText` node (transparent to the safety check)
        // with no node for its string.
        match kind {
            AstKind::JSXElement(e) => self.note_jsx_space(&e.children),
            AstKind::JSXFragment(f) => self.note_jsx_space(&f.children),
            _ => {}
        }
        let ours = match kind {
            AstKind::JSXExpressionContainer(_) if self.jsx_space.contains(&span.start) => {
                Some(k::JSX_TEXT)
            }
            AstKind::StringLiteral(_) if self.jsx_space.contains(&span.start) => None,
            _ => ours,
        };
        let make = ours.filter(|_| span.end > span.start);
        if let Some(kind) = make {
            self.nodes.push(Node {
                start: span.start,
                end: span.end,
                kind,
            });
        }
        self.made.push(make.is_some());
    }

    fn leave_node(&mut self, _kind: AstKind<'a>) {
        self.made.pop();
    }
}

struct Build<'a> {
    src: &'a str,
    tokens: Vec<Tok>,
    nodes: Vec<Node>,
    builder: TreeBuilder,
    errors: Vec<ParseError>,
}

impl<'a> Build<'a> {
    fn new(src: &'a str, tokens: Vec<Tok>, nodes: Vec<Node>) -> Self {
        Build {
            src,
            tokens,
            nodes,
            builder: TreeBuilder::new(),
            errors: Vec::new(),
        }
    }

    fn emit(&mut self, t: Tok) {
        self.builder
            .token(t.kind, &self.src[t.start as usize..t.end as usize]);
    }

    /// Replays tokens and nodes: each token takes its leading trivia, itself and its trailing
    /// trivia, inside the innermost node that contains it.
    fn run(&mut self) {
        let len = self.src.len() as u32;
        self.builder.start_node(k::PROGRAM);
        // The zero-width end of file token closes every node and owns the trivia of the tail.
        let eof = Tok {
            kind: k::EOF,
            start: len,
            end: len,
        };
        let tokens = std::mem::take(&mut self.tokens);
        let nodes = std::mem::take(&mut self.nodes);
        let mut nodes = nodes.iter().peekable();
        // The end of each open node, innermost last.
        let mut open: Vec<u32> = Vec::new();
        let mut open_kinds: Vec<SyntaxKind> = Vec::new();
        let mut carry: Vec<Tok> = Vec::new();
        let mut gap: Vec<Tok> = Vec::new();
        let first = tokens.first().map_or(len, |t| t.start);
        self.gap(0, first, &mut carry);
        let mut real = tokens.iter().copied().chain(Some(eof)).peekable();
        while let Some(t) = real.next() {
            while open.last().is_some_and(|&end| end <= t.start) {
                open.pop();
                open_kinds.pop();
                self.builder.finish_node();
            }
            while let Some(n) = nodes.next_if(|n| n.start <= t.start) {
                if n.end <= t.start {
                    continue;
                }
                // A node that overlaps its parent ends with it.
                let end = open.last().map_or(n.end, |&p| p.min(n.end));
                self.builder.start_node(n.kind);
                open.push(end);
                open_kinds.push(n.kind);
            }
            for c in carry.drain(..) {
                self.emit(c);
            }
            // The bars of a union type are separators, a bitwise `|` an operator.
            let t = match t.kind {
                k::PIPE if open_kinds.last() != Some(&k::TS_UNION_TYPE) => Tok {
                    kind: k::OPERATOR,
                    ..t
                },
                _ => t,
            };
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
            let line_end = || rest.find(is_js_line_break).unwrap_or(rest.len());
            let (kind, len) = if rest.starts_with("//")
                || rest.starts_with("<!--")
                || rest.starts_with("-->")
                || pos == 0 && rest.starts_with("#!")
            {
                (k::LINE_COMMENT, line_end())
            } else if let Some(body) = rest.strip_prefix("/*") {
                (
                    k::BLOCK_COMMENT,
                    body.find("*/").map_or(rest.len(), |i| i + 4),
                )
            } else if rest.starts_with("\r\n") {
                (k::NEWLINE, 2)
            } else if is_js_line_break(c) {
                (k::NEWLINE, c.len_utf8())
            } else if c.is_whitespace() || c == '\u{feff}' {
                let n = rest
                    .find(|c: char| !(c.is_whitespace() || c == '\u{feff}') || is_js_line_break(c))
                    .unwrap_or(rest.len());
                (k::WHITESPACE, n)
            } else {
                let n = rest.find(char::is_whitespace).unwrap_or(rest.len());
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

fn is_js_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
