//! What the shared rules ask a language about its own syntax. A language that tags its grammar
//! (`#mapping`, `#sequence`) gets the shared key and sequence rules with no code of its own; a
//! language whose keys or scalars need decoding implements the hooks it needs.

use crate::rule::FileContext;
use core_syntax::{GreenElement, SyntaxElement, SyntaxNode, TextRange, TextSize};
use std::borrow::Cow;

/// A key as the green tree has it: no red element is built to read it.
pub struct KeyRef<'a> {
    /// The `#key-value` entry the key is in, and where it starts in the file.
    pub entry: &'a core_syntax::GreenNodeData,
    pub entry_start: u32,
    pub key: GreenElement<'a>,
    /// Where the key starts in the file.
    pub start: u32,
}

impl KeyRef<'_> {
    pub fn range(&self) -> TextRange {
        let len = match self.key {
            GreenElement::Node(n) => n.text_len(),
            GreenElement::Token(t) => t.text_len(),
        };
        TextRange::at(TextSize::from(self.start), len)
    }
}

/// The name of the key of a red `entry` (for the rules that hold red nodes), through
/// [`RuleHooks::key_name`].
pub fn key_name_of<'a>(
    hooks: &dyn RuleHooks,
    entry: &SyntaxNode,
    key: &SyntaxElement,
    text: &'a str,
) -> Cow<'a, str> {
    let entry_green = entry.green();
    let key_node;
    let key_green = match key {
        SyntaxElement::Node(n) => {
            key_node = n.green();
            GreenElement::Node(&key_node)
        }
        SyntaxElement::Token(t) => GreenElement::Token(t.green()),
    };
    hooks.key_name(
        &KeyRef {
            entry: &entry_green,
            entry_start: entry.text_range().start().into(),
            key: key_green,
            start: key.text_range().start().into(),
        },
        text,
    )
}

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
    fn key_name<'a>(&self, key: &KeyRef<'_>, text: &'a str) -> Cow<'a, str> {
        let raw = text[key.range()].trim();
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
