//! Changes to the source text that the safety check could not follow if the formatter made them:
//! the generic check compares nodes and tokens, so a token the output adds or drops is added or
//! dropped here, and the tree is read again (`NodeFormatter::rewrite_source`).
//! - Semicolons that mean nothing (`};`, `;;`, a `;` after a class method) are dropped, as
//!   Prettier does.
//! - Parentheses around a type that its place does not need, and a leading `&` or lone `|` of a
//!   type (`format/ts/parens.rs`).
//! - `new Foo` becomes `new Foo()` and an arrow's lone parameter gets its parentheses.

use crate::generated::kind as k;
use core_config::{OptionValue, ResolvedOptions};
use core_format::comments;
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// A cut of the byte at `.0` or an insertion of `.1` before it.
type Edit = (usize, Option<&'static str>);

/// The text with the edits applied; `None` when there is none.
pub(crate) fn rewrite(
    tree: &SyntaxNode,
    text: &str,
    grammar: &Grammar,
    options: &ResolvedOptions,
) -> Option<String> {
    let avoid_parens =
        matches!(options.get("arrow_parens"), Some(OptionValue::String(s)) if s == "avoid");
    let mut edits: Vec<Edit> = Vec::new();
    // Comments in front of a cut token lead what follows it; ones after it would move.
    let mut cut = |tok: SyntaxToken| {
        edits.push((usize::from(tok.text_range().start()), None));
    };
    let semicolons = |node: &SyntaxNode| {
        node.children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::SEMICOLON)
            .collect::<Vec<_>>()
    };
    let mut inserts: Vec<Edit> = Vec::new();
    let mut later: Vec<Edit> = Vec::new();
    for node in tree.descendants() {
        // `) // c` then `{` on the next line becomes `) { // c`: the comment moves inside the block.
        if matches!(
            node.kind(),
            k::BLOCK_STATEMENT | k::FUNCTION_BODY | k::CLASS_BODY | k::TS_INTERFACE_BODY
        ) && let Some(open) = first_token(grammar, &node)
            && open.kind() == k::L_BRACE
            && comments::trailing_comments(grammar, &open).is_empty()
        {
            let mut between = Vec::new();
            let mut prev = open.prev_token();
            while let Some(t) = prev.clone().filter(|t| grammar.is_trivia(t.kind())) {
                prev = t.prev_token();
                between.push(t);
            }
            if let Some(prev) = prev
                && between.iter().any(|t| t.kind() == k::LINE_COMMENT)
                && between.iter().all(|t| t.kind() != k::BLOCK_COMMENT)
            {
                let brace = if node.kind() == k::TS_INTERFACE_BODY {
                    " {
"
                } else {
                    " {"
                };
                inserts.push((usize::from(prev.text_range().end()), Some(brace)));
                // The line break after the old `{` goes too, or a blank line would follow.
                if let Some(next) = open.next_token().filter(|t| t.kind() == k::NEWLINE) {
                    let at = usize::from(next.text_range().start());
                    let len = if next.text().starts_with("\r\n") {
                        2
                    } else {
                        1
                    };
                    (0..len).for_each(|i| later.push((at + i, None)));
                }
                cut(open);
            }
        }
        // `interface A { // c` puts the comment on a line of its own inside the body.
        if matches!(node.kind(), k::TS_INTERFACE_BODY | k::TS_TYPE_LITERAL)
            && let Some(open) = first_token(grammar, &node)
            && open.kind() == k::L_BRACE
            && comments::trailing_comments(grammar, &open)
                .first()
                .is_some_and(|c| c.kind == comments::CommentKind::Line)
        {
            inserts.push((usize::from(open.text_range().end()), Some("\n")));
        }
        // `a && (b && c)` is `a && b && c`: parentheses around a right operand of the same
        // logical operator go (the tree then has the shape of the output).
        if node.kind() == k::PARENTHESIZED_EXPRESSION
            && let Some(parent) = node.parent().filter(|p| p.kind() == k::LOGICAL_EXPRESSION)
            && parent.last_child().as_ref() == Some(&node)
            && let Some(inner) = node
                .first_child()
                .filter(|c| c.kind() == k::LOGICAL_EXPRESSION)
            && logical_operator(&parent) == logical_operator(&inner)
            && logical_operator(&inner).is_some()
        {
            node.children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| matches!(t.kind(), k::L_PAREN | k::R_PAREN))
                .for_each(&mut cut);
        }
        // `(x!) = y` is `x! = y`: the parentheses of an assignment target stay only where the
        // printer adds them back (a cast).
        if node.kind() == k::ASSIGNMENT_EXPRESSION
            && node.first_child().is_some_and(|t| {
                !matches!(
                    t.kind(),
                    k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION | k::TS_TYPE_ASSERTION
                )
            })
        {
            node.children_with_tokens()
                .filter_map(|e| e.into_token())
                .take_while(|t| t.kind() != k::OPERATOR)
                .filter(|t| matches!(t.kind(), k::L_PAREN | k::R_PAREN))
                .for_each(&mut cut);
        }
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
            // `x()\n// c\n;` is `x();` with the comment after it.
            k::EXPRESSION_STATEMENT => {
                let semi = node
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == k::SEMICOLON);
                if let Some(semi) = semi
                    && !comments::leading_comments(grammar, &semi).is_empty()
                    && comments::trailing_comments(grammar, &semi).is_empty()
                {
                    let mut prev = semi.prev_token();
                    while let Some(t) = prev.clone().filter(|t| grammar.is_trivia(t.kind())) {
                        prev = t.prev_token();
                    }
                    if let Some(prev) = prev {
                        inserts.push((usize::from(prev.text_range().end()), Some(";")));
                        cut(semi);
                    }
                }
            }
            // `typeof X[number]` is written `(typeof X)[number]`.
            k::TS_INDEXED_ACCESS_TYPE
                if node
                    .first_child()
                    .is_some_and(|c| c.kind() == k::TS_TYPE_QUERY) =>
            {
                if let (Some(first), Some(last)) = (
                    node.first_child().and_then(|c| first_token(grammar, &c)),
                    node.first_child().and_then(|c| last_token(grammar, &c)),
                ) {
                    inserts.push((usize::from(first.text_range().start()), Some("(")));
                    inserts.push((usize::from(last.text_range().end()), Some(")")));
                }
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
                if avoid_parens
                    && node.parent().is_some_and(|p| {
                        p.kind() == k::ARROW_FUNCTION_EXPRESSION
                            && !p.children().any(|c| {
                                matches!(
                                    c.kind(),
                                    k::TS_TYPE_ANNOTATION | k::TS_TYPE_PARAMETER_DECLARATION
                                )
                            })
                    })
                    && plain_lone_param(&node, grammar) =>
            {
                for t in node.children_with_tokens().filter_map(|e| e.into_token()) {
                    let at = usize::from(t.text_range().start());
                    // `async(x)` must not become `asyncx`.
                    if t.kind() == k::L_PAREN
                        && text[..at]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_alphanumeric)
                    {
                        inserts.push((at, Some(" ")));
                    }
                    cut(t);
                }
            }
            k::FORMAL_PARAMETERS
                if !avoid_parens
                    && node
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
    edits.extend(later);
    edits.extend(super::object::key_edits(tree, text, grammar, options));
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

/// `(x)`: parentheses around one plain name, with nothing else (no type, default, comment or
/// comma) between them.
fn plain_lone_param(params: &SyntaxNode, grammar: &Grammar) -> bool {
    let mut parts = params
        .children_with_tokens()
        .filter(|e| !grammar.is_trivia(e.kind()));
    let tok = |e: Option<core_syntax::SyntaxElement>, kind| {
        e.and_then(|e| e.into_token()).is_some_and(|t| {
            t.kind() == kind
                && comments::leading_comments(grammar, &t).is_empty()
                && comments::trailing_comments(grammar, &t).is_empty()
        })
    };
    tok(parts.next(), k::L_PAREN)
        && parts.next().and_then(|e| e.into_node()).is_some_and(|p| {
            p.kind() == k::FORMAL_PARAMETER
                && p.children_with_tokens()
                    .filter(|e| !grammar.is_trivia(e.kind()))
                    .count()
                    == 1
                && p.first_child()
                    .is_some_and(|c| c.kind() == k::BINDING_IDENTIFIER)
        })
        && tok(parts.next(), k::R_PAREN)
        && parts.next().is_none()
}

pub(super) fn first_token(grammar: &Grammar, node: &SyntaxNode) -> Option<SyntaxToken> {
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

/// The operator token of a logical expression.
fn logical_operator(node: &SyntaxNode) -> Option<String> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::OPERATOR)
        .map(|t| t.text().to_string())
}
