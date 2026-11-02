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
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::WalkEvent;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Enter(SyntaxKind),
    Leave(SyntaxKind),
    Token(SyntaxKind, String),
    /// A `#restyle` token: only its position is compared.
    Restyled,
}

fn events<'a>(g: &'a Grammar, root: &SyntaxNode) -> impl Iterator<Item = (usize, Event)> + use<'a> {
    let root = root.clone();
    root.preorder_with_tokens().filter_map(move |ev| match ev {
        WalkEvent::Enter(rowan::NodeOrToken::Node(n)) => {
            Some((n.text_range().start().into(), Event::Enter(n.kind())))
        }
        WalkEvent::Leave(rowan::NodeOrToken::Node(n)) => {
            Some((n.text_range().end().into(), Event::Leave(n.kind())))
        }
        WalkEvent::Enter(rowan::NodeOrToken::Token(t)) => significant(g, &t).then(|| {
            let event = if g.has_tag(t.kind(), "restyle") {
                Event::Restyled
            } else {
                Event::Token(t.kind(), t.text().to_string())
            };
            (t.text_range().start().into(), event)
        }),
        WalkEvent::Leave(rowan::NodeOrToken::Token(_)) => None,
    })
}

fn significant(g: &Grammar, t: &SyntaxToken) -> bool {
    !g.is_trivia(t.kind()) && !g.has_tag(t.kind(), "separator")
}

/// `Ok` when both trees have the same nodes and non-trivia tokens in the same order; else a
/// description of the first difference.
pub fn equal_ignoring_trivia(g: &Grammar, a: &SyntaxNode, b: &SyntaxNode) -> Result<(), String> {
    let (mut a, mut b) = (events(g, a), events(g, b));
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

fn describe(g: &Grammar, ev: Option<(usize, Event)>) -> String {
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
    before: &str,
    after: &str,
    reparse: impl FnOnce(&str) -> Result<SyntaxNode, String>,
) -> Result<(), FormatError> {
    let after_tree = reparse(after)
        .map_err(|e| FormatError::Safety(format!("the output does not parse: {e}")))?;
    equal_ignoring_trivia(g, before_tree, &after_tree).map_err(FormatError::Safety)?;
    formatter
        .extra_check(before, after)
        .map_err(FormatError::Safety)
}
