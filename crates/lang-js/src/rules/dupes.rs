//! Names and conditions that repeat: object keys, class members, `case` tests, `else if` chains.

use super::{Check, code_text, has_own, operator, span, test_of, un};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::unique::Repeats;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use std::collections::HashMap;

/// The statically known name of a property key node, as ESLint's `getStaticPropertyName`.
pub(super) fn key_name(key: &SyntaxNode, text: &str) -> Option<String> {
    let range = key.text_range();
    let raw = text[usize::from(range.start())..usize::from(range.end())].trim();
    match key.kind() {
        k::IDENTIFIER_NAME | k::PRIVATE_IDENTIFIER => Some(raw.to_string()),
        k::STRING_LITERAL => Some(raw[1..raw.len() - 1].to_string()),
        k::TEMPLATE_LITERAL if !raw.contains("${") => Some(raw[1..raw.len() - 1].to_string()),
        k::NUMERIC_LITERAL => {
            let digits = raw.replace('_', "");
            match digits.parse::<f64>() {
                Ok(v) => Some(v.to_string()),
                Err(_) => Some(digits),
            }
        }
        _ => None,
    }
}

/// `get` and `set` accessors pair up; anything else (a value, a method, a field) conflicts with
/// every earlier use of the name.
fn members(
    c: &Check,
    n: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
    kinds: &[SyntaxKind],
    class: bool,
) {
    let g = ctx.file.grammar;
    // name -> (init, get, set) seen, first range
    let mut seen: HashMap<String, (u8, TextRange)> = HashMap::new();
    for m in n.children().filter(|m| kinds.contains(&m.kind())) {
        let Some(key) = m.children().next() else {
            continue;
        };
        // A method without a body is a TypeScript overload signature.
        if m.kind() == k::METHOD_DEFINITION
            && !(m.children().find(|f| f.kind() == k::FUNCTION))
                .is_some_and(|f| f.children().any(|b| b.kind() == k::FUNCTION_BODY))
        {
            continue;
        }
        let Some(name) = key_name(&key, ctx.file.text) else {
            continue;
        };
        let kind = if has_own(&m, g, "get") {
            2
        } else if has_own(&m, g, "set") {
            4
        } else {
            1
        };
        let key_node = key.clone();
        let key = if class && has_own(&m, g, "static") {
            format!("static {name}")
        } else {
            name.clone()
        };
        let range = span(ctx, &m);
        let entry = seen.entry(key).or_insert((0, range));
        let clash = if kind == 1 {
            entry.0 != 0
        } else {
            entry.0 & (kind | 1) != 0
        };
        let first = entry.1;
        entry.0 |= kind;
        if clash {
            // an object's accessor is reported at its key (ESLint), a class member at its start
            let at = if class { range } else { span(ctx, &key_node) };
            c.hit(ctx, at, &name).related(first, "first defined here");
        }
    }
}

pub(super) fn dupe_keys(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    members(c, n, ctx, &[k::OBJECT_PROPERTY], false);
}

pub(super) fn dupe_class_members(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    members(
        c,
        n,
        ctx,
        &[k::METHOD_DEFINITION, k::PROPERTY_DEFINITION],
        true,
    );
}

pub(super) fn duplicate_case(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let mut seen = Repeats::default();
    for case in n.children().filter(|s| s.kind() == k::SWITCH_CASE) {
        if has_own(&case, g, "default") {
            continue;
        }
        let Some(test) = case.children().next() else {
            continue;
        };
        let range = span(ctx, &case);
        seen.check(
            ctx,
            code_text(&test, g),
            range,
            || c.message.clone(),
            "first case",
        );
    }
}

/// The operands of `node` joined by `op` (`||` or `&&`), parentheses looked through.
fn split(node: SyntaxNode, op: &str, g: &core_syntax::Grammar, out: &mut Vec<SyntaxNode>) {
    let node = un(node);
    if node.kind() == k::LOGICAL_EXPRESSION && operator(&node, g) == op {
        let mut parts = node.children();
        if let (Some(l), Some(r)) = (parts.next(), parts.last()) {
            split(l, op, g, out);
            split(r, op, g, out);
            return;
        }
    }
    out.push(node);
}

/// `node` as OR of ANDs of source texts.
fn dnf(node: SyntaxNode, g: &core_syntax::Grammar) -> Vec<Vec<String>> {
    let mut ors = Vec::new();
    split(node, "||", g, &mut ors);
    ors.into_iter()
        .map(|o| {
            let mut ands = Vec::new();
            split(o, "&&", g, &mut ands);
            ands.iter().map(|a| code_text(a, g)).collect()
        })
        .collect()
}

/// A condition of an `else if` that an earlier one of the chain already covers.
pub(super) fn dupe_else_if(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let Some(test) = test_of(n).map(un) else {
        return;
    };
    let mut conditions = vec![test.clone()];
    if test.kind() == k::LOGICAL_EXPRESSION && operator(&test, g) == "&&" {
        split(test.clone(), "&&", g, &mut conditions);
    }
    let mut list: Vec<Vec<Vec<String>>> = conditions.into_iter().map(|t| dnf(t, g)).collect();
    let mut current = n.clone();
    while let Some(parent) = current.parent() {
        let after_else = std::iter::successors(current.prev_sibling_or_token(), |e| {
            e.prev_sibling_or_token()
        })
        .find(|e| !e.as_token().is_some_and(|t| g.is_trivia(t.kind())))
        .is_some_and(|e| e.as_token().is_some_and(|t| t.text() == "else"));
        if parent.kind() != k::IF_STATEMENT || !after_else {
            return;
        }
        current = parent;
        let Some(earlier) = test_of(&current) else {
            return;
        };
        let earlier = dnf(earlier, g);
        for ors in &mut list {
            ors.retain(|o| !earlier.iter().any(|e| e.iter().all(|a| o.contains(a))));
        }
        if list.iter().any(Vec::is_empty) {
            c.hit(ctx, span(ctx, &test), "");
            return;
        }
    }
}
