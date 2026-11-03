//! The space (or none) between two neighbouring tokens that no other rule decides.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, SyntaxToken};

/// Words that take a space before an opening parenthesis (`if (`, `return (`, `var (`).
const SPACE_BEFORE_PAREN: &[&str] = &[
    "if", "while", "for", "foreach", "switch", "lock", "using", "catch", "fixed", "when", "return",
    "throw", "await", "in", "is", "and", "or", "not", "case", "yield", "else", "do", "var",
    "delegate", "select", "where", "equals", "by", "on", "group", "orderby", "let", "from", "join",
    "into", "ref", "out", "with", "async", "static",
];

fn ends_word(text: &str) -> bool {
    text.chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '"' | '\'' | '@'))
}

fn parent_kind(t: &SyntaxToken) -> Option<SyntaxKind> {
    t.parent().map(|p| p.kind())
}

/// Is there a space between `prev` and `next`, two significant tokens in the same sequence of a
/// node of kind `parent`?
pub fn space(prev: &SyntaxToken, next: &SyntaxToken, parent: SyntaxKind) -> bool {
    let (p, n) = (prev.text(), next.text());
    if parent == k::FOR_STATEMENT && p == ";" && n == ";" {
        return true; // `for (; ;)`
    }
    match n {
        ")" | "]" | "," | ";" | "." | "?." | "::" | "->" | "++" | "--" | ".." => return false,
        _ => {}
    }
    if p == ".." && parent_kind(prev) == Some(k::SPREAD_ELEMENT) {
        return true; // `[.. items]`
    }
    match p {
        "(" | "[" | "." | "?." | "::" | "->" | "~" | ".." => return false,
        "," | ";" => return true,
        _ => {}
    }
    match n {
        "(" => {
            if parent_kind(prev) == Some(k::OPERATOR_DECLARATION) {
                return false; // `operator ==(`
            }
            return if ends_word(p) {
                SPACE_BEFORE_PAREN.contains(&p) || parent_kind(next) == Some(k::TUPLE_TYPE)
            } else {
                !matches!(p, ")" | "]" | ">" | "!" | "?")
            };
        }
        "[" => {
            return if ends_word(p) {
                SPACE_BEFORE_PAREN.contains(&p)
            } else {
                !matches!(p, ")" | "]" | ">" | "?" | "!")
            };
        }
        "<" => {
            if matches!(
                parent_kind(next),
                Some(
                    k::TYPE_ARGUMENT_LIST
                        | k::TYPE_PARAMETER_LIST
                        | k::FUNCTION_POINTER_PARAMETER_LIST
                )
            ) {
                return false;
            }
        }
        ":" => {
            return matches!(
                parent_kind(next),
                Some(
                    k::BASE_LIST
                        | k::TYPE_PARAMETER_CONSTRAINT_CLAUSE
                        | k::BASE_CONSTRUCTOR_INITIALIZER
                        | k::THIS_CONSTRUCTOR_INITIALIZER
                )
            );
        }
        "?" => {
            return !matches!(
                parent_kind(next),
                Some(k::NULLABLE_TYPE | k::CLASS_CONSTRAINT | k::STRUCT_CONSTRAINT)
            );
        }
        "*" => {
            if matches!(
                parent_kind(next),
                Some(k::POINTER_TYPE | k::FUNCTION_POINTER_TYPE)
            ) {
                return false;
            }
        }
        _ => {}
    }
    if p == ")" && parent == k::CAST_EXPRESSION {
        return false;
    }
    true
}
