//! A language-neutral adapter from tree-sitter to our lossless tree.
//!
//! A language supplies a linked-in tree-sitter `Language` and a [`Mapping`]: tree-sitter node
//! name -> kind name of its `.grammar`. The adapter does the rest:
//!
//! * a mapped node kind becomes a node; a mapped token kind is a leaf, whatever its children
//!   (so a string with escape sequences is one token); an unmapped node with children is
//!   transparent; a mapped trivia kind (comments) is trivia;
//! * bytes between tokens are rebuilt as whitespace / newline tokens (anything else becomes an
//!   `Unknown` error token, so the tree stays lossless), then attached by the shared trivia rule;
//! * tree-sitter `ERROR` nodes become `SyntaxKind::ERROR` plus a parse error, `MISSING` nodes only
//!   a parse error (the formatter refuses files with errors);
//! * a zero-width `Eof` token ends the root node;
//! * a token listed in [`Mapping::wrap`] is put inside a node of its own, unless its tree-sitter
//!   parent already maps to that node (tree-sitter hides one-part keys and literals as bare
//!   tokens, while shapes such as lists and key-values want nodes).
//!
//! Parsing is not incremental and has no depth limit of its own (the walk is iterative; the
//! formatter recurses). See `docs/architecture/adding-a-language.md`.

mod provider;

pub use provider::{Spec, TreeSitterProvider};

use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TreeBuilder};
use provider_api::{ParseError, ParseResult};
use std::sync::{Mutex, OnceLock};
use tree_sitter::{Language, Node, Parser};

/// Grammar kind names the adapter itself emits, and the tree-sitter name -> kind name table.
pub struct Mapping {
    pub whitespace: &'static str,
    pub newline: &'static str,
    pub eof: &'static str,
    pub unknown: &'static str,
    /// Tree-sitter node name (anonymous tokens are named by their text) -> kind name.
    pub kinds: &'static [(&'static str, &'static str)],
    /// Token kind name -> node kind name wrapping it.
    pub wrap: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Copy)]
enum Entry {
    Node(SyntaxKind),
    Token(SyntaxKind, Option<SyntaxKind>),
    Trivia(SyntaxKind),
    Transparent,
}

/// The mapping resolved against one tree-sitter language: a table indexed by node kind id.
struct Converter {
    table: Vec<Entry>,
    names: Vec<String>,
    ws: SyntaxKind,
    nl: SyntaxKind,
    eof: SyntaxKind,
    unknown: SyntaxKind,
}

#[derive(Clone, Copy)]
struct Tok {
    kind: SyntaxKind,
    start: usize,
    end: usize,
}

enum Ev {
    Enter(SyntaxKind),
    Exit,
    /// The next token of the real-token list.
    Real,
}

impl Converter {
    fn new(language: &Language, grammar: &'static Grammar, m: &Mapping) -> Self {
        let kind = |n: &str| {
            grammar
                .kind_by_name(n)
                .unwrap_or_else(|| panic!("grammar has no `{n}`"))
        };
        let count = language.node_kind_count();
        let mut table = vec![Entry::Transparent; count];
        let mut names = vec![String::new(); count];
        for (id, slot) in table.iter_mut().enumerate() {
            let name = language.node_kind_for_id(id as u16).unwrap_or("");
            if let Some((_, target)) = m.kinds.iter().find(|(from, _)| *from == name) {
                let k = kind(target);
                *slot = match (k.is_token(), grammar.is_trivia(k)) {
                    (false, _) => Entry::Node(k),
                    (true, false) => Entry::Token(
                        k,
                        m.wrap
                            .iter()
                            .find(|(t, _)| t == target)
                            .map(|(_, w)| kind(w)),
                    ),
                    (true, true) => Entry::Trivia(k),
                };
            }
            names[id] = name.to_string();
        }
        Converter {
            table,
            names,
            ws: kind(m.whitespace),
            nl: kind(m.newline),
            eof: kind(m.eof),
            unknown: kind(m.unknown),
        }
    }

    fn convert(&self, src: &str, root: Node) -> ParseResult {
        let mut s = Scan::default();
        self.scan(root, &mut s);
        // The root's last event is its Exit: the Eof token goes just before it.
        s.real.push(Tok {
            kind: self.eof,
            start: src.len(),
            end: src.len(),
        });
        let exit = s.evs.pop();
        s.evs.push(Ev::Real);
        s.evs.extend(exit);
        let tree = self.build(src, &mut s);
        s.errors.sort_by_key(|e| e.start);
        ParseResult {
            tree,
            errors: s.errors,
        }
    }

    /// Depth-first walk collecting structure events, real tokens, comments and errors.
    fn scan(&self, root: Node, s: &mut Scan) {
        let mut cursor = root.walk();
        let mut pushed_stack: Vec<bool> = Vec::new();
        loop {
            let (pushed, descend) = self.enter(cursor.node(), s);
            if descend && cursor.goto_first_child() {
                pushed_stack.push(pushed);
                continue;
            }
            if pushed {
                s.evs.push(Ev::Exit);
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
                if pushed_stack.pop() == Some(true) {
                    s.evs.push(Ev::Exit);
                }
            }
        }
    }

    /// Returns (a node was opened, descend into the children).
    fn enter(&self, n: Node, s: &mut Scan) -> (bool, bool) {
        let (start, end) = (n.start_byte(), n.end_byte());
        let id = n.kind_id() as usize;
        let name = self.names.get(id).map_or("?", String::as_str);
        if n.is_missing() {
            s.error(start, 0, format!("missing {name}"));
            return (false, false);
        }
        if n.is_error() {
            s.error(start, end - start, "syntax error".into());
            s.evs.push(Ev::Enter(SyntaxKind::ERROR));
            if n.child_count() == 0 {
                s.token(Tok {
                    kind: self.unknown,
                    start,
                    end,
                });
            }
            return (true, true);
        }
        match self.table.get(id).copied().unwrap_or(Entry::Transparent) {
            Entry::Node(k) => {
                s.evs.push(Ev::Enter(k));
                (true, true)
            }
            Entry::Token(kind, wrap) => {
                if end > start {
                    let wrap = wrap.filter(|w| !self.parent_is(n, *w));
                    s.evs.extend(wrap.map(Ev::Enter));
                    s.token(Tok { kind, start, end });
                    s.evs.extend(wrap.map(|_| Ev::Exit));
                }
                (false, false)
            }
            Entry::Trivia(kind) => {
                s.comments.push(Tok { kind, start, end });
                (false, false)
            }
            Entry::Transparent if n.child_count() > 0 => (false, true),
            Entry::Transparent => {
                if end > start {
                    s.error(start, end - start, format!("unexpected {name}"));
                    s.token(Tok {
                        kind: self.unknown,
                        start,
                        end,
                    });
                }
                (false, false)
            }
        }
    }

    fn parent_is(&self, n: Node, kind: SyntaxKind) -> bool {
        n.parent().is_some_and(
            |p| matches!(self.table.get(p.kind_id() as usize), Some(Entry::Node(k)) if *k == kind),
        )
    }

    /// Splits the bytes `a..b` into trivia tokens (comments from tree-sitter, the rest lexed).
    fn gap(&self, src: &str, (a, b): (usize, usize), s: &mut Scan, out: &mut Vec<Tok>) {
        let bytes = src.as_bytes();
        let mut pos = a;
        while pos < b {
            while s
                .comments
                .get(s.ci)
                .is_some_and(|c| c.end <= pos && c.start < pos)
            {
                s.ci += 1;
            }
            if let Some(c) = s.comments.get(s.ci).filter(|c| c.start == pos) {
                out.push(*c);
                pos = c.end.min(b);
                s.ci += 1;
                continue;
            }
            let limit = s
                .comments
                .get(s.ci)
                .map_or(b, |c| c.start.clamp(pos + 1, b));
            let (kind, len) = match bytes[pos] {
                b'\n' => (self.nl, 1),
                b'\r' => (
                    self.nl,
                    if bytes.get(pos + 1) == Some(&b'\n') {
                        2
                    } else {
                        1
                    },
                ),
                b' ' | b'\t' | 0x0b | 0x0c => {
                    let n = bytes[pos..limit]
                        .iter()
                        .take_while(|c| matches!(c, b' ' | b'\t' | 0x0b | 0x0c))
                        .count();
                    (self.ws, n)
                }
                _ => {
                    let n = bytes[pos..limit]
                        .iter()
                        .take_while(|c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
                        .count();
                    s.error(pos, n, "unexpected text".into());
                    (self.unknown, n)
                }
            };
            out.push(Tok {
                kind,
                start: pos,
                end: pos + len,
            });
            pos += len;
        }
    }

    /// Replays the events; each real token takes its leading trivia, itself, its trailing trivia.
    fn build(&self, src: &str, s: &mut Scan) -> SyntaxNode {
        let mut b = TreeBuilder::new();
        let real = std::mem::take(&mut s.real);
        let ends_line = |t: &Tok| matches!(src.as_bytes()[t.end - 1], b'\n' | b'\r');
        let emit = |b: &mut TreeBuilder, t: &Tok| b.token(t.kind, &src[t.start..t.end]);
        let mut next = 0;
        let mut carry: Vec<Tok> = Vec::new();
        let mut gap: Vec<Tok> = Vec::new();
        // Everything before the first token is its leading trivia.
        self.gap(src, (0, real[0].start), s, &mut carry);
        for ev in std::mem::take(&mut s.evs) {
            match ev {
                Ev::Enter(k) => b.start_node(k),
                Ev::Exit => b.finish_node(),
                Ev::Real => {
                    let t = real[next];
                    carry.iter().for_each(|c| emit(&mut b, c));
                    emit(&mut b, &t);
                    carry.clear();
                    next += 1;
                    if let Some(after) = real.get(next) {
                        gap.clear();
                        self.gap(src, (t.end, after.start), s, &mut gap);
                        let cut = gap.iter().position(ends_line).map_or(gap.len(), |i| i + 1);
                        gap[..cut].iter().for_each(|c| emit(&mut b, c));
                        carry.extend_from_slice(&gap[cut..]);
                    }
                }
            }
        }
        SyntaxNode::new_root(b.finish())
    }
}

#[derive(Default)]
struct Scan {
    evs: Vec<Ev>,
    real: Vec<Tok>,
    comments: Vec<Tok>,
    /// Next comment not yet placed by `gap`.
    ci: usize,
    errors: Vec<ParseError>,
}

impl Scan {
    fn token(&mut self, t: Tok) {
        self.real.push(t);
        self.evs.push(Ev::Real);
    }

    fn error(&mut self, start: usize, len: usize, message: String) {
        self.errors.push(ParseError {
            start: start as u32,
            len: len as u32,
            code: "syntax-error".into(),
            message,
        });
    }
}

/// A parser pool plus the resolved mapping: one per language, shared by threads.
pub struct Engine {
    language: Language,
    grammar: &'static Grammar,
    mapping: &'static Mapping,
    converter: OnceLock<Converter>,
    pool: Mutex<Vec<Parser>>,
}

impl Engine {
    pub fn new(language: Language, grammar: &'static Grammar, mapping: &'static Mapping) -> Self {
        Engine {
            language,
            grammar,
            mapping,
            converter: OnceLock::new(),
            pool: Mutex::new(Vec::new()),
        }
    }

    fn new_parser(&self) -> Parser {
        let mut p = Parser::new();
        p.set_language(&self.language)
            .expect("the grammar's ABI version matches the tree-sitter runtime");
        p
    }

    /// Tree-sitter parse alone (the first step of [`Engine::parse`]).
    pub fn parse_raw(&self, text: &str) -> tree_sitter::Tree {
        let mut p = self
            .pool
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| self.new_parser());
        let tree = p
            .parse(text, None)
            .expect("no timeout or cancellation is set");
        self.pool.lock().unwrap().push(p);
        tree
    }

    /// Adapter conversion alone (the second step of [`Engine::parse`]).
    pub fn convert(&self, text: &str, tree: &tree_sitter::Tree) -> ParseResult {
        let conv = self
            .converter
            .get_or_init(|| Converter::new(&tree.language(), self.grammar, self.mapping));
        conv.convert(text, tree.root_node())
    }

    pub fn parse(&self, text: &str) -> ParseResult {
        self.convert(text, &self.parse_raw(text))
    }
}
