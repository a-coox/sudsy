//! Helpers over the shared tree that several languages' rules need.

use core_syntax::{Grammar, SyntaxNode, SyntaxToken, TextRange};
use std::cell::Cell;

/// The span of a node from its first token that is not trivia to the end of its last one:
/// comments and blank space around the node are not part of it. Found from the two ends, so it
/// costs the trivia at the edges rather than the size of the node.
pub fn significant_range(node: &SyntaxNode, grammar: &Grammar) -> TextRange {
    let whole = node.text_range();
    let keep = |t: &SyntaxToken| {
        whole.contains_range(t.text_range())
            && !grammar.is_trivia(t.kind())
            && !t.text_range().is_empty()
    };
    let inside = |t: &SyntaxToken| whole.contains_range(t.text_range());
    let mut first = node.first_token();
    while let Some(t) = &first {
        if keep(t) || !inside(t) {
            break;
        }
        first = t.next_token();
    }
    let mut last = node.last_token();
    while let Some(t) = &last {
        if keep(t) || !inside(t) {
            break;
        }
        last = t.prev_token();
    }
    match (first, last) {
        (Some(first), Some(last)) if keep(&first) && keep(&last) => {
            TextRange::new(first.text_range().start(), last.text_range().end())
        }
        _ => whole,
    }
}

/// What the rules run on one file share: the last [`significant_range`] computed (the rules a
/// node is dispatched to ask for it one after the other; it depends on the node's range only, so
/// equal ranges share the answer) and whether the file text is ASCII.
#[derive(Default)]
pub struct TraversalCache {
    range: Cell<Option<(TextRange, TextRange)>>,
    ascii: Cell<Option<bool>>,
}

impl TraversalCache {
    pub fn significant_range(&self, node: &SyntaxNode, grammar: &Grammar) -> TextRange {
        let whole = node.text_range();
        match self.range.get() {
            Some((key, range)) if key == whole => range,
            _ => {
                let range = significant_range(node, grammar);
                self.range.set(Some((whole, range)));
                range
            }
        }
    }

    /// Is the file's `text` ASCII? Scanned on the first ask.
    pub fn ascii(&self, text: &str) -> bool {
        self.ascii.get().unwrap_or_else(|| {
            let ascii = text.is_ascii();
            self.ascii.set(Some(ascii));
            ascii
        })
    }
}

/// The start of the first token of `node` that is code: not trivia, and not a token of a
/// structured trivia node (a directive, a documentation comment), which [`significant_range`]
/// counts as code.
pub fn code_start(node: &SyntaxNode, grammar: &Grammar) -> core_syntax::TextSize {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| {
            !grammar.is_trivia(t.kind())
                && !t.text_range().is_empty()
                && !t
                    .parent_ancestors()
                    .take_while(|a| a != node)
                    .any(|a| grammar.is_trivia(a.kind()))
        })
        .map_or_else(
            || significant_range(node, grammar).start(),
            |t| t.text_range().start(),
        )
}

/// [`significant_range`] (from [`code_start`]) plus the comments that follow on the same line: what moves along when an
/// entry (a key, a using directive) is reordered. A comment on a line of its own stays where it is.
pub fn entry_range(node: &SyntaxNode, grammar: &Grammar) -> TextRange {
    let significant = significant_range(node, grammar);
    // The first token is code unless it belongs to a structured trivia node (rare).
    let in_trivia = node
        .token_at_offset(significant.start())
        .right_biased()
        .is_some_and(|t| {
            t.parent_ancestors()
                .take_while(|a| a != node)
                .any(|a| grammar.is_trivia(a.kind()))
        });
    let significant = if in_trivia {
        TextRange::new(code_start(node, grammar), significant.end())
    } else {
        significant
    };
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

/// Are the entries of `container` separated by a token (`#separator`, a comma)?
pub fn has_separator(container: &SyntaxNode, grammar: &Grammar) -> bool {
    container
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| grammar.has_tag(t.kind(), "separator"))
}

/// The range of `entry` that moves when the entries of its container are reordered.
/// `separated` is [`has_separator`] of the container: entries separated by a token move without
/// a trailing comment, which would swallow the separator that ends up behind it; entries without
/// separators (block style) take the comment after them along ([`entry_range`]).
pub fn reorder_range(entry: &SyntaxNode, separated: bool, grammar: &Grammar) -> TextRange {
    if separated {
        significant_range(entry, grammar)
    } else {
        entry_range(entry, grammar)
    }
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
            crate::Edit::replace(at, &text[from])
        })
        .collect()
}

/// The structured trivia node (documentation comment, directive) around `token`; its children
/// need not be trivia themselves (a documentation comment's XML).
pub fn structured_trivia(token: &SyntaxToken, grammar: &Grammar) -> Option<SyntaxNode> {
    token
        .parent_ancestors()
        .find(|n| grammar.is_trivia(n.kind()))
}

/// Is `token` code: neither trivia nor inside a structured trivia node?
pub fn is_code(token: &SyntaxToken, grammar: &Grammar) -> bool {
    !grammar.is_trivia(token.kind()) && structured_trivia(token, grammar).is_none()
}
