//! JSON rules on values, both opt-in: `unsafe-numbers` and `sorted-array-values`.

use crate::generated::{AnyValue, Array, Literal, kind};
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

/// The scalar a string or number element holds.
enum Scalar<'a> {
    Text(std::borrow::Cow<'a, str>),
    Number(f64),
}

fn scalar(element: &AnyValue) -> Option<Scalar<'static>> {
    let token = Literal::cast(element.syntax().clone())?.value()?;
    match token.kind() {
        kind::STRING => Some(Scalar::Text(string_value(token.text()).into_owned().into())),
        kind::NUMBER => token.text().parse().ok().map(Scalar::Number),
        _ => None,
    }
}

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
        let elements: Vec<AnyValue> = array.elements().collect();
        let Some(values) = elements.iter().map(scalar).collect::<Option<Vec<_>>>() else {
            return;
        };
        let sorting = Sorting::new(ctx.options, "min_values");
        let grammar = ctx.file.grammar;
        let ranges: Vec<_> = elements
            .iter()
            .map(|e| entry_range(e.syntax(), grammar))
            .collect();
        let blank = sorting.blank_before(ctx.file.text, &ranges);
        let plan = match values.first() {
            Some(Scalar::Number(_)) => {
                let nums: Option<Vec<f64>> = values
                    .iter()
                    .map(|v| {
                        if let Scalar::Number(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .collect();
                nums.and_then(|nums| {
                    sorting.plan(
                        nums.len(),
                        &blank,
                        |_| false,
                        |a, b| {
                            sorting
                                .directed(nums[a].partial_cmp(&nums[b]).unwrap_or(Ordering::Equal))
                        },
                    )
                })
            }
            Some(Scalar::Text(_)) => {
                let texts: Option<Vec<&str>> = values
                    .iter()
                    .map(|v| {
                        if let Scalar::Text(t) = v {
                            Some(&**t)
                        } else {
                            None
                        }
                    })
                    .collect();
                texts.and_then(|texts| sorting.plan_keys(&texts, &blank))
            }
            None => None,
        };
        let Some(plan) = plan else {
            return;
        };
        let edits = reorder_edits(ctx.file.text, &ranges, &plan.sorted);
        let mut fix = Some(Fix::new(
            "Sort the values of this array",
            FixKind::Unsafe,
            edits,
        ));
        for i in plan.moved {
            let at = core_rules::tree::significant_range(elements[i].syntax(), grammar);
            let report = ctx.report(at, "values are not sorted");
            if let Some(f) = fix.take() {
                report.fix(f);
            }
        }
    }
}
