//! Compiling a pattern and matching it against the shared tree.
//!
//! A pattern is parsed by its language's own parser, so it is a tree of the same kinds as the
//! code it matches. Trivia is ignored on both sides. A metavariable node matches one element,
//! or for `$$$NAME` a (possibly empty) run of sibling elements.

use crate::metavar::{self, Meta, Style};
use core_rules::tree::significant_range;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, TextRange, TextSize};
use std::collections::BTreeMap;

/// Parses snippets of one language. `Ok(None)` means the text has syntax errors.
pub trait SnippetParser {
    fn grammar(&self) -> &Grammar;
    fn parse(&self, text: &str) -> Result<Option<SyntaxNode>, String>;
}

/// A compiled pattern node.
#[derive(Clone, Debug)]
enum Pat {
    Meta(Meta),
    Token {
        kind: SyntaxKind,
        text: String,
    },
    Node {
        kind: SyntaxKind,
        children: Vec<Pat>,
    },
}

#[derive(Clone, Debug)]
pub struct Pattern {
    root: Pat,
    kind: SyntaxKind,
    metas: Vec<Meta>,
}

/// What a metavariable matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub range: TextRange,
    /// The node kind when exactly one node was captured.
    pub kind: Option<SyntaxKind>,
    /// The significant tokens joined by a space: what repeated metavariables are compared by.
    normalized: String,
}

pub type Captures = BTreeMap<String, Capture>;

fn tokens(e: &SyntaxElement, g: &Grammar) -> Vec<core_syntax::SyntaxToken> {
    let keep = |t: &core_syntax::SyntaxToken| !g.is_trivia(t.kind()) && !t.text().is_empty();
    match e {
        SyntaxElement::Token(t) => Some(t.clone()).into_iter().filter(keep).collect(),
        SyntaxElement::Node(n) => n
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(keep)
            .collect(),
    }
}

fn significant_children(node: &SyntaxNode, g: &Grammar) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|e| match e {
            SyntaxElement::Token(t) => !g.is_trivia(t.kind()) && !t.text().is_empty(),
            SyntaxElement::Node(_) => true,
        })
        .collect()
}

fn element_range(e: &SyntaxElement, g: &Grammar) -> TextRange {
    match e {
        SyntaxElement::Token(t) => t.text_range(),
        SyntaxElement::Node(n) => significant_range(n, g),
    }
}

fn convert(e: &SyntaxElement, g: &Grammar) -> Pat {
    let joined: String = tokens(e, g).iter().map(|t| t.text().to_string()).collect();
    if let Some(meta) = metavar::decode(&joined) {
        return Pat::Meta(meta);
    }
    match e {
        SyntaxElement::Token(t) => Pat::Token {
            kind: t.kind(),
            text: t.text().to_string(),
        },
        SyntaxElement::Node(n) => Pat::Node {
            kind: n.kind(),
            children: significant_children(n, g)
                .iter()
                .map(|c| convert(c, g))
                .collect(),
        },
    }
}

/// What a snippet is put in when it does not parse as it is: a terminator (a C# expression
/// statement) or an object around it (a JSON member).
const CONTEXTS: [(&str, &str); 3] = [("", ""), ("", ";"), ("{", "}")];

impl Pattern {
    /// Parses `text` as a snippet of the language and keeps the node that spans all of it. A
    /// pattern that is not a complete construct on its own is retried in [`CONTEXTS`].
    pub fn compile(text: &str, parser: &dyn SnippetParser) -> Result<Pattern, String> {
        let metas = metavar::names(text);
        let grammar = parser.grammar();
        let trimmed = text.trim();
        for style in [Style::Identifier, Style::String] {
            let encoded = metavar::encode(trimmed, style);
            for (before, after) in CONTEXTS {
                let Some(tree) = parser.parse(&format!("{before}{encoded}{after}"))? else {
                    continue;
                };
                let start = TextSize::from(before.len() as u32);
                let whole = TextRange::at(start, TextSize::from(encoded.len() as u32));
                let node = tree
                    .descendants()
                    .filter(|n| significant_range(n, grammar) == whole)
                    .last();
                if let Some(node) = node {
                    let root = convert(&SyntaxElement::Node(node.clone()), grammar);
                    if matches!(root, Pat::Meta(_)) {
                        return Err("a pattern cannot be only a metavariable".into());
                    }
                    return Ok(Pattern {
                        root,
                        kind: node.kind(),
                        metas,
                    });
                }
            }
        }
        Err(format!(
            "`{trimmed}` is not a single syntax node of the language; \
             write a complete expression, statement or declaration"
        ))
    }

    /// The kind of node the pattern starts at.
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub fn metas(&self) -> &[Meta] {
        &self.metas
    }

    /// The captures when the pattern matches `node`. `text` is the file's source.
    pub fn matches(&self, node: &SyntaxNode, grammar: &Grammar) -> Option<Captures> {
        let mut cx = Cx {
            grammar,
            env: Captures::new(),
        };
        cx.matches(&self.root, &SyntaxElement::Node(node.clone()))
            .then_some(cx.env)
    }
}

struct Cx<'a> {
    grammar: &'a Grammar,
    env: Captures,
}

impl Cx<'_> {
    fn capture(&self, run: &[SyntaxElement], at: TextSize) -> Capture {
        let g = self.grammar;
        let range = match (run.first(), run.last()) {
            (Some(first), Some(last)) => TextRange::new(
                element_range(first, g).start(),
                element_range(last, g).end(),
            ),
            _ => TextRange::empty(at),
        };
        let normalized = run
            .iter()
            .flat_map(|e| tokens(e, g))
            .map(|t| t.text().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let kind = match run {
            [SyntaxElement::Node(n)] => Some(n.kind()),
            _ => None,
        };
        Capture {
            range,
            kind,
            normalized,
        }
    }

    /// Binds `meta` to `run`; false when the name is already bound to different text.
    fn bind(&mut self, meta: &Meta, run: &[SyntaxElement], at: TextSize) -> bool {
        if !meta.captures() {
            return true;
        }
        let capture = self.capture(run, at);
        match self.env.get(&meta.name) {
            Some(bound) => bound.normalized == capture.normalized,
            None => {
                self.env.insert(meta.name.clone(), capture);
                true
            }
        }
    }

    fn matches(&mut self, p: &Pat, t: &SyntaxElement) -> bool {
        let at = element_range(t, self.grammar).start();
        match (p, t) {
            (Pat::Meta(meta), _) => self.bind(meta, std::slice::from_ref(t), at),
            (Pat::Token { kind, text }, SyntaxElement::Token(t)) => {
                *kind == t.kind() && text == t.text()
            }
            (Pat::Node { kind, children }, SyntaxElement::Node(n)) => {
                *kind == n.kind() && {
                    let targets = significant_children(n, self.grammar);
                    self.sequence(children, &targets, at)
                }
            }
            _ => false,
        }
    }

    fn sequence(&mut self, ps: &[Pat], ts: &[SyntaxElement], at: TextSize) -> bool {
        let Some((first, rest)) = ps.split_first() else {
            return ts.is_empty();
        };
        if let Pat::Meta(meta) = first
            && meta.multi
        {
            for take in 0..=ts.len() {
                let saved = self.env.clone();
                let here = ts
                    .get(take)
                    .map_or(at, |e| element_range(e, self.grammar).start());
                if self.bind(meta, &ts[..take], here) && self.sequence(rest, &ts[take..], here) {
                    return true;
                }
                self.env = saved;
            }
            return false;
        }
        let Some((head, tail)) = ts.split_first() else {
            return false;
        };
        let saved = self.env.clone();
        if self.matches(first, head) && self.sequence(rest, tail, at) {
            return true;
        }
        self.env = saved;
        false
    }
}

impl Capture {
    /// The captured source text.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[usize::from(self.range.start())..usize::from(self.range.end())]
    }
}
