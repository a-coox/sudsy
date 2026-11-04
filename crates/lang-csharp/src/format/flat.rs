//! `preserve_single_line_blocks` and `preserve_single_line_statements`: a block or a compound
//! statement written on one line is printed on one line, everything below it with it.

use super::options::{preserve_single_line_blocks, preserve_single_line_statements};
use super::seq::R;
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::doc;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, TextRange};

/// Statements whose whole text is kept on its line by `preserve_single_line_statements`.
fn is_compound(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::IF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_STATEMENT
            | k::FOR_STATEMENT
            | k::FOR_EACH_STATEMENT
            | k::FOR_EACH_VARIABLE_STATEMENT
            | k::TRY_STATEMENT
            | k::USING_STATEMENT
            | k::LOCK_STATEMENT
            | k::FIXED_STATEMENT
            | k::CHECKED_STATEMENT
            | k::UNSAFE_STATEMENT
    )
}

/// Is `node` written on one line, without comments and without a `switch` or a label (which
/// always break lines)?
fn on_one_line(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return false;
    };
    let range = TextRange::new(first.text_range().start(), last.text_range().end());
    !cx.has_line_break(node, range)
        && !node.descendants_with_tokens().any(|e| match e {
            SyntaxElement::Node(n) => {
                matches!(n.kind(), k::SWITCH_STATEMENT | k::LABELED_STATEMENT)
            }
            SyntaxElement::Token(t) => cx.grammar.has_tag(t.kind(), "comment"),
        })
}

/// A block that `preserve_single_line_blocks` keeps on its line, or a compound statement that
/// `preserve_single_line_statements` keeps on its line.
pub fn is_root(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let kind = node.kind();
    let wanted = if kind == k::BLOCK {
        preserve_single_line_blocks(cx.options)
    } else {
        preserve_single_line_statements(cx.options) && is_compound(kind)
    };
    wanted && on_one_line(cx, node)
}

/// Is `node` itself or something around it a [`is_root`]: it prints on one line.
pub fn inside(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    (preserve_single_line_blocks(cx.options) || preserve_single_line_statements(cx.options))
        && node.ancestors().any(|a| is_root(cx, &a))
}

/// Is `statement`, written on one line, on the line of the token before it (`case 1: break;`)?
pub fn follows_on_line(cx: &FormatCtx<'_>, statement: &SyntaxNode) -> bool {
    let Some(first) = cx.first_token(statement) else {
        return false;
    };
    let before = first.prev_token().and_then(|mut t| {
        while cx.grammar.is_trivia(t.kind()) {
            t = t.prev_token()?;
        }
        Some(t)
    });
    before.is_some_and(|b| !core_format::comments::break_after(cx.grammar, &b))
        && core_format::comments::leading_comments(cx.grammar, &first).is_empty()
        && on_one_line(cx, statement)
}

/// `{ a; b; }` for a block that prints on one line (not an empty one).
pub fn block(cx: &FormatCtx<'_>, block: &SyntaxNode) -> R {
    let mut parts = vec![];
    for e in block.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if cx.grammar.is_trivia(t.kind()) => {}
            SyntaxElement::Node(n) if cx.grammar.is_trivia(n.kind()) => {}
            SyntaxElement::Token(t) if t.kind() == k::OPEN_BRACE_TOKEN => {
                parts.push(cx.token(&t));
            }
            SyntaxElement::Token(t) => {
                parts.push(doc::lit(" "));
                parts.push(cx.token(&t));
            }
            SyntaxElement::Node(n) => {
                parts.push(doc::lit(" "));
                parts.push(cx.format(&n)?);
            }
        }
    }
    Ok(doc::concat(parts))
}
