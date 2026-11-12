//! Grammar descriptions (`grammars/<lang>.grammar`).
//!
//! ```text
//! // comment
//! language json 1 "JSON"              // name, LanguageId (1..=31), display name (optional)
//! comments line "//" block "/*" "*/"   // either part optional
//! suppress disable-next-line "sudsy-disable-next-line"   // role, marker; roles: disable-next-line,
//!                                                        // disable-line, disable, enable, disable-file, ignore*
//! token Whitespace #trivia #whitespace // variable-text token with tags
//! token LBrace "{" #punctuation        // fixed-text token
//! node Member #key-value = key:String Colon value:#value
//! node Array #value #delimited-list = LBracket elements:#value* RBracket
//! ```
//!
//! A node body is a list of items `[label:]Ref[?|*]`. `Ref` is a node or token
//! name, or `#tag` meaning "any kind carrying that tag" (all nodes or all
//! tokens). Labeled items become typed accessors; unlabeled ones document the
//! shape. A declaration may continue on following indented lines. A body is matched against a
//! node's children in order (see [`crate::ast::assign`]), so the same type may appear in several
//! items; bodies are not enforced against trees, the conformance suites check them.
//!
//! Kind ids are assigned in declaration order, see [`crate::kind`]. The
//! grammar's version is a SHA-256 over its lexemes (comments and layout do not
//! change it; declaration order does).

mod parse;

use crate::kind::{LanguageId, SyntaxKind};
use crate::tree::{SyntaxElement, SyntaxNode};
use std::collections::HashMap;

pub use parse::GrammarError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Optional,
    Many,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Kind(SyntaxKind),
    Tag(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub label: Option<String>,
    pub target: Target,
    pub cardinality: Cardinality,
}

#[derive(Clone, Debug)]
pub struct KindDef {
    pub name: String,
    pub kind: SyntaxKind,
    /// Fixed text of a token (punctuation, keywords).
    pub text: Option<String>,
    pub tags: Vec<String>,
    /// Shape of a node; empty for tokens.
    pub body: Vec<Item>,
}

impl KindDef {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Labeled body items, i.e. the ones that become accessors.
    pub fn fields(&self) -> impl Iterator<Item = &Item> {
        self.body.iter().filter(|i| i.label.is_some())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommentSyntax {
    pub line: Option<String>,
    pub block: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suppression {
    pub role: String,
    pub marker: String,
}

#[derive(Clone, Debug)]
pub struct Grammar {
    pub language: String,
    pub language_id: LanguageId,
    /// The language's name for people (`C#`); the `language` name when the grammar gives none.
    pub title: String,
    pub comments: CommentSyntax,
    pub suppressions: Vec<Suppression>,
    nodes: Vec<KindDef>,
    tokens: Vec<KindDef>,
    by_name: HashMap<String, SyntaxKind>,
    hash: String,
    /// Hot tags per kind, indexed like `tokens` and `nodes`.
    token_flags: Vec<u16>,
    node_flags: Vec<u16>,
    /// Per node kind (indexed like `nodes`) and body item: the sorted kinds the item accepts.
    bodies: Vec<Vec<Vec<SyntaxKind>>>,
    /// Trivia and `ERROR`, sorted: invisible to bodies.
    skip: Vec<SyntaxKind>,
    /// Node kinds per tag, in id order.
    tag_nodes: HashMap<String, Vec<SyntaxKind>>,
}

/// Tags that formatting code asks about for every token or node: answered from a bit set
/// instead of scanning the tag strings.
const HOT_TAGS: [&str; 9] = [
    "trivia",
    "separator",
    "optional",
    "restyle",
    "comment",
    "delimited-list",
    "block",
    "key-value",
    "verbatim-trivia",
];

fn hot_flags(defs: &[KindDef]) -> Vec<u16> {
    defs.iter()
        .map(|d| {
            HOT_TAGS
                .iter()
                .enumerate()
                .filter(|(_, t)| d.has_tag(t))
                .fold(0u16, |acc, (i, _)| acc | 1 << i)
        })
        .collect()
}

/// One node kind's body as [`crate::ast::assign`] sees it.
#[derive(Clone, Copy)]
struct GrammarBody<'g> {
    grammar: &'g Grammar,
    kinds: &'g [Vec<SyntaxKind>],
    body: &'g [Item],
}

impl crate::ast::Body for GrammarBody<'_> {
    fn slots(&self) -> usize {
        self.kinds.len()
    }
    fn accepts(&self, slot: usize, kind: SyntaxKind) -> bool {
        self.kinds[slot].binary_search(&kind).is_ok()
    }
    fn many(&self, slot: usize) -> bool {
        self.body[slot].cardinality == Cardinality::Many
    }
    fn skips(&self, kind: SyntaxKind) -> bool {
        kind == SyntaxKind::ERROR || self.grammar.is_trivia(kind)
    }
}

impl Grammar {
    /// Fills the tables derived from the kinds: resolved bodies, the skipped kinds and the nodes
    /// per tag.
    fn resolve(&mut self) {
        let kinds_of = |target: &Target| -> Vec<SyntaxKind> {
            let mut kinds: Vec<SyntaxKind> = match target {
                Target::Kind(k) => vec![*k],
                Target::Tag(t) => self.kinds_with_tag(t).map(|d| d.kind).collect(),
            };
            kinds.sort();
            kinds
        };
        let bodies = self
            .nodes
            .iter()
            .map(|d| d.body.iter().map(|i| kinds_of(&i.target)).collect())
            .collect();
        let mut skip: Vec<SyntaxKind> = std::iter::once(SyntaxKind::ERROR)
            .chain(self.kinds_with_tag("trivia").map(|d| d.kind))
            .collect();
        skip.sort();
        let mut tag_nodes: HashMap<String, Vec<SyntaxKind>> = HashMap::new();
        for d in &self.nodes {
            for tag in &d.tags {
                tag_nodes.entry(tag.clone()).or_default().push(d.kind);
            }
        }
        self.bodies = bodies;
        self.skip = skip;
        self.tag_nodes = tag_nodes;
    }

    /// For each body item of a node kind, the sorted kinds it accepts.
    pub fn body_kinds(&self, node: SyntaxKind) -> Option<&[Vec<SyntaxKind>]> {
        self.def(node)?;
        self.bodies.get(node.index() as usize).map(Vec::as_slice)
    }

    /// Trivia kinds and `ERROR`, sorted: they take no part in matching bodies.
    pub fn skipped_kinds(&self) -> &[SyntaxKind] {
        &self.skip
    }

    /// The node kinds carrying `tag`, in id order (precomputed; empty for an unknown tag).
    pub fn nodes_with_tag(&self, tag: &str) -> &[SyntaxKind] {
        self.tag_nodes.get(tag).map_or(&[], Vec::as_slice)
    }

    /// The child of `node` in the body item labelled `label` (the first, for a repeated item).
    /// Found with the routine the generated accessors use, so it agrees with them.
    pub fn slot(&self, node: &SyntaxNode, label: &str) -> Option<SyntaxElement> {
        let def = self.def(node.kind())?;
        let index = def
            .body
            .iter()
            .position(|i| i.label.as_deref() == Some(label))?;
        let body = GrammarBody {
            grammar: self,
            kinds: self.bodies.get(node.kind().index() as usize)?,
            body: &def.body,
        };
        crate::ast::assign(node, body)
            .find(|(i, _)| *i == index)
            .map(|(_, e)| e)
    }

    pub fn parse(source: &str) -> Result<Grammar, GrammarError> {
        parse::parse(source)
    }

    /// Content hash (hex SHA-256); the grammar's version.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    pub fn nodes(&self) -> &[KindDef] {
        &self.nodes
    }

    pub fn tokens(&self) -> &[KindDef] {
        &self.tokens
    }

    pub fn kind_by_name(&self, name: &str) -> Option<SyntaxKind> {
        self.by_name.get(name).copied()
    }

    /// Definition of a kind of this language; `None` for core or foreign kinds.
    pub fn def(&self, kind: SyntaxKind) -> Option<&KindDef> {
        if kind.lang() != self.language_id {
            return None;
        }
        let list = if kind.is_token() {
            &self.tokens
        } else {
            &self.nodes
        };
        list.get(kind.index() as usize)
    }

    pub fn has_tag(&self, kind: SyntaxKind, tag: &str) -> bool {
        if let Some(bit) = HOT_TAGS.iter().position(|t| *t == tag) {
            return self.flags_of(kind) & (1 << bit) != 0;
        }
        self.def(kind).is_some_and(|d| d.has_tag(tag))
    }

    fn flags_of(&self, kind: SyntaxKind) -> u16 {
        if kind.lang() != self.language_id {
            return 0;
        }
        let list = if kind.is_token() {
            &self.token_flags
        } else {
            &self.node_flags
        };
        list.get(kind.index() as usize).copied().unwrap_or(0)
    }

    /// The text of a comment without its delimiters (`//`, or `/*` and `*/`), trimmed.
    pub fn comment_body<'a>(&self, text: &'a str) -> &'a str {
        let body = match (self.comments.line.as_deref(), &self.comments.block) {
            (Some(prefix), _) if text.starts_with(prefix) => &text[prefix.len()..],
            (_, Some((open, close))) => text
                .strip_prefix(open.as_str())
                .map(|t| t.strip_suffix(close.as_str()).unwrap_or(t))
                .unwrap_or(text),
            _ => text,
        };
        body.trim()
    }

    pub fn is_trivia(&self, kind: SyntaxKind) -> bool {
        self.flags_of(kind) & 1 != 0
    }

    /// All kinds (tokens first, then nodes) carrying `tag`, in id order.
    pub fn kinds_with_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a KindDef> {
        self.tokens
            .iter()
            .chain(&self.nodes)
            .filter(move |d| d.has_tag(tag))
    }

    pub fn name(&self, kind: SyntaxKind) -> &str {
        if kind == SyntaxKind::ERROR {
            return "ERROR";
        }
        self.def(kind).map_or("?", |d| d.name.as_str())
    }
}
