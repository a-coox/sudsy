//! Changes to the source text that the safety check could not follow if the formatter made them:
//! the generic check compares nodes and tokens, so a token the output adds or drops is added or
//! dropped here, and the tree is read again (`NodeFormatter::rewrite_source`).
//! - Semicolons that mean nothing (`};`, `;;`, a `;` after a class method) are dropped, as
//!   Prettier does.
//! - Parentheses around a type that its place does not need, and a leading `&` or lone `|` of a
//!   type (`format/ts/parens.rs`).
//! - `new Foo` becomes `new Foo()` and an arrow's lone parameter gets its parentheses.

use crate::generated::kind as k;
use core_format::comments;
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// A cut of the byte at `.0` or an insertion of `.1` before it.
type Edit = (usize, Option<&'static str>);

/// The text with the edits applied; `None` when there is none.
pub(crate) fn rewrite(tree: &SyntaxNode, text: &str, grammar: &Grammar) -> Option<String> {
    let mut edits: Vec<Edit> = Vec::new();
    let mut cut = |tok: SyntaxToken| {
        if comments::leading_comments(grammar, &tok).is_empty()
            && comments::trailing_comments(grammar, &tok).is_empty()
        {
            edits.push((usize::from(tok.text_range().start()), None));
        }
    };
    let semicolons = |node: &SyntaxNode| {
        node.children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::SEMICOLON)
            .collect::<Vec<_>>()
    };
    let mut inserts: Vec<Edit> = Vec::new();
    for node in tree.descendants() {
        match node.kind() {
            k::CLASS_BODY => semicolons(&node).into_iter().for_each(&mut cut),
            k::EMPTY_STATEMENT
                if node.parent().is_some_and(|p| {
                    matches!(
                        p.kind(),
                        k::PROGRAM
                            | k::BLOCK_STATEMENT
                            | k::FUNCTION_BODY
                            | k::SWITCH_CASE
                            | k::STATIC_BLOCK
                            | k::TS_MODULE_BLOCK
                    )
                }) =>
            {
                semicolons(&node).into_iter().for_each(&mut cut)
            }
            k::TS_PARENTHESIZED_TYPE | k::TS_UNION_TYPE | k::TS_INTERSECTION_TYPE => {
                super::ts::redundant_type_tokens(&node, grammar)
                    .into_iter()
                    .for_each(&mut cut)
            }
            k::NEW_EXPRESSION
                if !node
                    .children_with_tokens()
                    .any(|e| e.as_token().is_some_and(|t| t.kind() == k::L_PAREN)) =>
            {
                if let Some(last) = last_token(grammar, &node) {
                    inserts.push((usize::from(last.text_range().end()), Some("()")));
                }
            }
            k::FORMAL_PARAMETERS
                if node
                    .parent()
                    .is_some_and(|p| p.kind() == k::ARROW_FUNCTION_EXPRESSION)
                    && !node
                        .children_with_tokens()
                        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::L_PAREN)) =>
            {
                if let (Some(first), Some(last)) =
                    (first_token(grammar, &node), last_token(grammar, &node))
                {
                    inserts.push((usize::from(first.text_range().start()), Some("(")));
                    inserts.push((usize::from(last.text_range().end()), Some(")")));
                }
            }
            _ => {}
        }
    }
    edits.extend(inserts);
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|e| (e.0, e.1.is_none()));
    let mut out = String::with_capacity(text.len() + 8);
    let mut from = 0;
    for (at, insert) in edits {
        out.push_str(&text[from..at]);
        from = at;
        match insert {
            Some(s) => out.push_str(s),
            None => from = at + 1,
        }
    }
    out.push_str(&text[from..]);
    Some(out)
}

fn first_token(grammar: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| !grammar.is_trivia(t.kind()))
}

fn last_token(grammar: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
    let start = node.text_range().start();
    let mut cur = node.last_token();
    while let Some(t) = cur {
        if !grammar.is_trivia(t.kind()) {
            return Some(t);
        }
        cur = t.prev_token().filter(|p| p.text_range().start() >= start);
    }
    None
}
