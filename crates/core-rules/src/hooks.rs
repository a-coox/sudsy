//! What the shared rules ask a language about its own syntax. A language that tags its grammar
//! (`#mapping`, `#sequence`) gets the shared key and sequence rules with no code of its own; a
//! language whose keys or scalars need decoding implements the hooks it needs.

use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

/// A scalar the sorting of sequence items understands.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar<'a> {
    String(Cow<'a, str>),
    Number(f64),
}

pub trait RuleHooks: Send + Sync {
    /// The identity of a key (the `key` item of a `#key-value` entry): the name that is compared,
    /// sorted and checked. `text` is the file's text. The default is the key's text with one pair
    /// of matching quotes stripped.
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let range = key.text_range();
        let raw = text[usize::from(range.start())..usize::from(range.end())].trim();
        let inner = ['"', '\'']
            .iter()
            .find_map(|&q| raw.strip_prefix(q)?.strip_suffix(q));
        Cow::Borrowed(inner.unwrap_or(raw))
    }

    /// A key that merges other mappings into its own and may therefore repeat (YAML's `<<`).
    fn merge_key(&self) -> Option<&str> {
        None
    }

    /// The value of a string or number item of a sequence (`item` is the value node); `None` for
    /// anything else, which `sorted-array-values` leaves alone. The default has no scalars.
    fn scalar<'a>(&self, _item: &SyntaxNode, _text: &'a str) -> Option<Scalar<'a>> {
        None
    }
}

/// The hooks of a language that has none of its own.
pub struct DefaultHooks;

impl RuleHooks for DefaultHooks {}

pub static DEFAULT_HOOKS: DefaultHooks = DefaultHooks;
