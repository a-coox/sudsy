//! JSON rules on values, both opt-in: `unsafe-numbers` and `sorted-array-values`.

use crate::generated::{Array, Literal, kind};
use crate::string::string_value;
use core_config::Level;
use core_rules::sorting::{self, Sorting};
use core_rules::tree::{entry_range, reorder_edits};
use core_rules::{Category, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode};
use std::cmp::Ordering;

pub struct UnsafeNumbers;

static UNSAFE_NUMBERS: RuleMeta = RuleMeta {
    id: "unsafe-numbers",
    category: Category::Suspicious,
    description: "A number that a double-precision parser (JavaScript, most JSON libraries) \
                  cannot hold: an integer beyond 2^53 - 1, a value that overflows to infinity, \
                  underflows to zero or is subnormal.",
    example: "{ \"id\": 12345678901234567890, \"x\": 1e999 }   // both reported",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Why a number literal does not survive a round trip through `f64`, if it does not.
fn unsafe_reason(text: &str) -> Option<&'static str> {
    let value: f64 = text.parse().ok()?;
    let mantissa = text.split(['e', 'E']).next().unwrap_or(text);
    if value.is_infinite() {
        Some("overflows to infinity")
    } else if value == 0.0 && mantissa.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        Some("underflows to zero")
    } else if value != 0.0 && value.abs() < f64::MIN_POSITIVE {
        Some("is subnormal and loses precision")
    } else if !text.contains(['.', 'e', 'E']) && value.abs() > MAX_SAFE_INTEGER {
        Some("is beyond 2^53 - 1 and loses precision")
    } else {
        None
    }
}

impl Rule for UnsafeNumbers {
    fn meta(&self) -> &'static RuleMeta {
        &UNSAFE_NUMBERS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::LITERAL]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(token) = Literal::cast(node.clone()).and_then(|l| l.value()) else {
            return;
        };
        if token.kind() != kind::NUMBER {
            return;
        }
        if let Some(reason) = unsafe_reason(token.text()) {
            ctx.report(
                token.text_range(),
                format!("number `{}` {reason}", token.text()),
            );
        }
    }
}

pub struct SortedArrayValues;

static SORTED_ARRAY_VALUES: RuleMeta = RuleMeta {
    id: "sorted-array-values",
    category: Category::Style,
    description: "The elements of an array of only strings or only numbers are not in order \
                  (ascending by default; numbers compare as numbers). Arrays of other or mixed \
                  elements are skipped. The elements that would have to move (the fewest) are \
                  reported and the first report carries the fix; the fix is unsafe because \
                  array order can be data.",
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
            description: "Arrays with fewer elements are not checked.",
        },
        sorting::BLANK_LINE_GROUPS,
    ],
};

impl Rule for SortedArrayValues {
    fn meta(&self) -> &'static RuleMeta {
        &SORTED_ARRAY_VALUES
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::ARRAY]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(array) = Array::cast(node.clone()) else {
            return;
        };
        // A literal is one token, so the token is also the element's range.
        let Some(tokens) = array
            .elements()
            .map(|e| Literal::cast(e.syntax().clone()).and_then(|l| l.value()))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        let Some(first) = tokens.first().map(|t| t.kind()) else {
            return;
        };
        let sorting = Sorting::new(ctx.options, "min_values");
        let text = ctx.file.text;
        let blank = |i: usize| {
            let gap = tokens[i - 1].text_range().end()..tokens[i].text_range().start();
            sorting::has_blank_line(&text[usize::from(gap.start)..usize::from(gap.end)])
        };
        let plan = if first == kind::NUMBER {
            let numbers: Option<Vec<f64>> = tokens
                .iter()
                .map(|t| {
                    (t.kind() == kind::NUMBER)
                        .then(|| t.text().parse().ok())
                        .flatten()
                })
                .collect();
            numbers.and_then(|n| {
                let order = |a: usize, b: usize| {
                    sorting.directed(n[a].partial_cmp(&n[b]).unwrap_or(Ordering::Equal))
                };
                sorting.plan(n.len(), blank, |_| false, order)
            })
        } else if first == kind::STRING {
            let strings: Option<Vec<_>> = tokens
                .iter()
                .map(|t| (t.kind() == kind::STRING).then(|| string_value(t.text())))
                .collect();
            strings.and_then(|s| sorting.plan_keys(&s, blank))
        } else {
            None
        };
        let Some(plan) = plan else {
            return;
        };
        let grammar = ctx.file.grammar;
        let ranges: Vec<_> = tokens
            .iter()
            .filter_map(|t| t.parent())
            .map(|literal| entry_range(&literal, grammar))
            .collect();
        let edits = reorder_edits(ctx.file.text, &ranges, &plan.sorted);
        let mut fix = Some(Fix::new(
            "Sort the values of this array",
            FixKind::Unsafe,
            edits,
        ));
        for i in plan.moved {
            let report = ctx.report(tokens[i].text_range(), "values are not sorted");
            if let Some(f) = fix.take() {
                report.fix(f);
            }
        }
    }
}
