//! What the shared rules ask of YAML: a key is its scalar's value (so `a`, `'a'` and `"a"` are the
//! same key), `<<` merges mappings, and plain scalars resolve to strings or numbers by the core
//! schema.

use crate::generated::{Scalar as ScalarNode, kind};
use crate::model::{Value, resolve_plain};
use crate::scalar::{double_quoted_value, plain_value, single_quoted_value};
use core_rules::{KeyRef, RuleHooks, Scalar};
use core_syntax::ast::AstNode;
use core_syntax::{GreenElement, GreenNodeData, SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize, green_code_span};
use std::borrow::Cow;

pub struct YamlHooks;

fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, kind::WHITESPACE | kind::NEWLINE | kind::COMMENT)
}

/// The first string token of a green scalar node that starts at `start`, with its range.
fn scalar_token(node: &GreenNodeData, start: u32) -> Option<(SyntaxKind, TextRange)> {
    let mut at = start;
    for child in node.children() {
        let len = child.text_len();
        if let GreenElement::Token(t) = child
            && matches!(SyntaxKind(t.kind().0), kind::PLAIN | kind::SINGLE_QUOTED | kind::DOUBLE_QUOTED)
        {
            return Some((SyntaxKind(t.kind().0), TextRange::at(TextSize::from(at), len)));
        }
        at += u32::from(len);
    }
    None
}

/// The value of a scalar node: `None` for a quoted scalar with an invalid escape.
fn scalar_value(kind: SyntaxKind, raw: &str) -> Option<Cow<'_, str>> {
    match kind {
        kind::PLAIN if !raw.contains(['\n', '\r']) => Some(Cow::Borrowed(raw)),
        kind::PLAIN => Some(Cow::Owned(plain_value(raw))),
        kind::SINGLE_QUOTED => Some(Cow::Owned(single_quoted_value(raw))),
        kind::DOUBLE_QUOTED => double_quoted_value(raw).map(Cow::Owned),
        _ => None,
    }
}

impl RuleHooks for YamlHooks {
    fn key_name<'a>(&self, key: &KeyRef<'_>, text: &'a str) -> Cow<'a, str> {
        let GreenElement::Node(node) = key.key else {
            return Cow::Borrowed("");
        };
        if SyntaxKind(node.kind().0) == kind::SCALAR
            && let Some((token, range)) = scalar_token(node, key.start)
            && let Some(value) = scalar_value(token, &text[range])
        {
            return value;
        }
        // Any other key (a collection, an anchored or tagged scalar): its text without the
        // comments and blank space around it.
        match green_code_span(node, TextSize::from(key.start), &is_trivia) {
            Some(range) => Cow::Borrowed(&text[range]),
            None => Cow::Borrowed(""),
        }
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
        let value = scalar_value(token.kind(), &text[token.text_range()])?;
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
