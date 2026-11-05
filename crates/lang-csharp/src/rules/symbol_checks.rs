//! Rules that need a symbol or a type from the worker (`symbol_findings`): each reports the
//! places one check of the worker found. `debug-assert-message` and `debug-fail-message` also
//! work without the worker, by name.

use super::finding::report;
use super::{kind, text_of};
use core_rules::rule;
use core_rules::{FileContext, RuleContext};
use core_syntax::SyntaxNode;

/// `Debug.<method>(...)` written as such: the argument list's children, or `None`.
fn debug_call(node: &SyntaxNode, name: &str, ctx: &RuleContext<'_>) -> Option<Vec<SyntaxNode>> {
    let callee = node.children().next()?;
    if callee.kind() != kind::SIMPLE_MEMBER_ACCESS_EXPRESSION {
        return None;
    }
    let text: String = text_of(&callee, ctx)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let target = format!("Debug.{name}");
    (text == target || (text.ends_with(&format!(".{target}")) && text.starts_with("System."))).then(
        || {
            node.children()
                .find(|c| c.kind() == kind::ARGUMENT_LIST)
                .map(|list| {
                    list.children()
                        .filter(|c| c.kind() == kind::ARGUMENT)
                        .collect()
                })
                .unwrap_or_default()
        },
    )
}

/// Is the argument a string literal that is blank?
fn blank_literal(argument: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    argument
        .children()
        .find(|c| c.kind() == kind::STRING_LITERAL_EXPRESSION)
        .is_some_and(|literal| {
            let text = text_of(&literal, ctx);
            let inner = text.trim_start_matches('@').trim_matches('"');
            inner.trim().is_empty()
        })
}

/// By name: the call is `Debug.<name>` with fewer than `index + 1` arguments or a blank literal.
fn debug_message(
    node: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
    name: &str,
    index: usize,
    message: &str,
) {
    if ctx.file.semantics.is_some() {
        return; // `check_file` has the exact answer
    }
    let Some(arguments) = debug_call(node, name, ctx) else {
        return;
    };
    let missing = arguments.len() <= index;
    if missing || blank_literal(&arguments[index], ctx) {
        ctx.report(
            core_rules::tree::significant_range(node, ctx.file.grammar),
            message,
        );
    }
}

rule! {
    pub DebugAssertMessage {
        id: "debug-assert-message",
        category: Style,
        default: Off,
        description: "`Debug.Assert` without a message, or with a constant blank one. With the type layer the \
     call is checked against `System.Diagnostics.Debug`; without it any call written \
     `Debug.Assert` is. No fix.",
        example: "Debug.Assert(ok);   // add a message",
        uses: [Types],
        kinds: &[kind::INVOCATION_EXPRESSION],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        debug_message(
            node,
            ctx,
            "Assert",
            1,
            "Debug.Assert should provide message text",
        );
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "debug_assert_message",
            |_| "Debug.Assert should provide message text".into(),
            None,
        );
    }
}

rule! {
    pub DebugFailMessage {
        id: "debug-fail-message",
        category: Style,
        default: Off,
        description: "`Debug.Fail` with a constant blank message. With the type layer the call is checked \
     against `System.Diagnostics.Debug`; without it any call written `Debug.Fail` is. No fix.",
        example: "Debug.Fail(\"\");   // say what went wrong",
        uses: [Types],
        kinds: &[kind::INVOCATION_EXPRESSION],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        debug_message(
            node,
            ctx,
            "Fail",
            0,
            "Debug.Fail should provide message text",
        );
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        report(
            file,
            ctx,
            "debug_fail_message",
            |_| "Debug.Fail should provide message text".into(),
            None,
        );
    }
}
