//! Checks on one statement or expression of its own.

use super::dupes::key_name;
use super::{Check, code_text, has_own, operator, span, test_of, un};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::{Grammar, SyntaxNode};

fn first_and_last(n: &SyntaxNode) -> Option<(SyntaxNode, SyntaxNode)> {
    Some((n.children().next()?, n.children().last()?))
}

/// An empty block, or a `switch` with no case.
pub(super) fn empty(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if n.kind() == k::SWITCH_STATEMENT {
        if !n.children().any(|s| s.kind() == k::SWITCH_CASE) {
            let brace = n.children_with_tokens().find(|e| e.kind() == k::L_BRACE);
            c.hit(
                ctx,
                brace.map_or_else(|| span(ctx, n), |b| b.text_range()),
                "switch",
            );
        }
        return;
    }
    let commented = n.children_with_tokens().any(|e| {
        e.as_token()
            .is_some_and(|t| matches!(t.kind(), k::LINE_COMMENT | k::BLOCK_COMMENT))
    });
    // Only comments between the braces count, not one after the block.
    let inside = commented && {
        let end = n
            .children_with_tokens()
            .find(|e| e.kind() == k::R_BRACE)
            .map(|e| e.text_range().start());
        n.children_with_tokens()
            .filter(|e| {
                e.as_token()
                    .is_some_and(|t| matches!(t.kind(), k::LINE_COMMENT | k::BLOCK_COMMENT))
            })
            .any(|e| end.is_none_or(|end| e.text_range().start() < end))
    };
    if n.children().next().is_none() && !inside {
        c.hit(ctx, span(ctx, n), "block");
    }
}

/// `catch (e) { throw e; }`
pub(super) fn useless_catch(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let Some(handler) = n.children().find(|c| c.kind() == k::CATCH_CLAUSE) else {
        return;
    };
    let param = handler.children().find(|c| c.kind() == k::CATCH_PARAMETER);
    let Some(body) = handler.children().find(|c| c.kind() == k::BLOCK_STATEMENT) else {
        return;
    };
    let mut statements = body.children();
    let (Some(only), None) = (statements.next(), statements.next()) else {
        return;
    };
    let thrown = only.children().next().map(un);
    let (Some(param), Some(thrown)) = (param, thrown) else {
        return;
    };
    let named = param
        .children()
        .next()
        .is_some_and(|p| p.kind() == k::BINDING_IDENTIFIER);
    if only.kind() == k::THROW_STATEMENT
        && named
        && thrown.kind() == k::IDENTIFIER_REFERENCE
        && code_text(&thrown, g) == code_text(&param, g)
    {
        if has_own(n, g, "finally") {
            c.hit(ctx, span(ctx, &handler), "catch clause");
        } else {
            c.hit(ctx, span(ctx, n), "try/catch wrapper");
        }
    }
}

/// The text of a reference (`a`, `this.a.b`, `a["b"]`), `None` for anything else.
fn reference(n: &SyntaxNode, g: &Grammar, text: &str) -> Option<String> {
    let n = un(n.clone());
    match n.kind() {
        k::IDENTIFIER_REFERENCE | k::THIS_EXPRESSION | k::SUPER => Some(code_text(&n, g)),
        k::STATIC_MEMBER_EXPRESSION | k::COMPUTED_MEMBER_EXPRESSION => {
            let (object, property) = first_and_last(&n)?;
            let object = reference(&object, g, text)?;
            match key_name(&property, text) {
                Some(name) => Some(format!("{object}.{name}")),
                None => Some(format!("{object}[{}]", reference(&property, g, text)?)),
            }
        }
        _ => None,
    }
}

/// The value of an object property: the last node, or the identifier of a shorthand.
fn value(p: &SyntaxNode) -> Option<SyntaxNode> {
    let last = p.children().last()?;
    Some(
        last.children()
            .find(|c| c.kind() == k::IDENTIFIER_REFERENCE)
            .unwrap_or(last),
    )
}

/// Pairs of a target and the value assigned to it that are the same reference; collects the values.
fn pairs(l: &SyntaxNode, r: &SyntaxNode, g: &Grammar, text: &str, out: &mut Vec<SyntaxNode>) {
    let (l, r) = (un(l.clone()), un(r.clone()));
    match (l.kind(), r.kind()) {
        (k::ARRAY_ASSIGNMENT_TARGET, k::ARRAY_EXPRESSION) => {
            let (ls, rs): (Vec<_>, Vec<_>) = (l.children().collect(), r.children().collect());
            for (i, (a, b)) in ls.iter().zip(&rs).enumerate() {
                if a.kind() == k::ASSIGNMENT_TARGET_REST && i < rs.len() - 1 {
                    break;
                }
                pairs(a, b, g, text, out);
                if b.kind() == k::SPREAD_ELEMENT {
                    break;
                }
            }
        }
        (k::ASSIGNMENT_TARGET_REST, k::SPREAD_ELEMENT) => {
            if let (Some(a), Some(b)) = (l.children().next(), r.children().next()) {
                pairs(&a, &b, g, text, out);
            }
        }
        (k::OBJECT_ASSIGNMENT_TARGET, k::OBJECT_EXPRESSION) => {
            let props: Vec<_> = r.children().collect();
            let from = props
                .iter()
                .rposition(|p| p.kind() == k::SPREAD_ELEMENT)
                .map_or(0, |i| i + 1);
            for left in l.children() {
                let shorthand = left.kind() == k::ASSIGNMENT_TARGET_PROPERTY_IDENTIFIER;
                if (!shorthand && left.kind() != k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY)
                    || (shorthand && has_own(&left, g, "="))
                {
                    continue;
                }
                let key = if shorthand {
                    left.descendants()
                        .find(|d| d.kind() == k::IDENTIFIER_REFERENCE)
                } else {
                    left.children().next()
                };
                let Some(name) = key.and_then(|key| {
                    if shorthand {
                        Some(code_text(&key, g))
                    } else {
                        key_name(&key, text)
                    }
                }) else {
                    continue;
                };
                let right = props[from..].iter().find(|p| {
                    p.children()
                        .next()
                        .and_then(|key| key_name(&key, text))
                        .as_deref()
                        == Some(&name)
                });
                let target = if shorthand {
                    left.descendants()
                        .find(|d| d.kind() == k::IDENTIFIER_REFERENCE)
                } else {
                    left.children().last()
                };
                if let (Some(a), Some(b)) = (target, right.and_then(value)) {
                    pairs(&a, &b, g, text, out);
                }
            }
        }
        _ => {
            if reference(&l, g, text).is_some_and(|a| Some(a) == reference(&r, g, text)) {
                out.push(r);
            }
        }
    }
}

/// `a = a`, `[a, b] = [a, b]`, `a.b = a.b`.
pub(super) fn self_assign(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if !matches!(operator(n, g).as_str(), "=" | "&&=" | "||=" | "??=") {
        return;
    }
    let Some((left, right)) = first_and_last(n) else {
        return;
    };
    let mut found = Vec::new();
    pairs(&left, &right, g, ctx.file.text, &mut found);
    for value in found {
        c.hit(ctx, span(ctx, &value), &code_text(&value, g));
    }
}

/// An assignment as the condition, unless it is in its own parentheses.
pub(super) fn cond_assign(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    // ESLint wants the parentheses of an `if` or `while` doubled; of a `?:`, a pair is not enough.
    let Some(test) = test_of(n) else {
        return;
    };
    let test =
        if n.kind() == k::CONDITIONAL_EXPRESSION && test.kind() == k::PARENTHESIZED_EXPRESSION {
            test.children().next().unwrap_or(test)
        } else {
            test
        };
    if test.kind() == k::ASSIGNMENT_EXPRESSION {
        c.hit(ctx, span(ctx, &test), "");
    }
}

fn is_nan(n: &SyntaxNode, g: &Grammar) -> bool {
    matches!(code_text(&un(n.clone()), g).as_str(), "NaN" | "Number.NaN")
}

pub(super) fn use_isnan(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    match n.kind() {
        k::BINARY_EXPRESSION => {
            let compares = matches!(
                operator(n, g).as_str(),
                "==" | "===" | "!=" | "!==" | "<" | ">" | "<=" | ">="
            );
            if compares && first_and_last(n).is_some_and(|(l, r)| is_nan(&l, g) || is_nan(&r, g)) {
                c.hit(
                    ctx,
                    span(ctx, n),
                    "Use the isNaN function to compare with NaN.",
                );
            }
        }
        k::SWITCH_STATEMENT => {
            if n.children().next().is_some_and(|d| is_nan(&d, g)) {
                c.hit(ctx, span(ctx, n), "'switch(NaN)' can never match a case clause. Use Number.isNaN instead of the switch.");
            }
        }
        _ => {
            if has_own(n, g, "case") && n.children().next().is_some_and(|t| is_nan(&t, g)) {
                c.hit(
                    ctx,
                    span(ctx, n),
                    "'case NaN' can never match. Use Number.isNaN before the switch.",
                );
            }
        }
    }
}

const TYPEOF_VALUES: [&str; 8] = [
    "symbol",
    "undefined",
    "object",
    "boolean",
    "number",
    "string",
    "function",
    "bigint",
];

/// `typeof x === "strnig"`
pub(super) fn valid_typeof(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if !matches!(operator(n, g).as_str(), "==" | "===" | "!=" | "!==") {
        return;
    }
    let Some((l, r)) = first_and_last(n).map(|(l, r)| (un(l), un(r))) else {
        return;
    };
    let typeof_of = |x: &SyntaxNode| x.kind() == k::UNARY_EXPRESSION && operator(x, g) == "typeof";
    let other = if typeof_of(&l) {
        r
    } else if typeof_of(&r) {
        l
    } else {
        return;
    };
    let text = code_text(&other, g);
    let literal = match other.kind() {
        k::STRING_LITERAL => Some(&text[1..text.len() - 1]),
        k::TEMPLATE_LITERAL if !text.contains("${") => Some(&text[1..text.len() - 1]),
        _ => None,
    };
    let invalid = match literal {
        Some(value) => !TYPEOF_VALUES.contains(&value),
        None => other.kind() == k::IDENTIFIER_REFERENCE && text == "undefined",
    };
    if invalid {
        c.hit(ctx, span(ctx, &other), "");
    }
}

/// Is `node` where a boolean is expected anyway?
fn in_boolean_context(node: &SyntaxNode, g: &Grammar) -> bool {
    let mut outer = node.clone();
    while let Some(p) = outer
        .parent()
        .filter(|p| p.kind() == k::PARENTHESIZED_EXPRESSION)
    {
        outer = p;
    }
    let Some(parent) = outer.parent() else {
        return false;
    };
    match parent.kind() {
        k::IF_STATEMENT
        | k::WHILE_STATEMENT
        | k::DO_WHILE_STATEMENT
        | k::FOR_STATEMENT
        | k::CONDITIONAL_EXPRESSION => test_of(&parent).as_ref() == Some(&outer),
        k::UNARY_EXPRESSION => operator(&parent, g) == "!",
        k::CALL_EXPRESSION | k::NEW_EXPRESSION => {
            is_boolean_callee(&parent, g) && parent.children().nth(1).as_ref() == Some(&outer)
        }
        _ => false,
    }
}

fn is_boolean_callee(call: &SyntaxNode, g: &Grammar) -> bool {
    call.children()
        .next()
        .is_some_and(|c| c.kind() == k::IDENTIFIER_REFERENCE && code_text(&c, g) == "Boolean")
}

/// `!!x` or `Boolean(x)` where a boolean is expected anyway.
pub(super) fn extra_boolean_cast(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let cast = if n.kind() == k::UNARY_EXPRESSION {
        let inner = n.children().next().map(un);
        let double = operator(n, g) == "!"
            && inner.is_some_and(|i| i.kind() == k::UNARY_EXPRESSION && operator(&i, g) == "!");
        double.then_some("Redundant double negation.")
    } else {
        is_boolean_callee(n, g).then_some("Redundant Boolean call.")
    };
    if let Some(message) = cast.filter(|_| in_boolean_context(n, g)) {
        c.hit(ctx, span(ctx, n), message);
    }
}
