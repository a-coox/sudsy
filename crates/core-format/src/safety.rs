//! The generic safety check (plan.md "Safety checks"): the output must parse and must have the
//! same tree as the input once trivia are ignored.
//!
//! Tokens tagged `#separator` are ignored as well. A formatter may legitimately add or remove a
//! trailing comma, and a missing separator elsewhere changes the item structure or fails the
//! re-parse, so nothing is lost by not comparing them.
//!
//! Tokens tagged `#restyle` (YAML's scalars) may change their kind and text, for example from
//! single to double quotes or by re-indenting continuation lines; they only have to stay where
//! they are. The provider's [`NodeFormatter::extra_check`] must verify their meaning.

use crate::FormatError;
use crate::context::NodeFormatter;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};

#[derive(Debug, PartialEq, Eq)]
enum Event<'a> {
    Enter(SyntaxKind),
    Leave(SyntaxKind),
    Token(SyntaxKind, &'a str),
    /// A `#restyle` token: only its position is compared.
    Restyled,
}

/// What the comparison needs to know about a token kind, looked up once per kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ignored,
    Restyle,
    Compared,
}

/// The significant events of a tree in document order, each with its byte offset. It walks the
/// green tree directly: building red nodes for millions of tokens would cost more than the
/// comparison itself.
struct Events<'a> {
    g: &'a Grammar,
    /// Open nodes: their kind and the children still to visit.
    stack: Vec<(SyntaxKind, rowan::Children<'a>)>,
    /// Byte offset after the tokens seen so far.
    pos: usize,
    root: Option<&'a rowan::GreenNodeData>,
}

impl<'a> Events<'a> {
    fn new(g: &'a Grammar, root: &'a rowan::GreenNodeData) -> Self {
        Events {
            g,
            stack: Vec::new(),
            pos: 0,
            root: Some(root),
        }
    }

    fn enter(&mut self, node: &'a rowan::GreenNodeData) -> (usize, Event<'a>) {
        let kind = SyntaxKind(node.kind().0);
        self.stack.push((kind, node.children()));
        (self.pos, Event::Enter(kind))
    }

    fn class(&self, kind: SyntaxKind) -> Class {
        let g = self.g;
        if g.is_trivia(kind) || g.has_tag(kind, "separator") {
            Class::Ignored
        } else if g.has_tag(kind, "restyle") {
            Class::Restyle
        } else {
            Class::Compared
        }
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = (usize, Event<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.take() {
            return Some(self.enter(root));
        }
        loop {
            let (kind, children) = self.stack.last_mut()?;
            match children.next() {
                None => {
                    let kind = *kind;
                    self.stack.pop();
                    return Some((self.pos, Event::Leave(kind)));
                }
                Some(rowan::NodeOrToken::Node(n)) => return Some(self.enter(n)),
                Some(rowan::NodeOrToken::Token(t)) => {
                    let kind = SyntaxKind(t.kind().0);
                    let at = self.pos;
                    self.pos += t.text().len();
                    match self.class(kind) {
                        Class::Ignored => {}
                        Class::Restyle => return Some((at, Event::Restyled)),
                        Class::Compared => return Some((at, Event::Token(kind, t.text()))),
                    }
                }
            }
        }
    }
}

/// `Ok` when both trees have the same nodes and non-trivia tokens in the same order; else a
/// description of the first difference.
pub fn equal_ignoring_trivia(g: &Grammar, a: &SyntaxNode, b: &SyntaxNode) -> Result<(), String> {
    let (a, b) = (a.green(), b.green());
    let (mut a, mut b) = (Events::new(g, &a), Events::new(g, &b));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ok(()),
            (Some((_, x)), Some((_, y))) if x == y => {}
            (x, y) => {
                return Err(format!(
                    "the trees differ: input has {}, output has {}",
                    describe(g, x),
                    describe(g, y)
                ));
            }
        }
    }
}

fn describe(g: &Grammar, ev: Option<(usize, Event<'_>)>) -> String {
    match ev {
        None => "nothing more".into(),
        Some((at, Event::Enter(k))) => format!("start of {} at byte {at}", g.name(k)),
        Some((at, Event::Leave(k))) => format!("end of {} at byte {at}", g.name(k)),
        Some((at, Event::Token(k, text))) => {
            format!("{} `{text}` at byte {at}", g.name(k))
        }
        Some((at, Event::Restyled)) => format!("a restyled token at byte {at}"),
    }
}

/// Runs the generic check and then the provider's own. `reparse` parses the output and returns
/// an error message if it has parse errors.
pub fn verify(
    g: &Grammar,
    formatter: &dyn NodeFormatter,
    before_tree: &SyntaxNode,
    after: &str,
    reparse: impl FnOnce(&str) -> Result<SyntaxNode, String>,
) -> Result<(), FormatError> {
    let after_tree = reparse(after)
        .map_err(|e| FormatError::Safety(format!("the output does not parse: {e}")))?;
    equal_ignoring_trivia(g, before_tree, &after_tree).map_err(FormatError::Safety)?;
    formatter
        .extra_check(before_tree, &after_tree)
        .map_err(FormatError::Safety)
}
