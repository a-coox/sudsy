//! Rules about how a construct is written: `separate-attributes` and `no-empty-delegate-parens`.

use super::{kind, line_separator, text_of};
use core_rules::rule;
use core_rules::tree::significant_range;
use core_rules::{Edit, Fix, FixKind, RuleContext};
use core_syntax::{SyntaxNode, TextRange};

rule! {
    pub SeparateAttributes {
        id: "separate-attributes",
        category: Style,
        default: Off,
        fix: Safe,
        description: "One attribute per attribute list: `[A, B]` becomes `[A]` and `[B]`. Lists on \
                  parameters and type parameters are left alone. The fix is not offered when the \
                  list has a comment in it.",
        example: "[Serializable, Obsolete]   // write one list for each",
        kinds: &[kind::ATTRIBUTE_LIST],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if node
            .parent()
            .is_some_and(|p| matches!(p.kind(), kind::PARAMETER | kind::TYPE_PARAMETER))
        {
            return;
        }
        let attributes: Vec<SyntaxNode> = node
            .children()
            .filter(|c| c.kind() == kind::ATTRIBUTE)
            .collect();
        let Some(second) = attributes.get(1) else {
            return;
        };
        let grammar = ctx.file.grammar;
        let has_comment = node
            .descendants_with_tokens()
            .any(|e| grammar.has_tag(e.kind(), "comment"));
        let mut fix = None;
        if !has_comment {
            let target = node
                .children()
                .find(|c| c.kind() == kind::ATTRIBUTE_TARGET_SPECIFIER)
                .map_or(String::new(), |t| format!("{} ", text_of(&t, ctx)));
            let range = significant_range(node, grammar);
            let separator = line_separator(range, ctx);
            let lists: Vec<String> = attributes
                .iter()
                .map(|a| format!("[{target}{}]", text_of(a, ctx)))
                .collect();
            fix = Some(Fix::new(
                "Split into one list per attribute",
                FixKind::Safe,
                vec![Edit::replace(range, lists.join(&separator))],
            ));
        }
        let diagnostic = ctx.report(
            significant_range(second, grammar),
            "combined attributes: put each attribute in its own list",
        );
        if let Some(fix) = fix {
            diagnostic.fix(fix);
        }
    }
}

/// The range from the whitespace before `list` to its end, when only whitespace sits between the
/// parentheses of `list` (an empty list with no comment in it).
fn empty_parens_with_leading_space(list: &SyntaxNode, ctx: &RuleContext<'_>) -> Option<TextRange> {
    let range = significant_range(list, ctx.file.grammar);
    let text = ctx.file.text;
    let inner = text[usize::from(range.start())..usize::from(range.end())]
        .strip_prefix('(')?
        .strip_suffix(')')?;
    if !inner.trim().is_empty() {
        return None;
    }
    let before = &text[..usize::from(range.start())];
    let start = before.trim_end_matches([' ', '\t']).len();
    Some(TextRange::new((start as u32).into(), range.end()))
}

rule! {
    pub NoEmptyDelegateParens {
        id: "no-empty-delegate-parens",
        category: Style,
        default: Off,
        fix: Unsafe,
        description: "An anonymous method with an empty parameter list: `delegate() { }` is \
                  `delegate { }`. The fix is unsafe: without a parameter list the method also \
                  converts to delegate types that have parameters, which can change which \
                  overload is chosen.",
        example: "Action a = delegate() { };   // write: delegate { }",
        kinds: &[kind::PARAMETER_LIST],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let anonymous = node
            .parent()
            .is_some_and(|p| p.kind() == kind::ANONYMOUS_METHOD_EXPRESSION);
        if !anonymous || node.children().any(|c| c.kind() == kind::PARAMETER) {
            return;
        }
        let range = significant_range(node, ctx.file.grammar);
        let remove = empty_parens_with_leading_space(node, ctx);
        let diagnostic = ctx.report(
            TextRange::at(range.start(), 1.into()),
            "the anonymous method has an empty parameter list",
        );
        if let Some(remove) = remove {
            diagnostic.fix(Fix::new(
                "Remove the parentheses",
                FixKind::Unsafe,
                vec![Edit::delete(remove)],
            ));
        }
    }
}
