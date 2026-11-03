use super::{kind, text_of};
use crate::generated::{
    CompilationUnit, FileScopedNamespaceDeclaration, NamespaceDeclaration, UsingDirective,
};
use core_config::Level;
use core_rules::tree::{entry_range, reorder_edits, significant_range};
use core_rules::{Category, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

pub struct UsingOrder;

static META: RuleMeta = RuleMeta {
    id: "using-order",
    category: Category::Cleanup,
    description: "Plain `using` directives of a file or namespace are not in alphabetical order \
                  (ordinal). Aliases, `using static` and `global using` stay where they are, and \
                  so does a list that has preprocessor directives in it. Comments on lines of their \
                  own stay where they are; a comment after a directive moves with it.",
    example: "using Zeta;\nusing Alpha;   // Alpha goes first",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "system_first",
        default: Opt::Bool(true),
        description: "Put `System` and `System.*` before the other namespaces.",
    }],
};

fn is_plain(u: &UsingDirective) -> bool {
    u.alias().is_none()
        && u.static_keyword().is_none()
        && u.global_keyword().is_none()
        && u.unsafe_keyword().is_none()
}

impl Rule for UsingOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[
            kind::COMPILATION_UNIT,
            kind::NAMESPACE_DECLARATION,
            kind::FILE_SCOPED_NAMESPACE_DECLARATION,
        ]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let usings: Vec<UsingDirective> = if let Some(n) = CompilationUnit::cast(node.clone()) {
            n.usings().collect()
        } else if let Some(n) = NamespaceDeclaration::cast(node.clone()) {
            n.usings().collect()
        } else if let Some(n) = FileScopedNamespaceDeclaration::cast(node.clone()) {
            n.usings().collect()
        } else {
            return;
        };
        let (Some(first), Some(last)) = (usings.first(), usings.last()) else {
            return;
        };
        let span = TextRange::new(
            significant_range(first.syntax(), ctx.file.grammar).start(),
            significant_range(last.syntax(), ctx.file.grammar).end(),
        );
        let list = &ctx.file.text[usize::from(span.start())..usize::from(span.end())];
        if list.lines().any(|l| l.trim_start().starts_with('#')) {
            return;
        }
        let plain: Vec<&UsingDirective> = usings.iter().filter(|u| is_plain(u)).collect();
        let system_first = ctx.options.bool("system_first");
        let names: Vec<String> = plain
            .iter()
            .map(|u| {
                let name = u
                    .namespace_or_type()
                    .map_or("", |t| text_of(t.syntax(), ctx));
                name.split_whitespace().collect()
            })
            .collect();
        let rank = |name: &str| {
            let is_system = name == "System" || name.starts_with("System.");
            (!(system_first && is_system), name.to_string())
        };
        let mut sorted: Vec<usize> = (0..plain.len()).collect();
        sorted.sort_by_cached_key(|&i| rank(&names[i]));
        let Some(first_wrong) = (0..plain.len()).find(|&i| sorted[i] != i) else {
            return;
        };
        let ranges: Vec<TextRange> = plain
            .iter()
            .map(|u| entry_range(u.syntax(), ctx.file.grammar))
            .collect();
        let edits = reorder_edits(ctx.file.text, &ranges, &sorted);
        ctx.report(ranges[first_wrong], "using directives are not sorted")
            .fix(Fix::new("Sort the using directives", FixKind::Safe, edits));
    }
}
