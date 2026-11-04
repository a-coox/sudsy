//! Rules on the keys of mappings, written against the `#mapping` and `#key-value` grammar tags and
//! the language's [`RuleHooks`](crate::hooks::RuleHooks): `no-duplicate-keys`, `no-empty-keys` and
//! `key-name-casing`.

use crate::diagnostic::Category;
use crate::options::{Check, Opt, OptionDecl};
use crate::rule::{Rule, RuleContext, RuleMeta};
use crate::sorting;
use crate::tree::significant_range;
use core_config::Level;
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

pub struct NoDuplicateKeys;

static NO_DUPLICATE_KEYS: RuleMeta = RuleMeta {
    id: "no-duplicate-keys",
    category: Category::Correctness,
    description: "A mapping has the same key twice (after decoding escapes and quotes); parsers \
                  disagree on which value wins.",
    example: "{ \"a\": 1, \"a\": 2 }   // reported at the second \"a\"",
    default_level: Level::Error,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "merge_keys",
        default: Opt::Str("allow"),
        check: Check::OneOf(&["allow", "forbid"]),
        description: "Languages with a merge key (YAML `<<`) only. `allow` skips merge keys, \
                      which may repeat; `forbid` reports a second one like any other duplicate key.",
    }],
};

impl Rule for NoDuplicateKeys {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_KEYS
    }

    fn tags(&self) -> &[&'static str] {
        &["mapping"]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
        let merge_key = hooks
            .merge_key()
            .filter(|_| ctx.options.string("merge_keys") == "allow");
        let mut seen: HashMap<Cow<'_, str>, TextRange> = HashMap::new();
        for (_, key) in entries(node, grammar) {
            let name = hooks.key_name(&key, text);
            if merge_key == Some(&*name) {
                continue;
            }
            let range = key_range(&key, grammar);
            match seen.get(&name) {
                Some(&first) => {
                    ctx.report(range, format!("duplicate key `{name}`"))
                        .related(first, "first defined here");
                }
                None => {
                    seen.insert(name, range);
                }
            }
        }
    }
}

pub struct NoEmptyKeys;

static NO_EMPTY_KEYS: RuleMeta = RuleMeta {
    id: "no-empty-keys",
    category: Category::Suspicious,
    description: "A key is empty or only whitespace (after decoding escapes); some consumers \
                  cannot address such a property.",
    example: "{ \"\": 1, \" \": 2 }   // both keys reported",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for NoEmptyKeys {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_KEYS
    }

    fn tags(&self) -> &[&'static str] {
        &["mapping"]
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

pub struct KeyNameCasing;

const CONVENTIONS: [&str; 5] = [
    "camelCase",
    "PascalCase",
    "snake_case",
    "kebab-case",
    "SCREAMING_SNAKE_CASE",
];

static KEY_NAME_CASING: RuleMeta = RuleMeta {
    id: "key-name-casing",
    category: Category::Style,
    description: "A key does not follow one of the allowed naming conventions. The checks are \
                  those of eslint-plugin-jsonc: a key with punctuation or whitespace never \
                  matches, `camelCase` also rejects `_` and `-`, `snake_case` and `kebab-case` \
                  reject capitals. No fix: renaming a key changes the data.",
    example: "{ \"first_name\": 1 }   // reported with the default `camelCase`",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[
        OptionDecl {
            name: "allowed",
            default: Opt::List(&["camelCase"]),
            check: Check::OneOf(&CONVENTIONS),
            description: "Accepted conventions: `camelCase`, `PascalCase`, `snake_case`, \
                          `kebab-case`, `SCREAMING_SNAKE_CASE`. A key is fine when it matches one.",
        },
        sorting::IGNORED_KEYS,
    ],
};

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

impl Rule for KeyNameCasing {
    fn meta(&self) -> &'static RuleMeta {
        &KEY_NAME_CASING
    }

    fn tags(&self) -> &[&'static str] {
        &["mapping"]
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
            let name = hooks.key_name(&key, text);
            if ignored.iter().any(|r| r.is_match(&name))
                || allowed.iter().any(|c| matches_convention(c, &name))
            {
                continue;
            }
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
