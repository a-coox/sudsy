use super::kind;
use crate::generated::{CompilationUnit, NamespaceDeclaration};
use core_config::Level;
use core_rules::tree::significant_range;
use core_rules::{Category, Edit, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

pub struct FileScopedNamespace;

static META: RuleMeta = RuleMeta {
    id: "file-scoped-namespace",
    category: Category::Cleanup,
    description: "A file whose only top-level member is a block-scoped namespace can use \
                  `namespace X;` and save a level of indentation. The fix needs C# 10, so it is \
                  unsafe; the formatter re-indents the body afterwards.",
    example: "namespace Shop { class A { } }   // becomes namespace Shop; class A { }",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    options: &[],
};

impl Rule for FileScopedNamespace {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::COMPILATION_UNIT]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(unit) = CompilationUnit::cast(node.clone()) else {
            return;
        };
        let members: Vec<SyntaxNode> = unit.members().map(|m| m.syntax().clone()).collect();
        let [only] = members.as_slice() else { return };
        let Some(namespace) = NamespaceDeclaration::cast(only.clone()) else {
            return;
        };
        // A file-scoped namespace cannot contain another namespace.
        if only
            .descendants()
            .skip(1)
            .any(|n| n.kind() == kind::NAMESPACE_DECLARATION)
        {
            return;
        }
        let (Some(keyword), Some(name), Some(open), Some(close)) = (
            namespace.namespace_keyword(),
            namespace.name(),
            namespace.open_brace(),
            namespace.close_brace(),
        ) else {
            return;
        };
        let name_end = significant_range(name.syntax(), ctx.file.grammar).end();
        let close_end = namespace
            .semicolon()
            .map_or(close.text_range().end(), |s| s.text_range().end());
        let report = TextRange::new(keyword.text_range().start(), name_end);
        ctx.report(report, "use a file-scoped namespace")
            .fix(Fix::new(
                "Convert to a file-scoped namespace",
                FixKind::Unsafe,
                vec![
                    Edit::replace(TextRange::new(name_end, open.text_range().end()), ";"),
                    Edit::delete(TextRange::new(close.text_range().start(), close_end)),
                ],
            ));
    }
}
