//! What the shared key rules ask of HTML: attribute names are case-insensitive.

use core_rules::RuleHooks;
use core_syntax::SyntaxElement;
use std::borrow::Cow;

pub struct HtmlHooks;

impl RuleHooks for HtmlHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        Cow::Owned(text[key.text_range()].trim().to_ascii_lowercase())
    }
}
