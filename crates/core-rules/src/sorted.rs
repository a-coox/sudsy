//! Ordering rules on the shared tags: `sorted-keys` (every `#mapping`) and `sorted-array-values`
//! (every `#sequence` of only strings or only numbers).

use crate::diagnostic::{Category, Fix, FixKind};
use crate::hooks::Scalar;
use crate::keys::{key_range, keyed};
use crate::options::{Check, Opt, OptionDecl};
use crate::rule::{Rule, RuleContext, RuleMeta};
use crate::sorting::{self, Sorting};
use crate::tree::{reorder_edits, reorder_range, significant_range};
use core_config::Level;
use core_syntax::{SyntaxElement, SyntaxNode, TextRange};
use std::cmp::Ordering;

pub struct SortedKeys;

static SORTED_KEYS: RuleMeta = RuleMeta {
    id: "sorted-keys",
    category: Category::Style,
    description: "The keys of a mapping are not in order (alphabetical by default). The keys \
                  reported are chosen by `report`, and the first report carries the fix. \
                  Comments on lines of their own stay where they are, a comment after an entry \
                  moves with it; moving a YAML anchor below its alias breaks the document, so \
                  the fix is unsafe.",
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
        OptionDecl {
            name: "report",
            default: Opt::Str("moved"),
            check: Check::OneOf(&["moved", "misplaced"]),
            description: "Which keys are reported. `moved`: the fewest keys that would have to \
                          move (eslint-plugin-jsonc). `misplaced`: every key that sorts before \
                          one above it (yamllint).",
        },
    ],
};

impl Rule for SortedKeys {
    fn meta(&self) -> &'static RuleMeta {
        &SORTED_KEYS
    }

    fn tags(&self) -> &[&'static str] {
        &["mapping"]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
        let entries = keyed(node, grammar);
        let names: Vec<_> = entries
            .iter()
            .map(|(_, k)| hooks.key_name(k, text))
            .collect();
        let range = |i: usize| reorder_range(node, &entries[i].0, grammar);
        let sorting = Sorting::new(ctx.options, "min_keys");
        let blank = |i: usize| {
            sorting::has_blank_line(
                &text[usize::from(range(i - 1).end())..usize::from(range(i).start())],
            )
        };
        let Some(plan) = sorting.plan_keys(&names, blank) else {
            return;
        };
        let ranges: Vec<TextRange> = (0..entries.len()).map(range).collect();
        let edits = reorder_edits(text, &ranges, &plan.sorted);
        let mut fix = Some(Fix::new(
            "Sort the keys of this mapping",
            FixKind::Unsafe,
            edits,
        ));
        let reported = if ctx.options.string("report") == "misplaced" {
            &plan.misplaced
        } else {
            &plan.moved
        };
        for &i in reported {
            let report = ctx.report(key_range(&entries[i].1, grammar), "keys are not sorted");
            if let Some(f) = fix.take() {
                report.fix(f);
            }
        }
    }
}

pub struct SortedArrayValues;

static SORTED_ARRAY_VALUES: RuleMeta = RuleMeta {
    id: "sorted-array-values",
    category: Category::Style,
    description: "The items of a sequence of only strings or only numbers are not in order \
                  (ascending by default; numbers compare as numbers). Sequences of other or \
                  mixed items are skipped. The items that would have to move (the fewest) are \
                  reported and the first report carries the fix; the fix is unsafe because \
                  sequence order can be data.",
    example: "[\"b\", \"a\"]   // becomes [\"a\", \"b\"]",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[
        sorting::DIRECTION,
        sorting::COMPARE,
        OptionDecl {
            name: "min_values",
            default: Opt::Int(2),
            check: Check::Any,
            description: "Sequences with fewer items are not checked.",
        },
        sorting::BLANK_LINE_GROUPS,
    ],
};

impl Rule for SortedArrayValues {
    fn meta(&self) -> &'static RuleMeta {
        &SORTED_ARRAY_VALUES
    }

    fn tags(&self) -> &[&'static str] {
        &["sequence"]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (grammar, hooks, text) = (ctx.file.grammar, ctx.file.hooks, ctx.file.text);
        // `entry` is what moves (a YAML entry with its dash), `item` the value node in it.
        let mut entries: Vec<SyntaxNode> = Vec::new();
        let mut scalars: Vec<Scalar> = Vec::new();
        let mut at: Vec<TextRange> = Vec::new();
        for entry in node.children() {
            let item = if grammar.has_tag(entry.kind(), "value") {
                entry.clone()
            } else if let Some(SyntaxElement::Node(value)) = grammar.slot(&entry, "value") {
                value
            } else {
                return;
            };
            let Some(scalar) = hooks.scalar(&item, text) else {
                return;
            };
            at.push(significant_range(&item, grammar));
            scalars.push(scalar);
            entries.push(entry);
        }
        let sorting = Sorting::new(ctx.options, "min_values");
        let blank = |i: usize| {
            sorting::has_blank_line(&text[usize::from(at[i - 1].end())..usize::from(at[i].start())])
        };
        let plan = match scalars.first() {
            Some(Scalar::Number(_)) => {
                let numbers: Option<Vec<f64>> = scalars
                    .iter()
                    .map(|s| match s {
                        Scalar::Number(n) => Some(*n),
                        Scalar::String(_) => None,
                    })
                    .collect();
                numbers.and_then(|n| {
                    let order = |a: usize, b: usize| {
                        sorting.directed(n[a].partial_cmp(&n[b]).unwrap_or(Ordering::Equal))
                    };
                    sorting.plan(n.len(), blank, |_| false, order)
                })
            }
            Some(Scalar::String(_)) => {
                let strings: Option<Vec<_>> = scalars
                    .iter()
                    .map(|s| match s {
                        Scalar::String(v) => Some(v.as_ref()),
                        Scalar::Number(_) => None,
                    })
                    .collect();
                strings.and_then(|s| sorting.plan_keys(&s, blank))
            }
            None => None,
        };
        let Some(plan) = plan else {
            return;
        };
        let ranges: Vec<TextRange> = entries
            .iter()
            .map(|e| reorder_range(node, e, grammar))
            .collect();
        let edits = reorder_edits(text, &ranges, &plan.sorted);
        let mut fix = Some(Fix::new(
            "Sort the values of this sequence",
            FixKind::Unsafe,
            edits,
        ));
        for &i in &plan.moved {
            let report = ctx.report(at[i], "values are not sorted");
            if let Some(f) = fix.take() {
                report.fix(f);
            }
        }
    }
}
