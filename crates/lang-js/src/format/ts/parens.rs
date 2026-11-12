//! Tokens of a type that its place does not need: parentheses, and the leading `&` of an
//! intersection or the bar of a union with one member. They come out of the source before
//! formatting (`NodeFormatter::rewrite_source`), as the safety check compares the nodes.

use crate::generated::kind as k;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};

const PLAIN: &[SyntaxKind] = &[
    k::TS_TYPE_REFERENCE,
    k::TS_LITERAL_TYPE,
    k::TS_TYPE_LITERAL,
    k::TS_TUPLE_TYPE,
    k::TS_MAPPED_TYPE,
    k::TS_ARRAY_TYPE,
    k::TS_INDEXED_ACCESS_TYPE,
    k::TS_TEMPLATE_LITERAL_TYPE,
    k::TS_IMPORT_TYPE,
    k::TS_THIS_TYPE,
    k::TS_ANY_KEYWORD,
    k::TS_STRING_KEYWORD,
    k::TS_BOOLEAN_KEYWORD,
    k::TS_NUMBER_KEYWORD,
    k::TS_NEVER_KEYWORD,
    k::TS_UNKNOWN_KEYWORD,
    k::TS_NULL_KEYWORD,
    k::TS_UNDEFINED_KEYWORD,
    k::TS_VOID_KEYWORD,
    k::TS_SYMBOL_KEYWORD,
    k::TS_OBJECT_KEYWORD,
    k::TS_BIG_INT_KEYWORD,
    k::TS_PARENTHESIZED_TYPE,
];

/// The tokens of `node` (a parenthesized, union or intersection type) that can go, none when a
/// comment is attached to one of them.
pub(crate) fn redundant(node: &SyntaxNode, grammar: &Grammar) -> Vec<SyntaxToken> {
    let token = |kind| {
        node.children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == kind)
    };
    let tokens: Vec<SyntaxToken> = match node.kind() {
        k::TS_UNION_TYPE if node.children().count() == 1 => token(k::PIPE).into_iter().collect(),
        k::TS_INTERSECTION_TYPE => node
            .children_with_tokens()
            .find(|e| !grammar.is_trivia(e.kind()))
            .and_then(|e| e.into_token())
            .filter(|t| t.text() == "&")
            .into_iter()
            .collect(),
        k::TS_PARENTHESIZED_TYPE if parens_redundant(node) => token(k::L_PAREN)
            .into_iter()
            .chain(token(k::R_PAREN))
            .collect(),
        _ => Vec::new(),
    };
    // The comments next to a token that goes stay in the text and lead what follows.
    tokens
}

fn parens_redundant(node: &SyntaxNode) -> bool {
    let (Some(inner), Some(parent)) = (node.children().next(), node.parent()) else {
        return false;
    };
    // Where any type stands without parentheses: a union, function or conditional type needs
    // them only inside another type.
    let free = match parent.kind() {
        k::TS_TYPE_ALIAS_DECLARATION
        | k::TS_TYPE_PARAMETER_INSTANTIATION
        | k::TS_TUPLE_TYPE
        | k::TS_NAMED_TUPLE_MEMBER
        | k::TS_REST_TYPE
        | k::TS_MAPPED_TYPE
        | k::TS_AS_EXPRESSION
        | k::TS_SATISFIES_EXPRESSION => true,
        // A branch, not the checked or extended type.
        k::TS_CONDITIONAL_TYPE => parent.children().take(2).all(|c| c != *node),
        k::TS_TYPE_ANNOTATION => parent.parent().is_some_and(|p| {
            matches!(
                p.kind(),
                k::TS_FUNCTION_TYPE
                    | k::TS_CONSTRUCTOR_TYPE
                    | k::TS_PROPERTY_SIGNATURE
                    | k::VARIABLE_DECLARATOR
                    | k::FORMAL_PARAMETER
                    | k::PROPERTY_DEFINITION
            )
        }),
        _ => false,
    };
    PLAIN.contains(&inner.kind()) || free
}
