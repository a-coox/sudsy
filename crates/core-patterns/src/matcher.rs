//! Compiling a pattern and matching it against the shared tree.
//!
//! A pattern is parsed by its language's own parser, so it is a tree of the same kinds as the
//! code it matches. Trivia is ignored on both sides. A metavariable node matches one element,
//! or for `$$$NAME` a (possibly empty) run of sibling elements.

use crate::metavar::{self, Meta, Style};
use core_rules::tree::significant_range;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, TextRange, TextSize};
use std::cell::Cell;
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
    /// Capturing names that occur more than once: only their captures are compared, so only
    /// theirs need the text.
    repeated: Vec<String>,
    /// The text of the pattern's first token when it is not a metavariable: a node that starts
    /// with anything else cannot match, which most nodes of the right kind show at once.
    first: Option<String>,
}

/// What a metavariable matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub range: TextRange,
    /// The node kind when exactly one node was captured.
    pub kind: Option<SyntaxKind>,
    /// The significant tokens joined by a space: what repeated metavariables are compared by
    /// (only computed for those).
    normalized: Option<String>,
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

fn first_leaf(p: &Pat) -> Option<&str> {
    match p {
        Pat::Meta(_) => None,
        Pat::Token { text, .. } => Some(text),
        Pat::Node { children, .. } => children.first().and_then(first_leaf),
    }
}

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
                    let repeated = metas
                        .iter()
                        .filter(|m| m.captures())
                        .filter(|m| metas.iter().filter(|o| o.name == m.name).count() > 1)
                        .map(|m| m.name.clone())
                        .collect();
                    return Ok(Pattern {
                        repeated,
                        first: first_leaf(&root).map(String::from),
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
    /// `budget` is the steps left to the whole evaluation; at 0 nothing matches any more.
    pub fn matches(
        &self,
        node: &SyntaxNode,
        grammar: &Grammar,
        budget: &Cell<u32>,
    ) -> Option<Captures> {
        if node.kind() != self.kind {
            return None;
        }
        if let Some(first) = &self.first {
            let mut token = node.first_token();
            while let Some(t) = &token {
                if !grammar.is_trivia(t.kind()) && !t.text().is_empty() {
                    break;
                }
                token = t.next_token();
            }
            if token.is_none_or(|t| t.text() != first.as_str()) {
                return None;
            }
        }
        let mut cx = Cx {
            grammar,
            repeated: &self.repeated,
            env: Vec::new(),
            budget: budget.get(),
        };
        let found = cx.matches(&self.root, &SyntaxElement::Node(node.clone()));
        budget.set(cx.budget);
        found.then(|| {
            cx.env
                .into_iter()
                .map(|(name, capture)| (name.to_string(), capture))
                .collect()
        })
    }
}

struct Cx<'p> {
    grammar: &'p Grammar,
    repeated: &'p [String],
    /// Bindings in the order they were made; backtracking cuts the list back.
    env: Vec<(&'p str, Capture)>,
    /// Steps left; `$$$` runs make matching O(n^k), so this bounds repo-supplied patterns.
    budget: u32,
}

/// The kind a pattern node requires of its target, when it requires one.
fn required_kind(p: &Pat) -> Option<SyntaxKind> {
    match p {
        Pat::Meta(_) => None,
        Pat::Token { kind, .. } | Pat::Node { kind, .. } => Some(*kind),
    }
}

impl<'p> Cx<'p> {
    /// Spends a step; true when none are left.
    fn step(&mut self) -> bool {
        self.budget = self.budget.saturating_sub(1);
        self.budget == 0
    }

    fn capture(&self, run: &[SyntaxElement], at: TextSize, with_text: bool) -> Capture {
        let g = self.grammar;
        let range = match (run.first(), run.last()) {
            (Some(first), Some(last)) => TextRange::new(
                element_range(first, g).start(),
                element_range(last, g).end(),
            ),
            _ => TextRange::empty(at),
        };
        let normalized = with_text.then(|| {
            run.iter()
                .flat_map(|e| tokens(e, g))
                .map(|t| t.text().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        });
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
    fn bind(&mut self, meta: &'p Meta, run: &[SyntaxElement], at: TextSize) -> bool {
        if !meta.captures() {
            return true;
        }
        let repeated = self.repeated.contains(&meta.name);
        let capture = self.capture(run, at, repeated);
        match self.env.iter().find(|(name, _)| *name == meta.name) {
            Some((_, bound)) => bound.normalized == capture.normalized,
            None => {
                self.env.push((&meta.name, capture));
                true
            }
        }
    }

    fn matches(&mut self, p: &'p Pat, t: &SyntaxElement) -> bool {
        match (p, t) {
            (Pat::Meta(meta), _) => {
                let at = element_range(t, self.grammar).start();
                self.bind(meta, std::slice::from_ref(t), at)
            }
            (Pat::Token { kind, text }, SyntaxElement::Token(t)) => {
                *kind == t.kind() && text == t.text()
            }
            (Pat::Node { kind, children }, SyntaxElement::Node(n)) => {
                *kind == n.kind() && self.first_child_fits(children, n) && {
                    let targets = significant_children(n, self.grammar);
                    // Only an empty capture at the end uses the position.
                    self.sequence(children, &targets, n.text_range().end())
                }
            }
            _ => false,
        }
    }

    /// Does the first significant child of `n` have the kind the pattern's first child needs?
    /// Most nodes of the right kind are ruled out here, before their children are listed.
    fn first_child_fits(&self, children: &[Pat], n: &SyntaxNode) -> bool {
        let Some(wanted) = children.first().and_then(required_kind) else {
            return true;
        };
        n.children_with_tokens()
            .find(|e| match e {
                SyntaxElement::Token(t) => {
                    !self.grammar.is_trivia(t.kind()) && !t.text().is_empty()
                }
                SyntaxElement::Node(_) => true,
            })
            .is_some_and(|first| first.kind() == wanted)
    }

    fn sequence(&mut self, ps: &'p [Pat], ts: &[SyntaxElement], at: TextSize) -> bool {
        let Some((first, rest)) = ps.split_first() else {
            return ts.is_empty();
        };
        if let Pat::Meta(meta) = first
            && meta.multi
        {
            for take in 0..=ts.len() {
                if self.step() {
                    return false;
                }
                let saved = self.env.len();
                let here = ts
                    .get(take)
                    .map_or(at, |e| element_range(e, self.grammar).start());
                if self.bind(meta, &ts[..take], here) && self.sequence(rest, &ts[take..], here) {
                    return true;
                }
                self.env.truncate(saved);
            }
            return false;
        }
        let Some((head, tail)) = ts.split_first() else {
            return false;
        };
        let saved = self.env.len();
        if self.matches(first, head) && self.sequence(rest, tail, at) {
            return true;
        }
        self.env.truncate(saved);
        false
    }
}

impl Capture {
    /// The captured source text.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.range]
    }
}
