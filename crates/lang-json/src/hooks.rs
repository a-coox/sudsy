//! What the shared rules ask of JSON: keys and scalar items have their escapes decoded.

use crate::generated::{Literal, kind};
use crate::string::string_value;
use core_rules::{RuleHooks, Scalar};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

pub struct JsonHooks;

impl RuleHooks for JsonHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        match key {
            SyntaxElement::Token(token) => string_value(&text[token.text_range()]),
            SyntaxElement::Node(_) => Cow::Borrowed(""),
        }
    }

    fn scalar<'a>(&self, item: &SyntaxNode, text: &'a str) -> Option<Scalar<'a>> {
        let token = Literal::cast(item.clone())?.value()?;
        match token.kind() {
            kind::STRING => Some(Scalar::String(string_value(&text[token.text_range()]))),
            kind::NUMBER => text[token.text_range()].parse().ok().map(Scalar::Number),
            _ => None,
        }
    }
}
