//! The `max` family: a count that must not pass option `max`. Param `count` says what is counted
//! (per complex selector: `class`, `id`, `attribute`, `pseudo-class`, `type`, `universal`,
//! `compounds`, `combinators`; `nesting-depth` of a rule or at-rule; `declarations` of a
//! single-line block; `decimals` of a number). The message has `{count}` and `{max}`.

use super::model::{self, Part, split_dimension};
use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt};

pub const FAMILY: Family = Family { name: "max", build };

#[derive(Clone, Copy)]
enum Count {
    Part(Part),
    Compounds,
    Combinators,
    NestingDepth,
    Declarations,
    Decimals,
}

struct Max {
    meta: &'static RuleMeta,
    count: Count,
    message: String,
    kinds: &'static [SyntaxKind],
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    use Count::*;
    let count = p.choice(
        "count",
        true,
        &[
            ("class", Part(model::Part::Class)),
            ("id", Part(model::Part::Id)),
            ("attribute", Part(model::Part::Attribute)),
            ("pseudo-class", Part(model::Part::PseudoClass)),
            ("type", Part(model::Part::Type)),
            ("universal", Part(model::Part::Universal)),
            ("compounds", Compounds),
            ("combinators", Combinators),
            ("nesting-depth", NestingDepth),
            ("declarations", Declarations),
            ("decimals", Decimals),
        ],
    )?;
    let kinds: &'static [SyntaxKind] = match count {
        Part(_) | Compounds | Combinators => &[k::RULE],
        NestingDepth => &[k::RULE, k::AT_RULE],
        Declarations => &[k::BLOCK],
        Decimals => &[k::VALUE],
    };
    let rule = Max {
        meta,
        count,
        message: p.text("message")?,
        kinds,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Max {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let max = usize::try_from(ctx.options.int("max")).unwrap_or(0);
        let over = |ctx: &mut RuleContext<'_>, count: usize, range| {
            if count > max {
                let message = self.message.replace("{count}", &count.to_string());
                ctx.report(range, message.replace("{max}", &max.to_string()));
            }
        };
        match self.count {
            Count::Part(_) | Count::Compounds | Count::Combinators => {
                let Some(prelude) = node.child(k::PRELUDE) else {
                    return;
                };
                if model::in_keyframes(node) {
                    return;
                }
                for sel in model::selectors(&prelude) {
                    let Some(range) = sel.range else { continue };
                    over(
                        ctx,
                        match self.count {
                            Count::Part(part) => sel.count(part),
                            Count::Compounds => sel.compounds.len(),
                            _ => sel.combinators.len(),
                        },
                        range,
                    );
                }
            }
            Count::NestingDepth => {
                let depth = node
                    .ancestors()
                    .skip(1)
                    .filter(|a| matches!(a.kind(), k::RULE | k::AT_RULE))
                    .count();
                // Only the statements that open a nesting are counted from the root's first level.
                over(ctx, depth, significant_range(node, ctx.file.grammar));
            }
            Count::Declarations => {
                let range = significant_range(node, ctx.file.grammar);
                let text = &ctx.file.text[usize::from(range.start())..usize::from(range.end())];
                if !text.contains(['\n', '\r']) {
                    let n = node
                        .children()
                        .filter(|c| c.kind() == k::DECLARATION)
                        .count();
                    over(ctx, n, range);
                }
            }
            Count::Decimals => {
                for t in model::tokens(node) {
                    let number = match t.kind() {
                        k::NUMBER => t.text(),
                        k::PERCENTAGE => &t.text()[..t.text().len() - 1],
                        k::DIMENSION => split_dimension(t.text()).0,
                        _ => continue,
                    };
                    let mantissa = number.split(['e', 'E']).next().unwrap_or(number);
                    over(
                        ctx,
                        mantissa.split_once('.').map_or(0, |(_, f)| f.len()),
                        t.text_range(),
                    );
                }
            }
        }
    }
}
