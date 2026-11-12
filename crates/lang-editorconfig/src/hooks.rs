//! What the shared rules ask of EditorConfig: keys are case-insensitive.

use core_rules::{KeyRef, RuleHooks};
use std::borrow::Cow;

pub struct EditorConfigHooks;

impl RuleHooks for EditorConfigHooks {
    fn key_name<'a>(&self, key: &KeyRef<'_>, text: &'a str) -> Cow<'a, str> {
        Cow::Owned(text[key.range()].trim().to_ascii_lowercase())
    }
}
