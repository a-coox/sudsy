//! JSON rules on values: the opt-in `unsafe-numbers`.

use crate::generated::{Literal, kind};
use core_rules::rule;
use core_rules::{Rule, RuleContext};
use core_syntax::SyntaxNode;
use core_syntax::ast::AstNode;

pub static RULES: &[&dyn Rule] = &[&UnsafeNumbers];

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

rule! {
    pub UnsafeNumbers {
        id: "unsafe-numbers",
        category: Suspicious,
        default: Off,
        description: "A number that a double-precision parser (JavaScript, most JSON libraries) \
                  cannot hold: an integer beyond 2^53 - 1, a value that overflows to infinity, \
                  underflows to zero or is subnormal.",
        example: "{ \"id\": 12345678901234567890, \"x\": 1e999 }   // both reported",
        kinds: &[kind::LITERAL],
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
