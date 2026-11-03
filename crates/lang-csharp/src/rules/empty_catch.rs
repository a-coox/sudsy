use super::kind;
use crate::generated::CatchClause;
use core_config::Level;
use core_rules::{Category, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode};

pub struct NoEmptyCatch;

static META: RuleMeta = RuleMeta {
    id: "no-empty-catch",
    category: Category::Suspicious,
    description: "A catch block with no statements and no comment swallows the exception silently.",
    example: "try { Run(); } catch (Exception) { }   // add handling, or a comment saying why not",
    default_level: Level::Warn,
    fix: None,
    requires: &[],
    options: &[],
};

impl Rule for NoEmptyCatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::CATCH_CLAUSE]
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
