//! Grammar descriptions (`grammars/<lang>.grammar`).
//!
//! ```text
//! // comment
//! language json 1                      // name and LanguageId (1..=15)
//! comments line "//" block "/*" "*/"   // either part optional
//! suppress disable-next-line "sudsy-disable-next-line"   // role, marker
//! token Whitespace #trivia #whitespace // variable-text token with tags
//! token LBrace "{" #punctuation        // fixed-text token
//! node Member #key-value = key:String Colon value:#value
//! node Array #value #delimited-list = LBracket elements:#value* RBracket
//! ```
//!
//! A node body is a list of items `[label:]Ref[?|*]`. `Ref` is a node or token
//! name, or `#tag` meaning "any kind carrying that tag" (all nodes or all
//! tokens). Labeled items become typed accessors; unlabeled ones document the
//! shape. A declaration may continue on following indented lines. Bodies are
//! descriptive, not enforced against trees.
//!
//! Kind ids are assigned in declaration order, see [`crate::kind`]. The
//! grammar's version is a SHA-256 over its lexemes (comments and layout do not
//! change it; declaration order does).

mod parse;

use crate::kind::{LanguageId, SyntaxKind};
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
    pub comments: CommentSyntax,
    pub suppressions: Vec<Suppression>,
    nodes: Vec<KindDef>,
    tokens: Vec<KindDef>,
    by_name: HashMap<String, SyntaxKind>,
    hash: String,
    /// Hot tags per kind, indexed like `tokens` and `nodes`.
    token_flags: Vec<u8>,
    node_flags: Vec<u8>,
}

/// Tags that formatting code asks about for every token or node: answered from a bit set
/// instead of scanning the tag strings.
const HOT_TAGS: [&str; 7] = [
    "trivia",
    "separator",
    "restyle",
    "comment",
    "delimited-list",
    "block",
    "key-value",
];

fn hot_flags(defs: &[KindDef]) -> Vec<u8> {
    defs.iter()
        .map(|d| {
            HOT_TAGS
                .iter()
                .enumerate()
                .filter(|(_, t)| d.has_tag(t))
                .fold(0u8, |acc, (i, _)| acc | 1 << i)
        })
        .collect()
}

impl Grammar {
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

    fn flags_of(&self, kind: SyntaxKind) -> u8 {
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
        if kind == SyntaxKind::EMBEDDED {
            return "EMBEDDED";
        }
        self.def(kind).map_or("?", |d| d.name.as_str())
    }
}
