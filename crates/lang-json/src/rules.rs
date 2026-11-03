//! JSON rules (plan.md "Initial built-in rules"): `no-duplicate-keys` and the opt-in
//! `sorted-keys`.

use crate::generated::{Member, Object, kind};
use core_config::Level;
use core_rules::tree::{reorder_edits, significant_range};
use core_rules::{Category, Edit, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use std::collections::HashMap;

pub static RULES: &[&dyn Rule] = &[&NoDuplicateKeys, &SortedKeys];

/// A key without its quotes. Escapes are compared as written: `"a"` and `"a"` are different
/// keys to this rule.
fn key_text(token: &SyntaxToken) -> &str {
    let t = token.text();
    t.get(1..t.len().saturating_sub(1)).unwrap_or(t)
}

struct NoDuplicateKeys;

static NO_DUPLICATE_KEYS: RuleMeta = RuleMeta {
    id: "no-duplicate-keys",
    category: Category::Correctness,
    description: "An object has the same key twice; parsers disagree on which value wins.",
    example: "{ \"a\": 1, \"a\": 2 }   // reported at the second \"a\"",
    default_level: Level::Error,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for NoDuplicateKeys {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_KEYS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::OBJECT]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(object) = Object::cast(node.clone()) else {
            return;
        };
        let mut seen: HashMap<&str, core_syntax::TextRange> = HashMap::new();
        let keys: Vec<SyntaxToken> = object.members().filter_map(|m| m.key()).collect();
        for key in &keys {
            let name = key_text(key);
            match seen.get(name) {
                Some(&first) => {
                    ctx.report(key.text_range(), format!("duplicate key `{name}`"))
                        .related(first, "first defined here");
                }
                None => {
                    seen.insert(name, key.text_range());
                }
            }
        }
    }
}

struct SortedKeys;

static SORTED_KEYS: RuleMeta = RuleMeta {
    id: "sorted-keys",
    category: Category::Style,
    description: "The keys of an object are not in alphabetical order.",
    example: "{ \"b\": 1, \"a\": 2 }   // becomes { \"a\": 2, \"b\": 1 }",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "order",
        default: Opt::List(&[]),
        description: "Keys that come first, in this order; the rest follow alphabetically.",
    }],
};

impl Rule for SortedKeys {
    fn meta(&self) -> &'static RuleMeta {
        &SORTED_KEYS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::OBJECT]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(object) = Object::cast(node.clone()) else {
            return;
        };
        let members: Vec<Member> = object.members().filter(|m| m.key().is_some()).collect();
        let order = ctx.options.list("order");
        let rank = |m: &Member| {
            let token = m.key().expect("filtered");
            let key = key_text(&token);
            (
                order.iter().position(|o| *o == key).unwrap_or(order.len()),
                key.to_string(),
            )
        };
        let mut sorted: Vec<usize> = (0..members.len()).collect();
        sorted.sort_by_cached_key(|&i| rank(&members[i]));
        let Some(first_wrong) = (0..members.len()).find(|&i| sorted[i] != i) else {
            return;
        };
        let grammar = ctx.file.grammar;
        let ranges: Vec<_> = members
            .iter()
            .map(|m| significant_range(m.syntax(), grammar))
            .collect();
        let edits: Vec<Edit> = reorder_edits(ctx.file.text, &ranges, &sorted);
        let key = members[first_wrong].key().expect("filtered");
        ctx.report(key.text_range(), "keys are not sorted")
            .fix(Fix::new(
                "Sort the keys of this object",
                FixKind::Unsafe,
                edits,
            ));
    }
}
