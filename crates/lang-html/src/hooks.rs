//! What the shared key rules ask of HTML: attribute names are case-insensitive, and a Vue
//! directive (`:a`, `@a`, `#a`) is one name whose first token is the key.

use core_rules::RuleHooks;
use core_syntax::SyntaxElement;
use std::borrow::Cow;

pub struct HtmlHooks;

impl RuleHooks for HtmlHooks {
    fn is_name(&self, key: &SyntaxElement, text: &str) -> bool {
        let start = usize::from(key.text_range().start());
        let tag = &text[text[..start].rfind('<').unwrap_or(0)..start];
        !in_template(tag) && !text[start..].starts_with(['{', '<', '}', '%'])
    }

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

/// Whether the end of `before` is inside a template tag (`{{ ... }}`, `{% ... %}`) that is still open.
pub fn in_template(before: &str) -> bool {
    let open = before.rfind("{{").max(before.rfind("{%"));
    let close = before.rfind("}}").max(before.rfind("%}"));
    open.is_some_and(|o| close.is_none_or(|c| c < o))
}
