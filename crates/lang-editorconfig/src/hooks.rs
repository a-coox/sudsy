//! What the shared rules ask of EditorConfig: keys are case-insensitive.

use core_rules::RuleHooks;
use core_syntax::SyntaxElement;
use std::borrow::Cow;

pub struct EditorConfigHooks;

impl RuleHooks for EditorConfigHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        Cow::Owned(text[key.text_range()].trim().to_ascii_lowercase())
    }
}
