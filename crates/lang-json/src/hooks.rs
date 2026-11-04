//! What the shared rules ask of JSON: keys and scalar items have their escapes decoded.

use crate::generated::{Literal, kind};
use crate::string::string_value;
use core_rules::{RuleHooks, Scalar};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};
use std::borrow::Cow;

pub struct JsonHooks;

/// The token's text, borrowed from the file text (which outlives the tree walk).
fn text_of<'a>(token: &SyntaxToken, text: &'a str) -> &'a str {
    let range = token.text_range();
    &text[usize::from(range.start())..usize::from(range.end())]
}

impl RuleHooks for JsonHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        match key {
            SyntaxElement::Token(token) => string_value(text_of(token, text)),
            SyntaxElement::Node(_) => Cow::Borrowed(""),
        }
    }

    fn scalar<'a>(&self, item: &SyntaxNode, text: &'a str) -> Option<Scalar<'a>> {
        let token = Literal::cast(item.clone())?.value()?;
        match token.kind() {
            kind::STRING => Some(Scalar::String(string_value(text_of(&token, text)))),
            kind::NUMBER => text_of(&token, text).parse().ok().map(Scalar::Number),
            _ => None,
        }
    }
}
