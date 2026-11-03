//! JSON rules on single keys, all opt-in: `no-empty-keys` and `key-name-casing`.

use crate::generated::{Member, kind};
use crate::string::string_value;
use core_config::Level;
use core_rules::sorting;
use core_rules::{Category, Opt, OptionDecl, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode};

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

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::MEMBER]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(key) = Member::cast(node.clone()).and_then(|m| m.key()) else {
            return;
        };
        if string_value(key.text()).trim().is_empty() {
            ctx.report(key.text_range(), "empty key");
        }
    }
}

pub struct KeyNameCasing;

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

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::MEMBER]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(key) = Member::cast(node.clone()).and_then(|m| m.key()) else {
            return;
        };
        let name = string_value(key.text());
        let allowed = ctx.options.list("allowed");
        let ignored = ctx
            .options
            .list("ignored_keys")
            .into_iter()
            .filter_map(sorting::cached_regex)
            .any(|r| r.is_match(&name));
        if ignored || allowed.is_empty() || allowed.iter().any(|c| matches_convention(c, &name)) {
            return;
        }
        ctx.report(
            key.text_range(),
            format!(
                "key `{name}` must match one of the following formats: {}",
                allowed.join(", ")
            ),
        );
    }
}
