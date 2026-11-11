//! Rules about where control goes: jumps out of `finally`, unreachable code, a getter or generator
//! that does not do its job. `exits` answers from the syntax of the statements alone (jumps,
//! `if`/`else`, `try`, `switch`, loops that cannot end), not from a flow graph.

use super::dupes::key_name;
use super::{Check, code_text, has_own, span, test_of, un};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange};

fn is_loop(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHILE_STATEMENT
            | k::DO_WHILE_STATEMENT
            | k::FOR_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
    )
}

fn is_function(kind: SyntaxKind) -> bool {
    matches!(kind, k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION | k::CLASS)
}

fn label_of(n: &SyntaxNode, g: &Grammar) -> Option<String> {
    n.children()
        .find(|l| l.kind() == k::LABEL_IDENTIFIER)
        .map(|l| code_text(&l, g))
}

/// Is `block` the `finally` block of a `try`?
fn is_finalizer(block: &SyntaxNode, g: &Grammar) -> bool {
    let after_finally =
        std::iter::successors(block.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
            .find(|e| !e.as_token().is_some_and(|t| g.is_trivia(t.kind())))
            .is_some_and(|e| e.as_token().is_some_and(|t| t.text() == "finally"));
    after_finally && block.parent().is_some_and(|p| p.kind() == k::TRY_STATEMENT)
}

/// `return`, `throw`, and a `break` or `continue` that leaves the `finally` block.
pub(super) fn unsafe_finally(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let label = label_of(n, g);
    let jump = n.kind();
    let mut current = n.clone();
    while let Some(p) = current.parent() {
        match p.kind() {
            kind if is_function(kind) => return,
            kind if is_loop(kind)
                && label.is_none()
                && matches!(jump, k::BREAK_STATEMENT | k::CONTINUE_STATEMENT) =>
            {
                return;
            }
            k::SWITCH_STATEMENT if label.is_none() && jump == k::BREAK_STATEMENT => return,
            k::LABELED_STATEMENT if label.is_some() && label_of(&p, g) == label => return,
            k::BLOCK_STATEMENT if is_finalizer(&p, g) => {
                c.hit(ctx, span(ctx, n), g.name(jump));
                return;
            }
            _ => {}
        }
        current = p;
    }
}

/// Does a `break` leave `n` (to `label`, or an unlabeled one when not `nested` in an inner loop or
/// `switch`)?
fn breaks(n: &SyntaxNode, g: &Grammar, label: Option<&str>, nested: bool) -> bool {
    n.children().any(|c| match c.kind() {
        k::BREAK_STATEMENT => match label_of(&c, g) {
            None => !nested,
            Some(l) => Some(l.as_str()) == label,
        },
        kind if is_function(kind) => false,
        kind if is_loop(kind) || kind == k::SWITCH_STATEMENT => breaks(&c, g, label, true),
        _ => breaks(&c, g, label, nested),
    })
}

/// The label of the labeled statement that directly holds `n`.
fn own_label(n: &SyntaxNode, g: &Grammar) -> Option<String> {
    n.parent()
        .filter(|p| p.kind() == k::LABELED_STATEMENT)
        .and_then(|p| label_of(&p, g))
}

fn constant_true(test: Option<SyntaxNode>, g: &Grammar) -> bool {
    test.is_none_or(|t| matches!(code_text(&un(t), g).as_str(), "true" | "1"))
}

/// Does `n` hold a variable read, call, member access, `throw` or the like outside nested functions?
fn can_throw(n: &SyntaxNode) -> bool {
    n.children().any(|c| {
        matches!(
            c.kind(),
            k::IDENTIFIER_REFERENCE
                | k::CALL_EXPRESSION
                | k::NEW_EXPRESSION
                | k::STATIC_MEMBER_EXPRESSION
                | k::COMPUTED_MEMBER_EXPRESSION
                | k::PRIVATE_FIELD_EXPRESSION
                | k::THROW_STATEMENT
                | k::YIELD_EXPRESSION
                | k::AWAIT_EXPRESSION
                | k::IMPORT_EXPRESSION
                | k::TAGGED_TEMPLATE_EXPRESSION
        ) || (!is_function(c.kind()) && can_throw(&c))
    })
}

/// Control never reaches what follows `n`.
fn exits(n: &SyntaxNode, g: &Grammar) -> bool {
    let kind = n.kind();
    match kind {
        k::RETURN_STATEMENT | k::THROW_STATEMENT | k::BREAK_STATEMENT | k::CONTINUE_STATEMENT => {
            true
        }
        k::BLOCK_STATEMENT | k::FUNCTION_BODY => n.children().any(|s| exits(&s, g)),
        k::IF_STATEMENT => {
            let parts: Vec<_> = n.children().collect();
            parts.len() == 3 && exits(&parts[1], g) && exits(&parts[2], g)
        }
        k::TRY_STATEMENT => {
            let blocks: Vec<_> = n
                .children()
                .filter(|b| b.kind() == k::BLOCK_STATEMENT)
                .collect();
            let handler = n.children().find(|h| h.kind() == k::CATCH_CLAUSE);
            let handler_exits = handler.is_none_or(|h| h.children().any(|b| exits(&b, g)));
            (has_own(n, g, "finally") && blocks.last().is_some_and(|f| exits(f, g)))
                || (blocks.first().is_some_and(|b| exits(b, g)) && handler_exits)
        }
        k::SWITCH_STATEMENT => {
            let cases: Vec<_> = n
                .children()
                .filter(|c| c.kind() == k::SWITCH_CASE)
                .collect();
            cases.iter().any(|c| has_own(c, g, "default"))
                && cases
                    .last()
                    .is_some_and(|c| c.children().any(|s| exits(&s, g)))
                && !breaks(n, g, own_label(n, g).as_deref(), false)
        }
        k::LABELED_STATEMENT => {
            let label = label_of(n, g);
            n.children().last().is_some_and(|b| exits(&b, g))
                && !breaks(n, g, label.as_deref(), true)
        }
        k::WHILE_STATEMENT | k::DO_WHILE_STATEMENT | k::FOR_STATEMENT => {
            constant_true(test_of(n), g) && !breaks(n, g, own_label(n, g).as_deref(), false)
        }
        _ => false,
    }
}

/// The statements ESLint reports when unreachable; function declarations, `;` and `var` without a
/// value are left out (they hoist or do nothing).
fn is_target_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::VARIABLE_DECLARATION
            | k::BLOCK_STATEMENT
            | k::BREAK_STATEMENT
            | k::CLASS
            | k::CONTINUE_STATEMENT
            | k::DEBUGGER_STATEMENT
            | k::DO_WHILE_STATEMENT
            | k::EXPRESSION_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
            | k::FOR_STATEMENT
            | k::IF_STATEMENT
            | k::IMPORT_DECLARATION
            | k::LABELED_STATEMENT
            | k::RETURN_STATEMENT
            | k::SWITCH_STATEMENT
            | k::THROW_STATEMENT
            | k::TRY_STATEMENT
            | k::WHILE_STATEMENT
            | k::WITH_STATEMENT
            | k::EXPORT_NAMED_DECLARATION
            | k::EXPORT_DEFAULT_DECLARATION
            | k::EXPORT_ALL_DECLARATION
    )
}

fn is_target(n: &SyntaxNode, g: &Grammar) -> bool {
    let initialized = |d: SyntaxNode| {
        d.children_with_tokens()
            .any(|e| e.as_token().is_some_and(|t| t.text() == "="))
    };
    is_target_kind(n.kind())
        && (n.kind() != k::VARIABLE_DECLARATION
            || !has_own(n, g, "var")
            || n.children().any(initialized))
}

fn walk(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let list = matches!(
        n.kind(),
        k::PROGRAM
            | k::BLOCK_STATEMENT
            | k::FUNCTION_BODY
            | k::SWITCH_CASE
            | k::STATIC_BLOCK
            | k::TS_MODULE_BLOCK
    );
    // a handler no throw can reach (the block only jumps) is unreachable itself
    if n.kind() == k::TRY_STATEMENT {
        let block = n.children().find(|b| b.kind() == k::BLOCK_STATEMENT);
        let handler = n
            .children()
            .find(|h| h.kind() == k::CATCH_CLAUSE)
            .and_then(|h| h.children().find(|b| b.kind() == k::BLOCK_STATEMENT));
        if let (Some(block), Some(handler)) = (block, handler)
            && exits(&block, g)
            && !can_throw(&block)
        {
            c.hit(ctx, span(ctx, &handler), "");
        }
    }
    let (mut dead, mut reported) = (false, false);
    for child in n.children() {
        let statement = list
            && (is_target_kind(child.kind())
                || matches!(child.kind(), k::EMPTY_STATEMENT | k::FUNCTION));
        if dead && statement {
            // a run of unreachable statements is one report; a statement that is not reported ends it
            let target = is_target(&child, g);
            if target && !reported {
                c.hit(ctx, span(ctx, &child), "");
            }
            reported = target;
            continue;
        }
        walk(c, &child, ctx);
        dead |= statement && exits(&child, g);
    }
}

pub(super) fn unreachable(c: &Check, root: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    walk(c, root, ctx);
}

/// `return` statements of a function body, nested functions left out.
fn returns(n: &SyntaxNode, out: &mut Vec<SyntaxNode>) {
    for c in n.children() {
        if c.kind() == k::RETURN_STATEMENT {
            out.push(c.clone());
        }
        if !is_function(c.kind()) {
            returns(&c, out);
        }
    }
}

/// A getter must return a value on every path.
pub(super) fn getter_return(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if !has_own(n, g, "get") {
        return;
    }
    let Some(body) = n
        .children()
        .find(|f| f.kind() == k::FUNCTION)
        .and_then(|f| f.children().find(|b| b.kind() == k::FUNCTION_BODY))
    else {
        return;
    };
    let key = n
        .children()
        .next()
        .and_then(|key| key_name(&key, ctx.file.text));
    let name = key.map_or_else(|| "getter".to_string(), |key| format!("getter '{key}'"));
    let mut found = Vec::new();
    returns(&body, &mut found);
    for r in found.iter().filter(|r| r.children().next().is_none()) {
        c.hit(ctx, span(ctx, r), &name);
    }
    if !exits(&body, g) {
        let start = span(ctx, n).start();
        let end = n
            .children()
            .next()
            .map_or(start, |key| span(ctx, &key).end());
        c.hit(ctx, TextRange::new(start, end), &name);
    }
}

fn has_yield(n: &SyntaxNode) -> bool {
    n.children()
        .any(|c| c.kind() == k::YIELD_EXPRESSION || (c.kind() != k::FUNCTION && has_yield(&c)))
}

/// A generator with a body and no `yield`.
pub(super) fn require_yield(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let method = n
        .parent()
        .filter(|p| matches!(p.kind(), k::METHOD_DEFINITION | k::OBJECT_PROPERTY));
    let generator = has_own(n, g, "*") || method.is_some_and(|m| has_own(&m, g, "*"));
    let Some(body) = n.children().find(|b| b.kind() == k::FUNCTION_BODY) else {
        return;
    };
    if generator && body.children().next().is_some() && !has_yield(&body) {
        c.hit(ctx, span(ctx, n), "");
    }
}
