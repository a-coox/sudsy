//! Rules on the keys of mappings, written against the `#mapping` and `#key-value` grammar tags and
//! the language's [`RuleHooks`](crate::hooks::RuleHooks): `no-duplicate-keys`, `no-empty-keys` and
//! `key-name-casing`.

use crate::options::{Check, Opt, OptionDecl};
use crate::rule;
use crate::rule::RuleContext;
use crate::sorting;
use crate::tree::significant_range;
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, TextRange};
use std::borrow::Cow;
use std::collections::HashMap;

/// The entries of a `#mapping` node that have a key, with their keys.
pub fn entries<'a>(
    mapping: &SyntaxNode,
    grammar: &'a Grammar,
) -> impl Iterator<Item = (SyntaxNode, SyntaxElement)> + 'a {
    mapping
        .children()
        .filter(|c| grammar.has_tag(c.kind(), "key-value"))
        .filter_map(|entry| {
            let key = grammar.slot(&entry, "key")?;
            Some((entry, key))
        })
}

/// [`entries`] collected, for the rules that index into them.
pub fn keyed(mapping: &SyntaxNode, grammar: &Grammar) -> Vec<(SyntaxNode, SyntaxElement)> {
    entries(mapping, grammar).collect()
}

/// Where a key is reported: its token, or its node without the comments around it.
pub fn key_range(key: &SyntaxElement, grammar: &Grammar) -> TextRange {
    match key {
        SyntaxElement::Node(node) => significant_range(node, grammar),
        SyntaxElement::Token(token) => token.text_range(),
    }
}

rule! {
    pub NoDuplicateKeys {
        id: "no-duplicate-keys",
        category: Correctness,
        default: Error,
        description: "A mapping has the same key twice (after decoding escapes and quotes); parsers \
                  disagree on which value wins.",
        example: "{ \"a\": 1, \"a\": 2 }   // reported at the second \"a\"",
        options: [OptionDecl {
            name: "merge_keys",
            default: Opt::Str("allow"),
            check: Check::OneOf(&["allow", "forbid"]),
            description: "Languages with a merge key (YAML `<<`) only. `allow` skips merge keys, \
                      which may repeat; `forbid` reports a second one like any other duplicate key.",
        }],
        tags: &["mapping"],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
        let merge_key = hooks
            .merge_key()
            .filter(|_| ctx.options.string("merge_keys") == "allow");
        // Ranges are found only for the keys that are reported (finding one reads the tree), so
        // the first occurrence is kept as its index and looked up again on a duplicate.
        let mut seen = Seen::default();
        for (index, (_, key)) in entries(node, grammar).enumerate() {
            let name = hooks.key_name(&key, text);
            if merge_key == Some(&*name) || !hooks.is_name(&key, text) {
                continue;
            }
            if let Some((first, name)) = seen.first(name, index) {
                let first = entries(node, grammar)
                    .nth(first)
                    .map_or_else(|| key_range(&key, grammar), |(_, k)| key_range(&k, grammar));
                ctx.report(key_range(&key, grammar), format!("duplicate key `{name}`"))
                    .related(first, "first defined here");
            }
        }
    }
}

const SMALL: usize = 8;

/// The entry that first used each name: a linear scan for the few entries most mappings have, a
/// hash map beyond that (hashing and growing a map cost more than comparing a handful of names).
#[derive(Default)]
struct Seen<'a> {
    small: [(Cow<'a, str>, usize); SMALL],
    len: usize,
    map: HashMap<Cow<'a, str>, usize>,
}

impl<'a> Seen<'a> {
    /// The index of the earlier entry named `name`, with the name handed back; records `name` as
    /// `index` when there is none.
    fn first(&mut self, name: Cow<'a, str>, index: usize) -> Option<(usize, Cow<'a, str>)> {
        if self.map.is_empty() {
            if let Some((_, first)) = self.small[..self.len].iter().find(|(n, _)| *n == name) {
                let first = *first;
                return Some((first, name));
            }
            if self.len < SMALL {
                self.small[self.len] = (name, index);
                self.len += 1;
                return None;
            }
            let small = self.small.iter_mut().map(|(n, i)| (std::mem::take(n), *i));
            self.map.extend(small);
        }
        if let Some(&first) = self.map.get(&name) {
            return Some((first, name));
        }
        self.map.insert(name, index);
        None
    }
}

rule! {
    pub NoEmptyKeys {
        id: "no-empty-keys",
        category: Suspicious,
        default: Off,
        description: "A key is empty or only whitespace (after decoding escapes); some consumers \
                  cannot address such a property.",
        example: "{ \"\": 1, \" \": 2 }   // both keys reported",
        tags: &["mapping"],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
        for (_, key) in entries(node, grammar) {
            if hooks.key_name(&key, text).trim().is_empty() {
                ctx.report(key_range(&key, grammar), "empty key");
            }
        }
    }
}

const CONVENTIONS: [&str; 5] = [
    "camelCase",
    "PascalCase",
    "snake_case",
    "kebab-case",
    "SCREAMING_SNAKE_CASE",
];

fn has_symbol(c: char) -> bool {
    matches!(c, '\u{21}'..='\u{23}' | '\u{25}'..='\u{2c}' | '.' | '/' | '\u{3a}'..='\u{40}'
        | '\u{5b}'..='\u{5e}' | '`' | '\u{7b}'..='\u{7d}')
}

fn matches_convention(convention: &str, key: &str) -> bool {
    if key.chars().any(|c| has_symbol(c) || c.is_whitespace()) {
        return false;
    }
    let upper = key.chars().any(|c| c.is_ascii_uppercase());
    let lower = key.chars().any(|c| c.is_ascii_lowercase());
    let separators = key.contains(['-', '_']);
    let first = key.chars().next();
    match convention {
        "camelCase" => !first.is_some_and(|c| c.is_ascii_uppercase()) && !separators,
        "PascalCase" => !first.is_some_and(|c| c.is_ascii_lowercase()) && !separators,
        "snake_case" => !upper && !key.contains('-') && !key.contains("__"),
        "kebab-case" => {
            !upper && !key.starts_with('-') && !key.contains('_') && !key.contains("--")
        }
        "SCREAMING_SNAKE_CASE" => !lower && !key.contains('-') && !key.contains("__"),
        _ => false,
    }
}

rule! {
    pub KeyNameCasing {
        id: "key-name-casing",
        category: Style,
        default: Off,
        description: "A key does not follow one of the allowed naming conventions. The checks are \
                  those of eslint-plugin-jsonc: a key with punctuation or whitespace never \
                  matches, `camelCase` also rejects `_` and `-`, `snake_case` and `kebab-case` \
                  reject capitals. No fix: renaming a key changes the data.",
        example: "{ \"first_name\": 1 }   // reported with the default `camelCase`",
        options: [
            OptionDecl {
                name: "allowed",
                default: Opt::List(&["camelCase"]),
                check: Check::OneOf(&CONVENTIONS),
                description: "Accepted conventions: `camelCase`, `PascalCase`, `snake_case`, \
                          `kebab-case`, `SCREAMING_SNAKE_CASE`. A key is fine when it matches one.",
            },
            sorting::IGNORED_KEYS,
    ],
        tags: &["mapping"],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
        let allowed = ctx.options.list("allowed");
        let ignored: Vec<_> = ctx
            .options
            .list("ignored_keys")
            .into_iter()
            .filter_map(sorting::cached_regex)
            .collect();
        if allowed.is_empty() {
            return;
        }
        for (_, key) in entries(node, grammar) {
            let parts = hooks.key_parts(&key, text);
            let whole;
            let names: &[Cow<'_, str>] = match &parts {
                Some(parts) => parts,
                None => {
                    whole = [hooks.key_name(&key, text)];
                    &whole
                }
            };
            let Some(name) = names.iter().find(|name| {
                !ignored.iter().any(|r| r.is_match(name))
                    && !allowed.iter().any(|c| matches_convention(c, name))
            }) else {
                continue;
            };
            ctx.report(
                key_range(&key, grammar),
                format!(
                    "key `{name}` must match one of the following formats: {}",
                    allowed.join(", ")
                ),
            );
        }
    }
}
