//! What the shared rules ask of YAML: a key is its scalar's value (so `a`, `'a'` and `"a"` are the
//! same key), `<<` merges mappings, and plain scalars resolve to strings or numbers by the core
//! schema.

use crate::generated::{Scalar as ScalarNode, kind};
use crate::model::{Value, resolve_plain};
use crate::scalar::{double_quoted_value, plain_value, single_quoted_value};
use core_rules::{RuleHooks, Scalar};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use std::borrow::Cow;

pub struct YamlHooks;

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, kind::WHITESPACE | kind::NEWLINE | kind::COMMENT)
}

/// The value of a scalar node: `None` for a quoted scalar with an invalid escape.
fn scalar_value<'a>(token: &SyntaxToken, text: &'a str) -> Option<Cow<'a, str>> {
    let range = token.text_range();
    let raw = &text[range];
    match token.kind() {
        kind::PLAIN if !raw.contains(['\n', '\r']) => Some(Cow::Borrowed(raw)),
        kind::PLAIN => Some(Cow::Owned(plain_value(raw))),
        kind::SINGLE_QUOTED => Some(Cow::Owned(single_quoted_value(raw))),
        kind::DOUBLE_QUOTED => double_quoted_value(raw).map(Cow::Owned),
        _ => None,
    }
}

impl RuleHooks for YamlHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let SyntaxElement::Node(node) = key else {
            return Cow::Borrowed("");
        };
        if let Some(token) = ScalarNode::cast(node.clone()).and_then(|s| s.value())
            && let Some(value) = scalar_value(&token, text)
        {
            return value;
        }
        // Any other key (a collection, an anchored or tagged scalar): its text without the
        // comments and blank space around it.
        let mut code = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !is_trivia(t.kind()) && !t.text_range().is_empty());
        let Some(first) = code.next() else {
            return Cow::Borrowed("");
        };
        let last = code.last().unwrap_or_else(|| first.clone());
        let (start, end) = (first.text_range().start(), last.text_range().end());
        Cow::Borrowed(&text[usize::from(start)..usize::from(end)])
    }

    fn merge_key(&self) -> Option<&str> {
        Some("<<")
    }

    fn inline_mapping_line(&self, line: &str) -> bool {
        let mut body = line.trim_start_matches(' ');
        while let Some(rest) = body.strip_prefix("- ") {
            body = rest.trim_start_matches(' ');
        }
        // shortcut: a quoted key containing `: ` is split early, which only errs towards reporting.
        let Some((_, value)) = body.split_once(": ") else {
            return false;
        };
        let value = value.trim_start_matches(' ');
        !value.is_empty() && !value.contains(' ')
    }

    fn scalar<'a>(&self, item: &SyntaxNode, text: &'a str) -> Option<Scalar<'a>> {
        let token = ScalarNode::cast(item.clone())?.value()?;
        let value = scalar_value(&token, text)?;
        if token.kind() != kind::PLAIN {
            return Some(Scalar::String(value));
        }
        match resolve_plain(&value) {
            None => Some(Scalar::String(value)),
            Some(Value::Int(i)) => Some(Scalar::Number(i as f64)),
            Some(Value::Float(f)) => Some(Scalar::Number(f)),
            // null and booleans are neither strings nor numbers
            Some(_) => None,
        }
    }
}
