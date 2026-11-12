//! What the shared key rules ask of HTML: attribute names are case-insensitive, and a Vue
//! directive (`:a`, `@a`, `#a`) is one name whose first token is the key.

use core_rules::RuleHooks;
use core_syntax::SyntaxElement;
use std::borrow::Cow;

pub struct HtmlHooks;

impl RuleHooks for HtmlHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let range = key.text_range();
        let end = key.parent().map_or(range.end(), |p| p.text_range().end());
        let name = &text[usize::from(range.start())..usize::from(end)];
        let name = name
            .split(|c: char| c == '=' || c.is_whitespace())
            .next()
            .unwrap_or("");
        Cow::Owned(name.trim().to_ascii_lowercase())
    }
}
