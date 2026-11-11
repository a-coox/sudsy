//! Semicolons that mean nothing (`};`, `;;`, a `;` after a class method) are dropped, as
//! Prettier does. The generic safety check compares nodes, so the `;` comes out of the source
//! text and the tree is read again (`NodeFormatter::rewrite_source`) rather than the formatter
//! leaving the node out.

use crate::generated::kind as k;
use core_format::comments;
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// The text without the `;` of every empty statement that stands in a statement list and of
/// every `;` between class members, when it has no comment; `None` when there is none.
pub(crate) fn without_stray_semicolons(
    tree: &SyntaxNode,
    text: &str,
    grammar: &Grammar,
) -> Option<String> {
    let mut cuts: Vec<usize> = Vec::new();
    let mut cut = |tok: SyntaxToken| {
        if comments::leading_comments(grammar, &tok).is_empty()
            && comments::trailing_comments(grammar, &tok).is_empty()
        {
            cuts.push(usize::from(tok.text_range().start()));
        }
    };
    let semicolons = |node: &SyntaxNode| {
        node.children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::SEMICOLON)
            .collect::<Vec<_>>()
    };
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
            _ => {}
        }
    }
    if cuts.is_empty() {
        return None;
    }
    cuts.sort_unstable();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for at in cuts {
        out.push_str(&text[from..at]);
        from = at + 1;
    }
    out.push_str(&text[from..]);
    Some(out)
}
