//! What the shared rules ask a language about its own syntax. A language that tags its grammar
//! (`#mapping`, `#sequence`) gets the shared key and sequence rules with no code of its own; a
//! language whose keys or scalars need decoding implements the hooks it needs.

use crate::rule::FileContext;
use core_syntax::{SyntaxElement, SyntaxNode, TextRange};
use std::borrow::Cow;

/// A scalar the sorting of sequence items understands.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar<'a> {
    String(Cow<'a, str>),
    Number(f64),
}

pub trait RuleHooks: Send + Sync {
    /// Whether a key is a name at all: `no-duplicate-keys` skips the others (HTML: template
    /// words inside a start tag, `{{ if }}`). The default is every key.
    fn is_name(&self, _name: &str) -> bool {
        true
    }

    /// The identity of a key (the `key` item of a `#key-value` entry): the name that is compared,
    /// sorted and checked. `text` is the file's text. The default is the key's text with one pair
    /// of matching quotes stripped.
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let range = key.text_range();
        let raw = text[range].trim();
        let inner = ['"', '\'']
            .iter()
            .find_map(|&q| raw.strip_prefix(q)?.strip_suffix(q));
        Cow::Borrowed(inner.unwrap_or(raw))
    }

    /// The separately named parts of a key that is a path (TOML's `a.b`), which naming conventions
    /// are checked on one by one; `None` when the key is one name (the default).
    fn key_parts<'a>(&self, _key: &SyntaxElement, _text: &'a str) -> Option<Vec<Cow<'a, str>>> {
        None
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

    /// Is the line a `key: value` entry whose value is one unbreakable word (no space)? Lets
    /// `max-line-length` accept it with `allow_non_breakable_inline_mappings`. The default has no
    /// such lines.
    fn inline_mapping_line(&self, _line: &str) -> bool {
        false
    }

    /// Rules the code silences for a range by its own syntax (PowerShell's
    /// `SuppressMessageAttribute`), as `(rule id, range)`. The default has none.
    fn suppressed_ranges(&self, _file: &FileContext<'_>) -> Vec<(String, TextRange)> {
        Vec::new()
    }
}

/// The hooks of a language that has none of its own.
pub struct DefaultHooks;

impl RuleHooks for DefaultHooks {}

pub static DEFAULT_HOOKS: DefaultHooks = DefaultHooks;
