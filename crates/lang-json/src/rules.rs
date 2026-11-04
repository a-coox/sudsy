//! JSON rules: `no-duplicate-keys` and the opt-in `sorted-keys`. The other key rules are in
//! `key_rules`, the rules on values in `value_rules`.

use crate::generated::{Member, Object, kind};
use crate::string::string_value;
use crate::{key_rules, value_rules};
use core_config::Level;
use core_rules::sorting::{self, Sorting};
use core_rules::tree::{reorder_edits, significant_range};
use core_rules::{Category, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use std::borrow::Cow;
use std::collections::HashMap;

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicateKeys,
    &SortedKeys,
    &key_rules::NoEmptyKeys,
    &key_rules::KeyNameCasing,
    &value_rules::UnsafeNumbers,
    &value_rules::SortedArrayValues,
];

/// The members of an object that have a key, with their key tokens.
fn keyed(object: &Object) -> Vec<(Member, SyntaxToken)> {
    object
        .members()
        .filter_map(|m| m.key().map(|k| (m, k)))
        .collect()
}

struct NoDuplicateKeys;

static NO_DUPLICATE_KEYS: RuleMeta = RuleMeta {
    id: "no-duplicate-keys",
    category: Category::Correctness,
    description: "An object has the same key twice, after decoding escapes; parsers disagree on which value wins.",
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
        let mut seen: HashMap<Cow<'_, str>, core_syntax::TextRange> = HashMap::new();
        let keys: Vec<SyntaxToken> = object.members().filter_map(|m| m.key()).collect();
        for key in &keys {
            let name = string_value(key.text());
            match seen.get(&name) {
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
    description: "The keys of an object are not in order (alphabetical by default). The keys \
                  that would have to move (the fewest) are reported and the first report \
                  carries the fix. Comments on lines of their own stay where they are.",
    example: "{ \"b\": 1, \"a\": 2 }   // becomes { \"a\": 2, \"b\": 1 }",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[
        sorting::ORDER,
        sorting::DIRECTION,
        sorting::COMPARE,
        sorting::IGNORED_KEYS,
        sorting::MIN_KEYS,
        sorting::BLANK_LINE_GROUPS,
    ],
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
        let members = keyed(&object);
        let names: Vec<_> = members
            .iter()
            .map(|(_, k)| string_value(k.text()))
            .collect();
        let grammar = ctx.file.grammar;
        let text = ctx.file.text;
        let range = |i: usize| significant_range(members[i].0.syntax(), grammar);
        let sorting = Sorting::new(ctx.options, "min_keys");
        let blank = |i: usize| {
            sorting::has_blank_line(
                &text[usize::from(range(i - 1).end())..usize::from(range(i).start())],
            )
        };
        let Some(plan) = sorting.plan_keys(&names, blank) else {
            return;
        };
        let ranges: Vec<_> = (0..members.len()).map(range).collect();
        let edits = reorder_edits(ctx.file.text, &ranges, &plan.sorted);
        let mut fix = Some(Fix::new(
            "Sort the keys of this object",
            FixKind::Unsafe,
            edits,
        ));
        for i in plan.moved {
            let report = ctx.report(members[i].1.text_range(), "keys are not sorted");
            if let Some(f) = fix.take() {
                report.fix(f);
            }
        }
    }
}
