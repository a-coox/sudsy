//! Helpers over the shared tree that several languages' rules need.

use core_syntax::{Grammar, SyntaxNode, TextRange};

/// The span of a node from its first token that is not trivia to the end of its last one:
/// comments and blank space around the node are not part of it.
pub fn significant_range(node: &SyntaxNode, grammar: &Grammar) -> TextRange {
    let mut tokens = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !grammar.is_trivia(t.kind()) && !t.text_range().is_empty());
    match tokens.next() {
        Some(first) => {
            let last = tokens.last().unwrap_or_else(|| first.clone());
            TextRange::new(first.text_range().start(), last.text_range().end())
        }
        None => node.text_range(),
    }
}

/// [`significant_range`] plus the comments that follow on the same line: what moves along when an
/// entry (a key, a using directive) is reordered. A comment on a line of its own stays where it is.
pub fn entry_range(node: &SyntaxNode, grammar: &Grammar) -> TextRange {
    let significant = significant_range(node, grammar);
    let last = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !grammar.is_trivia(t.kind()) && !t.text_range().is_empty())
        .last();
    let mut end = significant.end();
    let mut next = last.and_then(|t| t.next_token());
    while let Some(token) = next {
        if !grammar.is_trivia(token.kind()) || token.text().contains(['\n', '\r']) {
            break;
        }
        if grammar.has_tag(token.kind(), "comment") {
            end = token.text_range().end();
        }
        next = token.next_token();
    }
    TextRange::new(significant.start(), end)
}

/// Fix edits that put `items` (ranges of nodes in the source) into the order `sorted`, which lists
/// indices into `items`: each position takes the text of the item that belongs there.
pub fn reorder_edits(text: &str, items: &[TextRange], sorted: &[usize]) -> Vec<crate::Edit> {
    items
        .iter()
        .zip(sorted)
        .enumerate()
        .filter(|(i, (_, s))| i != *s)
        .map(|(_, (&at, &s))| {
            let from = items[s];
            crate::Edit::replace(
                at,
                &text[usize::from(from.start())..usize::from(from.end())],
            )
        })
        .collect()
}
