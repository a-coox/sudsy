use super::{kind, text_of};
use crate::generated::MethodDeclaration;
use core_config::Level;
use core_rules::{Category, Layer, Rule, RuleContext, RuleMeta};
use core_semantics::DeclarationFlags;
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode};

pub struct AsyncSuffix;

static META: RuleMeta = RuleMeta {
    id: "async-suffix",
    category: Category::Style,
    description: "An async method returning Task or ValueTask has a name that does not end in \
                  `Async`. Overrides, explicit interface implementations and `Main` are left alone \
                  (their names are not the method's to choose). Renaming needs the binding layer, \
                  so there is no fix.",
    example: "async Task<int> Load() { }   // rename to LoadAsync",
    default_level: Level::Warn,
    fix: None,
    requires: &[],
    uses: &[Layer::Types],
    options: &[],
};

impl Rule for AsyncSuffix {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::METHOD_DECLARATION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(method) = MethodDeclaration::cast(node.clone()) else {
            return;
        };
        let modifiers: Vec<_> = method.modifiers().map(|m| m.kind()).collect();
        if !modifiers.contains(&kind::ASYNC_KEYWORD)
            || modifiers.contains(&kind::OVERRIDE_KEYWORD)
            || method.explicit_interface_specifier().is_some()
        {
            return;
        }
        let (Some(name), Some(returns)) = (method.identifier(), method.return_type()) else {
            return;
        };
        let name_text = name.text();
        if name_text.ends_with("Async") || name_text == "Main" {
            return;
        }
        let range = name.text_range();
        let declaration = ctx.file.semantics.as_ref().and_then(|view| {
            let (start, len) = (u32::from(range.start()), u32::from(range.len()));
            view.data
                .answers
                .declarations
                .iter()
                .find(|d| d.start == start && d.len == len)
        });
        match declaration {
            // The binding layer knows the real return type and whether the name is fixed by an
            // interface.
            Some(d) => {
                if !d.has(DeclarationFlags::RETURNS_TASK) || d.has(DeclarationFlags::IMPLEMENTS) {
                    return;
                }
            }
            // `fast` depth: judge by how the type is written.
            None => {
                let returns = text_of(returns.syntax(), ctx);
                let base = returns.split('<').next().unwrap_or(returns);
                let base = base.rsplit('.').next().unwrap_or(base).trim();
                if base != "Task" && base != "ValueTask" {
                    return;
                }
            }
        }
        ctx.report(
            name.text_range(),
            format!("async method `{name_text}` should be named `{name_text}Async`"),
        );
    }
}
