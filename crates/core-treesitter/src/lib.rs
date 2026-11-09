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
//! A [`Composite`] adds what a language made of several tree-sitter grammars needs: a host node
//! whose text a second grammar parses (Markdown's `inline` blocks) becomes a node whose children
//! are the injected tree's, text no mapped node covers
//! becomes tokens of one text kind, and a token whose text starts a given way gets another kind
//! (an HTML comment).
//!
//! Parsing is not incremental and has no depth limit of its own (the walk is iterative; the
//! formatter recurses). See `docs/architecture/adding-a-language.md`.

mod provider;

pub use provider::{Spec, TreeSitterProvider};

use core_syntax::grammars::LazyGrammar;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TreeBuilder};
use provider_api::{ParseError, ParseResult};
use std::sync::{Mutex, OnceLock};
use tree_sitter::{Language, Node, Parser, Range};

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

/// The `kinds` entry for a tree-sitter node. A `from` written `"name"` (quoted, as in tree-sitter
/// queries) matches only the anonymous node of that text and `name` only the named one; with no
/// quoted entry, `name` matches both. A grammar needs the quotes when a named rule and a keyword
/// share a name (`message`).
fn lookup(m: &Mapping, name: &str, named: bool) -> Option<&'static (&'static str, &'static str)> {
    let find = |q: bool| {
        m.kinds
            .iter()
            .find(|(from, _)| match from.strip_prefix('"') {
                Some(inner) => q && inner.strip_suffix('"') == Some(name),
                None => !q && *from == name,
            })
    };
    if named {
        find(false)
    } else {
        find(true).or_else(|| find(false))
    }
}

/// How a language made of several tree-sitter grammars is assembled (see the module docs).
#[derive(Clone, Copy)]
pub struct Composite {
    /// Kind name of the token for text that no mapped node covers (blanks excepted, per line).
    /// With it, an unmapped leaf is text rather than an error.
    pub text: &'static str,
    /// Grammars parsed inside host nodes.
    pub injected: &'static [Injected],
    /// `(token name, text prefix, kind name)`: a mapped token whose text starts with the prefix
    /// is the other kind.
    pub refine: &'static [(&'static str, &'static str, &'static str)],
    /// A `\` that ends a line is whitespace trivia, not text (shell line continuations).
    pub continuation: bool,
    /// Tree-sitter node names whose descendants' syntax errors are not reported: the text is
    /// kept as the node's own children (a language whose tokens swallowed these subtrees before
    /// they became nodes keeps accepting what the grammar mis-parses inside them).
    pub tolerant: &'static [&'static str],
}

/// A grammar parsed inside the nodes named `host` of the host grammar. The host node's own named
/// children are carved out of the injected text and handled by the host mapping.
pub struct Injected {
    pub host: &'static str,
    pub language: fn() -> Language,
    pub mapping: &'static Mapping,
}

#[derive(Clone, Copy)]
enum Entry {
    Node(SyntaxKind),
    Token(SyntaxKind, Option<SyntaxKind>, bool),
    Trivia(SyntaxKind),
    Transparent,
}

/// The mapping resolved against one tree-sitter language: a table indexed by node kind id.
struct Converter {
    table: Vec<Entry>,
    names: Vec<String>,
    text: Option<SyntaxKind>,
    continuation: bool,
    /// Node id -> errors below it are not reported ([`Composite::tolerant`]).
    tolerant: Vec<bool>,
    /// `(node id, prefix, kind)` of [`Composite::refine`].
    refine: Vec<(usize, &'static str, SyntaxKind)>,
    /// Node id -> index into `injected`.
    inject: Vec<Option<usize>>,
    injected: Vec<Inj>,
    ws: SyntaxKind,
    nl: SyntaxKind,
    eof: SyntaxKind,
    unknown: SyntaxKind,
}

/// A resolved [`Injected`]: its converter and a parser pool.
struct Inj {
    conv: Converter,
    pool: Mutex<Vec<Parser>>,
    language: Language,
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
    fn new(
        language: &Language,
        grammar: &'static Grammar,
        m: &Mapping,
        comp: Option<Composite>,
    ) -> Self {
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
            if let Some((_, target)) = lookup(m, name, language.node_kind_is_named(id as u16)) {
                let k = kind(target);
                *slot = match (k.is_token(), grammar.is_trivia(k)) {
                    (false, _) => Entry::Node(k),
                    (true, false) => Entry::Token(
                        k,
                        m.wrap
                            .iter()
                            .find(|(t, _)| t == target)
                            .map(|(_, w)| kind(w)),
                        grammar.has_tag(k, "checked"),
                    ),
                    (true, true) => Entry::Trivia(k),
                };
            }
            names[id] = name.to_string();
        }
        let id_of = |n: &str| names.iter().position(|x| x == n);
        let mut inject = vec![None; count];
        let (mut injected, mut refine, mut text) = (Vec::new(), Vec::new(), None);
        let mut tolerant = vec![false; count];
        let continuation = comp.is_some_and(|c| c.continuation);
        if let Some(c) = comp {
            text = Some(kind(c.text));
            for name in c.tolerant {
                if let Some(id) = id_of(name) {
                    tolerant[id] = true;
                }
            }
            for (token, prefix, to) in c.refine {
                if let Some(id) = id_of(token) {
                    refine.push((id, *prefix, kind(to)));
                }
            }
            for inj in c.injected {
                let host = id_of(inj.host).unwrap_or_else(|| panic!("no node `{}`", inj.host));
                inject[host] = Some(injected.len());
                let language = (inj.language)();
                // The injected grammar is converted with the same text kind and refinements.
                let inner = Composite { injected: &[], ..c };
                injected.push(Inj {
                    conv: Converter::new(&language, grammar, inj.mapping, Some(inner)),
                    pool: Mutex::new(Vec::new()),
                    language,
                });
            }
        }
        Converter {
            table,
            names,
            text,
            continuation,
            tolerant,
            refine,
            inject,
            injected,
            ws: kind(m.whitespace),
            nl: kind(m.newline),
            eof: kind(m.eof),
            unknown: kind(m.unknown),
        }
    }

    fn convert(&self, src: &str, root: Node) -> ParseResult {
        let mut s = Scan::default();
        self.scan(src, root, &mut s);
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
    fn scan(&self, src: &str, root: Node, s: &mut Scan) {
        let mut cursor = root.walk();
        let mut pushed_stack: Vec<bool> = Vec::new();
        loop {
            let (pushed, descend) = self.enter(src, cursor.node(), s);
            if descend && cursor.goto_first_child() {
                pushed_stack.push(pushed);
                continue;
            }
            if pushed {
                self.fill(src, s, cursor.node().end_byte());
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
                    self.fill(src, s, cursor.node().end_byte());
                    s.evs.push(Ev::Exit);
                }
            }
        }
    }

    /// Is `n` below a node of [`Composite::tolerant`]?
    fn tolerated(&self, n: Node) -> bool {
        let mut up = n.parent();
        while let Some(p) = up {
            if self.tolerant.get(p.kind_id() as usize) == Some(&true) {
                return true;
            }
            up = p.parent();
        }
        false
    }

    /// Returns (a node was opened, descend into the children).
    fn enter(&self, src: &str, n: Node, s: &mut Scan) -> (bool, bool) {
        let (start, end) = (n.start_byte(), n.end_byte());
        let id = n.kind_id() as usize;
        let name = self.names.get(id).map_or("?", String::as_str);
        let (error, missing) = (n.is_error(), n.is_missing());
        if (error || missing) && self.tolerated(n) {
            return (false, error);
        }
        if missing {
            s.error(start, 0, format!("missing {name}"));
            return (false, false);
        }
        // An unmapped leaf is text and joins the text before it.
        let text_leaf = self.text.is_some()
            && n.child_count() == 0
            && !error
            && matches!(self.table.get(id), Some(Entry::Transparent));
        if !text_leaf {
            self.fill(src, s, start);
        }
        if error {
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
                match self.inject[id] {
                    Some(i) => {
                        self.inject_into(src, n, &self.injected[i], s);
                        (true, false)
                    }
                    None => (true, true),
                }
            }
            Entry::Token(kind, wrap, string) => {
                // The children of a token are swallowed: an error inside a `#checked` token is reported here.
                if string && n.has_error() {
                    s.error(start, end - start, "syntax error".into());
                }
                // Blank text (XML character data between tags) is whitespace trivia, not a token.
                let blank = src.as_bytes()[start..end]
                    .iter()
                    .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'));
                if !blank {
                    let text = &src[start..end];
                    let kind = (self.refine.iter())
                        .find(|(i, prefix, _)| *i == id && text.starts_with(prefix))
                        .map_or(kind, |r| r.2);
                    let wrap = wrap.filter(|w| !self.parent_is(n, *w));
                    s.evs.extend(wrap.map(Ev::Enter));
                    s.token(Tok { kind, start, end });
                    s.evs.extend(wrap.map(|_| Ev::Exit));
                }
                (false, false)
            }
            Entry::Trivia(kind) => {
                // Blank trivia (a block continuation of indentation) is lexed from the gap.
                if !src[start..end].trim().is_empty() {
                    // A grammar may put the comment's line break inside the node.
                    let end = start + src[start..end].trim_end_matches(['\n', '\r']).len();
                    s.comments.push(Tok { kind, start, end });
                    s.covered = s.covered.max(end);
                }
                (false, false)
            }
            Entry::Transparent if n.child_count() > 0 => (false, true),
            // An unmapped leaf is text; the next `fill` makes it a token.
            Entry::Transparent if self.text.is_some() => (false, false),
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

    /// Parses the host node's text with the injected grammar and scans the result in place of
    /// the host's children. The host's own named children (block continuations) are carved out
    /// of the parsed text and emitted as trivia where they fall.
    fn inject_into(&self, src: &str, host: Node, inj: &Inj, s: &mut Scan) {
        let (mut ranges, mut carved) = (Vec::new(), Vec::new());
        let (mut from_b, mut from_p) = (host.start_byte(), host.start_position());
        let mut cursor = host.walk();
        for c in host.named_children(&mut cursor) {
            ranges.push(Range {
                start_byte: from_b,
                end_byte: c.start_byte(),
                start_point: from_p,
                end_point: c.start_position(),
            });
            (from_b, from_p) = (c.end_byte(), c.end_position());
            let (start, end) = (c.start_byte(), c.end_byte());
            if let Some(Entry::Trivia(kind)) = self.table.get(c.kind_id() as usize)
                && !src[start..end].trim().is_empty()
            {
                carved.push(Tok {
                    kind: *kind,
                    start,
                    end,
                });
            }
        }
        ranges.push(Range {
            start_byte: from_b,
            end_byte: host.end_byte(),
            start_point: from_p,
            end_point: host.end_position(),
        });
        let mut parser = inj.pool.lock().unwrap().pop().unwrap_or_else(|| {
            let mut p = Parser::new();
            p.set_language(&inj.language)
                .expect("the grammar's ABI version matches the tree-sitter runtime");
            p
        });
        let tree = parser
            .set_included_ranges(&ranges)
            .ok()
            .and_then(|()| parser.parse(src, None));
        inj.pool.lock().unwrap().push(parser);
        carved.reverse();
        s.pending = carved;
        if let Some(tree) = tree {
            inj.conv.scan(src, tree.root_node(), s);
        }
        self.fill(src, s, host.end_byte());
        s.pending.clear();
    }

    /// Makes the text up to `upto` that no token covers tokens of the text kind, one per line
    /// with its blanks trimmed; the carved-out trivia of an injection are emitted in order.
    fn fill(&self, src: &str, s: &mut Scan, upto: usize) {
        let Some(text) = self.text else { return };
        while let Some(c) = s.pending.last().copied().filter(|c| c.start < upto) {
            s.pending.pop();
            self.text_run(src, s, text, c.start);
            s.comments.push(c);
            s.covered = s.covered.max(c.end);
        }
        self.text_run(src, s, text, upto);
    }

    fn text_run(&self, src: &str, s: &mut Scan, text: SyntaxKind, upto: usize) {
        let mut pos = s.covered;
        while pos < upto {
            let end = src[pos..upto].find('\n').map_or(upto, |i| pos + i);
            let line = &src[pos..end];
            let trimmed = line.trim_start_matches([' ', '\t', '\r']);
            let start = pos + (line.len() - trimmed.len());
            let mut stop = start + trimmed.trim_end_matches([' ', '\t', '\r']).len();
            // A line continuation (`\` alone or after a blank) is trivia, which `gap` lexes.
            if self.continuation && src[start..stop].ends_with('\\') {
                let before = src[start..stop - 1].trim_end_matches([' ', '\t']);
                if before.len() < stop - 1 - start || before.is_empty() {
                    stop = start + before.len();
                }
            }
            if stop > start {
                s.token(Tok {
                    kind: text,
                    start,
                    end: stop,
                });
            }
            pos = end + 1;
        }
        s.covered = s.covered.max(upto);
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
                b'\\' if self.continuation && bytes[pos + 1..].starts_with(b"\n") => (self.ws, 2),
                b'\\' if self.continuation && bytes[pos + 1..].starts_with(b"\r\n") => (self.ws, 3),
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
    /// End of the text already made tokens or trivia.
    covered: usize,
    /// Trivia carved out of the injected text being scanned, last first.
    pending: Vec<Tok>,
    /// Next comment not yet placed by `gap`.
    ci: usize,
    errors: Vec<ParseError>,
}

impl Scan {
    fn token(&mut self, t: Tok) {
        self.covered = self.covered.max(t.end);
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
    load: fn() -> Language,
    language: OnceLock<Language>,
    grammar: LazyGrammar,
    mapping: &'static Mapping,
    composite: Option<Composite>,
    converter: OnceLock<Converter>,
    pool: Mutex<Vec<Parser>>,
}

impl Engine {
    pub fn new(load: fn() -> Language, grammar: LazyGrammar, mapping: &'static Mapping) -> Self {
        Self::with_composite(load, grammar, mapping, None)
    }

    pub fn with_composite(
        load: fn() -> Language,
        grammar: LazyGrammar,
        mapping: &'static Mapping,
        composite: Option<Composite>,
    ) -> Self {
        Engine {
            load,
            language: OnceLock::new(),
            grammar,
            mapping,
            composite,
            converter: OnceLock::new(),
            pool: Mutex::new(Vec::new()),
        }
    }

    fn new_parser(&self) -> Parser {
        let mut p = Parser::new();
        p.set_language(self.language.get_or_init(self.load))
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
        let conv = self.converter.get_or_init(|| {
            Converter::new(
                &tree.language(),
                self.grammar.get(),
                self.mapping,
                self.composite,
            )
        });
        conv.convert(text, tree.root_node())
    }

    pub fn parse(&self, text: &str) -> ParseResult {
        self.convert(text, &self.parse_raw(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(kinds: &'static [(&'static str, &'static str)]) -> Mapping {
        Mapping {
            whitespace: "",
            newline: "",
            eof: "",
            unknown: "",
            kinds,
            wrap: &[],
        }
    }

    #[test]
    fn quoted_entries_match_only_the_anonymous_node() {
        let both = mapping(&[("message", "Message"), ("\"message\"", "Keyword")]);
        assert_eq!(lookup(&both, "message", true).unwrap().1, "Message");
        assert_eq!(lookup(&both, "message", false).unwrap().1, "Keyword");
        // Without a quoted entry the name matches both; a quoted entry alone matches no named node.
        let plain = mapping(&[("{", "Brace")]);
        assert_eq!(lookup(&plain, "{", true).unwrap().1, "Brace");
        assert_eq!(lookup(&plain, "{", false).unwrap().1, "Brace");
        let quoted = mapping(&[("\"if\"", "Keyword")]);
        assert!(lookup(&quoted, "if", true).is_none());
    }
}
