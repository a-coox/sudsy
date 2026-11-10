//! Rules about how a construct is written: `separate-attributes` (`no-empty-delegate-parens` is a pattern file in `rules/`).

use super::{kind, line_separator, text_of};
use core_rules::rule;
use core_rules::tree::significant_range;
use core_rules::{Edit, RuleContext};
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;

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
                .child(kind::ATTRIBUTE_TARGET_SPECIFIER)
                .map_or(String::new(), |t| format!("{} ", text_of(&t, ctx)));
            let range = significant_range(node, grammar);
            let separator = line_separator(range, ctx);
            let lists: Vec<String> = attributes
                .iter()
                .map(|a| format!("[{target}{}]", text_of(a, ctx)))
                .collect();
            let edit = Edit::replace(range, lists.join(&separator));
            fix = Some(("Split into one list per attribute", vec![edit]));
        }
        ctx.report_fix(
            significant_range(second, grammar),
            "combined attributes: put each attribute in its own list",
            fix,
        );
    }
}
