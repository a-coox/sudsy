//! Rules about how an expression is written, each with a safe fix: `nullable-shorthand`,
//! `use-lambda`, `constant-on-right` and `use-literal-suffix`.

use super::{has_comment_in, kind, text_of};
use core_config::Level;
use core_rules::tree::significant_range;
use core_rules::{Category, Edit, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

pub struct NullableShorthand;
pub struct UseLambda;
pub struct ConstantOnRight;
pub struct UseLiteralSuffix;

static NULLABLE_SHORTHAND: RuleMeta = RuleMeta {
    id: "nullable-shorthand",
    category: Category::Style,
    description: "`int?` instead of `Nullable<int>` or `System.Nullable<int>`. Name-based: a \
                  type of your own called `Nullable` is reported too. Not reported in `using` \
                  aliases, `nameof`, `typeof(Nullable<>)` and before a member access \
                  (`Nullable<int>.Equals`). Equivalent to StyleCop SA1125.",
    example: "Nullable<int> x;   // write: int? x;",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

static USE_LAMBDA: RuleMeta = RuleMeta {
    id: "use-lambda",
    category: Category::Style,
    description: "A lambda instead of an anonymous method. The fix is offered when the anonymous \
                  method has a parameter list and no comment before its body; `delegate { }` \
                  without one converts to delegate types with any parameters, so it has no fix. \
                  Equivalent to StyleCop SA1130.",
    example: "Func<int, int> f = delegate (int x) { return x; };   // write: (int x) => { return x; }",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

static CONSTANT_ON_RIGHT: RuleMeta = RuleMeta {
    id: "constant-on-right",
    category: Category::Style,
    description: "A comparison with the constant on the left: `null == x` is `x == null`, \
                  `1 < x` is `x > 1`. Without the type layer a constant is a literal, `nameof`, \
                  `int.MaxValue`-style member or an expression of those; a named constant on the \
                  left is not recognised. Equivalent to StyleCop SA1131.",
    example: "if (null == x) { }   // write: if (x == null) { }",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

static USE_LITERAL_SUFFIX: RuleMeta = RuleMeta {
    id: "use-literal-suffix",
    category: Category::Style,
    description: "A numeric literal cast to `long`, `ulong`, `uint`, `float`, `double` or \
                  `decimal` when the literal's own type differs: `(long)1` is `1L`. Not reported \
                  for hexadecimal and binary literals cast to a floating-point type, which have \
                  no suffix. Equivalent to StyleCop SA1139.",
    example: "var x = (long)1;   // write: var x = 1L;",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

/// Is `left` `System` or `global::System`?
fn is_system(left: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    matches!(text_of(left, ctx), "System" | "global::System")
}

impl Rule for NullableShorthand {
    fn meta(&self) -> &'static RuleMeta {
        &NULLABLE_SHORTHAND
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::GENERIC_NAME]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let is_nullable = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| t.kind() == kind::IDENTIFIER_TOKEN && t.text() == "Nullable");
        let Some(arguments) = node
            .children()
            .find(|c| c.kind() == kind::TYPE_ARGUMENT_LIST)
        else {
            return;
        };
        let mut types = arguments.children();
        let (Some(argument), None) = (types.next(), types.next()) else {
            return;
        };
        if !is_nullable || argument.kind() == kind::OMITTED_TYPE_ARGUMENT {
            return;
        }
        for ancestor in node.ancestors().skip(1) {
            let nameof = ancestor.kind() == kind::INVOCATION_EXPRESSION
                && ancestor
                    .children()
                    .next()
                    .is_some_and(|c| text_of(&c, ctx) == "nameof");
            if nameof || ancestor.kind() == kind::USING_DIRECTIVE {
                return;
            }
        }
        // The name with its `System.` qualifier, if it has one.
        let mut outer = node.clone();
        if let Some(parent) = node.parent()
            && matches!(
                parent.kind(),
                kind::QUALIFIED_NAME | kind::SIMPLE_MEMBER_ACCESS_EXPRESSION
            )
            && parent.children().last().as_ref() == Some(node)
        {
            if !parent.children().next().is_some_and(|l| is_system(&l, ctx)) {
                return;
            }
            outer = parent;
        }
        let range = significant_range(&outer, ctx.file.grammar);
        let argument_text = text_of(&argument, ctx);
        let before_member = outer.parent().is_some_and(|p| {
            matches!(
                p.kind(),
                kind::QUALIFIED_NAME | kind::SIMPLE_MEMBER_ACCESS_EXPRESSION
            ) && p.children().next().as_ref() == Some(&outer)
        });
        // `int?.Equals(..)` is not valid C#, and StyleCop leaves such static calls alone.
        if before_member {
            return;
        }
        ctx.report(
            range,
            format!("use `{argument_text}?` instead of `Nullable<{argument_text}>`"),
        )
        .fix(Fix::new(
            "Use the nullable shorthand",
            FixKind::Safe,
            vec![Edit::replace(range, format!("{argument_text}?"))],
        ));
    }
}

impl Rule for UseLambda {
    fn meta(&self) -> &'static RuleMeta {
        &USE_LAMBDA
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::ANONYMOUS_METHOD_EXPRESSION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(keyword) = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == kind::DELEGATE_KEYWORD)
        else {
            return;
        };
        let parameters = node.children().find(|c| c.kind() == kind::PARAMETER_LIST);
        let fix = parameters.and_then(|p| {
            let end = significant_range(&p, ctx.file.grammar).end();
            let range = TextRange::new(keyword.text_range().start(), end);
            if has_comment_in(node, range, ctx) {
                return None;
            }
            let attached = ctx.file.text[usize::from(end)..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace());
            let tail = if attached { " " } else { "" };
            Some(Fix::new(
                "Use a lambda",
                FixKind::Safe,
                vec![Edit::replace(
                    range,
                    format!("{} =>{tail}", text_of(&p, ctx)),
                )],
            ))
        });
        let diagnostic = ctx.report(keyword.text_range(), "use lambda syntax");
        if let Some(fix) = fix {
            diagnostic.fix(fix);
        }
    }
}

/// Operators of the comparisons, and the operator after the operands swap places.
fn flipped(operator: &str) -> Option<&'static str> {
    Some(match operator {
        "==" => "==",
        "!=" => "!=",
        "<" => ">",
        "<=" => ">=",
        ">" => "<",
        ">=" => "<=",
        _ => return None,
    })
}

/// Is the value of `node` known without names: literals, `nameof`, `int.MaxValue`-style members
/// and expressions of those?
fn is_constant(node: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    match node.kind() {
        kind::NULL_LITERAL_EXPRESSION
        | kind::NUMERIC_LITERAL_EXPRESSION
        | kind::STRING_LITERAL_EXPRESSION
        | kind::CHARACTER_LITERAL_EXPRESSION
        | kind::TRUE_LITERAL_EXPRESSION
        | kind::FALSE_LITERAL_EXPRESSION
        | kind::DEFAULT_LITERAL_EXPRESSION => true,
        kind::PARENTHESIZED_EXPRESSION
        | kind::UNARY_MINUS_EXPRESSION
        | kind::UNARY_PLUS_EXPRESSION
        | kind::LOGICAL_NOT_EXPRESSION
        | kind::BITWISE_NOT_EXPRESSION
        | kind::ADD_EXPRESSION
        | kind::SUBTRACT_EXPRESSION
        | kind::MULTIPLY_EXPRESSION
        | kind::DIVIDE_EXPRESSION
        | kind::MODULO_EXPRESSION
        | kind::LEFT_SHIFT_EXPRESSION
        | kind::RIGHT_SHIFT_EXPRESSION
        | kind::BITWISE_AND_EXPRESSION
        | kind::BITWISE_OR_EXPRESSION
        | kind::EXCLUSIVE_OR_EXPRESSION => node.children().all(|c| is_constant(&c, ctx)),
        kind::INVOCATION_EXPRESSION => node
            .children()
            .next()
            .is_some_and(|c| c.kind() == kind::IDENTIFIER_NAME && text_of(&c, ctx) == "nameof"),
        kind::SIMPLE_MEMBER_ACCESS_EXPRESSION => {
            let mut parts = node.children();
            let (Some(owner), Some(member)) = (parts.next(), parts.next()) else {
                return false;
            };
            owner.kind() == kind::PREDEFINED_TYPE
                && matches!(text_of(&member, ctx), "MaxValue" | "MinValue")
        }
        _ => false,
    }
}

impl Rule for ConstantOnRight {
    fn meta(&self) -> &'static RuleMeta {
        &CONSTANT_ON_RIGHT
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[
            kind::EQUALS_EXPRESSION,
            kind::NOT_EQUALS_EXPRESSION,
            kind::LESS_THAN_EXPRESSION,
            kind::LESS_THAN_OR_EQUAL_EXPRESSION,
            kind::GREATER_THAN_EXPRESSION,
            kind::GREATER_THAN_OR_EQUAL_EXPRESSION,
        ]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut operands = node.children();
        let (Some(left), Some(right)) = (operands.next(), operands.last()) else {
            return;
        };
        if !is_constant(&left, ctx) || is_constant(&right, ctx) {
            return;
        }
        let grammar = ctx.file.grammar;
        let whole = significant_range(node, grammar);
        let flip = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| !grammar.is_trivia(t.kind()))
            .and_then(|t| flipped(t.text()));
        let fix = flip
            .filter(|_| !has_comment_in(node, whole, ctx))
            .map(|op| {
                Fix::new(
                    "Put the constant on the right",
                    FixKind::Safe,
                    vec![Edit::replace(
                        whole,
                        format!("{} {op} {}", text_of(&right, ctx), text_of(&left, ctx)),
                    )],
                )
            });
        let diagnostic = ctx.report(
            significant_range(&left, grammar),
            "put the constant on the right side of the comparison",
        );
        if let Some(fix) = fix {
            diagnostic.fix(fix);
        }
    }
}

/// The type of an integer or real literal and whether it is written in hexadecimal or binary,
/// from its text. `None` when it does not fit any type.
fn literal_type(text: &str) -> Option<(&'static str, bool)> {
    let lower = text.to_ascii_lowercase().replace('_', "");
    let radix = if lower.starts_with("0x") {
        Some(16)
    } else if lower.starts_with("0b") {
        Some(2)
    } else {
        None
    };
    let has_suffix = |s: &str| lower.ends_with(s);
    if radix.is_none() {
        if has_suffix("f") {
            return Some(("float", false));
        }
        if has_suffix("m") {
            return Some(("decimal", false));
        }
        if lower.contains('.') || lower.contains('e') || has_suffix("d") {
            return Some(("double", false));
        }
    }
    let digits = lower.trim_end_matches(['u', 'l']);
    let suffix = &lower[digits.len()..];
    let value = match radix {
        Some(base) => u128::from_str_radix(&digits[2..], base).ok()?,
        None => digits.parse::<u128>().ok()?,
    };
    let by_value = |signed_first: bool| {
        if signed_first && value <= i32::MAX as u128 {
            "int"
        } else if value <= u32::MAX as u128 {
            "uint"
        } else if signed_first && value <= i64::MAX as u128 {
            "long"
        } else {
            "ulong"
        }
    };
    let kind = match suffix {
        "" => by_value(true),
        "u" => by_value(false),
        "l" if value <= i64::MAX as u128 => "long",
        _ => "ulong",
    };
    Some((kind, radix.is_some()))
}

/// The suffix that gives a literal the type `target`.
fn suffix_of(target: &str) -> Option<&'static str> {
    Some(match target {
        "long" => "L",
        "ulong" => "UL",
        "uint" => "U",
        "float" => "F",
        "double" => "D",
        "decimal" => "M",
        _ => return None,
    })
}

impl Rule for UseLiteralSuffix {
    fn meta(&self) -> &'static RuleMeta {
        &USE_LITERAL_SUFFIX
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::CAST_EXPRESSION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut parts = node.children();
        let (Some(target), Some(mut operand)) = (parts.next(), parts.last()) else {
            return;
        };
        let target = text_of(&target, ctx);
        let (Some(suffix), true) = (
            suffix_of(target),
            node.children()
                .next()
                .is_some_and(|t| t.kind() == kind::PREDEFINED_TYPE),
        ) else {
            return;
        };
        let mut sign = "";
        loop {
            match operand.kind() {
                kind::PARENTHESIZED_EXPRESSION => {}
                kind::UNARY_MINUS_EXPRESSION if sign.is_empty() => sign = "-",
                kind::UNARY_PLUS_EXPRESSION if sign.is_empty() => sign = "+",
                kind::NUMERIC_LITERAL_EXPRESSION => break,
                _ => return,
            }
            let Some(inner) = operand.children().next() else {
                return;
            };
            operand = inner;
        }
        let literal = text_of(&operand, ctx);
        let Some((literal_kind, radix)) = literal_type(literal) else {
            return;
        };
        let integer_target = matches!(target, "long" | "ulong" | "uint");
        let integer_literal = !matches!(literal_kind, "float" | "double" | "decimal");
        let skip = literal_kind == target
            || (integer_target && !integer_literal)
            || (radix && !integer_target)
            || (sign == "-" && matches!(target, "ulong" | "uint"));
        if skip {
            return;
        }
        let letters: &[char] = if radix {
            &['u', 'U', 'l', 'L']
        } else {
            &['u', 'U', 'l', 'L', 'f', 'F', 'd', 'D', 'm', 'M']
        };
        let bare = literal.trim_end_matches(letters);
        let range = significant_range(node, ctx.file.grammar);
        let fix = (!has_comment_in(node, range, ctx)).then(|| {
            Fix::new(
                "Use a literal suffix",
                FixKind::Safe,
                vec![Edit::replace(range, format!("{sign}{bare}{suffix}"))],
            )
        });
        let diagnostic = ctx.report(
            range,
            format!("use the literal suffix `{suffix}` instead of casting to {target}"),
        );
        if let Some(fix) = fix {
            diagnostic.fix(fix);
        }
    }
}
