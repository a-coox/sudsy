use super::{kind, text_of};
use crate::generated::{
    AnyExpression, ConditionalExpression, EqualsExpression, NotEqualsExpression,
};
use core_config::Level;
use core_rules::tree::significant_range;
use core_rules::{Category, Edit, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode};

pub struct PreferNullCoalescing;

static META: RuleMeta = RuleMeta {
    id: "prefer-null-coalescing",
    category: Category::Style,
    description: "`a != null ? a : b` (or `a == null ? b : a`) for a name or member access `a` is \
                  `a ?? b`. The fix is unsafe because the type of the expression can change when \
                  `a` is a nullable value type.",
    example: "var x = name != null ? name : \"none\";   // becomes name ?? \"none\"",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    options: &[],
};

/// Expressions that bind tighter than `??`, so they need no parentheses as its right operand.
const SIMPLE: &[SyntaxKind] = &[
    kind::IDENTIFIER_NAME,
    kind::SIMPLE_MEMBER_ACCESS_EXPRESSION,
    kind::INVOCATION_EXPRESSION,
    kind::ELEMENT_ACCESS_EXPRESSION,
    kind::OBJECT_CREATION_EXPRESSION,
    kind::PARENTHESIZED_EXPRESSION,
    kind::DEFAULT_EXPRESSION,
    kind::THIS_EXPRESSION,
    kind::STRING_LITERAL_EXPRESSION,
    kind::NUMERIC_LITERAL_EXPRESSION,
    kind::CHARACTER_LITERAL_EXPRESSION,
    kind::TRUE_LITERAL_EXPRESSION,
    kind::FALSE_LITERAL_EXPRESSION,
    kind::NULL_LITERAL_EXPRESSION,
];

/// `(is_not_equals, x)` for `x != null` and `x == null` when `x` is a name or member access.
fn compared_with_null(condition: &AnyExpression) -> Option<(bool, AnyExpression)> {
    let node = condition.syntax().clone();
    let (not_equals, left, right) = if let Some(e) = NotEqualsExpression::cast(node.clone()) {
        (true, e.left()?, e.right()?)
    } else {
        let e = EqualsExpression::cast(node)?;
        (false, e.left()?, e.right()?)
    };
    let simple = matches!(
        left.syntax().kind(),
        kind::IDENTIFIER_NAME | kind::SIMPLE_MEMBER_ACCESS_EXPRESSION | kind::THIS_EXPRESSION
    );
    (simple && right.syntax().kind() == kind::NULL_LITERAL_EXPRESSION).then_some((not_equals, left))
}

impl Rule for PreferNullCoalescing {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::CONDITIONAL_EXPRESSION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(conditional) = ConditionalExpression::cast(node.clone()) else {
            return;
        };
        let (Some(condition), Some(when_true), Some(when_false)) = (
            conditional.condition(),
            conditional.when_true(),
            conditional.when_false(),
        ) else {
            return;
        };
        let Some((not_equals, subject)) = compared_with_null(&condition) else {
            return;
        };
        let (kept, fallback) = if not_equals {
            (when_true, when_false)
        } else {
            (when_false, when_true)
        };
        let squash = |s: &str| s.split_whitespace().collect::<String>();
        if squash(text_of(kept.syntax(), ctx)) != squash(text_of(subject.syntax(), ctx)) {
            return;
        }
        let subject_text = text_of(subject.syntax(), ctx);
        let fallback_text = text_of(fallback.syntax(), ctx);
        let fallback_text = if SIMPLE.contains(&fallback.syntax().kind()) {
            fallback_text.to_string()
        } else {
            format!("({fallback_text})")
        };
        let range = significant_range(node, ctx.file.grammar);
        let replacement = format!("{subject_text} ?? {fallback_text}");
        ctx.report(range, format!("use `{subject_text} ?? ...`"))
            .fix(Fix::new(
                format!("Replace with `{replacement}`"),
                FixKind::Unsafe,
                vec![Edit::replace(range, replacement)],
            ));
    }
}
