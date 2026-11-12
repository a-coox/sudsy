//! What the shared key rules ask of GraphQL: the name of an entry is its name without the `$` of
//! a variable, and a field with an alias is known by the alias (its response name).

use crate::generated::kind;
use core_rules::{KeyRef, RuleHooks};
use core_syntax::{GreenElement, GreenNodeData, SyntaxKind, TextRange, TextSize};
use std::borrow::Cow;

pub struct GraphqlHooks;

/// The first token of a node, as `(start, length)` relative to the node.
fn first_token(node: &GreenNodeData, at: &mut u32) -> Option<(u32, u32)> {
    for child in node.children() {
        match child {
            GreenElement::Node(n) => {
                if let Some(found) = first_token(n, at) {
                    return Some(found);
                }
            }
            GreenElement::Token(t) => {
                let start = *at;
                *at += u32::from(t.text_len());
                return Some((start, *at - start));
            }
        }
    }
    None
}

/// The first token of the alias of a field entry.
fn alias_range(key: &KeyRef<'_>) -> Option<TextRange> {
    if SyntaxKind(key.entry.kind().0) != kind::FIELD {
        return None;
    }
    let mut at = key.entry_start;
    for child in key.entry.children() {
        match child {
            GreenElement::Node(n) if SyntaxKind(n.kind().0) == kind::ALIAS => {
                let (start, len) = first_token(n, &mut at)?;
                return Some(TextRange::at(TextSize::from(start), TextSize::from(len)));
            }
            _ => at += u32::from(child.text_len()),
        }
    }
    None
}

impl RuleHooks for GraphqlHooks {
    fn key_name<'a>(&self, key: &KeyRef<'_>, text: &'a str) -> Cow<'a, str> {
        let alias = matches!(key.key, GreenElement::Token(_))
            .then(|| alias_range(key))
            .flatten();
        let range = alias.unwrap_or_else(|| key.range());
        let name = text[range].trim();
        Cow::Borrowed(name.strip_prefix('$').unwrap_or(name))
    }
}
