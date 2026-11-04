use super::kind;
use crate::generated::CatchClause;
use core_rules::RuleContext;
use core_rules::rule;
use core_syntax::SyntaxNode;
use core_syntax::ast::AstNode;

rule! {
    pub NoEmptyCatch {
        id: "no-empty-catch",
        category: Suspicious,
        default: Warn,
        description: "A catch block with no statements and no comment swallows the exception silently.",
        example: "try { Run(); } catch (Exception) { }   // add handling, or a comment saying why not",
        kinds: &[kind::CATCH_CLAUSE],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(clause) = CatchClause::cast(node.clone()) else {
            return;
        };
        let Some(block) = clause.block() else { return };
        let grammar = ctx.file.grammar;
        let has_comment = block
            .syntax()
            .descendants_with_tokens()
            .any(|e| grammar.has_tag(e.kind(), "comment"));
        if block.statements().next().is_some() || has_comment {
            return;
        }
        let range = clause
            .catch_keyword()
            .map_or(clause.syntax().text_range(), |t| t.text_range());
        ctx.report(range, "empty catch block swallows the exception");
    }
}
